//! # Qwen 3 Inference Demo - Kindly Brain
//!
//! Demonstrates streaming weight loading for LLM inference on 8GB VRAM constraint.
//! Uses GigaMetaWeightCapsule with VramCache -> RamCache -> SsdLoader tiers.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────────────┐
//! │                    Kindly Brain: Sovereign Personal Knowledge Assistant      │
//! │                                                                             │
//! │  ┌─────────────────────────────────────────────────────────────────────────┐│
//! │  │                    GigaMetaWeightCapsule (1024B)                        ││
//! │  │                    T6 Mixed Tier Metacapsule                            ││
//! │  └─────────────────────────────────────────────────────────────────────────┘│
//! │                                    │                                        │
//! │     ┌──────────────────────────────┼──────────────────────────────┐         │
//! │     │                              │                              │         │
//! │     ▼                              ▼                              ▼         │
//! │  ┌───────────┐              ┌───────────┐               ┌───────────┐       │
//! │  │ VRAM Cache│              │ RAM Cache │               │SSD Loader │       │
//! │  │  (6GB)    │◄────────────►│  (32GB)   │◄─────────────►│  (NVMe)   │       │
//! │  │  <100ns   │  evict/load  │  <200ns   │   prefetch    │  <50μs    │       │
//! │  │  16 slots │              │ 2048 blks │               │ io_uring  │       │
//! │  └───────────┘              └───────────┘               └───────────┘       │
//! │                                                                             │
//! │  ┌─────────────────────────────────────────────────────────────────────────┐│
//! │  │                    WeightAuditCapsule (128B)                            ││
//! │  │              Q34 Hash-Chain Integrity Verification                       ││
//! │  └─────────────────────────────────────────────────────────────────────────┘│
//! └─────────────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Usage
//!
//! ```bash
//! # Requires nightly Rust for portable_simd
//! cargo +nightly run --example qwen3_inference_demo --features 'std,portable_simd'
//! ```

use std::time::{Duration, Instant};

// Import inference primitives
use atomic_capsule::primitives::inference::{
    fnv1a_hash, GigaMetaConfig, GigaMetaWeightCapsule, RamCacheCapsule, SsdLoaderCapsule,
    VramCacheCapsule, WeightAuditCapsule, WeightBlock,
};

/// Simulated Qwen 3 model configuration
struct Qwen3ModelConfig {
    /// Model name for display
    name: &'static str,
    /// Number of transformer layers
    num_layers: u32,
    /// Hidden dimension size
    hidden_dim: u32,
    /// Approximate size in GB (Q4_K_M quantized)
    size_gb: f32,
    /// Blocks per layer (for weight streaming)
    blocks_per_layer: u32,
}

#[allow(dead_code)]
impl Qwen3ModelConfig {
    const fn qwen3_7b() -> Self {
        Self {
            name: "Qwen3-7B-Q4_K_M",
            num_layers: 32,
            hidden_dim: 4096,
            size_gb: 4.8,
            blocks_per_layer: 6, // qkv, mlp_up, mlp_down, mlp_gate, etc.
        }
    }

    const fn qwen3_14b() -> Self {
        Self {
            name: "Qwen3-14B-Q4_K_M",
            num_layers: 40,
            hidden_dim: 5120,
            size_gb: 9.6,
            blocks_per_layer: 8,
        }
    }

    const fn qwen3_32b() -> Self {
        Self {
            name: "Qwen3-32B-Q4_K_M",
            num_layers: 64,
            hidden_dim: 6144,
            size_gb: 21.0,
            blocks_per_layer: 10,
        }
    }

    fn total_blocks(&self) -> u32 {
        self.num_layers * self.blocks_per_layer
    }
}

/// Simulated inference pass metrics
struct InferenceMetrics {
    vram_hits: u32,
    ram_hits: u32,
    ssd_loads: u32,
    total_layers: u32,
    layer_latencies_ns: Vec<u64>,
}

impl InferenceMetrics {
    fn new() -> Self {
        Self {
            vram_hits: 0,
            ram_hits: 0,
            ssd_loads: 0,
            total_layers: 0,
            layer_latencies_ns: Vec::new(),
        }
    }

    fn vram_hit_rate(&self) -> f64 {
        if self.total_layers == 0 {
            0.0
        } else {
            self.vram_hits as f64 / self.total_layers as f64 * 100.0
        }
    }

    fn ram_hit_rate(&self) -> f64 {
        if self.total_layers == 0 {
            0.0
        } else {
            self.ram_hits as f64 / self.total_layers as f64 * 100.0
        }
    }

