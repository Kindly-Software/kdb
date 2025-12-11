//! # Performance Profiling Capsules for Capsule OS
//!
//! **T1/T4/T5 Multi-tier CPU profiling with flamegraph generation.**
//!
//! ## Overview
//!
//! This module provides production-grade CPU profiling tools following the
//! Computational Capsule architecture. All implementations are 100% lockfree
//! with <10ns sampling overhead.
//!
//! ## Capsules
//!
//! | Capsule | Tier | Size | Performance | Purpose |
//! |---------|------|------|-------------|---------|
//! | [`ProfilerCapsule`] | T5 Streaming | 1KB | <10ns sample | CPU sampling profiler |
//! | [`FlameGraphCapsule`] | T4 Batch | 2KB | <1ms generate | Flamegraph generation |
//! | [`PerfCounterCapsule`] | T1 Atomic | 256B | <5ns read | Hardware counter access |
//!
//! ## SOTA Research Integration (2024-2025)
//!
//! ### CPU Sampling (Brendan Gregg's Methodology)
//! - 99 Hz sampling to avoid lock-step artifacts
//! - Collapsed stack format for flamegraph compatibility
//! - Differential flame graphs for before/after comparison
//! - Source: [CPU Flame Graphs](https://www.brendangregg.com/FlameGraphs/cpuflamegraphs.html)
//!
//! ### Linux perf Integration
//! - perf_event_open() for hardware counter access
//! - PEBS (Precise Event-Based Sampling) support
//! - Per-CPU event buffers with mmap() ring buffers
//! - Source: [Linux perf](https://perf.wiki.kernel.org/)
//!
//! ### eBPF Profiling (Zero-Overhead)
//! - BPF_PROG_TYPE_PERF_EVENT for stack capture
//! - <1% overhead via eBPF uprobes
//! - Production-safe continuous profiling
//! - Source: [GPUprobe](https://dev.to/ethgraham/snooping-on-your-gpu-using-ebpf-to-build-zero-instrumentation-cuda-monitoring-2hh1)
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────┐
//! │                    Profiling Architecture                           │
//! ├─────────────────────────────────────────────────────────────────────┤
//! │                                                                     │
//! │  ┌─────────────┐    ┌──────────────┐    ┌────────────────┐         │
//! │  │ PerfCounter │───>│   Profiler   │───>│  FlameGraph    │         │
//! │  │ Capsule     │    │   Capsule    │    │  Capsule       │         │
//! │  │ (T1, 256B)  │    │ (T5, 1KB)    │    │ (T4, 2KB)      │         │
//! │  │ <5ns read   │    │ <10ns sample │    │ <1ms generate  │         │
//! │  └─────────────┘    └──────────────┘    └────────────────┘         │
//! │        │                   │                    │                   │
//! │        v                   v                    v                   │
//! │  ┌─────────────┐    ┌──────────────┐    ┌────────────────┐         │
//! │  │ Hardware    │    │ Stack Ring   │    │ Collapsed      │         │
//! │  │ Counters    │    │ Buffer       │    │ Stack Format   │         │
//! │  │ (PMU/RAPL)  │    │ (16K frames) │    │ (SVG output)   │         │
//! │  └─────────────┘    └──────────────┘    └────────────────┘         │
//! │                                                                     │
//! └─────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Chaos Compliance
//!
//! - **100% Lockfree**: Zero mutex/RwLock in hot paths
//! - **Generation Counters**: TOCTOU prevention on all state transitions
//! - **Cache-Aligned**: 64B/128B/256B alignment prevents false sharing
//! - **Bounded Capacity**: Ring buffers with overflow detection
//!
//! ## Usage
//!
//! ```rust,ignore
//! use atomic_capsule::profiling::{ProfilerCapsule, FlameGraphCapsule, PerfCounterCapsule};
//!
//! // Create profiler with 99 Hz sampling rate
//! let profiler = ProfilerCapsule::new(99);
//!
//! // Start profiling
//! profiler.start();
//!
//! // ... run workload ...
//!
//! // Stop and generate flamegraph
//! profiler.stop();
//!
//! let mut flamegraph = FlameGraphCapsule::new();
//! flamegraph.process_samples(&profiler);
//!
//! // Export to SVG
//! let svg = flamegraph.generate_svg();
//! ```
//!
//! ## Feature Flags
//!
//! - `profiling`: Enable all profiling capsules (default: off)
//! - `profiling-perf`: Enable Linux perf integration (requires libc)
//! - `profiling-ebpf`: Enable eBPF profiling (requires CAP_BPF)
//!
//! ## ASSUM Safety Framework
//!
//! All capsules are annotated with comprehensive ASSUM tags documenting:
//! - Memory ordering assumptions
//! - Platform-specific behavior
//! - Performance guarantees
//! - Safety invariants

pub mod profiler;
pub mod flamegraph;
pub mod perf_counter;

#[cfg(test)]
mod tests;

// Re-export capsules for convenience
pub use profiler::{ProfilerCapsule, ProfilerState, StackFrame, SampleEntry};
pub use flamegraph::{FlameGraphCapsule, FlameNode, CollapsedStack};
pub use perf_counter::{PerfCounterCapsule, CounterType, CounterValue, PerfEvent};
