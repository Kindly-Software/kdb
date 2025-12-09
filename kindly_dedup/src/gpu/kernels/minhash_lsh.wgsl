// MinHash LSH Compute Kernel - T7 Heterogeneous Tier
//
// GPU-accelerated MinHash + LSH for O(n) duplicate detection in kindly_dedup.
// This shader implements the COMPLETE LSH pipeline: MinHash signatures -> Band hashes
//
// Configuration: b=20 bands, r=6 rows per band (120/128 signature coverage)
// This configuration optimizes for DUPLICATE DETECTION (high sensitivity):
// - P(collision | J=0.8) = 1 - (1 - 0.8^6)^20 = 99.98% (near-duplicates caught)
// - P(collision | J=0.5) = 1 - (1 - 0.5^6)^20 = 27.1% (different docs rarely collide)
// - P(collision | J=0.3) = 1 - (1 - 0.3^6)^20 = 1.4% (very low false positives)
//
// Expected Performance (B32 validated targets):
// - MinHash kernel: 60K+ docs/sec (matching CPU LSH baseline)
// - LSH band kernel: 100K+ docs/sec (memory-bound, embarrassingly parallel)
// - End-to-end: 50K+ docs/sec (combined pipeline)
// - Memory: O(batch_size), capped at 100K docs per batch
//
// Framework Compliance:
// - UCE34: T7 Heterogeneous tier (GPU compute)
// - COCA: 100% parallel (no locks, no synchronization between workgroups)
// - B32: Fair baseline comparison with CPU SIMD implementation
// - T28: Property tests (GPU == CPU within tolerance, deterministic output)
// - Q34: Audit trail integration via host-side hash chain
//
// ASSUM Safety:
// - #ASSUME_WORKGROUP_256: 256 threads/workgroup optimal for compute shaders
// - #VERIFY_WORKGROUP_256: Tested on NVIDIA RTX 3000+, AMD RDNA2+, Intel Arc
// - #ASSUME_MURMURHASH3_QUALITY: MurmurHash3 provides sufficient band hash quality
// - #VERIFY_MURMURHASH3_QUALITY: Distribution validated via property tests
// - #ASSUME_BAND_CONFIG_20_6: b=20, r=6 optimal for 80%+ Jaccard detection
// - #VERIFY_BAND_CONFIG_20_6: Recall/precision validated against ground truth
// - #ASSUME_IGPU_COMPATIBLE: Shader works on integrated GPUs (Intel UHD, AMD Vega)
// - #VERIFY_IGPU_COMPATIBLE: Tested on Intel i7-155H iGPU, AMD 6900HX iGPU

// =============================================================================
// LSH Configuration Constants
// =============================================================================

// LSH band configuration for duplicate detection
// b=20 bands, r=6 rows per band = 120 signature values used (128 available)
// Remaining 8 values are padding (not used in band hashing)
const NUM_BANDS: u32 = 20u;
const ROWS_PER_BAND: u32 = 6u;
const SIGNATURE_SIZE: u32 = 128u;
const SIGNATURE_PACKED: u32 = 64u;  // 128 u16 packed as 64 u32

// Workgroup configuration
const WORKGROUP_SIZE: u32 = 256u;

// Hash constants
const FNV_OFFSET_BASIS: u32 = 2166136261u;
const FNV_PRIME: u32 = 16777619u;

// MurmurHash3 constants
const MURMUR_C1: u32 = 0xcc9e2d51u;
const MURMUR_C2: u32 = 0x1b873593u;
const MURMUR_SEED: u32 = 0x9747b28cu;  // Fixed seed for determinism

// Avalanche mixing constants
const AVALANCHE_MUL1: u32 = 2654435769u;
const AVALANCHE_MUL2: u32 = 1597334677u;

// =============================================================================
// Shared Memory (Workgroup Local)
// =============================================================================

// Permutation seeds loaded into shared memory for fast access
// 128 seeds x 4 bytes = 512 bytes (fits comfortably in shared memory)
var<workgroup> shared_seeds: array<u32, 128>;

// =============================================================================
// Kernel 1: MinHash Signature Computation
// =============================================================================

