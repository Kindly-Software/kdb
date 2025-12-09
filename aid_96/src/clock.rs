use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const EPOCH_2025_UNIX_SECS: i64 = 1_735_689_600;
const EPOCH_OFFSET_MS: i128 = (EPOCH_2025_UNIX_SECS as i128) * 1000;

pub struct MonotonicClock {
    offset_ms: AtomicI64,
    last_ms: AtomicU64,
}

impl MonotonicClock {
    pub const fn new() -> Self {
        Self {
            offset_ms: AtomicI64::new(0),
            last_ms: AtomicU64::new(0),
        }
    }

    pub fn now(&self) -> u64 {
        loop {
            let wall = unix_ms_since_2025_epoch();
            let offset = self.offset_ms.load(Ordering::Relaxed);
            let mut candidate = wall.saturating_add(offset);

            if candidate < 0 {
                // Bring the timeline back to zero and try again.
                let correction = -candidate;
                self.offset_ms.fetch_add(correction, Ordering::Relaxed);
                candidate = 0;
            }

            let candidate_u64 = candidate as u64;
            let previous = self.last_ms.load(Ordering::Relaxed);

            let final_now = if candidate_u64 <= previous {
                let delta = previous + 1 - candidate_u64;
                self.offset_ms.fetch_add(delta as i64, Ordering::Relaxed);
                previous + 1
            } else {
                candidate_u64
            };

            match self.last_ms.compare_exchange(
                previous,
                final_now,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => return final_now,
                Err(_) => std::hint::spin_loop(),
            }
        }
    }
}

fn unix_ms_since_2025_epoch() -> i64 {
    let now = SystemTime::now();
    let since_unix = match now.duration_since(UNIX_EPOCH) {
        Ok(dur) => dur,
        Err(err) => err.duration(),
    };
    let millis = duration_to_millis_i128(since_unix);
    let shifted = millis - EPOCH_OFFSET_MS;
    if shifted > i64::MAX as i128 {
        i64::MAX
    } else if shifted < i64::MIN as i128 {
        i64::MIN
    } else {
        shifted as i64
    }
}

fn duration_to_millis_i128(duration: Duration) -> i128 {
    let secs = duration.as_secs() as i128;
    let nanos = duration.subsec_nanos() as i128;
    secs * 1_000 + nanos / 1_000_000
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn clock_is_monotonic_under_concurrency() {
        let clock = std::sync::Arc::new(MonotonicClock::new());
        let mut handles = Vec::new();
        for _ in 0..8 {
            let clock_ref = clock.clone();
            handles.push(thread::spawn(move || {
                let mut last = 0;
                for _ in 0..10_000 {
                    let now = clock_ref.now();
                    assert!(now >= last);
                    last = now;
                }
            }));
        }
        for handle in handles {
            handle.join().expect("thread panicked");
        }
    }
}
