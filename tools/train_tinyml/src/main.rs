//! # TinyML Tree Training Pipeline
//!
//! Converts sklearn IsolationForest JSON export to Q8.8 quantized binary format
//! compatible with atomic_capsule's TinyMLTreeEnsemble.
//!
//! ## UCE34 Framework Analysis
//! - **Q10 (Tier)**: T3 Fixed-Point (Q8.8 quantization) + T0 Auditable (binary serialization)
//! - **Q11 (Rust)**: Zero external dependencies, pure Rust JSON parser
//! - **Q12 (Nightly)**: Not required (stable-compatible)
//!
//! ## Binary Format (ensemble.bin)
//! ```text
//! Header (16 bytes):
//!   magic: [u8; 4]       = b"TINY"
//!   version: u8          = 1
//!   num_trees: u8        = 1-8
//!   max_depth: u8        = 1-6
//!   reserved: u8         = 0
//!   tree_size: u32       = 252 (63 nodes * 4 bytes)
//!   checksum: u32        = FNV-1a hash of tree data
//!
//! Trees (num_trees * 252 bytes each):
//!   nodes: [DecisionTreeNode; 63]
//!     feature_idx: u8
//!     is_leaf: u8
//!     threshold_q8_8: i16 (little-endian)
//! ```

use std::env;
use std::fs;
use std::process;

// ============================================================================
// CONSTANTS
// ============================================================================

const MAGIC: [u8; 4] = *b"TINY";
const VERSION: u8 = 1;
const MAX_TREES: usize = 8;
const MAX_DEPTH: u8 = 6;
const MAX_NODES: usize = 63; // 2^6 - 1
const NODE_SIZE: usize = 4;
const TREE_SIZE: usize = MAX_NODES * NODE_SIZE; // 252 bytes
const HEADER_SIZE: usize = 16;

// ============================================================================
// Q8.8 FIXED-POINT CONVERSION
// ============================================================================

/// Convert f64 to Q8.8 fixed-point (i16)
/// Range: [-128.0, 127.996] with 0.00390625 precision
#[inline]
fn f64_to_q8_8(value: f64) -> i16 {
    let scaled = value * 256.0;
    if scaled >= i16::MAX as f64 {
        i16::MAX
    } else if scaled <= i16::MIN as f64 {
        i16::MIN
    } else {
        scaled.round() as i16
    }
}

/// Convert Q8.8 fixed-point (i16) to f64
#[inline]
#[allow(dead_code)]
fn q8_8_to_f64(value: i16) -> f64 {
    value as f64 / 256.0
}

// ============================================================================
// DECISION TREE NODE (4 bytes)
// ============================================================================

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct DecisionTreeNode {
    feature_idx: u8,
    is_leaf: u8,
    threshold_q8_8: i16,
}

impl DecisionTreeNode {
    fn new_internal(feature_idx: u8, threshold_q8_8: i16) -> Self {
        Self {
            feature_idx,
            is_leaf: 0,
            threshold_q8_8,
        }
    }

    fn new_leaf(depth: u8) -> Self {
        Self {
            feature_idx: 0,
            is_leaf: depth.min(MAX_DEPTH),
            threshold_q8_8: 0,
        }
    }

    fn to_bytes(&self) -> [u8; 4] {
        let threshold_bytes = self.threshold_q8_8.to_le_bytes();
        [self.feature_idx, self.is_leaf, threshold_bytes[0], threshold_bytes[1]]
    }
}

// ============================================================================
// SKLEARN JSON PARSER (Zero Dependencies)
// ============================================================================

/// Parsed sklearn tree node from JSON
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct SklearnNode {
    feature: i32,       // -2 for leaf, otherwise feature index
    threshold: f64,     // Split threshold (ignored for leaves)
    left_child: i32,    // -1 for no child
    right_child: i32,   // -1 for no child
    n_node_samples: i32, // Sample count (for potential future weighting)
}

