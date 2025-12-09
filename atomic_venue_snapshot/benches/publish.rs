#[cfg(not(feature = "std"))]
fn main() {}

#[cfg(feature = "std")]
mod bench_impl {
    use std::time::Duration;

    use atomic_venue_snapshot::{AvsWriter, WriterConfig, WriterInput};
    use criterion::{black_box, Criterion};

    pub fn bench_publish(c: &mut Criterion) {
        let mut group = c.benchmark_group("avs_writer");
        group.warm_up_time(Duration::from_millis(100));
        group.measurement_time(Duration::from_millis(400));

        let inputs = sample_inputs();
        group.bench_function("publish_batch", |b| {
            b.iter_with_large_drop(|| {
                let mut writer = AvsWriter::new(WriterConfig {
                    version: 1,
                    bp_per_tick: 1.1,
                    ..WriterConfig::default()
                });
                for input in inputs.iter().copied() {
                    let snapshot = writer.publish(input);
                    black_box(snapshot);
                }
                writer
            });
        });

        group.finish();
    }

    fn sample_inputs() -> Vec<WriterInput> {
        let mut inputs = Vec::with_capacity(128);
        let mut timestamp = 0u64;
        for i in 0..128 {
            timestamp += 40;
            let bid_px = 100_000 - (i as i64 % 5);
            let ask_px = bid_px + 50 + (i as i64 % 3) * 10;
            let bid_levels = [120 - i as u32 % 50, 90, 60];
            let ask_levels = [110 + i as u32 % 40, 80, 50];
            let marketable = if i % 16 == 0 { 80 } else { (i as u32) % 7 };
            let mut input = WriterInput::new(
                timestamp, bid_px, ask_px, bid_levels, ask_levels, marketable,
            );
            if i % 32 == 0 {
                input = input.with_marketable_volume(marketable + 120);
            }
            inputs.push(input);
        }
        inputs
    }
}

#[cfg(feature = "std")]
criterion::criterion_group!(benches, bench_impl::bench_publish);
#[cfg(feature = "std")]
criterion::criterion_main!(benches);
