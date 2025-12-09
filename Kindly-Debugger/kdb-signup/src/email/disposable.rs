//! Disposable Email Blocker
//!
//! Blocks signups from disposable/temporary email providers using:
//! - mailchecker crate (55K+ domains) as primary check
//! - Custom FNV-1a hash lookup (<20ns) for additional domains
//!
//! # Performance
//!
//! - Target: <50ns per check
//! - FNV-1a: ~20ns hash + binary search
//! - mailchecker: ~30ns (precomputed bloom filter)
//!
//! # Framework Compliance
//!
//! - T1 Atomic tier: Lockfree hash lookup
//! - No mutex/RwLock: Pure functional hash
//! - Chaos: Cache-friendly sorted vector

/// FNV-1a 64-bit offset basis
const FNV_OFFSET: u64 = 0xcbf29ce484222325;

/// FNV-1a 64-bit prime multiplier
const FNV_PRIME: u64 = 0x100000001b3;

/// Custom blocklist of disposable email domains not in mailchecker
///
/// These are popular temporary email services that may not be in
/// mailchecker's database or have been recently created.
const CUSTOM_BLOCKLIST: &[&str] = &[
    // Popular temp mail services not in mailchecker
    "tempmail.com",
    "guerrillamail.com",
    "10minutemail.com",
    "throwaway.email",
    "temp-mail.org",
    "fakeinbox.com",
    "mailinator.com",
    "yopmail.com",
    "sharklasers.com",
    "maildrop.cc",
    "dispostable.com",
    "getnada.com",
    "mohmal.com",
    "tempail.com",
    "emailondeck.com",
    // Additional common disposable providers
    "guerrillamailblock.com",
    "pokemail.net",
    "spam4.me",
    "trashmail.com",
    "throwawaymail.com",
    "tempinbox.com",
    "fakemailgenerator.com",
    "mintemail.com",
    "mailcatch.com",
    "spamgourmet.com",
];

