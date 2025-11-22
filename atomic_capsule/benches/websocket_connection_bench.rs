//! WebSocketConnectionCapsule Benchmarks (T1 Atomic)
//!
//! Measures latency of per-connection state machine operations.

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use std::sync::Arc;
use std::thread;

// Mock capsule for testing (standalone copy of implementation)
use std::sync::atomic::{AtomicU64, AtomicU32, AtomicI32, Ordering};

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConnectionState {
    Connecting = 0,
    Open = 1,
    Closing = 2,
    Closed = 3,
}

#[repr(C, align(64))]
pub struct WebSocketConnectionCapsule {
    state: AtomicU64,
    connection_id: AtomicU64,
    socket_fd: AtomicI32,
    _padding1: [u8; 4],
    established_time_ns: AtomicU64,
    last_activity_ns: AtomicU64,
    messages_sent: AtomicU32,
    messages_received: AtomicU32,
    bytes_sent: AtomicU64,
    bytes_received: AtomicU64,
}

impl WebSocketConnectionCapsule {
    pub fn new(connection_id: u64, socket_fd: Option<i32>) -> Self {
        Self {
            state: AtomicU64::new(ConnectionState::Connecting as u64),
            connection_id: AtomicU64::new(connection_id),
            socket_fd: AtomicI32::new(socket_fd.unwrap_or(-1)),
            _padding1: [0u8; 4],
            established_time_ns: AtomicU64::new(0),
            last_activity_ns: AtomicU64::new(0),
            messages_sent: AtomicU32::new(0),
            messages_received: AtomicU32::new(0),
            bytes_sent: AtomicU64::new(0),
            bytes_received: AtomicU64::new(0),
        }
    }

    pub fn get_state(&self) -> ConnectionState {
        let bits = self.state.load(Ordering::Acquire);
        match bits & 0x7 {
            0 => ConnectionState::Connecting,
            1 => ConnectionState::Open,
            2 => ConnectionState::Closing,
            3 => ConnectionState::Closed,
            _ => ConnectionState::Closed,
        }
    }

    pub fn set_state(&self, new_state: ConnectionState) {
        let new_state_bits = new_state as u64;
        let current = self.state.load(Ordering::Acquire);
        let new_value = (current & !0x7) | new_state_bits;
        self.state.store(new_value, Ordering::SeqCst);
    }

    pub fn is_open(&self) -> bool {
        let bits = self.state.load(Ordering::Relaxed);
        (bits & 0x7) == (ConnectionState::Open as u64)
    }

    pub fn on_message_sent(&self, bytes: usize) {
        self.messages_sent.fetch_add(1, Ordering::Relaxed);
        self.bytes_sent.fetch_add(bytes as u64, Ordering::Relaxed);
    }

    pub fn on_message_received(&self, bytes: usize) {
        self.messages_received.fetch_add(1, Ordering::Relaxed);
        self.bytes_received.fetch_add(bytes as u64, Ordering::Relaxed);
    }
}

fn benchmark_state_transition(c: &mut Criterion) {
    let mut group = c.benchmark_group("state_transition");

    for state_count in [10, 100, 1000].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(state_count), state_count, |b, &count| {
            b.iter(|| {
                let conn = black_box(WebSocketConnectionCapsule::new(1, Some(3)));
                for _ in 0..count {
                    conn.set_state(ConnectionState::Open);
                    conn.set_state(ConnectionState::Closing);
                    conn.set_state(ConnectionState::Closed);
                }
            })
        });
    }
    group.finish();
}

fn benchmark_get_state(c: &mut Criterion) {
    c.bench_function("get_state_relaxed", |b| {
        let conn = WebSocketConnectionCapsule::new(1, Some(3));
        conn.set_state(ConnectionState::Open);
        b.iter(|| {
            for _ in 0..1000 {
                black_box(conn.get_state());
            }
        })
    });
}

fn benchmark_metrics_sent(c: &mut Criterion) {
    c.bench_function("on_message_sent", |b| {
        let conn = WebSocketConnectionCapsule::new(1, Some(3));
        b.iter(|| {
            for i in 0..1000 {
                conn.on_message_sent(i * 10);
            }
        })
    });
}

fn benchmark_metrics_received(c: &mut Criterion) {
    c.bench_function("on_message_received", |b| {
        let conn = WebSocketConnectionCapsule::new(1, Some(3));
        b.iter(|| {
            for i in 0..1000 {
                conn.on_message_received(i * 20);
            }
        })
    });
}

fn benchmark_concurrent_metrics(c: &mut Criterion) {
    c.bench_function("concurrent_metrics_8threads", |b| {
        b.iter(|| {
            let conn = Arc::new(WebSocketConnectionCapsule::new(1, Some(3)));
            let handles: Vec<_> = (0..8)
                .map(|_| {
                    let conn_clone = Arc::clone(&conn);
                    thread::spawn(move || {
                        for i in 0..100 {
                            conn_clone.on_message_sent(i);
                            conn_clone.on_message_received(i * 2);
                        }
                    })
                })
                .collect();

            for handle in handles {
                handle.join().unwrap();
            }
        })
    });
}

criterion_group!(
    benches,
    benchmark_state_transition,
    benchmark_get_state,
    benchmark_metrics_sent,
    benchmark_metrics_received,
    benchmark_concurrent_metrics,
);

criterion_main!(benches);
