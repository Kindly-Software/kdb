//! Submission Pipeline Benchmarks
//!
//! Validates <500ns complete pipeline latency target.
//!
//! Based on B32 framework realistic expectations:
//! - Atomic operations: ~15ns hardware CAS latency
//! - 6 pipeline stages × ~50ns = 300ns (well under 500ns target)

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use kiang::{
    ContextCapsule, ContextState, ContextUpdate, FenceCapsule, GpuCircuitBreaker, GpuState,
    GpuStateCapsule, GucCtbState, GucReadyCapsule, SubmissionPipeline,
};
use std::sync::Arc;

fn setup_pipeline() -> SubmissionPipeline {
    let gpu_state = Arc::new(GpuStateCapsule::new());
    let breaker = Arc::new(GpuCircuitBreaker::new());
    let context = Arc::new(ContextCapsule::new());
    let guc_ctb = Arc::new(GucReadyCapsule::with_capacity(16 * 1024));
    let fence = Arc::new(FenceCapsule::new(0));

    // Set up all stages for success
    let state = GpuState {
        gpu_id: 0,
        frequency_mhz: 2100,
        power_mw: 45000,
        temp_celsius: 65,
        utilization: 50,
        valid: true,
    };
    gpu_state.publish(state);

    let ctx_update = ContextUpdate {
        context_id: 1,
        priority: 0,
        state: ContextState::Ready,
        last_fence: 0,
        batch_count: 0,
        error_count: 0,
        timestamp_us: 0,
        resource_gen: 0,
        mem_usage_mb: 0,
        submission_count: 0,
    };
    context.publish(ctx_update);

    let guc_state = GucCtbState {
        h2g_head: 0,
        h2g_tail: 1024,
        g2h_head: 0,
        g2h_tail: 0,
        capacity: 16 * 1024,
        pending_count: 0,
    };
    guc_ctb.publish(guc_state);

    fence.signal(10000, 1000);

    SubmissionPipeline::new(gpu_state, breaker, context, guc_ctb, fence)
}

fn bench_full_pipeline(c: &mut Criterion) {
    let pipeline = setup_pipeline();

    c.bench_function("submission_pipeline_full", |b| {
        let mut seqno = 0u32;
        b.iter(|| {
            seqno = seqno.wrapping_add(1);
            black_box(pipeline.submit_command(black_box(512), black_box(seqno), black_box(100)))
        });
    });
}

fn bench_fast_path(c: &mut Criterion) {
    let pipeline = setup_pipeline();

    c.bench_function("submission_pipeline_fast_path", |b| {
        b.iter(|| black_box(pipeline.can_submit_fast()));
    });
}

fn bench_pipeline_stages(c: &mut Criterion) {
    let mut group = c.benchmark_group("pipeline_stages");

    // Benchmark individual components
    let gpu_state = Arc::new(GpuStateCapsule::new());
    let state = GpuState {
        gpu_id: 0,
        frequency_mhz: 2100,
        power_mw: 45000,
        temp_celsius: 65,
        utilization: 50,
        valid: true,
    };
    gpu_state.publish(state);

    group.bench_function("gpu_state_read", |b| {
        b.iter(|| black_box(gpu_state.read()));
    });

    let breaker = Arc::new(GpuCircuitBreaker::new());
    group.bench_function("breaker_check", |b| {
        b.iter(|| black_box(breaker.should_allow_command()));
    });

    let context = Arc::new(ContextCapsule::new());
    let ctx_update = ContextUpdate {
        context_id: 1,
        priority: 0,
        state: ContextState::Ready,
        last_fence: 0,
        batch_count: 0,
        error_count: 0,
        timestamp_us: 0,
        resource_gen: 0,
        mem_usage_mb: 0,
        submission_count: 0,
    };
    context.publish(ctx_update);

    group.bench_function("context_can_submit", |b| {
        b.iter(|| black_box(context.can_submit()));
    });

    let guc_ctb = Arc::new(GucReadyCapsule::with_capacity(16 * 1024));
    let guc_state = GucCtbState {
        h2g_head: 0,
        h2g_tail: 1024,
        g2h_head: 0,
        g2h_tail: 0,
        capacity: 16 * 1024,
        pending_count: 0,
    };
    guc_ctb.publish(guc_state);

    group.bench_function("guc_ctb_has_space", |b| {
        b.iter(|| black_box(guc_ctb.has_space_for(512)));
    });

    let fence = Arc::new(FenceCapsule::new(0));
    fence.signal(10000, 1000);

    group.bench_function("fence_is_signaled", |b| {
        b.iter(|| black_box(fence.is_signaled(100)));
    });

    group.finish();
}

fn bench_submission_throughput(c: &mut Criterion) {
    let pipeline = setup_pipeline();

    let mut group = c.benchmark_group("submission_throughput");

    for batch_size in [1, 10, 100, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            batch_size,
            |b, &batch_size| {
                let mut seqno = 0u32;
                b.iter(|| {
                    for _ in 0..batch_size {
                        seqno = seqno.wrapping_add(1);
                        black_box(pipeline.submit_command(512, seqno, 100));
                    }
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_full_pipeline,
    bench_fast_path,
    bench_pipeline_stages,
    bench_submission_throughput
);
criterion_main!(benches);