// Bindings for MinHash kernel
@group(0) @binding(0) var<storage, read> minhash_seeds: array<u32, 128>;
@group(0) @binding(1) var<storage, read> tokens: array<u32>;
@group(0) @binding(2) var<storage, read> token_offsets: array<u32>;
@group(0) @binding(3) var<storage, read_write> signatures: array<u32>;

// -----------------------------------------------------------------------------
// Hash Helper Functions
// -----------------------------------------------------------------------------

/// FNV-1a hash with seed mixing for MinHash permutations
///
/// This function implements a fast hash suitable for MinHash:
/// - XOR seed with FNV offset basis for permutation
/// - Process token as single u32 (pre-hashed tokens)
/// - Avalanche mixing for good distribution
///
/// Performance: ~4 ALU ops + 2 multiplies per hash
fn hash_with_seed(token: u32, seed: u32) -> u32 {
    // Initialize with seed-permuted offset basis
    var h = seed ^ FNV_OFFSET_BASIS;

    // Mix in token (single-step, tokens are pre-hashed)
    h = h ^ token;
    h = h * FNV_PRIME;

    // Avalanche mixing (ensures good bit distribution)
    h = h ^ (h >> 16u);
    h = h * AVALANCHE_MUL1;
    h = h ^ (h >> 13u);
    h = h * AVALANCHE_MUL2;
    h = h ^ (h >> 16u);

    return h;
}

/// Process 4 hashes in parallel for better instruction-level parallelism
/// GPU ALUs can execute multiple independent operations simultaneously
fn hash_4_parallel(
    token: u32,
    seed0: u32,
    seed1: u32,
    seed2: u32,
    seed3: u32
) -> vec4<u32> {
    // Initialize all 4 hashes
    var h0 = seed0 ^ FNV_OFFSET_BASIS;
    var h1 = seed1 ^ FNV_OFFSET_BASIS;
    var h2 = seed2 ^ FNV_OFFSET_BASIS;
    var h3 = seed3 ^ FNV_OFFSET_BASIS;

    // XOR token into all hashes (independent operations)
    h0 = h0 ^ token;
    h1 = h1 ^ token;
    h2 = h2 ^ token;
    h3 = h3 ^ token;

    // Multiply by FNV prime
    h0 = h0 * FNV_PRIME;
    h1 = h1 * FNV_PRIME;
    h2 = h2 * FNV_PRIME;
    h3 = h3 * FNV_PRIME;

    // Avalanche step 1: XOR with shifted self
    h0 = h0 ^ (h0 >> 16u);
    h1 = h1 ^ (h1 >> 16u);
    h2 = h2 ^ (h2 >> 16u);
    h3 = h3 ^ (h3 >> 16u);

    // Avalanche step 2: multiply
    h0 = h0 * AVALANCHE_MUL1;
    h1 = h1 * AVALANCHE_MUL1;
    h2 = h2 * AVALANCHE_MUL1;
    h3 = h3 * AVALANCHE_MUL1;

    // Avalanche step 3: XOR with shifted self
    h0 = h0 ^ (h0 >> 13u);
    h1 = h1 ^ (h1 >> 13u);
    h2 = h2 ^ (h2 >> 13u);
    h3 = h3 ^ (h3 >> 13u);

    // Avalanche step 4: final multiply
    h0 = h0 * AVALANCHE_MUL2;
    h1 = h1 * AVALANCHE_MUL2;
    h2 = h2 * AVALANCHE_MUL2;
    h3 = h3 * AVALANCHE_MUL2;

    // Avalanche step 5: final XOR
    h0 = h0 ^ (h0 >> 16u);
    h1 = h1 ^ (h1 >> 16u);
    h2 = h2 ^ (h2 >> 16u);
    h3 = h3 ^ (h3 >> 16u);

    return vec4<u32>(h0, h1, h2, h3);
}

// -----------------------------------------------------------------------------
// MinHash Kernel Entry Point
// -----------------------------------------------------------------------------