    fn ssd_load_rate(&self) -> f64 {
        if self.total_layers == 0 {
            0.0
        } else {
            self.ssd_loads as f64 / self.total_layers as f64 * 100.0
        }
    }

    fn avg_latency_us(&self) -> f64 {
        if self.layer_latencies_ns.is_empty() {
            0.0
        } else {
            let sum: u64 = self.layer_latencies_ns.iter().sum();
            (sum as f64 / self.layer_latencies_ns.len() as f64) / 1000.0
        }
    }

    fn total_time_ms(&self) -> f64 {
        let sum: u64 = self.layer_latencies_ns.iter().sum();
        sum as f64 / 1_000_000.0
    }
}

/// Demonstration of 3-tier cache behavior
fn demo_three_tier_cache() {
    println!("\n=== Three-Tier Cache Architecture Demo ===\n");

    // Create VRAM cache with 16 slots (simulating 6GB for 32KB blocks = ~24 blocks realistic)
    let vram_cache = VramCacheCapsule::new(16);
    println!("VRAM Cache initialized:");
    println!("  - Capacity: 16 slots");
    println!("  - Size per slot: 32KB (WeightBlock)");
    println!("  - Total: {} MB", 16 * 32 / 1024);
    println!("  - Eviction: CLOCK with Q8.8 frequency weighting");
    println!();

    // Simulate loading first 8 layers into VRAM
    println!("Loading first 8 layers into VRAM...");
    for layer_id in 0u64..8 {
        let slot = vram_cache.insert(layer_id).expect("insert failed");
        print!("  Layer {}: slot {} ", layer_id, slot);

        // Simulate access pattern (embedding layer accessed more frequently)
        if layer_id == 0 {
            for _ in 0..10 {
                vram_cache.lookup(layer_id);
            }
            println!("(pinned - 10 accesses)");
        } else {
            vram_cache.lookup(layer_id);
            println!("(1 access)");
        }
    }

    // Pin embedding layer
    vram_cache.pin_block(0).expect("pin failed");
    println!("\nEmbedding layer (0) pinned to prevent eviction");

    // Get cache metrics
    let metrics = vram_cache.metrics();
    println!("\nVRAM Cache Metrics:");
    println!("  - Hits: {}", metrics.hits);
    println!("  - Misses: {}", metrics.misses);
    println!("  - Hit Rate: {:.1}%", metrics.hit_rate() * 100.0);

    // Demonstrate RAM cache
    println!("\n--- RAM Cache ---");
    let mut ram_cache = RamCacheCapsule::new(0x123456789abcdef0, 1024);
    ram_cache
        .init_mapping(0x7f00_0000_0000, 32 * 1024 * 1024)
        .expect("mmap failed");

    println!("RAM Cache initialized:");
    println!("  - Base address: 0x7f00_0000_0000 (mmap'd)");
    println!("  - Capacity: 1024 blocks");
    println!("  - Total: 32 MB");

    // Prefetch simulation
    for block_id in 0..8 {
        ram_cache.prefetch_request(block_id).expect("prefetch failed");
    }
    println!("  - Prefetch requested: 8 blocks");

    let ram_metrics = ram_cache.metrics();
    println!("  - Page faults: {}", ram_metrics.page_faults);
    println!("  - Prefetch hits: {}", ram_metrics.prefetch_hits);

    // Demonstrate SSD loader
    println!("\n--- SSD Loader ---");
    let mut ssd_loader = SsdLoaderCapsule::new(32 * 1024);
    ssd_loader
        .open_file(0xabcdef1234567890, 1000)
        .expect("open failed");

    println!("SSD Loader initialized:");
    println!("  - Block size: 32KB");
    println!("  - Total blocks: 1000");
    println!("  - Backend: io_uring (stubbed for portability)");

    // Submit batch read
    let block_ids = [0, 1, 2, 3, 4, 5, 6, 7];
    let offsets: Vec<u64> = block_ids.iter().map(|&id| id * 32 * 1024).collect();
    let submitted = ssd_loader
        .submit_batch(&block_ids, &offsets)
        .expect("batch failed");
    println!("  - Batch submitted: {} blocks", submitted);

    // Poll completions
    let mut completed = 0;
    while let Some((_, result)) = ssd_loader.poll_completion() {
        if result.is_ok() {
            completed += 1;
        }
    }
    println!("  - Completions polled: {}", completed);

    let ssd_metrics = ssd_loader.metrics();
    println!("  - Bytes read: {} KB", ssd_metrics.bytes_read / 1024);
    println!("  - IOPS: {}", ssd_metrics.iops);
}

