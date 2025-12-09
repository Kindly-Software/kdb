//! License key format and validation
//! [TRADE SECRET]
//!
//! # Key Format
//!
//! ```text
//! KDLY-XXXX-XXXX-XXXX-XXXX (25 chars)
//! │    │    │    │    │
//! │    │    │    │    └── Checksum (4 chars, CRC16 + Base32)
//! │    │    │    └─────── Hardware binding hint (4 chars)
//! │    │    └──────────── Product ID (4 chars, e.g., "KAV1")
//! │    └───────────────── Random entropy (4 chars)
//! └────────────────────── Prefix "KDLY"
//! ```
//!
//! # Base32 Encoding
//!
//! Uses Crockford Base32 (0-9, A-Z excluding I, L, O, U) for
//! human-readable keys that avoid ambiguous characters.
//!
//! # Hardware Binding
//!
//! The hardware hint is a 4-character encoding of the expected
//! hardware fingerprint prefix. This allows quick rejection of
//! keys meant for different machines without computing the full
//! fingerprint signature.
//!
//! # Checksum
//!
//! CRC16-CCITT over the first 20 characters, encoded as 4 Base32 chars.
//! Catches typos and transmission errors.

use super::fingerprint::HardwareFingerprint;

/// Crockford Base32 alphabet (excludes I, L, O, U for clarity)
const BASE32_ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// Reverse lookup table for Base32 decoding
const BASE32_DECODE: [i8; 128] = {
    let mut table = [-1i8; 128];
    let mut i = 0;
    while i < 32 {
        table[BASE32_ALPHABET[i] as usize] = i as i8;
        // Also accept lowercase
        if BASE32_ALPHABET[i] >= b'A' && BASE32_ALPHABET[i] <= b'Z' {
            table[(BASE32_ALPHABET[i] + 32) as usize] = i as i8;
        }
        i += 1;
    }
    // Handle common substitutions
    table[b'i' as usize] = 1; // i -> 1
    table[b'I' as usize] = 1;
    table[b'l' as usize] = 1; // l -> 1
    table[b'L' as usize] = 1;
    table[b'o' as usize] = 0; // o -> 0
    table[b'O' as usize] = 0;
    table
};

/// License key errors
#[derive(Debug, PartialEq, Eq)]
pub enum LicenseKeyError {
    InvalidFormat,
    InvalidCharacters,
    InvalidPrefix,
    InvalidChecksum,
    InvalidProductId,
    HardwareMismatch,
    Expired,
}

impl std::fmt::Display for LicenseKeyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidFormat => write!(f, "Invalid format: expected KDLY-XXXX-XXXX-XXXX-XXXX"),
            Self::InvalidCharacters => write!(f, "Invalid characters in key"),
            Self::InvalidPrefix => write!(f, "Invalid prefix: expected KDLY"),
            Self::InvalidChecksum => write!(f, "Invalid checksum"),
            Self::InvalidProductId => write!(f, "Invalid product ID"),
            Self::HardwareMismatch => write!(f, "Hardware mismatch"),
            Self::Expired => write!(f, "License expired"),
        }
    }
}

impl std::error::Error for LicenseKeyError {}

/// Parsed license key
///
/// Contains the decoded components of a license key for validation
/// and signature generation.
#[derive(Debug, Clone)]
pub struct LicenseKey {
    /// Original key string (normalized to uppercase)
    raw: String,

    /// Random entropy bytes (decoded from chars 5-8)
    entropy: [u8; 4],

    /// Product ID bytes (decoded from chars 10-13)
    product_id: [u8; 4],

    /// Hardware hint bytes (decoded from chars 15-18)
    hardware_hint: [u8; 4],

    /// Checksum bytes (decoded from chars 20-23)
    checksum: [u8; 4],

    /// Optional expiry timestamp (encoded in entropy high bits)
    expiry: Option<u64>,
}