/// Parse sklearn IsolationForest JSON export
///
/// Expected JSON structure:
/// ```json
/// {
///   "n_estimators": 8,
///   "max_samples": 256,
///   "estimators": [
///     {
///       "tree_": {
///         "feature": [...],
///         "threshold": [...],
///         "children_left": [...],
///         "children_right": [...],
///         "n_node_samples": [...]
///       }
///     },
///     ...
///   ]
/// }
/// ```
fn parse_sklearn_json(content: &str) -> Result<Vec<Vec<SklearnNode>>, String> {
    let mut trees = Vec::new();

    // Find estimators array
    let estimators_start = content.find("\"estimators\"")
        .ok_or("Missing 'estimators' field")?;

    // Parse each tree in estimators
    let mut pos = estimators_start;

    while let Some(tree_start) = find_pattern(&content[pos..], "\"tree_\"") {
        let tree_pos = pos + tree_start;

        // Extract arrays
        let feature = extract_int_array(&content[tree_pos..], "\"feature\"")?;
        let threshold = extract_float_array(&content[tree_pos..], "\"threshold\"")?;
        let children_left = extract_int_array(&content[tree_pos..], "\"children_left\"")?;
        let children_right = extract_int_array(&content[tree_pos..], "\"children_right\"")?;
        let n_node_samples = extract_int_array(&content[tree_pos..], "\"n_node_samples\"")?;

        if feature.len() != threshold.len()
            || feature.len() != children_left.len()
            || feature.len() != children_right.len()
            || feature.len() != n_node_samples.len() {
            return Err("Array length mismatch in tree".to_string());
        }

        let mut nodes = Vec::with_capacity(feature.len());
        for i in 0..feature.len() {
            nodes.push(SklearnNode {
                feature: feature[i],
                threshold: threshold[i],
                left_child: children_left[i],
                right_child: children_right[i],
                n_node_samples: n_node_samples[i],
            });
        }

        trees.push(nodes);

        // Move to next tree
        pos = tree_pos + 1;
        if trees.len() >= MAX_TREES {
            break;
        }
    }

    if trees.is_empty() {
        return Err("No trees found in JSON".to_string());
    }

    Ok(trees)
}

fn find_pattern(s: &str, pattern: &str) -> Option<usize> {
    s.find(pattern)
}

fn extract_int_array(s: &str, key: &str) -> Result<Vec<i32>, String> {
    let key_pos = s.find(key).ok_or(format!("Missing key: {}", key))?;
    let bracket_start = s[key_pos..].find('[')
        .ok_or(format!("Missing array start for {}", key))?;
    let array_start = key_pos + bracket_start;
    let bracket_end = s[array_start..].find(']')
        .ok_or(format!("Missing array end for {}", key))?;
    let array_content = &s[array_start + 1..array_start + bracket_end];

    let mut values = Vec::new();
    for part in array_content.split(',') {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }
        let value: i32 = trimmed.parse()
            .map_err(|_| format!("Invalid integer: {}", trimmed))?;
        values.push(value);
    }

    Ok(values)
}

fn extract_float_array(s: &str, key: &str) -> Result<Vec<f64>, String> {
    let key_pos = s.find(key).ok_or(format!("Missing key: {}", key))?;
    let bracket_start = s[key_pos..].find('[')
        .ok_or(format!("Missing array start for {}", key))?;
    let array_start = key_pos + bracket_start;
    let bracket_end = s[array_start..].find(']')
        .ok_or(format!("Missing array end for {}", key))?;
    let array_content = &s[array_start + 1..array_start + bracket_end];

    let mut values = Vec::new();
    for part in array_content.split(',') {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }
        // Handle special float values
        let value: f64 = if trimmed == "inf" || trimmed == "Infinity" {
            f64::INFINITY
        } else if trimmed == "-inf" || trimmed == "-Infinity" {
            f64::NEG_INFINITY
        } else if trimmed == "nan" || trimmed == "NaN" {
            0.0 // Convert NaN to 0 for leaf nodes
        } else {
            trimmed.parse()
                .map_err(|_| format!("Invalid float: {}", trimmed))?
        };
        values.push(value);
    }

    Ok(values)
}

// ============================================================================
// TREE CONVERSION
// ============================================================================

/// Convert sklearn tree to complete binary tree format (depth 6, 63 nodes)
fn convert_to_complete_tree(sklearn_nodes: &[SklearnNode], max_depth: u8) -> [DecisionTreeNode; MAX_NODES] {
    let mut nodes = [DecisionTreeNode::default(); MAX_NODES];

    if sklearn_nodes.is_empty() {
        return nodes;
    }

    // BFS conversion from sklearn's node array to complete binary tree
    convert_node_recursive(&mut nodes, sklearn_nodes, 0, 0, 0, max_depth);

    nodes
}

