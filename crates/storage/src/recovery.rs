//! The recovery key: 256 random bits shown once as Crockford base32 in groups
//! of four (`XXXX-XXXX-…`, 52 characters in 13 groups). It is never stored.
//!
//! Typing is forgiving: case, spaces and hyphens are ignored, `O` reads as
//! `0`, and `I`/`L` read as `1`. The last character carries one data bit and
//! four zero padding bits; anything else there is a typo and is rejected.

use secrecy::{ExposeSecret, SecretBox, SecretString};
use zeroize::{Zeroize, Zeroizing};

/// Bytes of key material in a recovery key.
pub const RECOVERY_KEY_BYTES: usize = 32;
/// Base32 characters in a recovery key (256 bits / 5 bits, rounded up).
pub const RECOVERY_KEY_CHARS: usize = 52;
/// Characters per displayed group.
pub const RECOVERY_GROUP_CHARS: usize = 4;
/// Number of displayed groups.
pub const RECOVERY_GROUPS: usize = RECOVERY_KEY_CHARS / RECOVERY_GROUP_CHARS;

const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// A recovery key. Holds only the raw bits, wiped on drop. No `Debug`, no
/// `Clone`, no `Serialize`: it can't end up in a log or a file by accident.
pub struct RecoveryKey {
    bytes: SecretBox<[u8; RECOVERY_KEY_BYTES]>,
}

impl RecoveryKey {
    /// A fresh key from the operating system's random number generator.
    pub(crate) fn generate() -> Result<Self, getrandom::Error> {
        let mut bytes = SecretBox::new(Box::new([0u8; RECOVERY_KEY_BYTES]));
        getrandom::fill(secrecy::ExposeSecretMut::expose_secret_mut(&mut bytes))?;
        Ok(Self { bytes })
    }

    /// Read a key the user typed or pasted. `None` if it isn't a well-formed key
    /// (the caller reports the same generic error as for a wrong key).
    pub fn parse(text: &str) -> Option<Self> {
        let mut values = Zeroizing::new(Vec::with_capacity(RECOVERY_KEY_CHARS));
        for c in text.chars() {
            let v = match c.to_ascii_uppercase() {
                ' ' | '-' | '\t' | '\n' | '\r' => continue,
                'O' => 0,
                'I' | 'L' => 1,
                upper => {
                    let byte = u8::try_from(upper).ok()?;
                    let pos = ALPHABET.iter().position(|&a| a == byte)?;
                    u8::try_from(pos).ok()?
                }
            };
            if values.len() == RECOVERY_KEY_CHARS {
                return None;
            }
            values.push(v);
        }
        if values.len() != RECOVERY_KEY_CHARS {
            return None;
        }
        // 52 * 5 = 260 bits: 256 data bits then 4 padding bits that must be zero.
        if values[RECOVERY_KEY_CHARS - 1] & 0x0f != 0 {
            return None;
        }
        let mut bytes = SecretBox::new(Box::new([0u8; RECOVERY_KEY_BYTES]));
        {
            let out = secrecy::ExposeSecretMut::expose_secret_mut(&mut bytes);
            let mut acc: u32 = 0;
            let mut bits = 0;
            let mut i = 0;
            for &v in values.iter() {
                acc = (acc << 5) | u32::from(v);
                bits += 5;
                if bits >= 8 && i < RECOVERY_KEY_BYTES {
                    bits -= 8;
                    out[i] = ((acc >> bits) & 0xff) as u8;
                    i += 1;
                }
                acc &= (1 << bits) - 1;
            }
            acc.zeroize();
        }
        Some(Self { bytes })
    }