/// MinHash kernel: One thread per document
///
/// Each thread computes all 128 MinHash values for its assigned document.
/// Uses shared memory for seeds to reduce global memory traffic.
///
/// Dispatch: ceil(num_docs / 256) workgroups
/// Memory pattern: Coalesced token reads, semi-coalesced signature writes
///
/// B32 Performance Target: 60K+ docs/sec (matching CPU baseline)
@compute @workgroup_size(256, 1, 1)
fn minhash_kernel(
    @builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(local_invocation_id) local_id: vec3<u32>,
) {
    let doc_id = global_id.x;
    let thread_id = local_id.x;

    // Phase 1: Cooperatively load seeds into shared memory
    // First 128 threads each load one seed (coalesced read)
    if (thread_id < 128u) {
        shared_seeds[thread_id] = minhash_seeds[thread_id];
    }
    workgroupBarrier();  // Ensure all seeds loaded before use

    // Get document count from offsets array
    // offsets has length num_docs + 1, so num_docs = arrayLength - 1
    let num_docs = arrayLength(&token_offsets) - 1u;

    // Bounds check: skip threads beyond document count
    if (doc_id >= num_docs) {
        return;
    }

    // Phase 2: Get token range for this document
    let start = token_offsets[doc_id];
    let end = token_offsets[doc_id + 1u];
    let num_tokens = end - start;

    // Phase 3: Initialize signature to maximum values
    // Using private memory (registers) for best performance
    // 64 u32 = 128 u16 packed (each u32 holds two signature values)
    var sig: array<u32, 64>;
    for (var i = 0u; i < 64u; i = i + 1u) {
        sig[i] = 0xFFFFFFFFu;  // Two u16::MAX packed
    }

    // Phase 4: Process each token, computing MinHash for all 128 permutations
    for (var t = start; t < end; t = t + 1u) {
        let token = tokens[t];

        // Process 4 hash pairs at a time (8 hashes) for better ILP
        for (var h = 0u; h < 64u; h = h + 4u) {
            // Compute 8 hashes (4 pairs)
            let hashes_a = hash_4_parallel(
                token,
                shared_seeds[h * 2u],
                shared_seeds[h * 2u + 1u],
                shared_seeds[h * 2u + 2u],
                shared_seeds[h * 2u + 3u]
            );
            let hashes_b = hash_4_parallel(
                token,
                shared_seeds[h * 2u + 4u],
                shared_seeds[h * 2u + 5u],
                shared_seeds[h * 2u + 6u],
                shared_seeds[h * 2u + 7u]
            );

            // Update 4 packed signature slots (8 u16 values)
            // Slot 0: hashes_a.x (lo), hashes_a.y (hi)
            let cur0 = sig[h];
            let new_lo0 = hashes_a.x & 0xFFFFu;
            let new_hi0 = hashes_a.y & 0xFFFFu;
            let min_lo0 = min(cur0 & 0xFFFFu, new_lo0);
            let min_hi0 = min((cur0 >> 16u) & 0xFFFFu, new_hi0);
            sig[h] = min_lo0 | (min_hi0 << 16u);

            // Slot 1: hashes_a.z (lo), hashes_a.w (hi)
            let cur1 = sig[h + 1u];
            let new_lo1 = hashes_a.z & 0xFFFFu;
            let new_hi1 = hashes_a.w & 0xFFFFu;
            let min_lo1 = min(cur1 & 0xFFFFu, new_lo1);
            let min_hi1 = min((cur1 >> 16u) & 0xFFFFu, new_hi1);
            sig[h + 1u] = min_lo1 | (min_hi1 << 16u);

            // Slot 2: hashes_b.x (lo), hashes_b.y (hi)
            let cur2 = sig[h + 2u];
            let new_lo2 = hashes_b.x & 0xFFFFu;
            let new_hi2 = hashes_b.y & 0xFFFFu;
            let min_lo2 = min(cur2 & 0xFFFFu, new_lo2);
            let min_hi2 = min((cur2 >> 16u) & 0xFFFFu, new_hi2);
            sig[h + 2u] = min_lo2 | (min_hi2 << 16u);

            // Slot 3: hashes_b.z (lo), hashes_b.w (hi)
            let cur3 = sig[h + 3u];
            let new_lo3 = hashes_b.z & 0xFFFFu;
            let new_hi3 = hashes_b.w & 0xFFFFu;
            let min_lo3 = min(cur3 & 0xFFFFu, new_lo3);
            let min_hi3 = min((cur3 >> 16u) & 0xFFFFu, new_hi3);
            sig[h + 3u] = min_lo3 | (min_hi3 << 16u);
        }
    }

    // Phase 5: Write signature to output (coalesced within workgroup)
    let out_base = doc_id * SIGNATURE_PACKED;
    for (var i = 0u; i < SIGNATURE_PACKED; i = i + 1u) {
        signatures[out_base + i] = sig[i];
    }
}

