use encoding_rs::{Decoder, Encoding, UTF_8};

use super::protocol::IAC;

pub(super) fn encoding_from_label(label: &str) -> &'static Encoding {
    let label = label.trim();
    if label.is_empty() {
        return UTF_8;
    }
    Encoding::for_label(label.as_bytes()).unwrap_or(UTF_8)
}

pub(super) fn decode_network_output(decoder: &mut Decoder, bytes: &[u8]) -> Vec<u8> {
    let Some(capacity) = decoder.max_utf8_buffer_length(bytes.len()) else {
        return bytes.to_vec();
    };
    let mut output = String::with_capacity(capacity);
    let _ = decoder.decode_to_string(bytes, &mut output, false);
    output.into_bytes()
}

pub(super) fn encode_client_input(bytes: &[u8], encoding: &'static Encoding) -> Vec<u8> {
    let mut encoded = if std::ptr::eq(encoding, UTF_8) {
        bytes.to_vec()
    } else {
        let text = String::from_utf8_lossy(bytes);
        let (encoded, _, _) = encoding.encode(&text);
        encoded.into_owned()
    };
    encoded = normalize_telnet_newlines(&encoded);
    escape_iac_bytes(&encoded)
}

pub(super) fn encode_client_input_binary(bytes: &[u8], encoding: &'static Encoding) -> Vec<u8> {
    let encoded = if std::ptr::eq(encoding, UTF_8) {
        bytes.to_vec()
    } else {
        let text = String::from_utf8_lossy(bytes);
        let (encoded, _, _) = encoding.encode(&text);
        encoded.into_owned()
    };
    escape_iac_bytes(&encoded)
}

fn normalize_telnet_newlines(bytes: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'\r' => {
                output.extend_from_slice(b"\r\n");
                if bytes.get(index + 1) == Some(&b'\n') {
                    index += 1;
                }
            }
            b'\n' => output.extend_from_slice(b"\r\n"),
            byte => output.push(byte),
        }
        index += 1;
    }
    output
}

fn escape_iac_bytes(bytes: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity(bytes.len());
    for byte in bytes {
        output.push(*byte);
        if *byte == IAC {
            output.push(IAC);
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use encoding_rs::UTF_8;

    use super::*;

    #[test]
    fn encodes_telnet_input_and_escapes_iac() {
        assert_eq!(
            encode_client_input(&[b'a', IAC, b'\r'], UTF_8),
            [b'a', IAC, IAC, b'\r', b'\n']
        );
        assert_eq!(
            encode_client_input(b"one\r\ntwo\n", UTF_8),
            b"one\r\ntwo\r\n"
        );
    }
}