/// Demonstration of weight audit for Q34 compliance
fn demo_weight_audit() {
    println!("\n=== Q34 Weight Audit Demonstration ===\n");

    let mut audit = WeightAuditCapsule::new();

    // Simulate 32 weight blocks (one per layer for demo)
    let num_blocks = 32;
    let mut block_data: Vec<Vec<u8>> = Vec::with_capacity(num_blocks);
    let mut expected_hashes: Vec<u64> = Vec::with_capacity(num_blocks);

    println!("Generating {} simulated weight blocks...", num_blocks);
    for layer_id in 0..num_blocks {
        // Generate deterministic "weight" data for each layer
        let data: Vec<u8> = (0..1024)
            .map(|i| ((layer_id * 256 + i) & 0xFF) as u8)
            .collect();
        let hash = fnv1a_hash(&data);
        expected_hashes.push(hash);
        block_data.push(data);
    }

    // Set expected hashes (from manifest)
    audit.set_expected_hashes(&expected_hashes).unwrap();
    println!("Expected hashes loaded from manifest: {} blocks", num_blocks);

    // Verify blocks
    println!("\nVerifying block integrity...");
    let start = Instant::now();
    let mut verified = 0;
    let mut failed = 0;

    for (layer_id, data) in block_data.iter().enumerate() {
        match audit.verify_block(layer_id as u64, data) {
            Ok(true) => {
                audit.mark_verified(layer_id as u64).unwrap();
                // Update chain hash
                audit.update_chain_hash(expected_hashes[layer_id]);
                verified += 1;
            }
            Ok(false) | Err(_) => {
                failed += 1;
            }
        }
    }
    let verify_time = start.elapsed();

    println!("  Verified: {}/{} blocks", verified, num_blocks);
    println!("  Failed: {} blocks", failed);
    println!("  Time: {:.2} us", verify_time.as_micros());
    println!(
        "  Per-block: {:.1} ns",
        verify_time.as_nanos() as f64 / num_blocks as f64
    );

    // Get chain hash
    let chain_hash = audit.get_chain_hash();
    println!("\nWeight Audit:");
    println!("  Chain Hash: 0x{:016x}", chain_hash);
    println!("  Verified Blocks: {}/{}", audit.verified_count(), num_blocks);
    println!("  Integrity: {}", if failed == 0 { "PASSED" } else { "FAILED" });

    // Demonstrate tampering detection
    println!("\n--- Tampering Detection Demo ---");
    let tampered_data = vec![0xFF; 1024];
    match audit.verify_block(0, &tampered_data) {
        Err(e) => println!("  Tampered block detected: {}", e),
        Ok(_) => println!("  WARNING: Tampering not detected!"),
    }
}

/// Simulate inference pass through transformer layers
fn simulate_inference_pass(model: &Qwen3ModelConfig) -> InferenceMetrics {
    let mut metrics = InferenceMetrics::new();
    let vram_cache = VramCacheCapsule::new(16);

    // Simulate loading first 24 layers into VRAM (fits in 6GB for 7B model)
    let vram_layers = (model.num_layers as usize).min(24);
    for layer_id in 0..vram_layers {
        let _ = vram_cache.insert(layer_id as u64);
    }

    // Pin embedding and output layers
    if vram_layers > 0 {
        let _ = vram_cache.pin_block(0);
    }

    // Simulate inference through all layers
    for layer_id in 0..model.num_layers {
        let start = Instant::now();
        metrics.total_layers += 1;

        // Check cache tiers
        if vram_cache.lookup(layer_id as u64).is_some() {
            // VRAM hit - fastest path
            metrics.vram_hits += 1;
            // Simulate ~100ns VRAM access
            std::thread::sleep(Duration::from_nanos(50));
        } else if layer_id < 48 {
            // RAM hit - medium path (simulated for layers 24-47)
            metrics.ram_hits += 1;
            // Simulate ~1us RAM->VRAM transfer
            std::thread::sleep(Duration::from_nanos(500));
        } else {
            // SSD load - slowest path
            metrics.ssd_loads += 1;
            // Simulate ~10us SSD->RAM->VRAM
            std::thread::sleep(Duration::from_micros(5));
        }

        metrics.layer_latencies_ns.push(start.elapsed().as_nanos() as u64);
    }

    metrics
}