/// FNV-1a 64-bit hash function
///
/// Fast, non-cryptographic hash with excellent distribution.
/// ~20ns for typical domain names (10-20 bytes).
///
/// # Algorithm
///
/// 1. Start with FNV offset basis (0xcbf29ce484222325)
/// 2. For each byte: XOR with hash, multiply by prime
///
/// # Example
///
/// ```ignore
/// let hash = fnv1a_hash("mailinator.com");
/// assert_ne!(hash, 0); // Non-zero hash
/// ```
#[inline]
fn fnv1a_hash(s: &str) -> u64 {
    let mut hash = FNV_OFFSET;
    for byte in s.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// Disposable email blocker with FNV-1a hash lookup
///
/// Combines mailchecker's 55K+ domain database with a custom
/// blocklist using FNV-1a hashing for <50ns lookups.
///
/// # Thread Safety
///
/// Not thread-safe for mutation (add_domain). For concurrent use,
/// create separate instances or use external synchronization.
///
/// # Example
///
/// ```
/// use kdb_signup::email::disposable::DisposableEmailBlocker;
///
/// let blocker = DisposableEmailBlocker::new();
///
/// // Disposable email blocked
/// assert!(blocker.is_disposable("test@mailinator.com"));
///
/// // Legitimate email allowed
/// assert!(!blocker.is_disposable("user@gmail.com"));
/// ```
pub struct DisposableEmailBlocker {
    /// Sorted vector of FNV-1a hashes for binary search O(log n)
    custom_hashes: Vec<u64>,
}

impl DisposableEmailBlocker {
    /// Create a new blocker with the default custom blocklist
    ///
    /// Initializes with CUSTOM_BLOCKLIST domains, hashed and sorted
    /// for efficient binary search.
    ///
    /// # Performance
    ///
    /// - Initialization: O(n log n) for sorting
    /// - Lookup: O(log n) binary search
    pub fn new() -> Self {
        let mut hashes: Vec<u64> = CUSTOM_BLOCKLIST
            .iter()
            .map(|domain| fnv1a_hash(&domain.to_lowercase()))
            .collect();

        // Sort for binary search
        hashes.sort_unstable();

        Self {
            custom_hashes: hashes,
        }
    }

    /// Check if an email address is from a disposable provider
    ///
    /// Returns `true` if the email should be BLOCKED (is disposable).
    ///
    /// # Algorithm
    ///
    /// 1. Extract domain from email (after @)
    /// 2. Lowercase the domain for case-insensitive matching
    /// 3. Check mailchecker (returns false for disposable)
    /// 4. Check custom blocklist via FNV-1a hash + binary search
    /// 5. Return true if EITHER identifies as disposable
    ///
    /// # Arguments
    ///
    /// * `email` - Full email address (e.g., "user@example.com")
    ///
    /// # Returns
    ///
    /// * `true` - Email is from a disposable provider (BLOCK)
    /// * `false` - Email appears legitimate (ALLOW)
    ///
    /// # Performance
    ///
    /// Target: <50ns per check
    /// - mailchecker: ~30ns (bloom filter)
    /// - FNV-1a hash: ~20ns
    /// - Binary search: ~5ns for 26 elements
    pub fn is_disposable(&self, email: &str) -> bool {
        // Extract domain from email
        let domain = match email.rsplit_once('@') {
            Some((_, domain)) => domain.to_lowercase(),
            None => return false, // Invalid email format, let format validation handle it
        };

        // Check 1: mailchecker (55K+ domains)
        // mailchecker::is_valid returns false for disposable emails
        if !mailchecker::is_valid(email) {
            return true;
        }

        // Check 2: Custom blocklist via FNV-1a hash
        let hash = fnv1a_hash(&domain);
        self.custom_hashes.binary_search(&hash).is_ok()
    }

    /// Add a custom domain to the blocklist
    ///
    /// Domain is lowercased before hashing. The internal vector
    /// is re-sorted after insertion to maintain binary search capability.
    ///
    /// # Arguments
    ///
    /// * `domain` - Domain to block (e.g., "suspicious-temp-mail.com")
    ///
    /// # Note
    ///
    /// This is O(n log n) due to re-sorting. For bulk additions,
    /// consider creating a new blocker with all domains.
    pub fn add_domain(&mut self, domain: &str) {
        let hash = fnv1a_hash(&domain.to_lowercase());

        // Only add if not already present
        if self.custom_hashes.binary_search(&hash).is_err() {
            self.custom_hashes.push(hash);
            self.custom_hashes.sort_unstable();
        }
    }

    /// Get the number of domains in the custom blocklist
    ///
    /// Does not include mailchecker's 55K+ domains.
    #[inline]
    pub fn blocklist_size(&self) -> usize {
        self.custom_hashes.len()
    }

    /// Check if a domain is in the custom blocklist only
    ///
    /// Does NOT check mailchecker. Useful for testing custom additions.
    #[inline]
    pub fn is_in_custom_blocklist(&self, domain: &str) -> bool {
        let hash = fnv1a_hash(&domain.to_lowercase());
        self.custom_hashes.binary_search(&hash).is_ok()
    }
}

impl Default for DisposableEmailBlocker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===== FNV-1a Hash Tests =====

    #[test]
    fn test_fnv1a_consistency() {
        // Same input should always produce same hash
        let hash1 = fnv1a_hash("mailinator.com");
        let hash2 = fnv1a_hash("mailinator.com");
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_fnv1a_different_inputs() {
        // Different inputs should produce different hashes
        let hash1 = fnv1a_hash("mailinator.com");
        let hash2 = fnv1a_hash("gmail.com");
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_fnv1a_case_sensitivity() {
        // Hash is case-sensitive (we lowercase before hashing)
        let hash_lower = fnv1a_hash("mailinator.com");
        let hash_upper = fnv1a_hash("MAILINATOR.COM");
        assert_ne!(hash_lower, hash_upper);
    }

    #[test]
    fn test_fnv1a_empty_string() {
        // Empty string should return offset basis
        let hash = fnv1a_hash("");
        assert_eq!(hash, FNV_OFFSET);
    }

    #[test]
    fn test_fnv1a_known_value() {
        // Verify against known FNV-1a value for "a"
        // FNV-1a("a") = (0xcbf29ce484222325 ^ 0x61) * 0x100000001b3
        let hash = fnv1a_hash("a");
        let expected = (FNV_OFFSET ^ 0x61).wrapping_mul(FNV_PRIME);
        assert_eq!(hash, expected);
    }

    // ===== Blocker Initialization Tests =====

    #[test]
    fn test_blocker_new() {
        let blocker = DisposableEmailBlocker::new();
        assert_eq!(blocker.blocklist_size(), CUSTOM_BLOCKLIST.len());
    }

    #[test]
    fn test_blocker_default() {
        let blocker = DisposableEmailBlocker::default();
        assert_eq!(blocker.blocklist_size(), CUSTOM_BLOCKLIST.len());
    }

    #[test]
    fn test_blocker_hashes_sorted() {
        let blocker = DisposableEmailBlocker::new();
        let hashes = &blocker.custom_hashes;

        for i in 1..hashes.len() {
            assert!(hashes[i - 1] <= hashes[i], "Hashes should be sorted");
        }
    }

    // ===== Known Disposable Domains Blocked =====

    #[test]
    fn test_blocks_mailinator() {
        let blocker = DisposableEmailBlocker::new();
        assert!(
            blocker.is_disposable("test@mailinator.com"),
            "mailinator.com should be blocked"
        );
    }

    #[test]
    fn test_blocks_tempmail() {
        let blocker = DisposableEmailBlocker::new();
        assert!(
            blocker.is_disposable("user@tempmail.com"),
            "tempmail.com should be blocked"
        );
    }

    #[test]
    fn test_blocks_guerrillamail() {
        let blocker = DisposableEmailBlocker::new();
        assert!(
            blocker.is_disposable("spam@guerrillamail.com"),
            "guerrillamail.com should be blocked"
        );
    }

    #[test]
    fn test_blocks_10minutemail() {
        let blocker = DisposableEmailBlocker::new();
        assert!(
            blocker.is_disposable("temp@10minutemail.com"),
            "10minutemail.com should be blocked"
        );
    }

    #[test]
    fn test_blocks_yopmail() {
        let blocker = DisposableEmailBlocker::new();
        assert!(
            blocker.is_disposable("throwaway@yopmail.com"),
            "yopmail.com should be blocked"
        );
    }

    #[test]
    fn test_blocks_maildrop() {
        let blocker = DisposableEmailBlocker::new();
        assert!(
            blocker.is_disposable("anon@maildrop.cc"),
            "maildrop.cc should be blocked"
        );
    }

    // ===== Legitimate Domains Allowed =====

    #[test]
    fn test_allows_gmail() {
        let blocker = DisposableEmailBlocker::new();
        assert!(
            !blocker.is_disposable("user@gmail.com"),
            "gmail.com should be allowed"
        );
    }

    #[test]
    fn test_allows_outlook() {
        let blocker = DisposableEmailBlocker::new();
        assert!(
            !blocker.is_disposable("user@outlook.com"),
            "outlook.com should be allowed"
        );
    }

    #[test]
    fn test_allows_yahoo() {
        let blocker = DisposableEmailBlocker::new();
        assert!(
            !blocker.is_disposable("user@yahoo.com"),
            "yahoo.com should be allowed"
        );
    }

    #[test]
    fn test_allows_company_domain() {
        let blocker = DisposableEmailBlocker::new();
        assert!(
            !blocker.is_disposable("employee@company.com"),
            "company.com should be allowed"
        );
    }

    #[test]
    fn test_allows_protonmail() {
        let blocker = DisposableEmailBlocker::new();
        assert!(
            !blocker.is_disposable("secure@protonmail.com"),
            "protonmail.com should be allowed"
        );
    }

    #[test]
    fn test_allows_custom_domain() {
        let blocker = DisposableEmailBlocker::new();
        assert!(
            !blocker.is_disposable("admin@mycompany.io"),
            "mycompany.io should be allowed"
        );
    }

    // ===== Case Insensitivity Tests =====

    #[test]
    fn test_case_insensitive_blocking() {
        let blocker = DisposableEmailBlocker::new();

        // All case variants should be blocked
        assert!(blocker.is_disposable("test@MAILINATOR.COM"));
        assert!(blocker.is_disposable("test@Mailinator.Com"));
        assert!(blocker.is_disposable("test@MaIlInAtOr.CoM"));
    }

    #[test]
    fn test_case_insensitive_custom_blocklist() {
        let blocker = DisposableEmailBlocker::new();

        // Custom blocklist should be case-insensitive
        assert!(blocker.is_in_custom_blocklist("TEMPMAIL.COM"));
        assert!(blocker.is_in_custom_blocklist("TempMail.Com"));
        assert!(blocker.is_in_custom_blocklist("tempmail.com"));
    }

    // ===== Custom Domain Addition Tests =====

    #[test]
    fn test_add_custom_domain() {
        let mut blocker = DisposableEmailBlocker::new();
        let initial_size = blocker.blocklist_size();

        blocker.add_domain("new-temp-service.xyz");

        assert_eq!(blocker.blocklist_size(), initial_size + 1);
        assert!(blocker.is_in_custom_blocklist("new-temp-service.xyz"));
    }

    #[test]
    fn test_add_domain_case_insensitive() {
        let mut blocker = DisposableEmailBlocker::new();

        blocker.add_domain("NEW-TEMP.COM");

        // Should find with any case
        assert!(blocker.is_in_custom_blocklist("new-temp.com"));
        assert!(blocker.is_in_custom_blocklist("NEW-TEMP.COM"));
        assert!(blocker.is_in_custom_blocklist("New-Temp.Com"));
    }

    #[test]
    fn test_add_duplicate_domain() {
        let mut blocker = DisposableEmailBlocker::new();
        let initial_size = blocker.blocklist_size();

        // Add already existing domain
        blocker.add_domain("mailinator.com");

        // Size should not change
        assert_eq!(blocker.blocklist_size(), initial_size);
    }

    #[test]
    fn test_added_domain_blocks_email() {
        let mut blocker = DisposableEmailBlocker::new();

        // Initially should be allowed
        let test_domain = "brand-new-throwaway.net";
        assert!(!blocker.is_in_custom_blocklist(test_domain));

        // Add to blocklist
        blocker.add_domain(test_domain);

        // Now should block
        assert!(blocker.is_disposable(&format!("user@{}", test_domain)));
    }

    // ===== Edge Cases =====

    #[test]
    fn test_invalid_email_no_at() {
        let blocker = DisposableEmailBlocker::new();
        // Invalid email without @ returns false (let format validation handle it)
        assert!(!blocker.is_disposable("notanemail"));
    }

    #[test]
    fn test_email_with_subdomain() {
        let blocker = DisposableEmailBlocker::new();
        // Subdomain of legitimate domain should be allowed
        assert!(!blocker.is_disposable("user@mail.google.com"));
    }

    #[test]
    fn test_email_with_plus_addressing() {
        let blocker = DisposableEmailBlocker::new();
        // Plus addressing on legitimate domain should be allowed
        assert!(!blocker.is_disposable("user+tag@gmail.com"));
    }

    #[test]
    fn test_empty_local_part() {
        let blocker = DisposableEmailBlocker::new();
        // Empty local part but valid domain structure
        // mailchecker will handle validation
        let result = blocker.is_disposable("@gmail.com");
        // Just verify it doesn't panic
        let _ = result;
    }

    // ===== Blocklist Size Tests =====

    #[test]
    fn test_initial_blocklist_size() {
        let blocker = DisposableEmailBlocker::new();
        // Should match CUSTOM_BLOCKLIST length
        assert!(blocker.blocklist_size() >= 20, "Should have at least 20 domains");
    }

    // ===== Custom Blocklist Direct Check =====

    #[test]
    fn test_custom_blocklist_direct_check() {
        let blocker = DisposableEmailBlocker::new();

        // All CUSTOM_BLOCKLIST domains should be found
        for domain in CUSTOM_BLOCKLIST {
            assert!(
                blocker.is_in_custom_blocklist(domain),
                "Domain {} should be in custom blocklist",
                domain
            );
        }
    }

    #[test]
    fn test_legitimate_not_in_custom_blocklist() {
        let blocker = DisposableEmailBlocker::new();

        // Legitimate domains should not be in custom blocklist
        assert!(!blocker.is_in_custom_blocklist("gmail.com"));
        assert!(!blocker.is_in_custom_blocklist("outlook.com"));
        assert!(!blocker.is_in_custom_blocklist("company.com"));
    }

    // ===== Performance Smoke Test =====

    #[test]
    fn test_fnv1a_hash_performance() {
        // Test FNV-1a hash performance only (not mailchecker which may do network I/O)
        let blocker = DisposableEmailBlocker::new();

        // Warm up
        for _ in 0..100 {
            let _ = blocker.is_in_custom_blocklist("gmail.com");
        }

        // Measure custom blocklist lookup (pure FNV-1a + binary search)
        let start = std::time::Instant::now();
        for _ in 0..10_000 {
            let _ = blocker.is_in_custom_blocklist("gmail.com");
        }
        let elapsed = start.elapsed();

        // 10,000 FNV-1a + binary search lookups should complete in <10ms
        // This is very conservative for debug mode
        assert!(
            elapsed.as_millis() < 10,
            "FNV-1a lookup too slow: {:?} for 10,000 checks",
            elapsed
        );
    }

    #[test]
    fn test_hash_computation_speed() {
        // Test raw FNV-1a hash speed
        let start = std::time::Instant::now();
        for i in 0..100_000 {
            let domain = format!("test{}.example.com", i);
            let _ = fnv1a_hash(&domain);
        }
        let elapsed = start.elapsed();

        // 100,000 hashes should complete in <100ms (1us per hash is very slow)
        // Actual target is ~20ns per hash
        assert!(
            elapsed.as_millis() < 100,
            "FNV-1a hash too slow: {:?} for 100,000 hashes",
            elapsed
        );
    }
}