    /// The key as shown to the user: 13 groups of 4 characters joined by `-`.
    pub fn display(&self) -> SecretString {
        let raw = self.bytes.expose_secret();
        let mut out = Zeroizing::new(String::with_capacity(
            RECOVERY_KEY_CHARS + RECOVERY_GROUPS - 1,
        ));
        let mut acc: u32 = 0;
        let mut bits = 0;
        let mut written = 0;
        let mut push = |v: u32, out: &mut String| {
            if written > 0 && written % RECOVERY_GROUP_CHARS == 0 {
                out.push('-');
            }
            out.push(char::from(ALPHABET[(v & 0x1f) as usize]));
            written += 1;
        };
        for &b in raw.iter() {
            acc = (acc << 8) | u32::from(b);
            bits += 8;
            while bits >= 5 {
                bits -= 5;
                push(acc >> bits, &mut out);
            }
            acc &= (1 << bits) - 1;
        }
        if bits > 0 {
            push(acc << (5 - bits), &mut out);
        }
        acc.zeroize();
        SecretString::from(out.as_str())
    }

    /// The raw key bits, used as the key derivation input.
    pub(crate) fn as_bytes(&self) -> &[u8; RECOVERY_KEY_BYTES] {
        self.bytes.expose_secret()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key_from(bytes: [u8; 32]) -> RecoveryKey {
        RecoveryKey {
            bytes: SecretBox::new(Box::new(bytes)),
        }
    }

    #[test]
    fn display_shape() {
        let key = RecoveryKey::generate().unwrap();
        let shown = key.display();
        let text = shown.expose_secret();
        let groups: Vec<&str> = text.split('-').collect();
        assert_eq!(groups.len(), RECOVERY_GROUPS);
        assert!(groups.iter().all(|g| g.len() == RECOVERY_GROUP_CHARS));
        assert!(text
            .chars()
            .all(|c| c == '-' || ALPHABET.contains(&(c as u8))));
    }

    #[test]
    fn known_values() {
        let zero = key_from([0; 32]);
        assert_eq!(
            zero.display().expose_secret(),
            "0000-0000-0000-0000-0000-0000-0000-0000-0000-0000-0000-0000-0000"
        );
        let ones = key_from([0xff; 32]);
        // 51 full symbols of 11111, then 1 data bit + 4 zero padding bits = 10000 = G.
        let expected = format!("{}G", "Z".repeat(51));
        let shown = ones.display();
        assert_eq!(shown.expose_secret().replace('-', ""), expected);
    }

    #[test]
    fn round_trip_random_keys() {
        for _ in 0..50 {
            let key = RecoveryKey::generate().unwrap();
            let shown = key.display();
            let back = RecoveryKey::parse(shown.expose_secret()).unwrap();
            assert_eq!(back.as_bytes(), key.as_bytes());
        }
    }

    #[test]
    fn parsing_is_forgiving() {
        let mut bytes = [0u8; 32];
        for (i, b) in bytes.iter_mut().enumerate() {
            *b = (i as u8).wrapping_mul(37).wrapping_add(11);
        }
        let key = key_from(bytes);
        let shown = key.display();
        let text = shown.expose_secret().to_string();
        let variants = [
            text.to_lowercase(),
            text.replace('-', " "),
            text.replace('-', ""),
            format!("  {}\n", text.replace('-', " - ")),
            text.replace('0', "O").replace('1', "I"),
            text.replace('1', "l"),
        ];
        for v in variants {
            let back = RecoveryKey::parse(&v).unwrap_or_else(|| panic!("failed: {v}"));
            assert_eq!(back.as_bytes(), &bytes);
        }
    }

    #[test]
    fn rejects_malformed_keys() {
        let key = key_from([0xab; 32]);
        let shown = key.display();
        let text = shown.expose_secret().replace('-', "");
        assert!(RecoveryKey::parse("").is_none());
        assert!(RecoveryKey::parse(&text[..51]).is_none(), "too short");
        assert!(
            RecoveryKey::parse(&format!("{text}0")).is_none(),
            "too long"
        );
        assert!(RecoveryKey::parse(&text.replacen(&text[..1], "U", 1)).is_none());
        assert!(RecoveryKey::parse(&text.replacen(&text[..1], "!", 1)).is_none());
        // Non-zero padding bits in the last character.
        let mut bad = text.clone();
        bad.pop();
        bad.push('1');
        assert!(RecoveryKey::parse(&bad).is_none());
    }
}