/// Main demo entry point
fn main() {
    println!("================================================================================");
    println!("           Kindly Brain: Sovereign Personal Knowledge Assistant                ");
    println!("              Qwen 3 Streaming Weight Demo (8GB VRAM)                          ");
    println!("================================================================================");

    // Select model based on simulated VRAM constraint
    let model = Qwen3ModelConfig::qwen3_7b();

    println!("\n=== Model Configuration ===\n");
    println!("Model: {} (simulated)", model.name);
    println!("Layers: {}", model.num_layers);
    println!("Hidden Dim: {}", model.hidden_dim);
    println!("Size: {:.1} GB (Q4_K_M quantized)", model.size_gb);
    println!("Total Blocks: {}", model.total_blocks());

    println!("\n=== Memory Budget ===\n");
    println!("VRAM Budget: 6144 MB (8GB card with 2GB KV cache reserve)");
    println!("RAM Budget: 32768 MB");
    println!("Block Size: 32 KB (256 Q4KM superblocks per block)");

    // Initialize GigaMeta system
    println!("\n=== Initializing 3-Tier Cache System ===\n");

    let config = GigaMetaConfig {
        vram_budget: 6 * 1024 * 1024 * 1024,  // 6GB
        ram_budget: 32 * 1024 * 1024 * 1024,  // 32GB
        block_size: 32 * 1024,                 // 32KB
        pinned_layers: vec![0, model.num_layers - 1], // Pin embedding + output
        prefetch_depth: 8,                     // 8 blocks ahead
    };

    let gigameta = GigaMetaWeightCapsule::with_config(&config);
    println!("GigaMetaWeightCapsule: {} bytes, {} bytes aligned",
             std::mem::size_of::<GigaMetaWeightCapsule>(),
             std::mem::align_of::<GigaMetaWeightCapsule>());

    // Transition to Ready
    println!("Phase: {:?} -> Ready", gigameta.phase());

    // Demonstrate cache tiers
    demo_three_tier_cache();

    // Demonstrate weight audit
    demo_weight_audit();

    // Simulate inference pass
    println!("\n=== Simulating Inference Pass ===\n");
    println!("Processing {} transformer layers...", model.num_layers);

    let inference_start = Instant::now();
    let metrics = simulate_inference_pass(&model);
    let total_inference_time = inference_start.elapsed();

    // Layer-by-layer output (first 8 + last 4)
    println!("\nLayer Access Pattern:");
    for (i, latency) in metrics.layer_latencies_ns.iter().enumerate().take(8) {
        let tier = if i < 16 { "VRAM" } else if i < 48 { "RAM" } else { "SSD" };
        let unit = if *latency < 1000 { "ns" } else { "us" };
        let value = if *latency < 1000 {
            *latency as f64
        } else {
            *latency as f64 / 1000.0
        };
        println!("  Layer {:2}: {} hit ({:.1} {})", i, tier, value, unit);
    }
    if model.num_layers > 12 {
        println!("  ...");
        for i in (model.num_layers - 4) as usize..model.num_layers as usize {
            let latency = metrics.layer_latencies_ns[i];
            let tier = if i < 16 { "VRAM" } else if i < 48 { "RAM" } else { "SSD" };
            let unit = if latency < 1000 { "ns" } else { "us" };
            let value = if latency < 1000 {
                latency as f64
            } else {
                latency as f64 / 1000.0
            };
            println!("  Layer {:2}: {} hit ({:.1} {})", i, tier, value, unit);
        }
    }

    // Summary metrics
    println!("\n=== Inference Metrics ===\n");
    println!("Cache Performance:");
    println!("  VRAM Hits: {}/{} ({:.1}%)",
             metrics.vram_hits, metrics.total_layers, metrics.vram_hit_rate());
    println!("  RAM Hits:  {}/{} ({:.1}%)",
             metrics.ram_hits, metrics.total_layers, metrics.ram_hit_rate());
    println!("  SSD Loads: {}/{} ({:.1}%)",
             metrics.ssd_loads, metrics.total_layers, metrics.ssd_load_rate());

    println!("\nTiming:");
    println!("  Avg Latency: {:.1} us/layer", metrics.avg_latency_us());
    println!("  Total Time: {:.2} ms ({} layers)",
             metrics.total_time_ms(), metrics.total_layers);
    println!("  Wall Clock: {:.2} ms", total_inference_time.as_secs_f64() * 1000.0);

    // GigaMeta snapshot
    let snapshot = gigameta.snapshot();
    println!("\n=== GigaMeta Snapshot ===\n");
    println!("Phase: {:?}", snapshot.phase);
    println!("Generation: {}", snapshot.generation);
    println!("VRAM Budget: {} GB", gigameta.vram_budget() / (1024 * 1024 * 1024));
    println!("RAM Budget: {} GB", gigameta.ram_budget() / (1024 * 1024 * 1024));
    println!("Block Size: {} KB", gigameta.block_size() / 1024);

    // Capsule sizes
    println!("\n=== Capsule Memory Footprint ===\n");
    println!("GigaMetaWeightCapsule: {} bytes (1024B aligned)",
             std::mem::size_of::<GigaMetaWeightCapsule>());
    println!("VramCacheCapsule:      {} bytes (512B aligned)",
             std::mem::size_of::<VramCacheCapsule>());
    println!("RamCacheCapsule:       {} bytes (256B aligned)",
             std::mem::size_of::<RamCacheCapsule>());
    println!("SsdLoaderCapsule:      {} bytes (256B aligned)",
             std::mem::size_of::<SsdLoaderCapsule>());
    println!("WeightAuditCapsule:    {} bytes (128B aligned)",
             std::mem::size_of::<WeightAuditCapsule>());
    println!("WeightBlock:           {} bytes (32KB aligned)",
             std::mem::size_of::<WeightBlock>());

    // Final summary
    println!("\n================================================================================");
    println!("                        Kindly Brain Demo Complete                             ");
    println!("================================================================================");
    println!();
    println!("Key Takeaways:");
    println!("  1. 3-tier caching enables running {} on 8GB VRAM", model.name);
    println!("  2. VRAM cache uses CLOCK eviction with Q8.8 frequency weighting");
    println!("  3. Pinned layers (embedding/output) never evicted");
    println!("  4. Q34-compliant FNV-1a hash chain for weight integrity");
    println!("  5. 100% lockfree architecture (NO mutex/RwLock)");
    println!();
    println!("Next Steps:");
    println!("  - Integrate GGUF parser for real model loading");
    println!("  - Implement actual GPU memory allocation via CUDA/ROCm");
    println!("  - Add KV cache compression for longer context");
    println!("  - Enable speculative decoding with draft model");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_configs() {
        let qwen7b = Qwen3ModelConfig::qwen3_7b();
        assert_eq!(qwen7b.num_layers, 32);
        assert_eq!(qwen7b.total_blocks(), 32 * 6);

        let qwen14b = Qwen3ModelConfig::qwen3_14b();
        assert_eq!(qwen14b.num_layers, 40);

        let qwen32b = Qwen3ModelConfig::qwen3_32b();
        assert_eq!(qwen32b.num_layers, 64);
    }

    #[test]
    fn test_inference_metrics() {
        let mut metrics = InferenceMetrics::new();
        metrics.vram_hits = 24;
        metrics.ram_hits = 6;
        metrics.ssd_loads = 2;
        metrics.total_layers = 32;
        metrics.layer_latencies_ns = vec![100; 32];

        assert!((metrics.vram_hit_rate() - 75.0).abs() < 0.1);
        assert!((metrics.ram_hit_rate() - 18.75).abs() < 0.1);
        assert!((metrics.ssd_load_rate() - 6.25).abs() < 0.1);
        assert!((metrics.avg_latency_us() - 0.1).abs() < 0.01);
    }

    #[test]
    fn test_capsule_sizes() {
        assert_eq!(std::mem::size_of::<GigaMetaWeightCapsule>(), 1024);
        assert_eq!(std::mem::size_of::<VramCacheCapsule>(), 512);
        assert_eq!(std::mem::size_of::<RamCacheCapsule>(), 256);
        assert_eq!(std::mem::size_of::<SsdLoaderCapsule>(), 256);
        assert_eq!(std::mem::size_of::<WeightAuditCapsule>(), 128);
        assert_eq!(std::mem::size_of::<WeightBlock>(), 32768);
    }

    #[test]
    fn test_vram_cache_pin() {
        let cache = VramCacheCapsule::new(16);
        cache.insert(0).unwrap();
        cache.pin_block(0).unwrap();
        assert!(cache.is_pinned(0));
    }

    #[test]
    fn test_weight_audit_chain() {
        let mut audit = WeightAuditCapsule::new();
        let data = b"test block data";
        let hash = fnv1a_hash(data);
        let hashes = vec![hash];
        audit.set_expected_hashes(&hashes).unwrap();

        assert!(audit.verify_block(0, data).unwrap());
        audit.mark_verified(0).unwrap();
        assert!(audit.is_verified(0));
    }
}