// =============================================================================
// Kernel 2: LSH Band Hash Computation
// =============================================================================

// Bindings for LSH band kernel (separate binding group for flexibility)
@group(1) @binding(0) var<storage, read> lsh_signatures: array<u32>;
@group(1) @binding(1) var<storage, read_write> band_hashes: array<u32>;
@group(1) @binding(2) var<uniform> lsh_num_docs: u32;

// -----------------------------------------------------------------------------
// MurmurHash3 Implementation for Band Hashing
// -----------------------------------------------------------------------------

/// MurmurHash3 32-bit finalizer (fmix32)
/// Used for final mixing of band hash values
fn fmix32(h_in: u32) -> u32 {
    var h = h_in;
    h = h ^ (h >> 16u);
    h = h * 0x85ebca6bu;
    h = h ^ (h >> 13u);
    h = h * 0xc2b2ae35u;
    h = h ^ (h >> 16u);
    return h;
}

/// MurmurHash3 32-bit for a single u32 key
/// Fast hash for combining multiple signature values into band hash
fn murmurhash3_32(key: u32, seed: u32) -> u32 {
    var h = seed;
    var k = key;

    // Body: single block
    k = k * MURMUR_C1;
    k = (k << 15u) | (k >> 17u);  // ROTL32(k, 15)
    k = k * MURMUR_C2;

    h = h ^ k;
    h = (h << 13u) | (h >> 19u);  // ROTL32(h, 13)
    h = h * 5u + 0xe6546b64u;

    // Finalization
    h = h ^ 4u;  // len = 4 bytes
    h = fmix32(h);

    return h;
}

/// MurmurHash3 for array of u32 values (variable length)
/// Used to hash r=6 signature values per band
fn murmurhash3_array(values: array<u32, 6>, len: u32, seed: u32) -> u32 {
    var h = seed;

    // Process each 32-bit value
    for (var i = 0u; i < len; i = i + 1u) {
        var k = values[i];

        k = k * MURMUR_C1;
        k = (k << 15u) | (k >> 17u);
        k = k * MURMUR_C2;

        h = h ^ k;
        h = (h << 13u) | (h >> 19u);
        h = h * 5u + 0xe6546b64u;
    }

    // Finalization with total length
    h = h ^ (len * 4u);
    h = fmix32(h);

    return h;
}

/// FNV-1a 32-bit for array of u32 values
/// Alternative hash for band computation (simpler, still high quality)
fn fnv1a_array(values: array<u32, 6>, len: u32) -> u32 {
    var h = FNV_OFFSET_BASIS;

    for (var i = 0u; i < len; i = i + 1u) {
        let val = values[i];

        // Process as 4 bytes
        h = h ^ (val & 0xFFu);
        h = h * FNV_PRIME;
        h = h ^ ((val >> 8u) & 0xFFu);
        h = h * FNV_PRIME;
        h = h ^ ((val >> 16u) & 0xFFu);
        h = h * FNV_PRIME;
        h = h ^ ((val >> 24u) & 0xFFu);
        h = h * FNV_PRIME;
    }

    return h;
}

// -----------------------------------------------------------------------------
// Signature Value Extraction
// -----------------------------------------------------------------------------

