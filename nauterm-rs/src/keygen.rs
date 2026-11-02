//! Key generation is CPU work, called off the UI and database threads.
//! Private material and passphrases stay in memory: no subprocess arguments,
//! temporary key files, OS agent, or reference application storage are involved.
use crate::vault::KeyRecord;
use anyhow::{Context, Result, ensure};
use rand::{
    SeedableRng,
    rngs::{StdRng, SysRng},
};
use ssh_key::{Algorithm, EcdsaCurve, LineEnding, PrivateKey, private::RsaKeypair};
use zeroize::Zeroizing;

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub enum KeyType {
    Ed25519,
    #[default]
    Ecdsa,
    Rsa,
}
impl KeyType {
    pub const ALL: [Self; 3] = [Self::Ed25519, Self::Ecdsa, Self::Rsa];
    pub fn label(self) -> &'static str {
        match self {
            Self::Ed25519 => "ED25519",
            Self::Ecdsa => "ECDSA",
            Self::Rsa => "RSA",
        }
    }
    pub fn suggested_name(self) -> String {
        format!("{} key", self.label())
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Parameters {
    pub kind: KeyType,
    pub ecdsa_bits: usize,
    pub rsa_bits: usize,
}
impl Default for Parameters {
    fn default() -> Self {
        Self {
            kind: KeyType::Ecdsa,
            ecdsa_bits: 521,
            rsa_bits: 4096,
        }
    }
}

pub fn generate(
    parameters: Parameters,
    name: String,
    passphrase: Zeroizing<String>,
) -> Result<KeyRecord> {
    let name = name.trim().to_owned();
    ensure!(!name.is_empty(), "Name is required.");
    ensure!(
        !name.contains(['\r', '\n', '\0']),
        "Name must be a single line."
    );
    // Seed once through the fallible OS API so entropy errors become a visible
    // generation error, rather than an UnwrapErr panic on the background task.
    let mut rng = StdRng::try_from_rng(&mut SysRng).context("System randomness is unavailable")?;
    let mut key = match parameters.kind {
        KeyType::Ed25519 => PrivateKey::random(&mut rng, Algorithm::Ed25519)?,
        KeyType::Ecdsa => {
            let curve = match parameters.ecdsa_bits {
                256 => EcdsaCurve::NistP256,
                384 => EcdsaCurve::NistP384,
                521 => EcdsaCurve::NistP521,
                _ => anyhow::bail!("Unsupported ECDSA curve size."),
            };
            PrivateKey::random(&mut rng, Algorithm::Ecdsa { curve })?
        }
        KeyType::Rsa => {
            ensure!(
                [1024, 2048, 4096].contains(&parameters.rsa_bits),
                "Unsupported RSA key size."
            );
            PrivateKey::new(
                RsaKeypair::random(&mut rng, parameters.rsa_bits)?.into(),
                name.clone(),
            )?
        }
    };
    key.set_comment(name.clone());
    let public = key.public_key().to_openssh()?;
    if !passphrase.is_empty() {
        key = key
            .encrypt(&mut rng, passphrase.as_bytes())
            .context("Could not encrypt the generated key")?;
    }
    let private = key.to_openssh(LineEnding::LF)?;
    Ok(KeyRecord {
        id: None,
        name,
        private_key: Some(private.to_string()),
        public_key: Some(public),
        certificate: None,
        // The passphrase encrypts the private key; it is NOT saved alongside it.
        passphrase: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_pair(record: &KeyRecord, algorithm: Algorithm) {
        let private = PrivateKey::from_openssh(record.private_key.as_deref().unwrap()).unwrap();
        let public =
            ssh_key::PublicKey::from_openssh(record.public_key.as_deref().unwrap()).unwrap();
        assert_eq!(private.algorithm(), algorithm);
        assert_eq!(private.public_key(), &public);
        assert_eq!(public.comment().as_str().unwrap(), record.name);
        assert!(record.passphrase.is_none());
        assert!(record.id.is_none());
        assert!(record.certificate.is_none());
    }

    #[test]
    fn default_and_ed25519_generation_preserve_names_and_formats() {
        let default = generate(
            Parameters::default(),
            "ECDSA key".into(),
            Zeroizing::new(String::new()),
        )
        .unwrap();
        assert_eq!(default.name, "ECDSA key");
        assert_pair(
            &default,
            Algorithm::Ecdsa {
                curve: EcdsaCurve::NistP521,
            },
        );
        let ed25519 = generate(
            Parameters {
                kind: KeyType::Ed25519,
                ..Parameters::default()
            },
            " Test key ".into(),
            Zeroizing::new(String::new()),
        )
        .unwrap();
        assert_eq!(ed25519.name, "Test key");
        assert_pair(&ed25519, Algorithm::Ed25519);
    }

    #[test]
    fn empty_and_whitespace_names_are_rejected_before_generation() {
        for name in ["", " ", "\t\r\n"] {
            let error = generate(
                Parameters::default(),
                name.into(),
                Zeroizing::new(String::new()),
            )
            .err()
            .expect("empty names must not generate a key");
            assert_eq!(error.to_string(), "Name is required.");
        }
    }

    #[test]
    fn every_reference_curve_size_round_trips() {
        for (ecdsa_bits, curve) in [
            (256, EcdsaCurve::NistP256),
            (384, EcdsaCurve::NistP384),
            (521, EcdsaCurve::NistP521),
        ] {
            let record = generate(
                Parameters {
                    ecdsa_bits,
                    ..Parameters::default()
                },
                "Curve test".into(),
                Zeroizing::new(String::new()),
            )
            .unwrap();
            assert_pair(&record, Algorithm::Ecdsa { curve });
        }
    }

    #[test]
    fn encryption_requires_the_passphrase_and_does_not_store_it() {
        let record = generate(
            Parameters {
                kind: KeyType::Ed25519,
                ..Parameters::default()
            },
            "Encrypted test".into(),
            Zeroizing::new("test-only-passphrase".into()),
        )
        .unwrap();
        let encrypted = PrivateKey::from_openssh(record.private_key.as_deref().unwrap()).unwrap();
        assert!(encrypted.is_encrypted());
        assert!(encrypted.decrypt("wrong-passphrase").is_err());
        let decrypted = encrypted.decrypt("test-only-passphrase").unwrap();
        assert_eq!(
            decrypted.public_key().to_openssh().unwrap(),
            record.public_key.as_deref().unwrap()
        );
        assert!(record.passphrase.is_none());
        assert!(
            !serde_json::to_string(&record)
                .unwrap()
                .contains("test-only-passphrase")
        );
    }

    #[test]
    fn unsupported_sizes_and_multiline_comments_are_rejected() {
        for parameters in [
            Parameters {
                ecdsa_bits: 123,
                ..Parameters::default()
            },
            Parameters {
                kind: KeyType::Rsa,
                rsa_bits: 512,
                ..Parameters::default()
            },
        ] {
            assert!(generate(parameters, "Test".into(), Zeroizing::new(String::new())).is_err());
        }
        assert!(
            generate(
                Parameters::default(),
                "one\ntwo".into(),
                Zeroizing::new(String::new())
            )
            .is_err()
        );
    }

    #[test]
    fn every_reference_rsa_size_round_trips() {
        for rsa_bits in [1024, 2048, 4096] {
            let record = generate(
                Parameters {
                    kind: KeyType::Rsa,
                    rsa_bits,
                    ..Parameters::default()
                },
                "RSA test".into(),
                Zeroizing::new(String::new()),
            )
            .unwrap();
            assert_pair(&record, Algorithm::Rsa { hash: None });
            let private = PrivateKey::from_openssh(record.private_key.as_deref().unwrap()).unwrap();
            assert_eq!(
                private.key_data().rsa().unwrap().key_size() as usize,
                rsa_bits
            );
        }
    }
}
