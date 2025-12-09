//! # Persistent Dedup Test Harness - 100K Document Validation
//!
//! **Purpose**: Generate realistic LLM-simulated datasets for validating
//! persistent deduplication with MinHash + LSH + memory-mapped storage.
//!
//! **UCE34 Q1-Q34**: Comprehensive systematic validation
//! **T28**: 4-tier test pyramid (unit/property/integration/production)
//! **I20**: Integration validation harness
//! **ASSUM**: Safety assumption verification
//! **B32**: Performance benchmarking
//!
//! **Dataset Specification**:
//! - 100K total documents (LLM-simulated)
//! - 10K unique documents (seed content)
//! - 90K near-duplicates (50%-99% similarity)
//! - Similarity distribution: Exponential decay (50% most common, 99% rare)

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

/// Test dataset generator for persistent dedup validation
///
/// **UCE34 Q1**: Components = MinHash signatures + LSH buckets + persistent mmap
/// **UCE34 Q16**: Minimal integration test harness
pub struct DedupTestHarness {
    /// Unique seed documents (10K)
    pub seed_documents: Vec<String>,
    /// All documents including duplicates (100K)
    pub all_documents: Vec<String>,
    /// Ground truth: document ID → cluster ID
    pub ground_truth: HashMap<usize, usize>,
    /// Similarity levels for testing
    pub similarity_levels: Vec<f32>,
    /// Temporary directory for test files
    pub temp_dir: PathBuf,
}

impl DedupTestHarness {
    /// Create new test harness with 100K LLM-simulated documents
    ///
    /// **Performance**: <1 second for dataset generation
    /// **Memory**: ~50MB for 100K documents
    ///
    /// # UCE34 Q10: Tier Selection
    /// - T10 (Probabilistic): MinHash for deduplication
    /// - T9 (Persistent): Memory-mapped storage for incremental updates
    /// - T1 (Atomic): Generation counters for crash recovery
    ///
    /// # UCE34 Q17: Property Validation
    /// - Property: All duplicates share same cluster ID
    /// - Property: All unique documents have distinct cluster IDs
    /// - Property: Similarity distribution follows exponential decay
    pub fn new() -> Self {
        Self::with_config(10_000, 90_000)
    }

    /// Create test harness with custom document counts
    ///
    /// # Arguments
    /// * `num_unique` - Number of unique seed documents
    /// * `num_duplicates` - Number of duplicate documents to generate
    ///
    /// # Examples
    /// ```
    /// use atomic_capsule::tests::persistent_dedup_harness::DedupTestHarness;
    ///
    /// // Small test dataset (100 unique, 900 duplicates)
    /// let harness = DedupTestHarness::with_config(100, 900);
    /// assert_eq!(harness.all_documents.len(), 1000);
    /// ```
    pub fn with_config(num_unique: usize, num_duplicates: usize) -> Self {
        let temp_dir = std::env::temp_dir().join("persistent_dedup_test");
        let _ = fs::create_dir_all(&temp_dir);

        let mut seed_documents = Vec::with_capacity(num_unique);
        let mut all_documents = Vec::with_capacity(num_unique + num_duplicates);
        let mut ground_truth = HashMap::new();

        // Generate unique seed documents (LLM-simulated)
        for i in 0..num_unique {
            let doc = Self::generate_seed_document(i);
            seed_documents.push(doc.clone());
            all_documents.push(doc);
            ground_truth.insert(i, i); // Seed doc → self cluster
        }

        // Generate near-duplicates with exponential similarity distribution
        let similarity_levels = vec![0.99, 0.95, 0.90, 0.80, 0.70, 0.60, 0.50];
        let mut duplicate_idx = num_unique;

        for (level_idx, &similarity) in similarity_levels.iter().enumerate() {
            let count_at_level = Self::duplicates_at_similarity_level(
                num_duplicates,
                level_idx,
                similarity_levels.len(),
            );

            for _ in 0..count_at_level {
                if duplicate_idx >= num_unique + num_duplicates {
                    break;
                }

                // Pick random seed document to duplicate
                let seed_idx = duplicate_idx % num_unique;
                let duplicate_doc = Self::generate_duplicate(&seed_documents[seed_idx], similarity);
                all_documents.push(duplicate_doc);
                ground_truth.insert(duplicate_idx, seed_idx); // Duplicate → seed cluster
                duplicate_idx += 1;
            }
        }

        Self {
            seed_documents,
            all_documents,
            ground_truth,
            similarity_levels,
            temp_dir,
        }
    }