/// Extract u16 signature value from packed u32 array
/// sig_idx: 0-127 (index into 128 u16 signature)
/// doc_base: doc_id * 64 (base offset for document in signatures array)
fn extract_signature_value(doc_base: u32, sig_idx: u32) -> u32 {
    let packed_idx = sig_idx / 2u;
    let is_high = sig_idx % 2u;
    let packed = lsh_signatures[doc_base + packed_idx];

    if (is_high == 1u) {
        return (packed >> 16u) & 0xFFFFu;
    } else {
        return packed & 0xFFFFu;
    }
}

/// Compute band hash for a single band
/// band_idx: 0-19 (which band)
/// doc_base: doc_id * 64 (base offset for document)
/// Returns: 32-bit band hash (MurmurHash3 of r=6 signature values)
fn compute_band_hash(doc_base: u32, band_idx: u32) -> u32 {
    // Start index in signature for this band
    let start_sig = band_idx * ROWS_PER_BAND;

    // Collect r=6 signature values for this band
    var band_values: array<u32, 6>;
    for (var r = 0u; r < ROWS_PER_BAND; r = r + 1u) {
        band_values[r] = extract_signature_value(doc_base, start_sig + r);
    }

    // Use band_idx as seed for different hash per band
    // This ensures same signature values in different bands produce different hashes
    return murmurhash3_array(band_values, ROWS_PER_BAND, MURMUR_SEED ^ band_idx);
}

// -----------------------------------------------------------------------------
// LSH Band Kernel Entry Point
// -----------------------------------------------------------------------------

/// LSH Band kernel: One thread per (document, band) pair
///
/// Each thread computes one band hash for one document.
/// Output: 20 band hashes per document (one per band)
///
/// Dispatch: ceil((num_docs * NUM_BANDS) / 256) workgroups
/// Memory pattern: Coalesced signature reads, coalesced band hash writes
///
/// B32 Performance Target: 100K+ docs/sec (memory-bound, embarrassingly parallel)
@compute @workgroup_size(256, 1, 1)
fn lsh_band_kernel(
    @builtin(global_invocation_id) global_id: vec3<u32>,
) {
    // Map thread to (doc_id, band_idx)
    let work_id = global_id.x;
    let doc_id = work_id / NUM_BANDS;
    let band_idx = work_id % NUM_BANDS;

    // Bounds check
    if (doc_id >= lsh_num_docs) {
        return;
    }

    // Compute band hash
    let doc_base = doc_id * SIGNATURE_PACKED;
    let band_hash = compute_band_hash(doc_base, band_idx);

    // Write output (1 u32 per band hash)
    let out_idx = doc_id * NUM_BANDS + band_idx;
    band_hashes[out_idx] = band_hash;
}

/// LSH Band kernel (per-document variant): One thread per document
///
/// Each thread computes all 20 band hashes for one document.
/// Better for small batches (< 10K docs) due to reduced thread overhead.
///
/// Dispatch: ceil(num_docs / 256) workgroups
@compute @workgroup_size(256, 1, 1)
fn lsh_band_per_doc_kernel(
    @builtin(global_invocation_id) global_id: vec3<u32>,
) {
    let doc_id = global_id.x;

    // Bounds check
    if (doc_id >= lsh_num_docs) {
        return;
    }

    let doc_base = doc_id * SIGNATURE_PACKED;
    let out_base = doc_id * NUM_BANDS;

    // Compute all bands for this document
    for (var band_idx = 0u; band_idx < NUM_BANDS; band_idx = band_idx + 1u) {
        let band_hash = compute_band_hash(doc_base, band_idx);
        band_hashes[out_base + band_idx] = band_hash;
    }
}

// =============================================================================
// Combined Kernel: MinHash + LSH Band (Fused)
// =============================================================================

// Bindings for combined kernel
@group(2) @binding(0) var<storage, read> combined_seeds: array<u32, 128>;
@group(2) @binding(1) var<storage, read> combined_tokens: array<u32>;
@group(2) @binding(2) var<storage, read> combined_offsets: array<u32>;
@group(2) @binding(3) var<storage, read_write> combined_signatures: array<u32>;
@group(2) @binding(4) var<storage, read_write> combined_band_hashes: array<u32>;

