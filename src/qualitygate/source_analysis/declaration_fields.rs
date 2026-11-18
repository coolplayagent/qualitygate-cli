//! Parses structured fields in captured source declarations.

use std::collections::BTreeMap;

/// Parse assignments outside quoted strings, preventing a description containing
/// `author='somebody'` from masquerading as an author field.
pub(crate) fn fields(text: &str) -> BTreeMap<String, String> {
    let bytes = text.as_bytes();
    let mut values = BTreeMap::new();
    let mut offset = 0;
    while offset < bytes.len() {
        if matches!(bytes[offset], b'\'' | b'"') {
            let (_, next) = quoted(bytes, offset);
            offset = next;
            continue;
        }
        if !bytes[offset].is_ascii_alphabetic() && bytes[offset] != b'_' {
            offset += 1;
            continue;
        }
        let start = offset;
        while offset < bytes.len()
            && (bytes[offset].is_ascii_alphanumeric()
                || matches!(bytes[offset], b'_' | b'-' | b'.'))
        {
            offset += 1;
        }
        let name_end = offset;
        while offset < bytes.len() && bytes[offset].is_ascii_whitespace() {
            offset += 1;
        }
        if offset >= bytes.len() || !matches!(bytes[offset], b':' | b'=') {
            continue;
        }
        offset += 1;
        while offset < bytes.len() && bytes[offset].is_ascii_whitespace() {
            offset += 1;
        }
        if offset == bytes.len() {
            break;
        }
        let value = if matches!(bytes[offset], b'\'' | b'"') {
            let (value, next) = quoted(bytes, offset);
            offset = next;
            value
        } else {
            let start = offset;
            while offset < bytes.len()
                && !bytes[offset].is_ascii_whitespace()
                && !b",;)}/".contains(&bytes[offset])
            {
                offset += 1;
            }
            String::from_utf8_lossy(&bytes[start..offset]).into_owned()
        };
        let key = String::from_utf8_lossy(&bytes[start..name_end]).into_owned();
        // Repeated fields are ambiguous evidence, including one empty occurrence.
        values
            .entry(key)
            .and_modify(|value: &mut String| value.clear())
            .or_insert(value);
    }
    values
}

fn quoted(bytes: &[u8], start: usize) -> (String, usize) {
    let mut offset = start + 1;
    while offset < bytes.len() {
        if bytes[offset] == b'\\' {
            offset += 2;
        } else if bytes[offset] == bytes[start] {
            return (
                String::from_utf8_lossy(&bytes[start + 1..offset])
                    .trim()
                    .into(),
                offset + 1,
            );
        } else {
            offset += 1;
        }
    }
    (String::new(), bytes.len())
}
