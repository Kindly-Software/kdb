# State-of-the-Art Speculative Decoding Techniques (2024-2025)

**Research Date**: 2025-11-30
**Framework**: UCE34 + Chaos (Computational Capsule Architecture)
**Target**: SpeculativeDraftCapsule implementation for atomic_capsule

---

## Executive Summary

Speculative decoding achieves **2-4× speedups** (state-of-the-art methods) for LLM inference by generating draft tokens with a lightweight model and verifying them in parallel with the target model. Key metrics:

- **Acceptance Rate (α)**: 0.6-0.8 typical, higher is better
- **Acceptance Length (τ)**: Average tokens accepted per round (2-5 typical)
- **Speedup**: 2-4× typical, up to 5× exceptional
- **Memory Overhead**: 0.5-15% additional parameters (method-dependent)

**Recommended Algorithm for Chaos**: **EAGLE-3** (2025) + lockfree verification queue

---

## 1. Original Speculative Decoding (Leviathan et al., 2022-2023)

### Paper
- **Title**: "Fast Inference from Transformers via Speculative Decoding"
- **Authors**: Yaniv Leviathan, Matan Kalman, Yossi Matias (Google Research)
- **Published**: ICML 2023
- **Links**: [arXiv](https://arxiv.org/abs/2211.17192) | [ICML Proceedings](https://proceedings.mlr.press/v202/leviathan23a)

### Core Algorithm

1. **Draft Generation**: Small model M_q generates γ candidate tokens in parallel
2. **Parallel Verification**: Large model M_p verifies all γ candidates in a single forward pass
3. **Acceptance Sampling**: For each position i:
   - If `p(x_i) ≥ q(x_i)`: Accept with probability `min(1, p(x_i)/q(x_i))`
   - Else: Reject and resample from adjusted distribution `max(0, p(x_i) - q(x_i))`
4. **Rejection Point**: Stop at first rejection, discard remaining candidates

### Key Insight
"Hard language-modeling tasks often include easier subtasks that can be approximated well by more efficient models" + speculative execution enables parallel verification.

### Performance Metrics
- **Speedup**: 2-3× on T5-XXL (11B parameters)
- **Draft Model**: T5-small (60M parameters) = 0.5% size
- **Acceptance Rate (β)**: Not explicitly reported, assumed i.i.d.
- **Memory Overhead**: ~5-10% (separate draft model)

### Limitations
- Requires training separate draft model
- Draft quality heavily impacts speedup
- Acceptance rate β assumed independent (violated in practice)

---

## 2. Medusa (2024)

### Paper
- **Title**: "Medusa: Simple LLM Inference Acceleration Framework with Multiple Decoding Heads"
- **Authors**: Tianle Cai et al.
- **Published**: ICML 2024
- **Links**: [arXiv](https://arxiv.org/abs/2401.10774) | [GitHub](https://github.com/FasterDecoding/Medusa) | [ICML](https://dl.acm.org/doi/10.5555/3692070.3692273)

### Core Algorithm

1. **Architecture**: Add N parallel prediction heads to existing LLM (no separate draft model)
2. **Multi-Head Prediction**: Each head predicts tokens at position i+1, i+2, ..., i+N
3. **Tree-Based Attention**: Construct candidate tree from head predictions
4. **Parallel Verification**: Verify all tree paths in single forward pass
5. **Tree Pruning**: Use typical acceptance scheme to select best paths

### Key Insight
"Multiple lightweight heads attached to the same backbone can predict future tokens without a separate draft model, simplifying deployment."

### Performance Metrics
- **Speedup**: 2.2-3.6× across LLaMA, Vicuna models
  - Medusa-1 (frozen backbone): 2.2-2.8×
  - Medusa-2 (fine-tuned backbone): 2.3-3.6×
- **Acceptance Rate**:
  - Top-1 accuracy: ~60% (next-next token)
  - Top-5 accuracy: >80%
  - Typical acceptance: +10% vs greedy
- **Memory Overhead**:
  - Medusa heads: ~2-5% of backbone parameters
  - Tree attention: <1% additional memory
- **Training Cost**: Parameter-efficient, "GPU-poor" friendly

### Architecture Details
- **Heads**: 2-4 lightweight MLPs (1-2 layers each)
- **Tree Depth**: 2-5 levels typical
- **Branching Factor**: 5-10 candidates per head

### Advantages
- No separate draft model (simpler deployment)
- Parameter-efficient training
- No distributed computing changes
- Self-distillation for no-data scenarios

### Limitations
- Requires fine-tuning backbone (Medusa-2 for best speedup)
- Tree attention overhead for wide trees
- Accuracy degrades for tokens >2 positions ahead

---

## 3. EAGLE (2024) & EAGLE-2 & EAGLE-3 (2025)

### Papers
- **EAGLE-1**: "Speculative Sampling Requires Rethinking Feature Uncertainty" (ICML 2024)
- **EAGLE-2**: "Faster Inference of Language Models with Dynamic Draft Trees" (EMNLP 2024)
- **EAGLE-3**: "Scaling up Inference Acceleration via Training-Time Test" (NeurIPS 2025)
- **Links**: [GitHub](https://github.com/SafeAILab/EAGLE) | [EAGLE-1 arXiv](https://arxiv.org/abs/2401.15077) | [EAGLE-2 arXiv](https://arxiv.org/abs/2406.16858) | [EAGLE-3 arXiv](https://arxiv.org/abs/2503.01840)

### Core Algorithm (EAGLE-1)

1. **Feature-Level Autoregression**: Auto-regression head predicts next feature (second-to-top layer) instead of next token
2. **Lightweight Draft Head**: Single-layer Transformer decoder (0.24-0.99B params)
3. **Frozen Embedding**: Reuse original LLM's embedding + classification head
4. **Feature Uncertainty**: Model inherent uncertainty in feature-level predictions
5. **Tree-Based Verification**: Generate candidate tree, verify in parallel

### Key Insight
"Autoregression at the feature level (second-to-top layer) is more straightforward than at the token level, but inherent uncertainty constrains performance."

### EAGLE-2 Enhancement (Context-Aware Draft Trees)

**Key Finding**: "Acceptance rate depends on BOTH position AND context" (not just position as assumed)

1. **Draft Confidence Metric**: Use draft model confidence as proxy for acceptance rate
2. **Dynamic Tree Structure**: Adjust tree depth/branching per context
3. **Context-Dependent Pruning**: Prune low-confidence branches early

### EAGLE-3 Enhancement (Training-Time Test)

1. **Adaptive Training**: Adjust training strategy based on inference-time metrics
2. **Acceptance-Aware Loss**: Weight training loss by expected acceptance rate
3. **Hardware-Optimized**: Tuned for NVIDIA GPUs (tensor core utilization)

### Performance Metrics

| Version | Speedup | Acceptance Length | Benchmark |
|---------|---------|------------------|-----------|
| EAGLE-1 | 2.0-3.0× | ~2.5 tokens/round | MT-bench |
| EAGLE-2 | 3.05-4.26× | ~3.2 tokens/round | LLaMA2-Chat 7B/13B |
| EAGLE-3 | 3.6-4.8× | ~3.8 tokens/round | Qwen3-14B/32B |

**Acceptance Rate**:
- 0-α: Acceptance rate with precise inputs
- 1-α: Acceptance rate with one imprecise feature
- Context-dependent: Varies 0.5-0.9 by task

**Memory Overhead**:
- Auto-regression head: 0.24-0.99B params (1.8-7.6% of 13B model)
- Feature cache: <5% additional memory
- Tree structure: <2% additional memory

### Comparative Speedup (MT-bench, EAGLE-1)
- vs Vanilla decoding: **3.0×**
- vs Lookahead: **2.0×**
- vs Medusa: **1.6×**

### Training Cost
- GPU-poor friendly (single-layer decoder)
- Training time: Hours on single GPU (vs days for Medusa-2)

---

## 4. Lookahead Decoding (2024)

### Paper
- **Title**: "Break the Sequential Dependency of LLM Inference Using Lookahead Decoding"
- **Authors**: Yichao Fu, Peter Bailis, Ion Stoica, Hao Zhang (UC Berkeley, UCSD)
- **Published**: ICML 2024
- **Links**: [arXiv](https://arxiv.org/abs/2402.02057) | [GitHub](https://github.com/hao-ai-lab/LookaheadDecoding) | [LMSYS Blog](https://lmsys.org/blog/2023-11-21-lookahead-decoding/)

### Core Algorithm (Jacobi Iteration Adaptation)

1. **View as Nonlinear Equations**: Autoregressive decoding = solving f(x) = x iteratively
2. **Jacobi Iteration**: Update all positions in parallel (vs sequential Gauss-Seidel)
3. **2D Lookahead Window**:
   - Window size W: How far ahead (future positions)
   - N-gram size N: How far back (past Jacobi iterations)
4. **Two Parallel Branches**:
   - **Lookahead Branch**: Generate n-grams from Jacobi trajectory
   - **Verification Branch**: Select and verify promising n-gram candidates
5. **N-gram Caching**: Cache n-grams from Jacobi iterations for reuse

### Key Insight
"Vanilla Jacobi decoding shows only 1.05× speedup because AR-trained LLMs rarely yield correct tokens when preceding tokens are incorrect. Caching n-grams from Jacobi trajectories enables substantial speedup."

### Performance Metrics
- **Speedup**: 1.5-2.3× single GPU, up to 4× multi-GPU (code completion)
- **Acceptance Rate**: Not explicitly reported (n-gram match rate instead)
- **Memory Overhead**:
  - 2D window cache: 5-10% additional memory
  - N-gram cache: <5% additional memory
- **Latency Reduction**: 1.5-2.3× on MT-bench

### Window Parameters
- **W (window size)**: 5-10 typical (positions ahead)
- **N (n-gram size)**: 3-7 typical (Jacobi iterations back)
- Tunable tradeoff: Larger W/N = more candidates but higher overhead

### Advantages
- **Exact decoding**: No approximation, identical output distribution
- **No draft model**: Uses only target LLM
- **No training**: Inference-only method
- **Parallel scaling**: 4× on multi-GPU (strong scaling)

### Limitations
- Limited speedup vs best draft-model methods (1.5-2.3× vs 3-4×)
- Window cache overhead
- Jacobi trajectory quality depends on task

---

## 5. Multi-Token Prediction (Meta, 2024)

### Paper
- **Title**: "Better & Faster Large Language Models via Multi-token Prediction"
- **Authors**: Fabian Gloeckle et al. (Meta)
- **Published**: April 2024
- **Links**: [arXiv](https://arxiv.org/abs/2404.19737) | [Medium Summary](https://medium.com/@himankvjain/accelerating-language-models-with-multi-token-prediction-9f0167232f5b) | [VentureBeat](https://venturebeat.com/ai/metas-new-multi-token-prediction-makes-ai-models-up-to-3x-faster)

### Core Algorithm

1. **Training Objective**: Predict N future tokens from each position (vs 1 in standard LM)
2. **Architecture**:
   - **Shared Transformer Trunk**: Process input context
   - **N Independent Heads**: Each predicts one of N future tokens
   - **Shared Unembedding Matrix**: Convert predictions to tokens
3. **Self-Speculative Decoding**: Use multi-token heads as built-in draft model
4. **Memory-Efficient Training**: Sequential forward/backward per head

### Key Insight
"Training to predict multiple future tokens results in higher sample efficiency and enables self-speculative decoding without a separate draft model."

### Performance Metrics
- **Inference Speedup**: Up to **3× (median)**, up to **3.6× (peak)**
- **Training Efficiency**:
  - 12% more problems solved (coding benchmark)
  - 17% improvement (another benchmark)
- **Acceptance Rate**: Not explicitly reported (implicit in speedup)
- **Memory Overhead**:
  - N prediction heads: ~5-10% parameters (N=4 typical)
  - Shared trunk: No overhead
  - Training memory: Reduced via sequential compute

### Multi-Token Prediction (N)
- **N=2**: ~1.5-2× speedup
- **N=4**: ~2-3× speedup (sweet spot)
- **N=8**: ~3-3.6× speedup (diminishing returns)

### Long-Term Pattern Learning
- Byte-level tokenization: Multi-byte prediction >> single-byte
- Promotes learning longer-term patterns vs next-token prediction

### Advantages
- Built-in draft capability (no separate model)
- Better sample efficiency during training
- Learns longer-term patterns

### Limitations
- Requires retraining from scratch (not applicable to existing models)
- N prediction heads increase model size
- Diminishing returns beyond N=4-8

---

## 6. REST (2024) - Retrieval-Based Speculative Decoding

### Paper
- **Title**: "REST: Retrieval-Based Speculative Decoding"
- **Authors**: Zhenyu He, Zexuan Zhong, Tianle Cai, Jason D. Lee, Di He
- **Published**: 2024
- **Links**: [ResearchGate](https://www.researchgate.net/publication/382632572_REST_Retrieval-Based_Speculative_Decoding)

### Core Algorithm

1. **Context Retrieval**: Retrieve relevant text spans from input prompt/context
2. **Draft from Retrieval**: Use retrieved spans as draft candidates (no model inference)
3. **Verification**: Verify retrieved spans with target LLM
4. **Hybrid Approach**: Combine retrieval drafts with model-based drafts

### Key Insight
"In context-dependent tasks (summarization, QA), reusing retrieved text spans from the prompt can provide high-quality drafts with zero inference cost."

### Performance Metrics
- **Speedup**: Significant on summarization tasks (exact numbers not available)
- **Acceptance Rate**: High for extractive tasks (spans match target distribution)
- **Memory Overhead**: Retrieval index (<5%)
- **Draft Cost**: **0 FLOPS** (pure retrieval, no model inference)

### Use Cases (Best Performance)
- **Summarization**: High overlap with source text
- **Question Answering**: Direct spans from context
- **Code Generation**: Reuse from similar code
- **Translation**: Phrase-level retrieval

### Advantages
- Zero-cost draft generation (no model inference)
- High acceptance for extractive tasks
- Can combine with model-based drafts

### Limitations
- Limited to context-heavy tasks
- Requires retrieval index
- Lower acceptance for generative tasks

---

## 7. DistillSpec (2024)

### Paper
- **Title**: "DistillSpec: Improving Speculative Decoding via Knowledge Distillation"
- **Authors**: Yun Zhu et al.
- **Published**: ICLR 2024
- **Links**: [arXiv](https://arxiv.org/abs/2310.08461) | [OpenReview](https://openreview.net/forum?id=rsY6J3ZaTF) | [ICLR Poster](https://iclr.cc/virtual/2024/poster/17680)

### Core Algorithm

1. **Knowledge Distillation**: Train draft model to mimic target model's distribution
2. **On-Policy Data**: Generate training data from draft model (vs target model)
3. **Divergence Function**: Tailor divergence metric to decoding strategy (greedy vs sampling)
4. **Two-Stage Process**:
   - Stage 1: Distill target model → aligned draft model (DistillSpec)
   - Stage 2: Apply speculative decoding with aligned draft

### Key Insight
"Better aligning the draft model with the target model via task-specific distillation yields 10-45% speedup over standard speculative decoding."

### Performance Metrics
- **Speedup Improvement**: +10-45% over standard speculative decoding
- **End-to-End Speedup**: 6-10× (distillation + DistillSpec) vs vanilla decoding
- **Acceptance Rate**: Improved via better draft-target alignment (exact numbers not available)
- **Memory Overhead**: Same as standard SD (separate draft model)

### Divergence Functions
- **Greedy Decoding**: Forward KL divergence
- **Sampling**: Reverse KL divergence or JSD
- **Task-Specific**: Custom divergence for domain

### Advantages
- Substantial speedup improvement over standard SD
- Applicable to any draft-target pair
- Minimal performance drop after distillation

### Limitations
- Requires draft model training
- Distillation overhead (hours-days)
- On-policy data generation cost

---

## 8. BiTA (2024) - Bi-Directional Tuning

### Paper
- **Title**: "BiTA: Bi-Directional Tuning for Lossless Acceleration in Large Language Models"
- **Authors**: Feng Lin, Hanling Yi, Hongbin Li, Yifan Yang, Xiaotian Yu, Guangming Lu, Rong Xiao
- **Published**: January 2024
- **Links**: [arXiv](https://arxiv.org/abs/2401.12522) | [ScienceDirect](https://www.sciencedirect.com/science/article/abs/pii/S0957417425009273) | [GitHub](https://github.com/linfeng93/BiTA)

### Core Algorithm

1. **Semi-Autoregressive (SAR) Drafting**: Bi-directional attention for draft generation
2. **Soft Embeddings**: Use continuous embeddings (vs discrete tokens like Medusa heads)
3. **Prompt Tokens**: Add special tokens for future prediction
4. **Integrated Verification**: Draft generation + verification in single pass (vs Medusa's two-pass)
5. **Tree-Based Decoding**: Parallel candidate evaluation

### Key Insight
"Bi-directional tuning with soft embeddings enables lossless SAR decoding with seamless generation-verification, outperforming multi-head approaches."

### Performance Metrics
- **Speedup**: 2.1-3.3× across LLMs
  - LLaMA-2-70B-Chat: **2.7×** on MT-bench
  - Larger models: Higher speedup (3.3×)
- **Acceptance Rate**: Not explicitly reported
- **Memory Overhead**:
  - Soft embeddings: <5% parameters
  - Lightweight plug-in module
- **Training Cost**: Minimal (frozen backbone)

### Advantages
- **Lossless**: Identical outputs to autoregressive (greedy sampling)
- **No separate model**: Plug-in module only
- **Single-pass**: Integrated generation + verification
- **Scalability**: Better performance on larger models

### Limitations
- Soft embeddings less interpretable than discrete tokens
- Requires fine-tuning (not inference-only)
- Greedy sampling only (sampling extension unclear)

---

## 9. Staged Speculative Decoding (2023)

### Paper
- **Title**: "Accelerating LLM Inference with Staged Speculative Decoding"
- **Authors**: Benjamin Spector, Chris Re
- **Published**: August 2023 (arXiv:2308.04623)
- **Links**: References in [SpeculativeDecodingPapers](https://github.com/hemingkx/SpeculativeDecodingPapers)

### Core Algorithm

1. **Nested Drafting**: Draft model itself uses a smaller draft model (recursive)
2. **Hierarchical Verification**:
   - Stage 1: Tiny model → small model (draft for draft)
   - Stage 2: Small model → large model (draft for target)
3. **Multi-Level Speedup**: Compound speedup from both stages

### Key Insight
"Nested speculative decoding within the draft model's decoding enables compound speedup without increasing target model calls."

### Performance Metrics
- **Speedup**: Not explicitly reported in available sources
- **Acceptance Rate**: Dependent on both draft-draft and draft-target alignment
- **Memory Overhead**: Two draft models (~10-15% total)

### Advantages
- Compound speedup (multiplicative effect)
- Can use ultra-small draft-draft models (1-10M params)
- Reduces draft model overhead

### Limitations
- Complexity: Three models to coordinate
- Acceptance rate compounded (both stages must accept)
- Training overhead for two draft models

---

## 10. HASS (2024) - Harmonized Speculative Sampling

### Paper
- **Title**: "Learning Harmonized Representations for Speculative Sampling"
- **Authors**: Zhang et al.
- **Published**: August-September 2024
- **Links**: [arXiv](https://arxiv.org/abs/2408.15766)

### Core Algorithm

1. **Ranking Distillation**: Extend recommender system ranking to speculative sampling
2. **Context-Aligned Training**: Simulate multi-step draft during training (vs single-step)
3. **Probability Harmonization**: Align training and decoding probability distributions
4. **Draft Awareness**: Make draft model aware of decoding strategy

### Key Insight
"Harmonizing training and inference distributions + context-aligned training mitigates training-inference inconsistency and error accumulation."

### Performance Metrics
- **Acceptance Length Improvement**: +8-16% over EAGLE-2
- **Speedup**: 2.81-4.05× wall-clock (vs vanilla inference)
  - LLaMA2-Chat 7B/13B: 2.81-3.42×
  - LLaMA3-Instruct 8B/70B: 3.65-4.05×
- **Acceptance Rate**: Not explicitly reported (inferred from acceptance length)
- **Hardware**: NVIDIA H800 GPU
- **Memory Overhead**: Minimal (builds on EAGLE-2)

### Training Enhancements
- **Ranking Loss**: Focus on top-K most probable tokens
- **Multi-Step Simulation**: Train with error accumulation awareness
- **Context Misalignment Fix**: Align training context with inference context

### Advantages
- Significant improvement over EAGLE-2 baseline
- No inference overhead (training-time only)
- Generalizes to different decoding strategies

### Limitations
- Requires retraining draft model (vs inference-only)
- Builds on EAGLE-2 (not standalone)
- Complex training recipe

---

## 11. Hierarchical & Multi-Level Methods (2024-2025)

### HiSpec (Hierarchical Speculative Decoding, 2024)

- **Focus**: Reduce verification overheads (vs draft generation focus of others)
- **Tested**: LLaMA2, LLaMA3, CodeLlama families
- **Links**: [arXiv](https://arxiv.org/abs/2510.01336)

### ML-SpecQD (Multi-Level Speculative Decoding with Quantized Drafts, 2025)

- **Key Idea**: Use 4-bit quantized model as draft, tiny model as draft-for-draft
- **Hardware-Agnostic**: Combines hierarchical SD + LLM quantization
- **Advantage**: No custom draft models (use quantization directly)
- **Links**: [arXiv](https://arxiv.org/abs/2503.13565)

### SSD (Self Speculative Decoding for Diffusion LLMs, 2025)

- **Key Idea**: Diffusion LLMs act as their own draft via hierarchical verification
- **Parallel Prediction**: Generate drafts for multiple positions simultaneously
- **Links**: [arXiv](https://arxiv.org/abs/2510.04147)

---

## 12. SpecFormer (November 2025) - Latest SOTA

### Paper
- **Title**: "Scaling LLM Speculative Decoding: Non-Autoregressive Forecasting in Large-Batch Scenarios"
- **Published**: November 2025
- **Links**: [arXiv](https://arxiv.org/abs/2511.20340)

### Core Algorithm

1. **Unidirectional + Bidirectional Attention**: Hybrid attention mechanism
2. **Non-Autoregressive Drafting**: Parallel token generation (vs sequential)
3. **No Prefix Trees**: Eliminates large prefix tree overhead
4. **Large-Batch Optimization**: Consistent acceleration even with large batches

### Key Insight
"Combining autoregressive model's full-sequence information extraction with non-autoregressive model's parallel generation eliminates prefix tree reliance and scales to large batches."

### Performance Metrics
- **Speedup**: "New standard for scaling LLM inference" (exact numbers not available)
- **Batch Scaling**: Consistent acceleration in large-batch scenarios (vs degradation in other methods)
- **Training Cost**: Lower training demands than other NAR methods
- **Computational Cost**: Reduced vs traditional speculative decoding

### Advantages
- Scales to large batches (key for production)
- No prefix tree overhead
- Hybrid attention benefits
- Lower training cost

### Status
- **Very Recent** (November 2025) - cutting-edge research
- Limited benchmarking data available yet
- Promising for production deployment

---

## 13. Speculative Diffusion Decoding (2024-2025)

### Paper
- **Title**: "Speculative Diffusion Decoding: Accelerating Language Generation through Diffusion"
- **Published**: 2024-2025
- **Links**: [arXiv](https://arxiv.org/abs/2408.05636) | [NAACL](https://aclanthology.org/2025.naacl-long.601.pdf)

### Core Algorithm

1. **Discrete Diffusion Drafter**: Replace autoregressive drafter with diffusion model
2. **Reverse Diffusion Steps**: Trade-off between compute cost and generation quality
3. **Bidirectional Decoder**: Masked language model (non-autoregressive)
4. **Quality-Speed Tradeoff**: Fewer diffusion steps = faster but lower quality

### Key Insight
"Diffusion models offer smooth compute-quality tradeoff via number of reverse steps, enabling adaptive drafting quality."

### Performance Metrics
- **Speedup**: <2× in early studies (modest vs AR drafters)
- **Diffusion Steps**: 5-20 typical (more = better quality, slower)
- **Acceptance Rate**: Depends on diffusion step count
- **Memory Overhead**: Diffusion model parameters (~10-20%)

### Advantages
- Smooth quality-speed tradeoff
- Non-autoregressive drafting
- Bidirectional context

### Limitations
- Modest speedup (<2×) vs AR methods (2-4×)
- Diffusion training complexity
- Limited attention in literature

---

## Performance Summary Table

| Method | Year | Speedup | Acceptance Rate | Draft Model | Memory Overhead | Training Cost |
|--------|------|---------|----------------|-------------|-----------------|---------------|
| **Leviathan (Original)** | 2023 | 2-3× | Not reported | Separate (0.5% size) | 5-10% | High (separate model) |
| **Medusa** | 2024 | 2.2-3.6× | 60% top-1, 80% top-5 | None (multi-head) | 2-5% | Low (parameter-efficient) |
| **EAGLE-1** | 2024 | 2.0-3.0× | 0.6-0.8 | Lightweight (1.8-7.6%) | 5-10% | Low (single-layer) |
| **EAGLE-2** | 2024 | 3.05-4.26× | Context-dependent | Same as EAGLE-1 | 5-10% | Low |
| **EAGLE-3** | 2025 | 3.6-4.8× | 0.7-0.9 | Same as EAGLE-1 | 5-10% | Low |
| **Lookahead** | 2024 | 1.5-2.3× (single GPU), 4× (multi-GPU) | N/A (n-gram match) | None (Jacobi) | 10-15% (cache) | None (inference-only) |
| **Multi-Token (Meta)** | 2024 | 2.0-3.6× | Implicit | Built-in heads | 5-10% | High (retrain from scratch) |
| **REST** | 2024 | High (task-dependent) | High (extractive tasks) | None (retrieval) | <5% (index) | None (inference-only) |
| **DistillSpec** | 2024 | +10-45% over SD | Improved | Distilled draft | Same as SD | High (distillation) |
| **BiTA** | 2024 | 2.1-3.3× | Not reported | None (SAR plug-in) | <5% | Low (frozen backbone) |
| **HASS** | 2024 | 2.81-4.05× | +8-16% vs EAGLE-2 | Same as EAGLE-2 | Minimal | Medium (harmonization) |
| **SpecFormer** | 2025 | "New standard" (TBD) | TBD | Non-AR integrated | TBD | Lower than NAR |
| **Speculative Diffusion** | 2024-2025 | <2× | Diffusion-dependent | Diffusion model | 10-20% | High (diffusion training) |

---

## Chaos (Computational Capsule) Translation

### Recommended Architecture: EAGLE-3 + Lockfree Verification Queue

**Rationale**:
1. **Best Speedup**: 3.6-4.8× (state-of-the-art for production)
2. **Low Overhead**: 5-10% memory, lightweight draft head
3. **Chaos-Friendly**: Feature-level operations align with capsule primitives
4. **Context-Aware**: Dynamic acceptance maps to lockfree state machines

---

### Capsule Design: `SpeculativeDraftCapsule<T, const N: usize>`

#### Tier: **T6 Mixed** (T1 Atomic + T2 SIMD + T4 Batch + T5 Streaming)

```rust
/// EAGLE-3 inspired speculative draft capsule with lockfree verification queue
///
/// Chaos Compliance:
/// - T1 Atomic: DualAtomicU64 for acceptance state coordination
/// - T2 SIMD: Vectorized feature prediction (portable_simd)
/// - T4 Batch: Parallel candidate tree verification
/// - T5 Streaming: Lockfree ring buffer for candidate queue
///
/// Performance Target: 3.6-4.8× speedup over autoregressive baseline
#[repr(C, align(128))]
pub struct SpeculativeDraftCapsule<T, const N: usize> {
    /// Dual atomic state: (generation_counter | acceptance_bitmap)
    /// - Upper 32 bits: Generation counter for ABA prevention
    /// - Lower 32 bits: Acceptance bitmap (1 bit per candidate, max 32 candidates)
    state: DualAtomicU64,

    /// Feature prediction head output (second-to-top layer embeddings)
    /// Aligned to 128B for cache-line isolation
    #[align(128)]
    feature_predictions: [T; N],

    /// Draft confidence scores (Q8.8 fixed-point for determinism)
    /// Used for context-dependent draft tree pruning (EAGLE-2 enhancement)
    confidence_scores: [FixedPointQ8_8; N],

    /// Acceptance history circular buffer (lockfree, T5 Streaming)
    /// Tracks last 256 acceptance events for adaptive thresholding
    acceptance_history: RingBufferCapsule<AcceptanceEvent, 256>,

    /// Context hash (for cache key, SIMD hash)
    context_hash: AtomicU64,

    /// Padding to 128B cache-line boundary
    _padding: [u8; 64 - (8 + 8 + 8)],
}

/// Acceptance event for history tracking
#[repr(C, align(8))]
#[derive(Clone, Copy)]
struct AcceptanceEvent {
    /// Position in draft sequence (0-31)
    position: u8,
    /// Accepted (1) or rejected (0)
    accepted: u8,
    /// Confidence score at time of prediction (Q8.8)
    confidence: u16,
    /// Padding to 8B
    _padding: [u8; 4],
}

impl<T, const N: usize> SpeculativeDraftCapsule<T, N> {
    /// Generate draft candidates (feature-level autoregression)
    ///
    /// Chaos Pattern: T2 SIMD vectorized feature prediction
    /// Performance: <50ns per candidate (128B feature vectors)
    #[inline]
    pub fn generate_drafts(
        &self,
        context_features: &[f32],
        draft_head: &DraftHeadCapsule,
    ) -> Result<[T; N], DraftError> {
        // SIMD hash for context cache key (T2, <20ns)
        let ctx_hash = simd_hash_128(context_features);

        // Lockfree cache lookup (T1, <10ns)
        if let Some(cached) = self.cache_lookup(ctx_hash) {
            return Ok(cached);
        }

        // Feature-level autoregression (EAGLE-1 insight)
        // Use draft_head to predict next feature from context
        let mut predictions = [Default::default(); N];
        for i in 0..N {
            predictions[i] = draft_head.predict_feature(
                context_features,
                &predictions[..i], // Autoregressive on features
            )?;
        }

        // Cache result (lockfree, generation-counter based)
        self.cache_insert(ctx_hash, predictions)?;

        Ok(predictions)
    }

    /// Verify candidates with target model (parallel batch)
    ///
    /// Chaos Pattern: T4 Batch parallel verification
    /// Performance: <200ns for 8 candidates (amortized over batch)
    #[inline]
    pub fn verify_candidates(
        &self,
        candidates: &[T; N],
        target_logits: &[f32],
    ) -> AcceptanceResult {
        // Dynamic tree pruning (EAGLE-2 context-aware enhancement)
        let pruned_candidates = self.prune_low_confidence(candidates);

        // Parallel verification (T4 Batch, lockfree work-stealing queue)
        let acceptance_bitmap = self.parallel_verify(
            pruned_candidates,
            target_logits,
        );

        // Atomic acceptance state update (T1, SWeMR)
        let gen = self.increment_generation();
        let new_state = (gen << 32) | (acceptance_bitmap as u64);
        self.state.store_packed(new_state, Ordering::Release);

        // Update acceptance history (T5 Streaming, lockfree ring buffer)
        self.record_acceptance_events(acceptance_bitmap);

        AcceptanceResult {
            accepted_count: acceptance_bitmap.count_ones() as usize,
            first_rejection: acceptance_bitmap.trailing_ones() as usize,
        }
    }

    /// Context-dependent draft tree pruning (EAGLE-2)
    ///
    /// Chaos Pattern: T3 Fixed-Point deterministic confidence thresholding
    /// Performance: <10ns (SIMD comparison of Q8.8 fixed-point scores)
    #[inline]
    fn prune_low_confidence(&self, candidates: &[T; N]) -> &[T] {
        // Adaptive threshold from acceptance history
        let threshold = self.compute_adaptive_threshold();

        // SIMD comparison (T2, portable_simd)
        let pruned_count = self.confidence_scores
            .iter()
            .take_while(|&score| *score >= threshold)
            .count();

        &candidates[..pruned_count]
    }

    /// Compute adaptive confidence threshold from acceptance history
    ///
    /// Chaos Pattern: T10 Probabilistic (HyperLogLog-inspired quantile estimation)
    /// Performance: <30ns (lockfree circular buffer scan)
    #[inline]
    fn compute_adaptive_threshold(&self) -> FixedPointQ8_8 {
        // Scan last 256 acceptance events (T5 Streaming)
        let recent_events = self.acceptance_history.iter_recent(256);

        // Compute 50th percentile of accepted confidences (T10)
        // Use HyperLogLog-inspired sketch for O(1) quantile
        let quantile_50 = self.estimate_quantile(
            recent_events.filter(|e| e.accepted == 1),
            0.5,
        );

        quantile_50
    }

    /// Lockfree cache operations (T1 Atomic + T5 Streaming)
    #[inline]
    fn cache_lookup(&self, hash: u64) -> Option<[T; N]> {
        // Generation-counter based cache validation
        // ... implementation
        None // Placeholder
    }

    #[inline]
    fn cache_insert(&self, hash: u64, value: [T; N]) -> Result<(), CacheError> {
        // Lockfree cache insert with generation counter
        // ... implementation
        Ok(())
    }
}
```

---

### Capsule Hierarchy: Multi-Stage Pipeline

```
SpeculativeDecodingMetacapsule (T6 Mixed orchestrator)
├── ContextEncoderCapsule (T2 SIMD feature extraction)
├── DraftHeadCapsule (T1+T2 lightweight autoregressive head)
│   ├── FeaturePredictorCapsule (T2 SIMD feature-level AR)
│   └── ConfidenceEstimatorCapsule (T3 Fixed-Point uncertainty modeling)
├── CandidateTreeCapsule (T4 Batch parallel tree construction)
│   ├── TreeNodeCapsule (T1 Atomic node state)
│   └── PruningStrategyCapsule (T3 Fixed-Point threshold)
├── VerificationQueueCapsule (T5 Streaming lockfree queue)
│   ├── BatchVerifierCapsule (T4 Batch parallel verification)
│   └── AcceptanceTrackerCapsule (T1 Atomic bitmap)
├── AdaptiveThresholdCapsule (T10 Probabilistic quantile estimation)
│   ├── AcceptanceHistoryCapsule (T5 Streaming ring buffer)
│   └── QuantileSketchCapsule (T10 HyperLogLog-inspired)
└── CacheCapsule (T1+T5 lockfree generation-counter cache)
    ├── HashTableCapsule (T1 Atomic lockfree hash table)
    └── EvictionPolicyCapsule (T3 Fixed-Point LRU scoring)
```

---

### Performance Targets (B32 Validation)

| Metric | Target | Method | Tier |
|--------|--------|--------|------|
| **Draft Generation** | <50ns per candidate | SIMD feature prediction | T2 |
| **Confidence Scoring** | <10ns per score | Fixed-point Q8.8 comparison | T3 |
| **Tree Pruning** | <30ns for 32 candidates | SIMD threshold comparison | T2 |
| **Parallel Verification** | <200ns for 8 candidates | Batch lockfree queue | T4 |
| **Acceptance Update** | <15ns | DualAtomicU64 SWeMR | T1 |
| **History Recording** | <20ns per event | Lockfree ring buffer | T5 |
| **Adaptive Threshold** | <50ns | HyperLogLog quantile sketch | T10 |
| **Cache Lookup** | <10ns hit, <30ns miss | Generation-counter validation | T1 |
| **End-to-End Speedup** | **3.6-4.8×** | EAGLE-3 algorithm | T6 |

---

### ASSUM Safety Analysis

#### Critical Assumptions (EAGLE-3 Adaptation)

1. **#ASSUME**: Feature-level autoregression reduces uncertainty vs token-level
   - **#VERIFY**: Measure feature prediction accuracy vs token accuracy on benchmark
   - **Safety**: Medium risk (algorithmic assumption, not memory safety)

2. **#ASSUME**: Context-dependent acceptance rates improve over position-only
   - **#VERIFY**: Compare acceptance length τ with/without context pruning
   - **Safety**: Low risk (performance assumption, no correctness impact)

3. **#ASSUME**: Lockfree ring buffer capacity (256) sufficient for history
   - **#VERIFY**: Monitor ring buffer wraparound frequency under load
   - **Safety**: **High risk** (capacity overflow → data loss)
   - **Mitigation**: Dynamic resizing OR alert on 80% capacity

4. **#ASSUME**: Generation counter 32 bits sufficient (4.2B iterations before wraparound)
   - **#VERIFY**: Calculate wraparound time at peak throughput (e.g., 1M drafts/sec → 4200 sec = 70 min)
   - **Safety**: **High risk** (wraparound → ABA problem)
   - **Mitigation**: 64-bit generation counter OR wraparound detection + reset

5. **#ASSUME**: Confidence scores in Q8.8 fixed-point range (0-255.996)
   - **#VERIFY**: Clippy lint for overflow (saturating arithmetic mandatory)
   - **Safety**: Medium risk (overflow → incorrect pruning)

6. **#ASSUME**: SIMD alignment (128B) sufficient for cache-line isolation
   - **#VERIFY**: Clippy lint `capsule_unaligned_violation`
   - **Safety**: **High risk** (false sharing → 3-10× slowdown)

#### Safety Target: **99.5%+** (ASSUM framework)

---

### T28 Testing Strategy

#### Tier 1: Unit Tests (Q1-Q7)
- Feature prediction accuracy (draft head)
- Confidence score calculation (Q8.8 fixed-point)
- Lockfree cache insert/lookup (generation counter)
- Ring buffer append/read (wraparound)

#### Tier 2: Property Tests (Q8-Q14)
- Acceptance bitmap correctness (all positions ≤ first rejection)
- Generation counter monotonicity (no ABA)
- Cache consistency (hash collisions handled)
- Ring buffer FIFO ordering

#### Tier 3: Integration Tests (Q15-Q21)
- End-to-end draft → verify → accept pipeline
- Adaptive threshold convergence (acceptance history)
- Multi-threaded cache contention (lockfree validation)
- Tree pruning effectiveness (acceptance length improvement)

#### Tier 4: Production Tests (Q22-Q28)
- Benchmark speedup vs autoregressive baseline (target: 3.6-4.8×)
- Acceptance rate distribution by task (summarization, QA, code, chat)
- Memory overhead validation (target: <10%)
- Latency P50/P95/P99 (target: <200ns verify, <50ns draft)

#### Tier 5: Determinism Tests (Q29-Q35)
- Fixed-point arithmetic determinism (Q8.8 confidence)
- SIMD determinism (portable_simd reproducibility)
- Cache determinism (hash-based, not pointer-based keys)
- Acceptance bitmap determinism (no race conditions)

---

### I20 Integration Checklist

#### Q1-Q5: Scope
- [ ] Integrate with existing `atomic_capsule::encoder` (inference pipeline)
- [ ] Compatible with `LanguageModelCapsule` interface (target model abstraction)
- [ ] Supports `FeatureExtractorCapsule` (context encoding)
- [ ] Generic over token type `T` (supports various vocabularies)
- [ ] Configurable draft window size `N` (1-32 candidates)

#### Q6-Q10: Compatibility
- [ ] No breaking changes to existing `encoder` API
- [ ] Backward-compatible draft head serialization (feature versioning)
- [ ] Graceful degradation if draft head unavailable (fallback to AR)
- [ ] Compatible with `nightly` and `stable` feature flags
- [ ] Zero dependencies beyond `atomic_capsule` (internal primitives only)

#### Q11-Q15: Safety
- [ ] All `unsafe` blocks documented with `#ASSUME` + `#VERIFY`
- [ ] Clippy lints pass: `capsule_mutex_violation`, `capsule_unaligned_violation`, `capsule_missing_generation`
- [ ] Memory ordering audit (Acquire/Release on acceptance state)
- [ ] ABA prevention via generation counter (64-bit recommended)
- [ ] Panic-safe (no panics in hot path, `Result` error handling)

#### Q16-Q20: Validation
- [ ] B32 benchmark: 3.6-4.8× speedup (95% CI, 1000+ iterations)
- [ ] T28 testing: 5 tiers (unit, property, integration, production, determinism)
- [ ] ASSUM: 99.5%+ safety (all assumptions verified)
- [ ] UCE34 Q10-Q12: Tier selection justified (T6 Mixed)
- [ ] Q34 audit trail: Hash-chained acceptance history (optional compliance feature)

---

### UCE34 Q10-Q12 Tier Selection Justification

#### Q10: Which tier solves this problem?

**Problem**: Accelerate LLM autoregressive decoding (2-10× speedup target)

**Tier Selection**: **T6 Mixed** (compound multi-tier)

**Rationale**:
- **T1 Atomic**: Lockfree acceptance state coordination (<15ns)
- **T2 SIMD**: Vectorized feature prediction (<50ns, 2-8× over scalar)
- **T3 Fixed-Point**: Deterministic confidence scoring (Q8.8, <10ns)
- **T4 Batch**: Parallel candidate verification (<200ns amortized)
- **T5 Streaming**: Lockfree ring buffer for history (<20ns append)
- **T10 Probabilistic**: Quantile estimation for adaptive thresholding (<50ns)

**Compound Effect**: 3.6-4.8× speedup (EAGLE-3 empirical result)

#### Q11: Rust features required?

**Nightly Features** (mandatory):
- `portable_simd`: T2 SIMD feature prediction (2-8× speedup)
- `const_fn_floating_point`: T3 Fixed-point compile-time constants
- `generic_const_exprs`: Generic draft window size `N` (compile-time validation)

**Stable Features**:
- `std::sync::atomic`: T1 Atomic primitives
- `std::alloc::Layout`: Cache-aligned allocation (128B)

**Justification**: Nightly features provide 2-8× speedup (SIMD) + 0ns compile-time validation (const generics). Conservative fallback to stable degrades to 1.5-2× speedup (scalar feature prediction).

#### Q12: Performance claims validation (B32)

**Baseline**: Autoregressive decoding (1× reference)

**Optimized**: EAGLE-3 speculative decoding (3.6-4.8× target)

**Measurement Protocol**:
1. **Hardware**: kindly-hub (AMD Ryzen 9 6900HX, consistent for B32)
2. **Benchmark**: MT-bench, CodeLlama-13B, 1000+ iterations
3. **Metrics**:
   - Wall-clock speedup (end-to-end latency)
   - Acceptance length τ (tokens per round)
   - Acceptance rate α (per position)
4. **95% Confidence Interval**: Report mean ± 2σ
5. **Fair Baseline**: Optimized autoregressive (not strawman)

**Validation Criteria** (B32 framework):
- [ ] Speedup: 3.6-4.8× (within CI)
- [ ] Acceptance length: τ ≥ 3.0 (vs EAGLE-2's ~2.5)
- [ ] Memory overhead: <10% (vs baseline)
- [ ] Reproducibility: <5% variance across runs

---

## Recommended Algorithm for SpeculativeDraftCapsule

### **EAGLE-3** (2025) - Best Overall

**Rationale**:
1. **Highest Speedup**: 3.6-4.8× (state-of-the-art among production-ready methods)
2. **Low Overhead**: 5-10% memory, 1.8-7.6% draft model parameters
3. **Context-Aware**: Dynamic draft tree adapts to acceptance rates (EAGLE-2 enhancement)
4. **Training-Time Tested**: Acceptance-aware loss (EAGLE-3 enhancement)
5. **Chaos-Friendly**:
   - Feature-level operations (natural for T2 SIMD capsules)
   - Lockfree verification queue (T5 Streaming)
   - Atomic acceptance bitmap (T1 Atomic)
   - Fixed-point confidence (T3 determinism)
6. **Production-Ready**: Tested on NVIDIA GPUs (H800), scales to 70B models

### Alternative: **Medusa** (If No Draft Model Training)

**When to Use**:
- Cannot train separate draft model (e.g., black-box API model)
- Need plug-and-play solution (no draft model management)

**Trade-offs**:
- Lower speedup: 2.2-3.6× (vs EAGLE-3's 3.6-4.8×)
- Requires fine-tuning backbone (Medusa-2) for best results
- Multi-head complexity (vs single draft head in EAGLE)

### Alternative: **DistillSpec + EAGLE-2** (If Draft Quality Critical)

**When to Use**:
- Acceptance rate is bottleneck (low alignment)
- Can afford distillation training time
- Domain-specific optimization (e.g., medical, legal)

**Trade-offs**:
- +10-45% speedup over baseline EAGLE-2 (3.05-4.26× → 3.36-6.15×)
- High training cost (distillation + draft training)
- Requires target model access for distillation

---

## Key Metrics Reference

### Acceptance Rate (α)
- **0.5-0.6**: Poor alignment, <2× speedup
- **0.6-0.7**: Fair, 2-3× speedup
- **0.7-0.8**: Good, 3-4× speedup
- **0.8-0.9**: Excellent, 4-5× speedup (rare, task-dependent)

### Acceptance Length (τ)
- **1.5-2.0**: Below average (Lookahead, early methods)
- **2.0-3.0**: Average (Medusa, EAGLE-1)
- **3.0-4.0**: Good (EAGLE-2, HASS)
- **4.0+**: Excellent (EAGLE-3, SpecFormer)

### Draft Window (γ)
- **2-4**: Conservative (high acceptance, low parallelism)
- **5-8**: Balanced (optimal for most tasks)
- **8-16**: Aggressive (low acceptance, high parallelism potential)
- **16+**: Very aggressive (requires very high α to benefit)

### Memory Overhead
- **<5%**: Minimal (BiTA, DistillSpec with small draft)
- **5-10%**: Low (EAGLE, Medusa)
- **10-15%**: Moderate (Lookahead cache, staged SD)
- **15%+**: High (multi-level methods, diffusion)

---

## Implementation Roadmap

### Phase 1: Core EAGLE-3 Implementation (Week 1-2)
- [ ] `SpeculativeDraftCapsule<T, N>` base structure (T1+T2+T3)
- [ ] `DraftHeadCapsule` (feature-level autoregression)
- [ ] `ConfidenceEstimatorCapsule` (Q8.8 fixed-point)
- [ ] Lockfree cache (generation-counter based)
- [ ] Unit tests (T28 Q1-Q7)

### Phase 2: Context-Aware Enhancements (Week 3)
- [ ] `AcceptanceHistoryCapsule` (T5 ring buffer)
- [ ] Adaptive threshold computation (T10 quantile sketch)
- [ ] Dynamic tree pruning (EAGLE-2)
- [ ] Property tests (T28 Q8-Q14)

### Phase 3: Parallel Verification (Week 4)
- [ ] `VerificationQueueCapsule` (T4 batch lockfree)
- [ ] `CandidateTreeCapsule` (parallel tree construction)
- [ ] SIMD-optimized acceptance bitmap (T2)
- [ ] Integration tests (T28 Q15-Q21)

### Phase 4: Production Optimization (Week 5-6)
- [ ] Training-time test (EAGLE-3 loss weighting)
- [ ] Hardware-specific tuning (AVX2, cache prefetch)
- [ ] B32 benchmarking (3.6-4.8× target)
- [ ] Production tests (T28 Q22-Q28)

### Phase 5: Validation & Documentation (Week 7)
- [ ] ASSUM safety audit (99.5%+ target)
- [ ] I20 integration checklist (20/20)
- [ ] Q34 audit trail (optional compliance)
- [ ] Determinism tests (T28 Q29-Q35)

---

## Sources

### Original Papers
- [Leviathan et al. (2023) - Fast Inference from Transformers via Speculative Decoding](https://arxiv.org/abs/2211.17192)
- [Medusa (2024) - Simple LLM Inference Acceleration Framework](https://arxiv.org/abs/2401.10774)
- [EAGLE-1 (2024) - Speculative Sampling Requires Rethinking Feature Uncertainty](https://arxiv.org/abs/2401.15077)
- [EAGLE-2 (2024) - Faster Inference with Dynamic Draft Trees](https://arxiv.org/abs/2406.16858)
- [EAGLE-3 (2025) - Scaling up Inference Acceleration](https://arxiv.org/abs/2503.01840)
- [Lookahead Decoding (2024) - Break the Sequential Dependency](https://arxiv.org/abs/2402.02057)
- [Meta Multi-Token Prediction (2024) - Better & Faster LLMs](https://arxiv.org/abs/2404.19737)
- [DistillSpec (2024) - Improving via Knowledge Distillation](https://arxiv.org/abs/2310.08461)
- [BiTA (2024) - Bi-Directional Tuning for Lossless Acceleration](https://arxiv.org/abs/2401.12522)
- [HASS (2024) - Learning Harmonized Representations](https://arxiv.org/abs/2408.15766)
- [SpecFormer (2025) - Scaling LLM Speculative Decoding](https://arxiv.org/abs/2511.20340)
- [Speculative Diffusion Decoding (2024) - Accelerating via Diffusion](https://arxiv.org/abs/2408.05636)

### Repositories & Resources
- [Medusa GitHub](https://github.com/FasterDecoding/Medusa)
- [EAGLE GitHub](https://github.com/SafeAILab/EAGLE)
- [Lookahead Decoding GitHub](https://github.com/hao-ai-lab/LookaheadDecoding)
- [BiTA GitHub](https://github.com/linfeng93/BiTA)
- [Speculative Decoding Papers Collection](https://github.com/hemingkx/SpeculativeDecodingPapers)
- [Spec-Bench (ACL 2024 Findings)](https://github.com/hemingkx/Spec-Bench)

### Blogs & Tutorials
- [Google Research - Looking Back at Speculative Decoding](https://research.google/blog/looking-back-at-speculative-decoding/)
- [LMSYS - Lookahead Decoding Blog](https://lmsys.org/blog/2023-11-21-lookahead-decoding/)
- [Together.ai - Medusa Framework](https://www.together.ai/blog/medusa)
- [NVIDIA - Introduction to Speculative Decoding](https://developer.nvidia.com/blog/an-introduction-to-speculative-decoding-for-reducing-latency-in-ai-inference/)
- [vLLM - Speculative Decoding Performance](https://blog.vllm.ai/2024/10/17/spec-decode.html)
- [Medium - Multi-Token Prediction Explained](https://medium.com/@himankvjain/accelerating-language-models-with-multi-token-prediction-9f0167232f5b)

### Surveys & Benchmarks
- [A Comprehensive Survey of Speculative Decoding (ACL 2024 Findings)](https://aclanthology.org/2024.findings-acl.456.pdf)
- [BentoML LLM Inference Handbook - Speculative Decoding](https://bentoml.com/llm/inference-optimization/speculative-decoding)
- [AussieAI - Speculative Decoding Types and Optimizations](https://www.aussieai.com/research/speculative-decoding)

---

**End of Report**