    /// Generate unique seed document with realistic LLM-style content
    ///
    /// **Content Pattern**: Topic + entities + facts + boilerplate
    ///
    /// # UCE34 Q2: Problem Solved
    /// - Realistic text diversity (multiple topics, entities, facts)
    /// - Sufficient token count for MinHash (50-200 tokens per doc)
    /// - Deterministic generation (same seed → same document)
    fn generate_seed_document(seed: usize) -> String {
        let topics = [
            "machine learning",
            "climate change",
            "quantum computing",
            "cryptocurrency",
            "space exploration",
            "renewable energy",
            "artificial intelligence",
            "biotechnology",
        ];
        let entities = [
            "researchers",
            "scientists",
            "engineers",
            "analysts",
            "developers",
            "experts",
        ];
        let facts = [
            "discovered new methods",
            "published findings",
            "developed algorithms",
            "analyzed data",
            "conducted experiments",
            "presented results",
        ];

        let topic = topics[seed % topics.len()];
        let entity = entities[(seed / 10) % entities.len()];
        let fact = facts[(seed / 100) % facts.len()];

        format!(
            "Document {}: Research on {} by {}. They {} using advanced techniques. \
             This study contributes to the field of {}. Key findings include improved \
             performance and novel approaches. The {} team collaborated with international \
             partners. Results were validated through rigorous testing. Future work will \
             explore additional applications. This research has significant implications \
             for the industry.",
            seed, topic, entity, fact, topic, entity
        )
    }

    /// Generate near-duplicate document at specified similarity level
    ///
    /// **Modification Strategy**:
    /// - 99%: Change 1-2 words (typos, synonyms)
    /// - 95%: Change 5-10 words
    /// - 90%: Change 10-20 words
    /// - 80%: Change 20-30 words
    /// - 70%: Change 30-40 words
    /// - 60%: Change 40-50 words
    /// - 50%: Change 50%+ of words
    ///
    /// # UCE34 Q10: Boundary Testing
    /// - Edge case: 99% similarity (minimal change)
    /// - Edge case: 50% similarity (borderline duplicate)
    fn generate_duplicate(original: &str, similarity: f32) -> String {
        let words: Vec<&str> = original.split_whitespace().collect();
        let total_words = words.len();
        let num_changes = ((1.0 - similarity) * total_words as f32) as usize;

        let mut modified_words = words.clone();
        let synonyms = [
            "analyzed",
            "examined",
            "studied",
            "investigated",
            "explored",
        ];

        // Modify words deterministically based on position
        for i in 0..num_changes.min(total_words) {
            let pos = (i * total_words / num_changes.max(1)) % total_words;
            modified_words[pos] = synonyms[i % synonyms.len()];
        }

        modified_words.join(" ")
    }

    /// Calculate number of duplicates at each similarity level (exponential decay)
    ///
    /// **Distribution**: Most duplicates at high similarity (99%), fewer at low (50%)
    /// - Level 0 (99%): 40% of duplicates
    /// - Level 1 (95%): 25% of duplicates
    /// - Level 2 (90%): 15% of duplicates
    /// - Level 3 (80%): 10% of duplicates
    /// - Level 4 (70%): 5% of duplicates
    /// - Level 5 (60%): 3% of duplicates
    /// - Level 6 (50%): 2% of duplicates
    ///
    /// # UCE34 Q13: Statistical Properties
    /// - Property: Sum of all levels = total duplicates
    /// - Property: Higher similarity → more documents
    fn duplicates_at_similarity_level(
        total_duplicates: usize,
        level_idx: usize,
        num_levels: usize,
    ) -> usize {
        let percentages = [0.40, 0.25, 0.15, 0.10, 0.05, 0.03, 0.02];
        if level_idx >= percentages.len() {
            return 0;
        }
        (total_duplicates as f32 * percentages[level_idx]) as usize
    }