/// Combined MinHash + LSH kernel: Fused pipeline
///
/// Computes MinHash signatures AND LSH band hashes in a single pass.
/// Avoids intermediate buffer write/read for signatures.
///
/// Trade-off: Higher register pressure but eliminates memory round-trip.
/// Best for: Large batches where memory bandwidth is the bottleneck.
///
/// Dispatch: ceil(num_docs / 256) workgroups
/// Memory: O(1) intermediate (signatures in registers)
@compute @workgroup_size(256, 1, 1)
fn minhash_lsh_combined_kernel(
    @builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(local_invocation_id) local_id: vec3<u32>,
) {
    let doc_id = global_id.x;
    let thread_id = local_id.x;

    // Phase 1: Cooperatively load seeds
    if (thread_id < 128u) {
        shared_seeds[thread_id] = combined_seeds[thread_id];
    }
    workgroupBarrier();

    let num_docs = arrayLength(&combined_offsets) - 1u;

    if (doc_id >= num_docs) {
        return;
    }

    // Phase 2: Get token range
    let start = combined_offsets[doc_id];
    let end = combined_offsets[doc_id + 1u];

    // Phase 3: Compute MinHash signatures (in registers)
    var sig: array<u32, 64>;
    for (var i = 0u; i < 64u; i = i + 1u) {
        sig[i] = 0xFFFFFFFFu;
    }

    for (var t = start; t < end; t = t + 1u) {
        let token = combined_tokens[t];

        for (var h = 0u; h < 64u; h = h + 1u) {
            let hash_lo = hash_with_seed(token, shared_seeds[h * 2u]);
            let hash_hi = hash_with_seed(token, shared_seeds[h * 2u + 1u]);

            let cur = sig[h];
            let new_lo = hash_lo & 0xFFFFu;
            let new_hi = hash_hi & 0xFFFFu;
            let min_lo = min(cur & 0xFFFFu, new_lo);
            let min_hi = min((cur >> 16u) & 0xFFFFu, new_hi);
            sig[h] = min_lo | (min_hi << 16u);
        }
    }

    // Phase 4: Write signatures (optional, for debugging/validation)
    let sig_base = doc_id * SIGNATURE_PACKED;
    for (var i = 0u; i < SIGNATURE_PACKED; i = i + 1u) {
        combined_signatures[sig_base + i] = sig[i];
    }

    // Phase 5: Compute LSH band hashes directly from registers
    let band_base = doc_id * NUM_BANDS;

    for (var band_idx = 0u; band_idx < NUM_BANDS; band_idx = band_idx + 1u) {
        let start_sig = band_idx * ROWS_PER_BAND;

        // Extract r=6 values from local signature array
        var band_values: array<u32, 6>;
        for (var r = 0u; r < ROWS_PER_BAND; r = r + 1u) {
            let sig_idx = start_sig + r;
            let packed_idx = sig_idx / 2u;
            let is_high = sig_idx % 2u;
            let packed = sig[packed_idx];

            if (is_high == 1u) {
                band_values[r] = (packed >> 16u) & 0xFFFFu;
            } else {
                band_values[r] = packed & 0xFFFFu;
            }
        }

        // Compute and write band hash
        let band_hash = murmurhash3_array(band_values, ROWS_PER_BAND, MURMUR_SEED ^ band_idx);
        combined_band_hashes[band_base + band_idx] = band_hash;
    }
}

// =============================================================================
// Utility Kernels
// =============================================================================

/// Zero-initialize signatures buffer
/// Useful for batch reset between processing runs
@compute @workgroup_size(256, 1, 1)
fn zero_signatures(
    @builtin(global_invocation_id) global_id: vec3<u32>,
) {
    let idx = global_id.x;
    if (idx < arrayLength(&signatures)) {
        signatures[idx] = 0xFFFFFFFFu;
    }
}

/// Zero-initialize band hashes buffer
@compute @workgroup_size(256, 1, 1)
fn zero_band_hashes(
    @builtin(global_invocation_id) global_id: vec3<u32>,
) {
    let idx = global_id.x;
    if (idx < arrayLength(&band_hashes)) {
        band_hashes[idx] = 0u;
    }
}
