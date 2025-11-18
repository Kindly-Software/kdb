// Rust scalar baseline for B32 fair comparison
// NO SIMD, NO parallel, NO Bloom filter optimizations
// Pure scalar implementation to isolate optimization gains

use std::collections::HashMap;

/// Scalar MinHash implementation (no SIMD)
///
/// B32 Compliance: Isolates SIMD speedup component by using scalar operations only
pub struct ScalarMinHash {
    /// Hash values (scalar, not SIMD)
    hashes: Vec<u16>,
}

impl ScalarMinHash {
    /// Create new MinHash with num_hashes permutations
    pub fn new(num_hashes: usize) -> Self {
        Self {
            hashes: vec![u16::MAX; num_hashes],
        }
    }

    /// Update MinHash with token (scalar hash computation)
    pub fn update(&mut self, token: &str) {
        let token_bytes = token.as_bytes();

        // Scalar loop (NO SIMD)
        for (i, hash) in self.hashes.iter_mut().enumerate() {
            // MurmurHash3 (same as kindly_dedup, but scalar)
            let h = murmur3_hash_u16(token_bytes, i as u32);
            *hash = (*hash).min(h); // Scalar min (NOT SIMD)
        }
    }

    /// Get signature for similarity comparison
    pub fn signature(&self) -> &[u16] {
        &self.hashes
    }

    /// Estimate Jaccard similarity
    pub fn jaccard(&self, other: &ScalarMinHash) -> f64 {
        let matches = self
            .hashes
            .iter()
            .zip(other.hashes.iter())
            .filter(|(a, b)| a == b)
            .count();

        matches as f64 / self.hashes.len() as f64
    }
}

/// MurmurHash3 32-bit hash (scalar implementation)
///
/// Identical to atomic_capsule implementation for fairness
/// Source: atomic_capsule/src/probabilistic/minhash.rs lines 282-330
fn murmur3_hash(data: &[u8], seed: u32) -> u32 {
    const C1: u32 = 0xcc9e_2d51;
    const C2: u32 = 0x1b87_3593;
    const R1: u32 = 15;
    const R2: u32 = 13;
    const M: u32 = 5;
    const N: u32 = 0xe654_6b64;

    let mut hash = seed;
    let mut i = 0;

    // Process 4 bytes at a time
    while i + 4 <= data.len() {
        let mut k = u32::from_le_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]]);

        k = k.wrapping_mul(C1);
        k = k.rotate_left(R1);
        k = k.wrapping_mul(C2);

        hash ^= k;
        hash = hash.rotate_left(R2);
        hash = hash.wrapping_mul(M).wrapping_add(N);

        i += 4;
    }

    // Process remaining bytes
    let mut k = 0u32;
    let remaining = data.len() - i;
    if remaining > 0 {
        for j in 0..remaining {
            k ^= (data[i + j] as u32) << (j * 8);
        }
        k = k.wrapping_mul(C1);
        k = k.rotate_left(R1);
        k = k.wrapping_mul(C2);
        hash ^= k;
    }

    // Finalization
    hash ^= data.len() as u32;
    hash ^= hash >> 16;
    hash = hash.wrapping_mul(0x85eb_ca6b);
    hash ^= hash >> 13;
    hash = hash.wrapping_mul(0xc2b2_ae35);
    hash ^= hash >> 16;

    hash
}

/// Truncate MurmurHash3 to u16
#[inline(always)]
fn murmur3_hash_u16(data: &[u8], seed: u32) -> u16 {
    let hash32 = murmur3_hash(data, seed);
    (hash32 & 0xFFFF) as u16
}

/// Scalar tokenization (same as kindly_dedup)
pub fn scalar_tokenize(text: &str) -> Vec<String> {
    text.split_whitespace().map(|s| s.to_lowercase()).collect()
}

/// Scalar LSH implementation (basic band hashing)
pub struct ScalarLSH {
    /// Number of bands
    num_bands: usize,

    /// Rows per band
    rows_per_band: usize,

    /// Buckets: band_idx -> bucket_hash -> Vec<doc_id>
    buckets: Vec<HashMap<Vec<u16>, Vec<usize>>>,
}

impl ScalarLSH {
    /// Create new scalar LSH
    pub fn new(num_bands: usize, rows_per_band: usize) -> Self {
        Self {
            num_bands,
            rows_per_band,
            buckets: vec![HashMap::new(); num_bands],
        }
    }

    /// Insert MinHash signature into LSH
    pub fn insert(&mut self, doc_id: usize, signature: &[u16]) {
        for band_idx in 0..self.num_bands {
            let start = band_idx * self.rows_per_band;
            let end = start + self.rows_per_band;
            let band = signature[start..end].to_vec();

            self.buckets[band_idx].entry(band).or_insert_with(Vec::new).push(doc_id);
        }
    }

