//! Canonical content identities shared by pure contracts and I/O owners.
use sha2::{Digest, Sha256};

/// Hash exact bytes without changing serialization, framing, or text encoding.
pub fn sha256(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

/// Accept only the canonical algorithm prefix and 64 lowercase hexadecimal digits.
pub fn is_sha256(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_vectors_preserve_exact_bytes_and_canonical_format() {
        for (bytes, expected) in [
            (
                b"".as_slice(),
                "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            ),
            (
                b"abc".as_slice(),
                "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
            ),
        ] {
            assert_eq!(sha256(bytes), expected);
            assert!(is_sha256(expected));
        }
        assert_ne!(sha256(b"abc"), sha256(b"abc\n"));
        assert_ne!(sha256(b"abc\n"), sha256(b"abc\r\n"));
        assert_ne!(sha256(b"abc"), sha256(b"abc\0"));
    }

    #[test]
    fn rejects_noncanonical_prefix_length_case_and_non_ascii_without_panicking() {
        let valid = sha256(b"abc");
        for invalid in [
            String::new(),
            valid[7..].to_owned(),
            valid.replace("sha256:", "SHA256:"),
            valid.replace("sha256:", "sha512:"),
            format!("sha256:{}", "a".repeat(63)),
            format!("sha256:{}", "a".repeat(65)),
            format!("sha256:{}", "A".repeat(64)),
            format!("sha256:{}", "g".repeat(64)),
            format!("sha256:{}", "é".repeat(32)),
            format!(" {valid}"),
            format!("{valid}\n"),
        ] {
            assert!(!is_sha256(&invalid), "accepted {invalid:?}");
        }
        assert!(is_sha256(&format!("sha256:{}", "0".repeat(64))));
        assert!(is_sha256(&format!("sha256:{}", "f".repeat(64))));
    }
}
