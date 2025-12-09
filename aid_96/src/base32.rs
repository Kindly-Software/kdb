use std::fmt;

/// Crockford Base32 alphabet (uppercase, no I/L/O/U).
const CROCKFORD_ALPHABET: [char; 32] = [
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'J',
    'K', 'M', 'N', 'P', 'Q', 'R', 'S', 'T', 'V', 'W', 'X', 'Y', 'Z',
];

/// Encode 96 bits (12 bytes) into a 20-character Crockford Base32 string.
pub fn encode(bytes: &[u8; 12]) -> String {
    let mut value = 0u128;
    for &byte in bytes {
        value = (value << 8) | byte as u128;
    }

    // Pad the low bits so 96 bits become 100 bits (20 symbols × 5 bits).
    value <<= 4;

    let mut encoded = String::with_capacity(20);
    for group in (0..20).rev() {
        let index = ((value >> (group * 5)) & 0x1F) as usize;
        encoded.push(CROCKFORD_ALPHABET[index]);
    }
    encoded
}

/// Errors that can occur while decoding a Crockford Base32 string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    InvalidLength { expected: usize, found: usize },
    InvalidChar { position: usize, found: char },
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DecodeError::InvalidLength { expected, found } => {
                write!(f, "expected {expected} characters, found {found}")
            }
            DecodeError::InvalidChar { position, found } => {
                write!(f, "invalid character '{found}' at position {position}")
            }
        }
    }
}

impl std::error::Error for DecodeError {}

/// Decode a 20-character Crockford Base32 string back into 12 bytes.
pub fn decode(input: &str) -> Result<[u8; 12], DecodeError> {
    if input.len() != 20 {
        return Err(DecodeError::InvalidLength {
            expected: 20,
            found: input.len(),
        });
    }

    let mut value = 0u128;
    for (idx, ch) in input.chars().enumerate() {
        let bits = decode_char(ch).ok_or(DecodeError::InvalidChar {
            position: idx,
            found: ch,
        })?;
        value = (value << 5) | bits as u128;
    }

    // Drop the padding we added during encode.
    value >>= 4;

    let mut out = [0u8; 12];
    for byte in out.iter_mut().rev() {
        *byte = (value & 0xFF) as u8;
        value >>= 8;
    }

    Ok(out)
}

fn decode_char(ch: char) -> Option<u8> {
    match ch {
        '0' => Some(0),
        'O' | 'o' => Some(0),
        '1' => Some(1),
        'I' | 'i' | 'L' | 'l' => Some(1),
        '2'..='9' => Some(ch as u8 - b'0'),
        'A'..='Z' | 'a'..='z' => {
            let upper = ch.to_ascii_uppercase();
            match upper {
                'A' => Some(10),
                'B' => Some(11),
                'C' => Some(12),
                'D' => Some(13),
                'E' => Some(14),
                'F' => Some(15),
                'G' => Some(16),
                'H' => Some(17),
                'J' => Some(18),
                'K' => Some(19),
                'M' => Some(20),
                'N' => Some(21),
                'P' => Some(22),
                'Q' => Some(23),
                'R' => Some(24),
                'S' => Some(25),
                'T' => Some(26),
                'V' => Some(27),
                'W' => Some(28),
                'X' => Some(29),
                'Y' => Some(30),
                'Z' => Some(31),
                _ => None,
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_base32() {
        let input = [0xAB; 12];
        let encoded = encode(&input);
        assert_eq!(encoded.len(), 20);
        let decoded = decode(&encoded).expect("decode should succeed");
        assert_eq!(decoded, input);
    }

    #[test]
    fn decode_is_case_insensitive_and_handles_aliases() {
        let encoded = "0o1labcdefghjkmnpqrs";
        let decoded = decode(encoded).expect("decode should succeed");
        let reencoded = encode(&decoded);
        assert_eq!(reencoded, reencoded.to_ascii_uppercase());
    }

    #[test]
    fn decode_rejects_wrong_length() {
        let err = decode("ABC").unwrap_err();
        assert!(matches!(
            err,
            DecodeError::InvalidLength {
                expected: 20,
                found: 3
            }
        ));
    }
}