impl LicenseKey {
    /// Parse license key from string
    ///
    /// Accepts keys in format: `KDLY-XXXX-XXXX-XXXX-XXXX`
    /// Also accepts without dashes: `KDLYXXXXXXXXXXXXXXXX`
    ///
    /// # Example
    ///
    /// ```ignore
    /// let key = LicenseKey::parse("KDLY-A1B2-KAV1-C3D4-E5F6")?;
    /// ```
    pub fn parse(s: &str) -> Result<Self, LicenseKeyError> {
        // Normalize: uppercase and remove dashes
        let normalized: String = s
            .to_uppercase()
            .chars()
            .filter(|c| *c != '-')
            .collect();

        // Must be exactly 20 characters
        if normalized.len() != 20 {
            return Err(LicenseKeyError::InvalidFormat);
        }

        // Verify prefix
        if !normalized.starts_with("KDLY") {
            return Err(LicenseKeyError::InvalidPrefix);
        }

        // Decode each segment
        // Note: entropy, hardware_hint, checksum are decoded for internal use
        // product_id keeps raw ASCII bytes for comparison with known IDs like b"KAV1"
        let entropy = Self::decode_segment(&normalized[4..8])?;
        let product_id: [u8; 4] = normalized[8..12].as_bytes().try_into().unwrap();
        let hardware_hint = Self::decode_segment(&normalized[12..16])?;
        let checksum = Self::decode_segment(&normalized[16..20])?;

        let key = Self {
            raw: Self::format_key(&normalized),
            entropy,
            product_id,
            hardware_hint,
            checksum,
            expiry: None, // Computed separately if needed
        };

        // Validate checksum
        if !key.validate_checksum() {
            return Err(LicenseKeyError::InvalidChecksum);
        }

        Ok(key)
    }

    /// Validate the key checksum
    ///
    /// Uses CRC16-CCITT over the first 16 characters (prefix + entropy + product + hardware).
    pub fn validate_checksum(&self) -> bool {
        // Get the data portion (without checksum)
        let data: String = self.raw.chars().filter(|c| *c != '-').take(16).collect();

        // Calculate expected checksum
        let expected_crc = Self::crc16_ccitt(data.as_bytes());
        let expected_bytes = [
            ((expected_crc >> 12) & 0x0F) as u8,
            ((expected_crc >> 8) & 0x0F) as u8,
            ((expected_crc >> 4) & 0x0F) as u8,
            (expected_crc & 0x0F) as u8,
        ];

        // Compare with stored checksum (each byte represents a nibble)
        self.checksum[0] == expected_bytes[0]
            && self.checksum[1] == expected_bytes[1]
            && self.checksum[2] == expected_bytes[2]
            && self.checksum[3] == expected_bytes[3]
    }

    /// Verify against hardware fingerprint
    ///
    /// The hardware hint encodes the first 4 bytes of the expected fingerprint.
    /// This allows quick rejection before computing the full signature.
    pub fn verify_hardware(&self, fingerprint: &HardwareFingerprint) -> bool {
        let fp_bytes = fingerprint.as_bytes();

        // Hardware hint should match first 4 bytes of fingerprint hash
        // We use a derived value to avoid exposing raw fingerprint
        let derived = Self::derive_hardware_hint(fp_bytes);

        self.hardware_hint == derived
    }

    /// Generate signature for storage
    ///
    /// Creates a 32-byte Blake3 hash combining the key data with the
    /// hardware fingerprint. This signature is stored on disk and
    /// verified on subsequent loads.
    pub fn generate_signature(&self, fingerprint: &HardwareFingerprint) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();

        // Include key components
        hasher.update(&self.entropy);
        hasher.update(&self.product_id);
        hasher.update(&self.hardware_hint);

        // Include hardware fingerprint
        hasher.update(fingerprint.as_bytes());

        // Include a domain separator
        hasher.update(b"kindly-av1-license-v1");