    /// Query LSH for candidate duplicates
    pub fn query(&self, signature: &[u16]) -> Vec<usize> {
        let mut candidates = Vec::new();

        for band_idx in 0..self.num_bands {
            let start = band_idx * self.rows_per_band;
            let end = start + self.rows_per_band;
            let band = &signature[start..end];

            if let Some(docs) = self.buckets[band_idx].get(band) {
                candidates.extend_from_slice(docs);
            }
        }

        // Deduplicate candidates
        candidates.sort_unstable();
        candidates.dedup();
        candidates
    }
}

/// Scalar deduplication pipeline (baseline)
///
/// B32 Compliance:
/// - NO SIMD (scalar MinHash computation)
/// - NO parallel processing (single-threaded)
/// - NO Bloom filter (no pre-filtering)
/// - NO lockfree collections (HashMap)
///
/// This isolates the speedup from tier optimizations
pub struct ScalarDedupPipeline {
    /// Document signatures
    signatures: HashMap<usize, ScalarMinHash>,

    /// LSH index
    lsh: ScalarLSH,

    /// Number of bands (default 5)
    num_bands: usize,

    /// Rows per band (default 25 for 128 hashes)
    rows_per_band: usize,
}

impl ScalarDedupPipeline {
    /// Create new scalar dedup pipeline
    pub fn new(num_bands: usize) -> Self {
        let rows_per_band = 128 / num_bands;

        Self {
            signatures: HashMap::new(),
            lsh: ScalarLSH::new(num_bands, rows_per_band),
            num_bands,
            rows_per_band,
        }
    }

    /// Add document to pipeline (scalar processing)
    pub fn add_document(&mut self, doc_id: usize, text: &str) {
        // Scalar tokenization
        let tokens = scalar_tokenize(text);

        // Scalar MinHash computation (NO SIMD)
        let mut minhash = ScalarMinHash::new(128);
        for token in tokens {
            minhash.update(&token);
        }

        // Insert into LSH
        self.lsh.insert(doc_id, minhash.signature());

        // Store signature
        self.signatures.insert(doc_id, minhash);
    }

    /// Find duplicates with Jaccard threshold
    pub fn find_duplicates(&self, threshold: f64) -> Vec<Vec<usize>> {
        let mut clusters = Vec::new();
        let mut visited = std::collections::HashSet::new();

        for (&doc_id, signature) in &self.signatures {
            if visited.contains(&doc_id) {
                continue;
            }

            // Query LSH for candidates
            let candidates = self.lsh.query(signature.signature());

            // Verify with Jaccard similarity
            let mut cluster = vec![doc_id];
            visited.insert(doc_id);

            for &candidate_id in &candidates {
                if candidate_id == doc_id || visited.contains(&candidate_id) {
                    continue;
                }

                if let Some(candidate_sig) = self.signatures.get(&candidate_id) {
                    let similarity = signature.jaccard(candidate_sig);
                    if similarity >= threshold {
                        cluster.push(candidate_id);
                        visited.insert(candidate_id);
                    }
                }
            }

            if cluster.len() > 1 {
                clusters.push(cluster);
            }
        }

        clusters
    }

    /// Get number of documents processed
    pub fn num_documents(&self) -> usize {
        self.signatures.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scalar_minhash() {
        let mut mh1 = ScalarMinHash::new(128);
        let mut mh2 = ScalarMinHash::new(128);

        mh1.update("quick");
        mh1.update("brown");
        mh1.update("fox");

        mh2.update("quick");
        mh2.update("brown");
        mh2.update("fox");

        // Identical sets should have Jaccard ~1.0
        let similarity = mh1.jaccard(&mh2);
        assert!(similarity > 0.9);
    }

    #[test]
    fn test_scalar_pipeline() {
        let mut pipeline = ScalarDedupPipeline::new(5);

        pipeline.add_document(0, "The quick brown fox");
        pipeline.add_document(1, "The quick brown fox");
        pipeline.add_document(2, "A different document");

        let clusters = pipeline.find_duplicates(0.85);

        // Should find 1 cluster (docs 0 and 1)
        assert_eq!(clusters.len(), 1);
        assert_eq!(clusters[0].len(), 2);
    }

    #[test]
    fn test_murmur3_consistency() {
        let data = b"test string";

        // Same seed should produce same hash
        let h1 = murmur3_hash(data, 42);
        let h2 = murmur3_hash(data, 42);
        assert_eq!(h1, h2);

        // Different seeds should produce different hashes
        let h3 = murmur3_hash(data, 43);
        assert_ne!(h1, h3);
    }
}