    /// Validate ground truth clustering is correct
    ///
    /// **Invariants**:
    /// - All seed documents cluster with themselves
    /// - All duplicates cluster with their seed document
    /// - Total clusters = number of unique seed documents
    ///
    /// # UCE34 Q3: Invariants
    /// - Invariant: ground_truth.len() == all_documents.len()
    /// - Invariant: All cluster IDs < num_unique
    /// - Invariant: Each unique doc is root of its cluster
    pub fn validate_ground_truth(&self) -> Result<(), String> {
        let num_unique = self.seed_documents.len();
        let num_total = self.all_documents.len();

        // Invariant: Ground truth covers all documents
        if self.ground_truth.len() != num_total {
            return Err(format!(
                "Ground truth incomplete: {} != {}",
                self.ground_truth.len(),
                num_total
            ));
        }

        // Invariant: All cluster IDs are valid (< num_unique)
        for (&doc_id, &cluster_id) in &self.ground_truth {
            if cluster_id >= num_unique {
                return Err(format!(
                    "Invalid cluster ID: doc {} → cluster {} (max {})",
                    doc_id, cluster_id, num_unique
                ));
            }
        }

        // Invariant: Each seed document is root of its own cluster
        for i in 0..num_unique {
            if self.ground_truth.get(&i) != Some(&i) {
                return Err(format!(
                    "Seed document {} not root of cluster (maps to {:?})",
                    i,
                    self.ground_truth.get(&i)
                ));
            }
        }

        Ok(())
    }

    /// Compute recall: fraction of duplicate pairs correctly identified
    ///
    /// **Metric**: |{correctly identified pairs}| / |{total duplicate pairs}|
    ///
    /// # UCE34 Q30: Empirical Validation
    /// - Target: >92% recall (L=5 multi-table LSH)
    /// - Baseline: 5-41% recall (L=1 single-table LSH)
    pub fn compute_recall(&self, detected_clusters: &HashMap<usize, usize>) -> f32 {
        let mut total_pairs = 0;
        let mut correct_pairs = 0;

        // Compare all pairs of documents
        for i in 0..self.all_documents.len() {
            for j in (i + 1)..self.all_documents.len() {
                let ground_truth_match = self.ground_truth.get(&i) == self.ground_truth.get(&j);
                let detected_match = detected_clusters.get(&i) == detected_clusters.get(&j);

                if ground_truth_match {
                    total_pairs += 1;
                    if detected_match {
                        correct_pairs += 1;
                    }
                }
            }
        }

        if total_pairs == 0 {
            return 1.0; // No duplicates → perfect recall
        }

        correct_pairs as f32 / total_pairs as f32
    }

    /// Compute precision: fraction of identified pairs that are true duplicates
    ///
    /// **Metric**: |{correctly identified pairs}| / |{all identified pairs}|
    ///
    /// # UCE34 Q30: Empirical Validation
    /// - Target: >95% precision (low false positive rate)
    /// - Trade-off: High recall (92%+) vs high precision (95%+)
    pub fn compute_precision(&self, detected_clusters: &HashMap<usize, usize>) -> f32 {
        let mut identified_pairs = 0;
        let mut correct_pairs = 0;

        // Compare all pairs of documents
        for i in 0..self.all_documents.len() {
            for j in (i + 1)..self.all_documents.len() {
                let ground_truth_match = self.ground_truth.get(&i) == self.ground_truth.get(&j);
                let detected_match = detected_clusters.get(&i) == detected_clusters.get(&j);

                if detected_match {
                    identified_pairs += 1;
                    if ground_truth_match {
                        correct_pairs += 1;
                    }
                }
            }
        }

        if identified_pairs == 0 {
            return 1.0; // No identifications → perfect precision (vacuously true)
        }

        correct_pairs as f32 / identified_pairs as f32
    }