        *hasher.finalize().as_bytes()
    }

    /// Get the raw key string
    pub fn raw(&self) -> &str {
        &self.raw
    }

    /// Get the product ID bytes
    pub fn product_id(&self) -> &[u8; 4] {
        &self.product_id
    }

    /// Get optional expiry timestamp
    ///
    /// If the key encodes an expiry date (professional/enterprise tiers),
    /// returns the Unix timestamp. Returns None for perpetual licenses.
    pub fn expiry_timestamp(&self) -> Option<u64> {
        // Expiry is encoded in entropy high byte
        // 0 = perpetual, otherwise days since 2020-01-01
        let days = self.entropy[0] as u64;
        if days == 0 {
            None
        } else {
            // Base date: 2020-01-01 00:00:00 UTC
            const BASE_TIMESTAMP: u64 = 1577836800;
            Some(BASE_TIMESTAMP + days * 86400)
        }
    }

    /// Generate a new license key (for key generation tools)
    ///
    /// # Arguments
    ///
    /// * `fingerprint` - Target hardware fingerprint
    /// * `product_id` - Product identifier (e.g., b"KAV1")
    /// * `expiry_days` - Days until expiry (0 = perpetual)
    /// * `entropy` - Random entropy bytes
    #[allow(dead_code)]
    pub fn generate(
        fingerprint: &HardwareFingerprint,
        product_id: [u8; 4],
        expiry_days: u8,
        entropy: [u8; 3],
    ) -> Self {
        // Build entropy with expiry
        let entropy_bytes = [expiry_days, entropy[0], entropy[1], entropy[2]];

        // Derive hardware hint
        let hardware_hint = Self::derive_hardware_hint(fingerprint.as_bytes());

        // Build key string for checksum
        // Note: product_id is stored as raw ASCII bytes (e.g., b"KAV1")
        // so we use it directly in the key string without Base32 encoding
        let mut key_data = String::with_capacity(16);
        key_data.push_str("KDLY");
        key_data.push_str(&Self::encode_segment(&entropy_bytes));
        key_data.push_str(std::str::from_utf8(&product_id).unwrap_or("????"));
        key_data.push_str(&Self::encode_segment(&hardware_hint));

        // Calculate checksum
        let crc = Self::crc16_ccitt(key_data.as_bytes());
        let checksum = [
            ((crc >> 12) & 0x0F) as u8,
            ((crc >> 8) & 0x0F) as u8,
            ((crc >> 4) & 0x0F) as u8,
            (crc & 0x0F) as u8,
        ];

        // Format final key
        key_data.push_str(&Self::encode_segment(&checksum));

        Self {
            raw: Self::format_key(&key_data),
            entropy: entropy_bytes,
            product_id,
            hardware_hint,
            checksum,
            expiry: if expiry_days == 0 {
                None
            } else {
                const BASE_TIMESTAMP: u64 = 1577836800;
                Some(BASE_TIMESTAMP + expiry_days as u64 * 86400)
            },
        }
    }

    /// Decode a 4-character Base32 segment to 4 bytes
    fn decode_segment(s: &str) -> Result<[u8; 4], LicenseKeyError> {
        if s.len() != 4 {
            return Err(LicenseKeyError::InvalidFormat);
        }

        let bytes: Vec<u8> = s.bytes().collect();
        let mut result = [0u8; 4];

        for (i, &b) in bytes.iter().enumerate() {
            if b >= 128 {
                return Err(LicenseKeyError::InvalidCharacters);
            }
            let val = BASE32_DECODE[b as usize];
            if val < 0 {
                return Err(LicenseKeyError::InvalidCharacters);
            }
            result[i] = val as u8;
        }

        Ok(result)
    }

    /// Encode 4 bytes to a 4-character Base32 segment
    fn encode_segment(bytes: &[u8; 4]) -> String {
        bytes
            .iter()
            .map(|&b| BASE32_ALPHABET[(b & 0x1F) as usize] as char)
            .collect()
    }

    /// Format key with dashes
    fn format_key(s: &str) -> String {
        let chars: Vec<char> = s.chars().collect();
        format!(
            "{}-{}-{}-{}-{}",
            chars[0..4].iter().collect::<String>(),
            chars[4..8].iter().collect::<String>(),
            chars[8..12].iter().collect::<String>(),
            chars[12..16].iter().collect::<String>(),
            chars[16..20].iter().collect::<String>(),
        )
    }

    /// Derive hardware hint from fingerprint
    fn derive_hardware_hint(fingerprint: &[u8; 32]) -> [u8; 4] {
        // Use first 4 bytes of fingerprint directly
        // This ensures different fingerprints produce different hints
        let mut hint = [0u8; 4];
        hint.copy_from_slice(&fingerprint[0..4]);
        // Mask to Base32 range (0-31)
        for b in &mut hint {
            *b &= 0x1F;
        }
        hint
    }

    /// CRC16-CCITT checksum
    fn crc16_ccitt(data: &[u8]) -> u16 {
        const POLYNOMIAL: u16 = 0x1021;
        let mut crc: u16 = 0xFFFF;

        for &byte in data {
            crc ^= (byte as u16) << 8;
            for _ in 0..8 {
                if crc & 0x8000 != 0 {
                    crc = (crc << 1) ^ POLYNOMIAL;
                } else {
                    crc <<= 1;
                }
            }
        }

        crc
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_key() {
        // Generate a test key
        let fingerprint = HardwareFingerprint::from_bytes([0xAA; 32]);
        let key = LicenseKey::generate(&fingerprint, *b"KAV1", 0, [0x12, 0x34, 0x56]);

        // Parse it back
        let parsed = LicenseKey::parse(key.raw()).unwrap();
        assert_eq!(parsed.product_id(), b"KAV1");
    }

    #[test]
    fn test_invalid_format() {
        assert_eq!(
            LicenseKey::parse("KDLY-1234").unwrap_err(),
            LicenseKeyError::InvalidFormat
        );
        assert_eq!(
            LicenseKey::parse("XXXX-1234-5678-9012-3456").unwrap_err(),
            LicenseKeyError::InvalidPrefix
        );
    }

    #[test]
    fn test_base32_decoding() {
        // Test valid characters
        let segment = LicenseKey::decode_segment("0123").unwrap();
        assert_eq!(segment, [0, 1, 2, 3]);

        // Test letters
        let segment = LicenseKey::decode_segment("ABCD").unwrap();
        assert_eq!(segment, [10, 11, 12, 13]);
    }

    #[test]
    fn test_hardware_verification() {
        let fingerprint = HardwareFingerprint::from_bytes([0xAA; 32]);
        let key = LicenseKey::generate(&fingerprint, *b"KAV1", 0, [0x12, 0x34, 0x56]);

        // Should verify against same fingerprint
        assert!(key.verify_hardware(&fingerprint));

        // Should fail against different fingerprint
        let different_fp = HardwareFingerprint::from_bytes([0xBB; 32]);
        assert!(!key.verify_hardware(&different_fp));
    }

    #[test]
    fn test_signature_generation() {
        let fingerprint = HardwareFingerprint::from_bytes([0xAA; 32]);
        let key = LicenseKey::generate(&fingerprint, *b"KAV1", 0, [0x12, 0x34, 0x56]);

        let sig1 = key.generate_signature(&fingerprint);
        let sig2 = key.generate_signature(&fingerprint);

        // Same inputs should produce same signature
        assert_eq!(sig1, sig2);

        // Different fingerprint should produce different signature
        let different_fp = HardwareFingerprint::from_bytes([0xBB; 32]);
        let sig3 = key.generate_signature(&different_fp);
        assert_ne!(sig1, sig3);
    }

    #[test]
    fn test_expiry_encoding() {
        let fingerprint = HardwareFingerprint::from_bytes([0xAA; 32]);

        // Perpetual license (0 days)
        let perpetual = LicenseKey::generate(&fingerprint, *b"KAV1", 0, [0x12, 0x34, 0x56]);
        assert!(perpetual.expiry_timestamp().is_none());

        // 30-day license
        let limited = LicenseKey::generate(&fingerprint, *b"KAV1", 30, [0x12, 0x34, 0x56]);
        assert!(limited.expiry_timestamp().is_some());
    }

    #[test]
    fn test_crc16_ccitt() {
        // Test vectors from https://reveng.sourceforge.io/crc-catalogue/16.htm
        let data = b"123456789";
        let crc = LicenseKey::crc16_ccitt(data);
        assert_eq!(crc, 0x29B1);
    }

    #[test]
    fn test_case_insensitive() {
        let fingerprint = HardwareFingerprint::from_bytes([0xAA; 32]);
        let key = LicenseKey::generate(&fingerprint, *b"KAV1", 0, [0x12, 0x34, 0x56]);

        // Should parse lowercase
        let lower = key.raw().to_lowercase();
        let parsed = LicenseKey::parse(&lower).unwrap();
        assert_eq!(parsed.product_id(), key.product_id());
    }

    #[test]
    fn test_without_dashes() {
        let fingerprint = HardwareFingerprint::from_bytes([0xAA; 32]);
        let key = LicenseKey::generate(&fingerprint, *b"KAV1", 0, [0x12, 0x34, 0x56]);

        // Should parse without dashes
        let no_dashes: String = key.raw().chars().filter(|c| *c != '-').collect();
        let parsed = LicenseKey::parse(&no_dashes).unwrap();
        assert_eq!(parsed.product_id(), key.product_id());
    }
}
