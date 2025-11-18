//! Synthetic corpus generator for kindly_dedup testing
//!
//! Generates realistic test documents with controlled duplication patterns.
//! Much faster and more reliable than downloading from Common Crawl.
//!
//! Features:
//! - Configurable document count (default: 100K)
//! - Realistic text generation (Lorem ipsum + variations)
//! - Controlled near-duplicate creation (20% similarity range)
//! - Exact duplicates (5%)
//! - High similarity docs (15%, 80-95% similar)
//! - Unique docs (80%)

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Write;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub id: usize,
    pub url: String,
    pub text: String,
}

/// Lorem ipsum base text templates
const TEMPLATES: &[&str] = &[
    "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.",
    "Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat.",
    "Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur.",
    "Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum.",
    "Sed ut perspiciatis unde omnis iste natus error sit voluptatem accusantium doloremque laudantium.",
    "Totam rem aperiam eaque ipsa quae ab illo inventore veritatis et quasi architecto beatae vitae dicta sunt explicabo.",
    "Nemo enim ipsam voluptatem quia voluptas sit aspernatur aut odit aut fugit sed quia consequuntur magni dolores.",
    "Neque porro quisquam est qui dolorem ipsum quia dolor sit amet consectetur adipisci velit.",
    "At vero eos et accusamus et iusto odio dignissimos ducimus qui blanditiis praesentium voluptatum deleniti atque corrupti.",
    "Quos dolores et quas molestias excepturi sint occaecati cupiditate non provident similique sunt in culpa qui officia.",
];

/// Generate base document text
fn generate_base_text(seed: usize) -> String {
    let mut text = String::new();
    let base = seed % TEMPLATES.len();

    // Generate 5-15 paragraphs
    let paragraphs = 5 + (seed % 11);

    for i in 0..paragraphs {
        let template_idx = (base + i) % TEMPLATES.len();
        text.push_str(TEMPLATES[template_idx]);
        text.push(' ');

        // Add some variation
        let variation = seed + i;
        text.push_str(&format!("Document variation {}", variation));
        text.push_str(". ");
    }

    text
}

/// Create near-duplicate by modifying original
fn create_near_duplicate(original: &str, similarity: f32, seed: usize) -> String {
    let words: Vec<&str> = original.split_whitespace().collect();
    let keep_count = (words.len() as f32 * similarity) as usize;

    let mut result = String::new();
    for (i, word) in words.iter().enumerate().take(keep_count) {
        result.push_str(word);
        result.push(' ');

        // Occasionally insert variations
        if (seed + i) % 20 == 0 {
            result.push_str(&format!("variation{} ", seed % 100));
        }
    }

    result
}

/// Generate synthetic corpus
fn generate_corpus(count: usize) -> Vec<Document> {
    println!("Generating {} synthetic documents...", count);

    let mut documents: Vec<Document> = Vec::with_capacity(count);

    // Track which documents to duplicate
    let exact_duplicates = (count as f32 * 0.05) as usize; // 5%
    let near_duplicates = (count as f32 * 0.15) as usize; // 15%

    let mut unique_count = 0;
    let mut exact_dup_count = 0;
    let mut near_dup_count = 0;

    for i in 0..count {
        let url = format!("https://example.com/doc{}", i);

        let text = if i < exact_duplicates && i > 0 {
            // Exact duplicate of earlier document
            let source_idx = i / 2;
            exact_dup_count += 1;
            documents[source_idx].text.clone()
        } else if i < exact_duplicates + near_duplicates && i > exact_duplicates {
            // Near-duplicate (80-95% similar)
            let source_idx = i - exact_duplicates;
            let similarity = 0.80 + ((i % 15) as f32 * 0.01);
            near_dup_count += 1;
            create_near_duplicate(&documents[source_idx].text, similarity, i)
        } else {
            // Unique document
            unique_count += 1;
            generate_base_text(i)
        };

        documents.push(Document { id: i, url, text });

        if (i + 1) % 10000 == 0 {
            println!("  Generated {}/{} documents...", i + 1, count);
        }
    }

    println!();
    println!("Generation complete:");
    println!(
        "  Unique: {} ({:.1}%)",
        unique_count,
        unique_count as f32 / count as f32 * 100.0
    );
    println!(
        "  Exact duplicates: {} ({:.1}%)",
        exact_dup_count,
        exact_dup_count as f32 / count as f32 * 100.0
    );
    println!(
        "  Near duplicates: {} ({:.1}%)",
        near_dup_count,
        near_dup_count as f32 / count as f32 * 100.0
    );

    documents
}

/// Save documents to JSON
fn save_documents(documents: &[Document], output_path: &Path) -> Result<()> {
    println!("Saving {} documents to {:?}...", documents.len(), output_path);

    let mut file = File::create(output_path).context("Failed to create output file")?;

    let json = serde_json::to_string_pretty(documents).context("Failed to serialize documents")?;

    file.write_all(json.as_bytes()).context("Failed to write JSON")?;

    Ok(())
}

fn main() -> Result<()> {
    let count = std::env::var("COUNT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(100_000); // Default: 100K

    let output_path = std::env::var("OUTPUT").unwrap_or_else(|_| format!("test_data/synthetic_{}k.json", count / 1000));

    let output = Path::new(&output_path);

    // Create output directory
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent).context("Failed to create output directory")?;
    }

    // Generate corpus
    let start = std::time::Instant::now();
    let documents = generate_corpus(count);
    let elapsed = start.elapsed();

    println!("Generation time: {:.2}s", elapsed.as_secs_f64());
    println!("Rate: {:.0} docs/s", count as f64 / elapsed.as_secs_f64());
    println!();

    // Save to JSON
    save_documents(&documents, output)?;

    // Print statistics
    let total_chars: usize = documents.iter().map(|d| d.text.len()).sum();
    let avg_chars = total_chars / documents.len();

    println!();
    println!("Output statistics:");
    println!("  File: {}", output.display());
    println!("  Documents: {}", documents.len());
    println!("  Total characters: {}", total_chars);
    println!("  Average length: {} chars", avg_chars);

    if let Ok(metadata) = std::fs::metadata(output) {
        println!("  File size: {:.2} MB", metadata.len() as f64 / 1_000_000.0);
    }

    Ok(())
}
