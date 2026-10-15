//! Tiny lowercase hex helpers (no extra crate). Used for `vault.json` fields
//! and the SQLCipher raw-key pragma.

use zeroize::Zeroizing;

const DIGITS: &[u8; 16] = b"0123456789abcdef";

/// Lowercase hex of `bytes`.
pub fn encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(char::from(DIGITS[usize::from(b >> 4)]));
        out.push(char::from(DIGITS[usize::from(b & 0x0f)]));
    }
    out
}

/// Lowercase hex of secret bytes, in a buffer that is wiped when dropped.
pub(crate) fn encode_secret(bytes: &[u8]) -> Zeroizing<String> {
    let mut out = Zeroizing::new(String::with_capacity(bytes.len() * 2));
    for b in bytes {
        out.push(char::from(DIGITS[usize::from(b >> 4)]));
        out.push(char::from(DIGITS[usize::from(b & 0x0f)]));
    }
    out
}

fn nibble(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

/// Decode hex (either case). `None` for odd length or a non-hex character.
pub fn decode(text: &str) -> Option<Vec<u8>> {
    let raw = text.as_bytes();
    if !raw.len().is_multiple_of(2) {
        return None;
    }
    let (pairs, _) = raw.as_chunks::<2>();
    pairs
        .iter()
        .map(|&[hi, lo]| Some((nibble(hi)? << 4) | nibble(lo)?))
        .collect()
}

/// Decode hex into exactly `N` bytes.
pub fn decode_array<const N: usize>(text: &str) -> Option<[u8; N]> {
    let bytes = decode(text)?;
    <[u8; N]>::try_from(bytes.as_slice()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let bytes = [0x00, 0x01, 0x7f, 0x80, 0xab, 0xff];
        let text = encode(&bytes);
        assert_eq!(text, "00017f80abff");
        assert_eq!(decode(&text), Some(bytes.to_vec()));
        assert_eq!(decode("ABFF"), Some(vec![0xab, 0xff]));
        assert_eq!(encode_secret(&bytes).as_str(), text);
        assert_eq!(decode(""), Some(vec![]));
    }

    #[test]
    fn rejects_bad_input() {
        assert_eq!(decode("abc"), None);
        assert_eq!(decode("zz"), None);
        assert_eq!(decode("0x00"), None);
        assert_eq!(decode(" 00"), None);
        assert_eq!(decode_array::<2>("0001"), Some([0, 1]));
        assert_eq!(decode_array::<2>("000102"), None);
    }
}