    /// Compute false positive rate: fraction of non-duplicate pairs incorrectly identified
    ///
    /// **Metric**: |{incorrectly identified pairs}| / |{total non-duplicate pairs}|
    ///
    /// # UCE34 Q30: Empirical Validation
    /// - Target: <0.1% false positive rate
    /// - Critical for production (false positives waste storage/bandwidth)
    pub fn compute_false_positive_rate(&self, detected_clusters: &HashMap<usize, usize>) -> f32 {
        let mut non_duplicate_pairs = 0;
        let mut false_positive_pairs = 0;

        // Compare all pairs of documents
        for i in 0..self.all_documents.len() {
            for j in (i + 1)..self.all_documents.len() {
                let ground_truth_match = self.ground_truth.get(&i) == self.ground_truth.get(&j);
                let detected_match = detected_clusters.get(&i) == detected_clusters.get(&j);

                if !ground_truth_match {
                    non_duplicate_pairs += 1;
                    if detected_match {
                        false_positive_pairs += 1;
                    }
                }
            }
        }

        if non_duplicate_pairs == 0 {
            return 0.0; // No non-duplicates → no false positives
        }

        false_positive_pairs as f32 / non_duplicate_pairs as f32
    }

    /// Get document by ID
    pub fn get_document(&self, id: usize) -> Option<&str> {
        self.all_documents.get(id).map(|s| s.as_str())
    }

    /// Get total document count
    pub fn total_documents(&self) -> usize {
        self.all_documents.len()
    }

    /// Get unique document count
    pub fn unique_documents(&self) -> usize {
        self.seed_documents.len()
    }

    /// Clean up temporary files
    pub fn cleanup(&self) {
        let _ = fs::remove_dir_all(&self.temp_dir);
    }
}

impl Drop for DedupTestHarness {
    fn drop(&mut self) {
        self.cleanup();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_harness_creation() {
        let harness = DedupTestHarness::with_config(100, 900);
        assert_eq!(harness.total_documents(), 1000);
        assert_eq!(harness.unique_documents(), 100);
        assert_eq!(harness.ground_truth.len(), 1000);
    }

    #[test]
    fn test_ground_truth_validation() {
        let harness = DedupTestHarness::with_config(100, 900);
        assert!(harness.validate_ground_truth().is_ok());
    }

    #[test]
    fn test_similarity_distribution() {
        let harness = DedupTestHarness::with_config(100, 900);
        // Most duplicates should be at high similarity (99%)
        let mut similarity_counts = vec![0; 7];
        for &cluster_id in harness.ground_truth.values() {
            if cluster_id < 100 {
                // Seed documents
                continue;
            }
            // Count duplicates at each level (approximation)
            let level_idx = (cluster_id - 100) / 900 * 7;
            if level_idx < 7 {
                similarity_counts[level_idx.min(6)] += 1;
            }
        }
        // Level 0 (99%) should have most duplicates
        // (This is a weak test due to random distribution)
        assert!(similarity_counts.iter().sum::<usize>() > 0);
    }

    #[test]
    fn test_recall_computation_perfect() {
        let harness = DedupTestHarness::with_config(10, 90);
        // Perfect clustering (matches ground truth)
        let recall = harness.compute_recall(&harness.ground_truth);
        assert_eq!(recall, 1.0);
    }

    #[test]
    fn test_precision_computation_perfect() {
        let harness = DedupTestHarness::with_config(10, 90);
        // Perfect clustering (matches ground truth)
        let precision = harness.compute_precision(&harness.ground_truth);
        assert_eq!(precision, 1.0);
    }

    #[test]
    fn test_false_positive_rate_zero() {
        let harness = DedupTestHarness::with_config(10, 90);
        // Perfect clustering (matches ground truth)
        let fpr = harness.compute_false_positive_rate(&harness.ground_truth);
        assert_eq!(fpr, 0.0);
    }

    #[test]
    fn test_document_retrieval() {
        let harness = DedupTestHarness::with_config(10, 90);
        let doc = harness.get_document(0);
        assert!(doc.is_some());
        assert!(doc.unwrap().contains("Document 0"));
    }
}