fn convert_node_recursive(
    output: &mut [DecisionTreeNode; MAX_NODES],
    sklearn: &[SklearnNode],
    sklearn_idx: usize,
    output_idx: usize,
    depth: u8,
    max_depth: u8,
) {
    if output_idx >= MAX_NODES || sklearn_idx >= sklearn.len() {
        return;
    }

    let sklearn_node = &sklearn[sklearn_idx];

    // Check if leaf (sklearn uses feature == -2 for leaves)
    let is_leaf = sklearn_node.feature < 0 || depth >= max_depth;

    if is_leaf {
        // Leaf node: store depth as path length
        output[output_idx] = DecisionTreeNode::new_leaf(depth);
    } else {
        // Internal node: store feature and threshold
        let feature_idx = (sklearn_node.feature as u8).min(255);
        let threshold_q8_8 = f64_to_q8_8(sklearn_node.threshold);
        output[output_idx] = DecisionTreeNode::new_internal(feature_idx, threshold_q8_8);

        // Recursively convert children
        let left_child_output = 2 * output_idx + 1;
        let right_child_output = 2 * output_idx + 2;

        if sklearn_node.left_child >= 0 {
            convert_node_recursive(
                output, sklearn, sklearn_node.left_child as usize,
                left_child_output, depth + 1, max_depth
            );
        }

        if sklearn_node.right_child >= 0 {
            convert_node_recursive(
                output, sklearn, sklearn_node.right_child as usize,
                right_child_output, depth + 1, max_depth
            );
        }
    }
}

// ============================================================================
// BINARY SERIALIZATION
// ============================================================================

