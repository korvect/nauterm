use ctap_hid_fido2::{
    fidokey::{
        get_info::InfoOption, CredentialSupportedKeyType, GetAssertionArgsBuilder,
        MakeCredentialArgsBuilder,
    },
    get_fidokey_devices,
    public_key::PublicKeyType,
    public_key_credential_user_entity::PublicKeyCredentialUserEntity,
    Cfg, FidoKeyHidFactory, HidParam,
};
use russh::keys::ssh_key::{
    private, public, Cipher, Kdf, LineEnding, PrivateKey, PublicKey, Signature,
};
use serde::{Deserialize, Serialize};

const OPENSSH_APPLICATION: &str = "ssh:";
const SSH_SK_USER_PRESENCE_REQUIRED: u8 = 0x01;
const SSH_SK_USER_VERIFICATION_REQUIRED: u8 = 0x04;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Fido2Device {
    pub id: String,
    pub name: String,
    pub vendor_id: u16,
    pub product_id: u16,
    pub has_pin: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Fido2GenerateRequest {
    pub device_id: String,
    pub label: String,
    #[serde(default = "default_key_type")]
    pub key_type: String,
    #[serde(default)]
    pub pin: String,
    #[serde(default = "default_true")]
    pub require_user_presence: bool,
    #[serde(default)]
    pub require_user_verification: bool,
    #[serde(default)]
    pub resident: bool,
    #[serde(default)]
    pub passphrase: String,
    #[serde(default = "default_cipher")]
    pub cipher: String,
    #[serde(default = "default_rounds")]
    pub rounds: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Fido2GeneratedKey {
    pub private_key: String,
    pub public_key: String,
    pub key_type: String,
    pub application: String,
}

fn default_key_type() -> String {
    "ecdsa".to_owned()
}

fn default_true() -> bool {
    true
}

fn default_cipher() -> String {
    "aes256-ctr".to_owned()
}

fn default_rounds() -> u32 {
    100
}

pub fn list_devices() -> Vec<Fido2Device> {
    get_fidokey_devices()
        .into_iter()
        .filter_map(|device| {
            let HidParam::Path(id) = device.param else {
                return None;
            };
            let name = if device.product_string.trim().is_empty() {
                format!("FIDO2 security key {:04x}:{:04x}", device.vid, device.pid)
            } else {
                device.product_string
            };
            let cfg = Cfg::init().with_enable_log(false);
            let has_pin = FidoKeyHidFactory::create_by_params(&[HidParam::Path(id.clone())], &cfg)
                .ok()
                .and_then(|device| device.enable_info_option(&InfoOption::ClientPin).ok())
                .flatten()
                .unwrap_or(false);
            Some(Fido2Device {
                id,
                name,
                vendor_id: device.vid,
                product_id: device.pid,
                has_pin,
            })
        })
        .collect()
}

pub fn verify_pin(device_id: String, pin: String) -> Result<(), String> {
    if pin.is_empty() {
        return Err("Enter the security key PIN.".to_owned());
    }
    let cfg = Cfg::init().with_enable_log(false);
    let mut device = FidoKeyHidFactory::create_by_params(&[HidParam::Path(device_id)], &cfg)
        .map_err(|error| error.to_string())?;
    device.enable_keep_alive_msg = false;
    device
        .set_pin_uv_auth_protocol_two()
        .map_err(|error| friendly_error(&error.to_string()))?;
    device
        .get_pin_token(&pin)
        .map(|_| ())
        .map_err(|error| friendly_error(&error.to_string()))
}

pub fn generate_key(request: Fido2GenerateRequest) -> Result<Fido2GeneratedKey, String> {
    let label = request.label.trim();
    if label.is_empty() {
        return Err("Label is required.".to_owned());
    }

    let mut challenge = [0_u8; 32];
    getrandom::fill(&mut challenge).map_err(|error| error.to_string())?;
    let mut user_id = [0_u8; 32];
    getrandom::fill(&mut user_id).map_err(|error| error.to_string())?;

    let cfg = Cfg::init().with_enable_log(false);
    let mut device =
        FidoKeyHidFactory::create_by_params(&[HidParam::Path(request.device_id)], &cfg)
            .map_err(|error| error.to_string())?;
    device.enable_keep_alive_msg = false;
    if !request.pin.is_empty() {
        device
            .set_pin_uv_auth_protocol_two()
            .map_err(|error| friendly_error(&error.to_string()))?;
    }

    let key_type = match request.key_type.as_str() {
        "ed25519" => CredentialSupportedKeyType::Ed25519,
        "ecdsa" => CredentialSupportedKeyType::Ecdsa256,
        _ => return Err("Unsupported FIDO2 key type.".to_owned()),
    };
    let user = PublicKeyCredentialUserEntity::new(Some(&user_id), Some(label), Some(label));
    let mut builder = MakeCredentialArgsBuilder::new(OPENSSH_APPLICATION, &challenge)
        .key_type(key_type)
        .user_entity(&user);
    if request.resident {
        builder = builder.resident_key();
    }
    builder = if request.pin.is_empty() {
        builder.without_pin_and_uv()
    } else {
        builder.pin(&request.pin)
    };

    let attestation = device
        .make_credential_with_args(&builder.build())
        .map_err(|error| friendly_error(&error.to_string()))?;

    let mut flags = 0_u8;
    if request.require_user_presence {
        flags |= SSH_SK_USER_PRESENCE_REQUIRED;
    }
    if request.require_user_verification {
        flags |= SSH_SK_USER_VERIFICATION_REQUIRED;
    }

    let mut private_key = match attestation.credential_publickey.key_type {
        PublicKeyType::Ecdsa256 => {
            let point = match public::EcdsaPublicKey::from_sec1_bytes(
                &attestation.credential_publickey.der,
            )
            .map_err(|error| error.to_string())?
            {
                public::EcdsaPublicKey::NistP256(point) => point,
                _ => return Err("Authenticator returned an unexpected EC curve.".to_owned()),
            };
            let public = public::SkEcdsaSha2NistP256::new(point, OPENSSH_APPLICATION);
            PrivateKey::from(
                private::SkEcdsaSha2NistP256::new(
                    public,
                    flags,
                    attestation.credential_descriptor.id,
                )
                .map_err(|error| error.to_string())?,
            )
        }
        PublicKeyType::Ed25519 => {
            let public_key =
                public::Ed25519PublicKey::try_from(attestation.credential_publickey.der.as_slice())
                    .map_err(|error| error.to_string())?;
            let public = public::SkEd25519::new(public_key, OPENSSH_APPLICATION);
            PrivateKey::from(
                private::SkEd25519::new(public, flags, attestation.credential_descriptor.id)
                    .map_err(|error| error.to_string())?,
            )
        }
        PublicKeyType::Unknown => {
            return Err("Authenticator returned an unsupported public key.".to_owned())
        }
    };
    private_key.set_comment(label);
    let public_key = PublicKey::new(private_key.public_key().key_data().clone(), label);
    if !request.passphrase.is_empty() {
        if request.rounds == 0 || request.rounds > 1_000_000 {
            return Err("Rounds must be between 1 and 1000000.".to_owned());
        }
        let cipher = match request.cipher.as_str() {
            "aes256-ctr" => Cipher::Aes256Ctr,
            "aes128-ctr" => Cipher::Aes128Ctr,
            _ => return Err("Unsupported private key cipher.".to_owned()),
        };
        let mut salt = vec![0_u8; 16];
        getrandom::fill(&mut salt).map_err(|error| error.to_string())?;
        let mut checkint = [0_u8; 4];
        getrandom::fill(&mut checkint).map_err(|error| error.to_string())?;
        private_key = private_key
            .encrypt_with(
                cipher,
                Kdf::Bcrypt {
                    salt,
                    rounds: request.rounds,
                },
                u32::from_be_bytes(checkint),
                request.passphrase.as_bytes(),
            )
            .map_err(|error| error.to_string())?;
    }

    Ok(Fido2GeneratedKey {
        private_key: private_key
            .to_openssh(LineEnding::LF)
            .map_err(|error| error.to_string())?
            .to_string(),
        public_key: public_key.to_openssh().map_err(|error| error.to_string())?,
        key_type: match attestation.credential_publickey.key_type {
            PublicKeyType::Ed25519 => "ed25519",
            _ => "ecdsa",
        }
        .to_owned(),
        application: OPENSSH_APPLICATION.to_owned(),
    })
}

fn friendly_error(message: &str) -> String {
    let lower = message.to_ascii_lowercase();
    if lower.contains("pin invalid") || lower.contains("pin_invalid") {
        "The PIN is incorrect.".to_owned()
    } else if lower.contains("pin blocked") || lower.contains("pin_blocked") {
        "The authenticator PIN is blocked.".to_owned()
    } else if lower.contains("keepalive") || lower.contains("timeout") {
        "Timed out waiting for the security key.".to_owned()
    } else {
        message.to_owned()
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Fido2SshAlgorithm {
    Ecdsa,
    Ed25519,
}

pub fn sign_ssh(
    application: &str,
    key_handle: &[u8],
    algorithm: Fido2SshAlgorithm,
    message: &[u8],
    pin: Option<&str>,
    require_user_presence: bool,
) -> Result<Vec<u8>, String> {
    let mut last_error = "FIDO2 security key not found.".to_owned();
    for info in get_fidokey_devices() {
        let cfg = Cfg::init().with_enable_log(false);
        let mut device = match FidoKeyHidFactory::create_by_params(&[info.param], &cfg) {
            Ok(device) => device,
            Err(error) => {
                last_error = error.to_string();
                continue;
            }
        };
        device.enable_keep_alive_msg = false;
        if pin.is_some_and(|value| !value.is_empty()) {
            if let Err(error) = device.set_pin_uv_auth_protocol_two() {
                last_error = friendly_error(&error.to_string());
                continue;
            }
        }
        let mut builder =
            GetAssertionArgsBuilder::new(application, message).credential_id(key_handle);
        if !require_user_presence {
            builder = builder.without_up();
        }
        builder = match pin.filter(|value| !value.is_empty()) {
            Some(pin) => builder.pin(pin),
            None => builder.without_pin_and_uv(),
        };
        match device.get_assertion_with_args(&builder.build()) {
            Ok(assertions) => {
                let assertion = assertions
                    .into_iter()
                    .next()
                    .ok_or_else(|| "Authenticator returned no assertion.".to_owned())?;
                return encode_ssh_signature(
                    algorithm,
                    &assertion.signature,
                    assertion.flags.as_u8(),
                    assertion.sign_count,
                );
            }
            Err(error) => last_error = friendly_error(&error.to_string()),
        }
    }
    Err(last_error)
}

fn encode_ssh_signature(
    algorithm: Fido2SshAlgorithm,
    authenticator_signature: &[u8],
    flags: u8,
    counter: u32,
) -> Result<Vec<u8>, String> {
    let signature_data = match algorithm {
        Fido2SshAlgorithm::Ecdsa => {
            let signature = p256::ecdsa::Signature::from_der(authenticator_signature)
                .map_err(|error| error.to_string())?;
            Signature::try_from(signature)
                .map_err(|error| error.to_string())?
                .as_bytes()
                .to_vec()
        }
        Fido2SshAlgorithm::Ed25519 => {
            if authenticator_signature.len() != 64 {
                return Err("Authenticator returned an invalid Ed25519 signature.".to_owned());
            }
            authenticator_signature.to_vec()
        }
    };
    let algorithm_name = match algorithm {
        Fido2SshAlgorithm::Ecdsa => "sk-ecdsa-sha2-nistp256@openssh.com",
        Fido2SshAlgorithm::Ed25519 => "sk-ssh-ed25519@openssh.com",
    };
    let algorithm_length =
        u32::try_from(algorithm_name.len()).map_err(|error| error.to_string())?;
    let signature_length =
        u32::try_from(signature_data.len()).map_err(|error| error.to_string())?;
    let payload_length = 4_u32
        .checked_add(algorithm_length)
        .and_then(|length| length.checked_add(4))
        .and_then(|length| length.checked_add(signature_length))
        .and_then(|length| length.checked_add(5))
        .ok_or_else(|| "FIDO2 signature is too large.".to_owned())?;
    let mut framed = Vec::with_capacity(payload_length as usize + 4);
    framed.extend_from_slice(&payload_length.to_be_bytes());
    framed.extend_from_slice(&algorithm_length.to_be_bytes());
    framed.extend_from_slice(algorithm_name.as_bytes());
    framed.extend_from_slice(&signature_length.to_be_bytes());
    framed.extend_from_slice(&signature_data);
    framed.push(flags);
    framed.extend_from_slice(&counter.to_be_bytes());
    Ok(framed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use russh::keys::ssh_key::Algorithm;

    fn decode_framed_signature(encoded: &[u8]) -> Signature {
        assert!(encoded.len() >= 4);
        let length = u32::from_be_bytes(encoded[..4].try_into().unwrap()) as usize;
        assert_eq!(length, encoded.len() - 4);
        Signature::try_from(&encoded[4..]).unwrap()
    }

    #[test]
    fn encodes_ed25519_security_key_signature() {
        let encoded = encode_ssh_signature(Fido2SshAlgorithm::Ed25519, &[7; 64], 0x05, 42)
            .expect("valid signature");
        let signature = decode_framed_signature(&encoded);
        assert_eq!(signature.algorithm(), Algorithm::SkEd25519);
        assert_eq!(&signature.as_bytes()[64..], &[0x05, 0, 0, 0, 42]);
    }

    #[test]
    fn converts_der_ecdsa_security_key_signature() {
        let signature = p256::ecdsa::Signature::from_scalars([1; 32], [2; 32]).unwrap();
        let encoded = encode_ssh_signature(
            Fido2SshAlgorithm::Ecdsa,
            signature.to_der().as_bytes(),
            0x01,
            9,
        )
        .expect("valid signature");
        let signature = decode_framed_signature(&encoded);
        assert_eq!(signature.algorithm(), Algorithm::SkEcdsaSha2NistP256);
        assert_eq!(
            &signature.as_bytes()[signature.as_bytes().len() - 5..],
            &[1, 0, 0, 0, 9]
        );
    }
}
