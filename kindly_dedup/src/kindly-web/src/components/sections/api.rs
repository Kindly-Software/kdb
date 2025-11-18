use crate::components::molecular::{CodeBlock, SectionContainer};
use leptos::prelude::*;

#[component]
pub fn Api() -> impl IntoView {
    let cli_example = r#"# CLI Usage - Works with ANY programming language
# No Python bindings needed!

# Basic deduplication (1M documents in ~17 seconds)
cat documents.jsonl | kindly_dedup --threshold 0.85 > clusters.json

# With custom parameters
kindly_dedup \
  --threshold 0.85 \
  --num-hashes 128 \
  --num-tables 5 \
  --input documents.jsonl \
  --output clusters.json

# Performance: 60K+ docs/sec single-threaded
# Multi-threaded: 912K docs/sec @ 16 cores (95% efficiency)
"#;

    let rust_library_example = r#"use kindly_dedup::{DedupPipeline, ParallelDedupPipeline};

// RAM-based pipeline (simple, fast)
let mut pipeline = DedupPipeline::new(1_000_000);

for (doc_id, text) in documents {
    pipeline.add_document(doc_id, text);
}

let clusters = pipeline.find_duplicates(0.85)?;
println!("Found {} duplicate clusters", clusters.len());

// Parallel pipeline (16 cores, 912K docs/sec)
let mut parallel = ParallelDedupPipeline::new(10_000_000, 16)?;

parallel.add_documents(&batch_docs)?;
let clusters = parallel.find_duplicates(0.85)?;

// Persistent pipeline (3.5 GB for 10M docs, 93% memory reduction)
let mut persistent = PersistentDedupPipeline::create(
    "dedup.mmap",
    10_000_000
)?;

persistent.add_document(0, "The quick brown fox")?;
let is_dup = persistent.is_duplicate("Similar text here")?;

// Performance:
// - Single-threaded: 60K+ docs/sec (38× vs Python)
// - Multi-threaded: 912K docs/sec @ 16 cores (95% efficiency)
// - Persistent: 93% memory reduction, crash-safe
"#;

    view! {
        <SectionContainer id="api" class="api-section">
            <h2 class="section-title">"API Examples"</h2>
            <p class="section-subtitle">
                "Simple APIs for CLI and Rust library integration"
            </p>

            <div class="api-modes">
                <div class="api-mode">
                    <h3 class="mode-title">"CLI Mode - Universal"</h3>
                    <p class="mode-description">
                        "Works with Python, JavaScript, Java, Go, or any language that can spawn processes. "
                        "No language bindings required—just pipe documents via stdin/stdout. Perfect for "
                        "integrating with existing data pipelines."
                    </p>
                    <CodeBlock code=cli_example language="bash" />
                    <div class="mode-performance">
                        <h4>"CLI Performance"</h4>
                        <ul>
                            <li>"Single-threaded: 60K+ docs/sec (38× vs Python datasketch)"</li>
                            <li>"Multi-threaded: 912K docs/sec @ 16 cores (95% efficiency)"</li>
                            <li>"Memory: 3.5 GB for 10M docs (persistent mode)"</li>
                            <li>"Latency: <1ms per document (single-threaded)"</li>
                        </ul>
                    </div>
                </div>

                <div class="api-mode">
                    <h3 class="mode-title">"Rust Library - Native Performance"</h3>
                    <p class="mode-description">
                        "Native Rust API with three pipeline modes: DedupPipeline (RAM-based, simplest), "
                        "ParallelDedupPipeline (16 cores, 912K docs/sec), PersistentDedupPipeline "
                        "(low memory, crash-safe). All modes use same computational capsule primitives."
                    </p>
                    <CodeBlock code=rust_library_example language="rust" />
                    <div class="mode-performance">
                        <h4>"Library Performance"</h4>
                        <ul>
                            <li>"DedupPipeline: 60K+ docs/sec (single-threaded)"</li>
                            <li>"ParallelDedupPipeline: 912K docs/sec @ 16 cores (95% efficiency)"</li>
                            <li>"PersistentDedupPipeline: 93% memory reduction (3.5 GB vs 40 GB)"</li>
                            <li>"Zero-copy architecture with computational capsules"</li>
                        </ul>
                    </div>
                </div>
            </div>

            <div class="api-features">
                <h3 class="features-title">"API Features"</h3>
                <div class="features-grid">
                    <div class="feature-card">
                        <h4>"🚀 Zero Dependencies"</h4>
                        <p>
                            "Core library is no_std with optional features. No Python runtime, "
                            "no external databases, no complex setup. Just add to Cargo.toml."
                        </p>
                    </div>
                    <div class="feature-card">
                        <h4>"🔒 Type Safety"</h4>
                        <p>
                            "100% safe Rust with computational capsule architecture. Compile-time "
                            "verification prevents data races and undefined behavior."
                        </p>
                    </div>
                    <div class="feature-card">
                        <h4>"⚡ Lockfree"</h4>
                        <p>
                            "Zero mutex/RwLock usage. 100% lockfree coordination with atomic capsules "
                            "for predictable sub-100ns latency and perfect multi-core scaling."
                        </p>
                    </div>
                    <div class="feature-card">
                        <h4>"📊 B32 Validated"</h4>
                        <p>
                            "All performance claims validated with B32 benchmarking framework (95% CI, "
                            "1000+ iterations, fair baselines). Demo lets you verify on your hardware."
                        </p>
                    </div>
                </div>
            </div>

            <div class="api-installation">
                <h3 class="installation-title">"Installation"</h3>
                <CodeBlock
                    code="# Add to Cargo.toml\n[dependencies]\nkindly_dedup = \"1.6\"\n\n# Or with all features\nkindly_dedup = { version = \"1.6\", features = [\"full\"] }"
                    language="toml"
                />
            </div>

            <div class="api-disclaimer">
                <p class="disclaimer-text">
                    <strong>"⚠️ Performance Note:"</strong>
                    " Throughput and latency depend on hardware (CPU, RAM speed, cores), "
                    "corpus characteristics (document length, duplication rate), and system "
                    "configuration (other processes, thermal throttling). Numbers shown are "
                    "measured on AMD Ryzen 9 6900HX with 64 GB DDR5-4800. Demo binary validates "
                    "actual performance on your specific hardware."
                </p>
            </div>
        </SectionContainer>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_compiles() {
        // Ensures component compiles
    }
}