/// FNV-1a hash for checksum
fn fnv1a_hash(data: &[u8]) -> u32 {
    const FNV_PRIME: u32 = 0x01000193;
    const FNV_OFFSET: u32 = 0x811c9dc5;

    let mut hash = FNV_OFFSET;
    for &byte in data {
        hash ^= byte as u32;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// Serialize trees to binary format
fn serialize_ensemble(trees: &[[DecisionTreeNode; MAX_NODES]], num_trees: u8, max_depth: u8) -> Vec<u8> {
    let num_trees_clamped = (num_trees as usize).min(trees.len()).min(MAX_TREES) as u8;

    // Calculate tree data for checksum
    let mut tree_data = Vec::with_capacity(num_trees_clamped as usize * TREE_SIZE);
    for i in 0..num_trees_clamped as usize {
        for node in &trees[i] {
            tree_data.extend_from_slice(&node.to_bytes());
        }
    }

    let checksum = fnv1a_hash(&tree_data);

    // Build output buffer
    let mut buffer = Vec::with_capacity(HEADER_SIZE + tree_data.len());

    // Header (16 bytes)
    buffer.extend_from_slice(&MAGIC);                      // 4 bytes: magic
    buffer.push(VERSION);                                   // 1 byte: version
    buffer.push(num_trees_clamped);                        // 1 byte: num_trees
    buffer.push(max_depth);                                // 1 byte: max_depth
    buffer.push(0);                                        // 1 byte: reserved
    buffer.extend_from_slice(&(TREE_SIZE as u32).to_le_bytes()); // 4 bytes: tree_size
    buffer.extend_from_slice(&checksum.to_le_bytes());     // 4 bytes: checksum

    // Tree data
    buffer.extend_from_slice(&tree_data);

    buffer
}

/// Verify binary format integrity
fn verify_ensemble(data: &[u8]) -> Result<(), String> {
    if data.len() < HEADER_SIZE {
        return Err(format!("File too small: {} bytes (minimum {})", data.len(), HEADER_SIZE));
    }

    // Check magic
    if &data[0..4] != &MAGIC {
        return Err("Invalid magic number".to_string());
    }

    // Check version
    let version = data[4];
    if version != VERSION {
        return Err(format!("Unsupported version: {} (expected {})", version, VERSION));
    }

    let num_trees = data[5] as usize;
    let max_depth = data[6];
    let tree_size = u32::from_le_bytes([data[8], data[9], data[10], data[11]]) as usize;
    let stored_checksum = u32::from_le_bytes([data[12], data[13], data[14], data[15]]);

    // Verify data length
    let expected_len = HEADER_SIZE + num_trees * tree_size;
    if data.len() < expected_len {
        return Err(format!("File truncated: {} bytes (expected {})", data.len(), expected_len));
    }

    // Verify checksum
    let tree_data = &data[HEADER_SIZE..HEADER_SIZE + num_trees * tree_size];
    let computed_checksum = fnv1a_hash(tree_data);
    if computed_checksum != stored_checksum {
        return Err(format!("Checksum mismatch: computed 0x{:08x}, stored 0x{:08x}",
            computed_checksum, stored_checksum));
    }

    println!("Verification passed:");
    println!("  Version: {}", version);
    println!("  Trees: {}", num_trees);
    println!("  Max depth: {}", max_depth);
    println!("  Tree size: {} bytes", tree_size);
    println!("  Checksum: 0x{:08x}", stored_checksum);

    Ok(())
}

// ============================================================================
// CLI
// ============================================================================

fn print_usage(program: &str) {
    eprintln!("TinyML Tree Training Pipeline v{}", VERSION);
    eprintln!();
    eprintln!("Converts sklearn IsolationForest JSON to Q8.8 quantized binary format.");
    eprintln!();
    eprintln!("USAGE:");
    eprintln!("  {} --input <json> --output <bin> [OPTIONS]", program);
    eprintln!("  {} --verify <bin>", program);
    eprintln!();
    eprintln!("OPTIONS:");
    eprintln!("  --input <path>     sklearn IsolationForest JSON export");
    eprintln!("  --output <path>    Output binary file");
    eprintln!("  --num-trees <n>    Number of trees to export (1-8, default: auto)");
    eprintln!("  --max-depth <n>    Maximum tree depth (1-6, default: 6)");
    eprintln!("  --verify <path>    Verify an existing binary file");
    eprintln!("  --help, -h         Show this help");
    eprintln!();
    eprintln!("EXAMPLES:");
    eprintln!("  # Convert sklearn export to binary");
    eprintln!("  {} --input sklearn_forest.json --output ensemble.bin", program);
    eprintln!();
    eprintln!("  # Specify 8 trees with depth 6");
    eprintln!("  {} --input forest.json --output out.bin --num-trees 8 --max-depth 6", program);
    eprintln!();
    eprintln!("  # Verify binary file integrity");
    eprintln!("  {} --verify ensemble.bin", program);
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let program = &args[0];

    if args.len() < 2 {
        print_usage(program);
        process::exit(1);
    }

    let mut input_path: Option<String> = None;
    let mut output_path: Option<String> = None;
    let mut verify_path: Option<String> = None;
    let mut num_trees: Option<u8> = None;
    let mut max_depth: u8 = MAX_DEPTH;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--help" | "-h" => {
                print_usage(program);
                process::exit(0);
            }
            "--input" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("Error: --input requires a path");
                    process::exit(1);
                }
                input_path = Some(args[i].clone());
            }
            "--output" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("Error: --output requires a path");
                    process::exit(1);
                }
                output_path = Some(args[i].clone());
            }
            "--verify" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("Error: --verify requires a path");
                    process::exit(1);
                }
                verify_path = Some(args[i].clone());
            }
            "--num-trees" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("Error: --num-trees requires a number");
                    process::exit(1);
                }
                let n: u8 = args[i].parse().unwrap_or_else(|_| {
                    eprintln!("Error: Invalid num-trees value");
                    process::exit(1);
                });
                if n < 1 || n > 8 {
                    eprintln!("Error: num-trees must be 1-8");
                    process::exit(1);
                }
                num_trees = Some(n);
            }
            "--max-depth" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("Error: --max-depth requires a number");
                    process::exit(1);
                }
                max_depth = args[i].parse().unwrap_or_else(|_| {
                    eprintln!("Error: Invalid max-depth value");
                    process::exit(1);
                });
                if max_depth < 1 || max_depth > 6 {
                    eprintln!("Error: max-depth must be 1-6");
                    process::exit(1);
                }
            }
            other => {
                eprintln!("Error: Unknown option: {}", other);
                print_usage(program);
                process::exit(1);
            }
        }
        i += 1;
    }

    // Verify mode
    if let Some(path) = verify_path {
        let data = fs::read(&path).unwrap_or_else(|e| {
            eprintln!("Error reading file '{}': {}", path, e);
            process::exit(1);
        });

        match verify_ensemble(&data) {
            Ok(()) => {
                println!("File '{}' is valid.", path);
                process::exit(0);
            }
            Err(e) => {
                eprintln!("Verification failed: {}", e);
                process::exit(1);
            }
        }
    }

    // Convert mode
    let input = input_path.unwrap_or_else(|| {
        eprintln!("Error: --input is required");
        print_usage(program);
        process::exit(1);
    });

    let output = output_path.unwrap_or_else(|| {
        eprintln!("Error: --output is required");
        print_usage(program);
        process::exit(1);
    });

    // Read input JSON
    let content = fs::read_to_string(&input).unwrap_or_else(|e| {
        eprintln!("Error reading '{}': {}", input, e);
        process::exit(1);
    });

    // Parse sklearn JSON
    let sklearn_trees = parse_sklearn_json(&content).unwrap_or_else(|e| {
        eprintln!("Error parsing JSON: {}", e);
        process::exit(1);
    });

    let actual_num_trees = num_trees.unwrap_or(sklearn_trees.len().min(MAX_TREES) as u8);

    println!("Parsed {} trees from sklearn JSON", sklearn_trees.len());
    println!("Exporting {} trees with max depth {}", actual_num_trees, max_depth);

    // Convert trees
    let mut converted_trees: Vec<[DecisionTreeNode; MAX_NODES]> = Vec::new();
    for (i, sklearn_tree) in sklearn_trees.iter().enumerate().take(actual_num_trees as usize) {
        println!("  Converting tree {} ({} sklearn nodes)...", i, sklearn_tree.len());
        let converted = convert_to_complete_tree(sklearn_tree, max_depth);
        converted_trees.push(converted);
    }

    // Serialize
    let binary = serialize_ensemble(&converted_trees, actual_num_trees, max_depth);

    // Write output
    fs::write(&output, &binary).unwrap_or_else(|e| {
        eprintln!("Error writing '{}': {}", output, e);
        process::exit(1);
    });

    println!("Wrote {} bytes to '{}'", binary.len(), output);

    // Verify output
    match verify_ensemble(&binary) {
        Ok(()) => println!("Output verification passed."),
        Err(e) => {
            eprintln!("Warning: Output verification failed: {}", e);
            process::exit(1);
        }
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_q8_8_conversion() {
        assert_eq!(f64_to_q8_8(0.5), 128);
        assert_eq!(f64_to_q8_8(1.0), 256);
        assert_eq!(f64_to_q8_8(-0.5), -128);
        assert_eq!(f64_to_q8_8(0.0), 0);
        // 127.0 * 256 = 32512 (within range)
        assert_eq!(f64_to_q8_8(127.0), 32512);
        // 128.0 * 256 = 32768 (exceeds i16::MAX, saturates)
        assert_eq!(f64_to_q8_8(128.0), i16::MAX);
        // -128.0 * 256 = -32768 = i16::MIN
        assert_eq!(f64_to_q8_8(-128.0), i16::MIN);
        // -129.0 * 256 = -33024 (exceeds i16::MIN, saturates)
        assert_eq!(f64_to_q8_8(-129.0), i16::MIN);
    }

    #[test]
    fn test_q8_8_roundtrip() {
        let values = [0.0, 0.5, 1.0, -0.5, 10.25, -10.75];
        for &v in &values {
            let q = f64_to_q8_8(v);
            let recovered = q8_8_to_f64(q);
            assert!((v - recovered).abs() < 0.01, "Roundtrip failed for {}", v);
        }
    }

    #[test]
    fn test_decision_tree_node_size() {
        assert_eq!(std::mem::size_of::<DecisionTreeNode>(), 4);
    }

    #[test]
    fn test_node_serialization() {
        let node = DecisionTreeNode::new_internal(5, 128);
        let bytes = node.to_bytes();
        assert_eq!(bytes[0], 5);  // feature_idx
        assert_eq!(bytes[1], 0);  // is_leaf = 0 (internal)
        assert_eq!(i16::from_le_bytes([bytes[2], bytes[3]]), 128);
    }

    #[test]
    fn test_leaf_node() {
        let leaf = DecisionTreeNode::new_leaf(3);
        assert_eq!(leaf.feature_idx, 0);
        assert_eq!(leaf.is_leaf, 3);
        assert_eq!(leaf.threshold_q8_8, 0);
    }

    #[test]
    fn test_fnv1a_hash() {
        let data = b"test data";
        let hash = fnv1a_hash(data);
        assert_ne!(hash, 0);

        // Same data should produce same hash
        assert_eq!(fnv1a_hash(data), fnv1a_hash(data));

        // Different data should produce different hash
        assert_ne!(fnv1a_hash(data), fnv1a_hash(b"different"));
    }

    #[test]
    fn test_serialize_empty() {
        let trees: [[DecisionTreeNode; MAX_NODES]; 0] = [];
        let binary = serialize_ensemble(&trees, 0, 6);
        assert_eq!(binary.len(), HEADER_SIZE);
        assert_eq!(&binary[0..4], b"TINY");
    }

    #[test]
    fn test_serialize_single_tree() {
        let mut tree = [DecisionTreeNode::default(); MAX_NODES];
        tree[0] = DecisionTreeNode::new_internal(0, 128);
        tree[1] = DecisionTreeNode::new_leaf(1);
        tree[2] = DecisionTreeNode::new_leaf(1);

        let trees = [tree];
        let binary = serialize_ensemble(&trees, 1, 6);

        assert_eq!(binary.len(), HEADER_SIZE + TREE_SIZE);
        assert!(verify_ensemble(&binary).is_ok());
    }

    #[test]
    fn test_parse_int_array() {
        let json = r#"{"feature": [1, 2, 3, -2, 5]}"#;
        let result = extract_int_array(json, "\"feature\"").unwrap();
        assert_eq!(result, vec![1, 2, 3, -2, 5]);
    }

    #[test]
    fn test_parse_float_array() {
        let json = r#"{"threshold": [0.5, 1.0, -0.25, inf, -inf]}"#;
        let result = extract_float_array(json, "\"threshold\"").unwrap();
        assert_eq!(result[0], 0.5);
        assert_eq!(result[1], 1.0);
        assert_eq!(result[2], -0.25);
        assert!(result[3].is_infinite() && result[3] > 0.0);
        assert!(result[4].is_infinite() && result[4] < 0.0);
    }

    #[test]
    fn test_convert_simple_tree() {
        let sklearn_nodes = vec![
            SklearnNode { feature: 0, threshold: 0.5, left_child: 1, right_child: 2, n_node_samples: 100 },
            SklearnNode { feature: -2, threshold: 0.0, left_child: -1, right_child: -1, n_node_samples: 40 },
            SklearnNode { feature: -2, threshold: 0.0, left_child: -1, right_child: -1, n_node_samples: 60 },
        ];

        let converted = convert_to_complete_tree(&sklearn_nodes, 6);

        // Root should be internal node
        assert_eq!(converted[0].feature_idx, 0);
        assert_eq!(converted[0].is_leaf, 0);
        assert_eq!(converted[0].threshold_q8_8, 128); // 0.5 in Q8.8

        // Children should be leaves at depth 1
        assert_eq!(converted[1].is_leaf, 1);
        assert_eq!(converted[2].is_leaf, 1);
    }

    #[test]
    fn test_full_pipeline() {
        // Minimal sklearn JSON
        let json = r#"{
            "n_estimators": 1,
            "estimators": [
                {
                    "tree_": {
                        "feature": [0, -2, -2],
                        "threshold": [0.5, 0.0, 0.0],
                        "children_left": [1, -1, -1],
                        "children_right": [2, -1, -1],
                        "n_node_samples": [100, 40, 60]
                    }
                }
            ]
        }"#;

        let sklearn_trees = parse_sklearn_json(json).unwrap();
        assert_eq!(sklearn_trees.len(), 1);
        assert_eq!(sklearn_trees[0].len(), 3);

        let converted = convert_to_complete_tree(&sklearn_trees[0], 6);
        let trees = [converted];
        let binary = serialize_ensemble(&trees, 1, 6);

        assert!(verify_ensemble(&binary).is_ok());
    }
}
