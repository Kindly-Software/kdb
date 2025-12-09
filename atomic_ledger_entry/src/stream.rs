use crate::{derive_genesis_hash, AleEvent, AleKey, AleRing, Writer, WriterConfig};
use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;
use core::hint::spin_loop;
use core::time::Duration;
use crossbeam_queue::ArrayQueue;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamStats {
    pub appended: u64,
    pub meta_errors: u64,
}

#[derive(Debug)]
pub enum StreamBuildError {
    RingCapacityNotPowerOfTwo { capacity: usize },
    QueueCapacityZero,
    ThreadSpawn(std::io::Error),
}

impl fmt::Display for StreamBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StreamBuildError::RingCapacityNotPowerOfTwo { capacity } => {
                write!(f, "ring capacity {capacity} must be a power of two")
            }
            StreamBuildError::QueueCapacityZero => write!(f, "queue capacity must be positive"),
            StreamBuildError::ThreadSpawn(err) => write!(f, "failed to spawn writer thread: {err}"),
        }
    }
}

impl std::error::Error for StreamBuildError {}

#[derive(Debug)]
pub struct StreamJoinError(Box<dyn std::any::Any + Send + 'static>);

impl fmt::Display for StreamJoinError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ledger writer thread panicked")
    }
}

impl std::error::Error for StreamJoinError {}

impl StreamJoinError {
    pub fn into_inner(self) -> Box<dyn std::any::Any + Send + 'static> {
        self.0
    }
}

#[derive(Default)]
struct Metrics {
    appended: AtomicU64,
    meta_errors: AtomicU64,
}

#[derive(Clone)]
pub struct LedgerProducer {
    queue: Arc<ArrayQueue<AleEvent>>,
    running: Arc<AtomicBool>,
}

impl LedgerProducer {
    pub fn try_enqueue(&self, event: AleEvent) -> Result<(), AleEvent> {
        if !self.running.load(Ordering::Acquire) {
            return Err(event);
        }
        match self.queue.push(event) {
            Ok(()) => Ok(()),
            Err(returned) => Err(returned),
        }
    }

    pub fn enqueue_blocking(&self, mut event: AleEvent) -> Result<(), AleEvent> {
        let mut spin_count = 0u32;
        loop {
            if !self.running.load(Ordering::Acquire) {
                return Err(event);
            }
            match self.queue.push(event) {
                Ok(()) => return Ok(()),
                Err(returned) => {
                    event = returned;
                    if spin_count < 32 {
                        spin_loop();
                        spin_count += 1;
                    } else {
                        spin_count = 0;
                        thread::yield_now();
                    }
                }
            }
        }
    }

    pub fn enqueue_lossy(&self, event: AleEvent) -> bool {
        self.try_enqueue(event).is_ok()
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }
}

enum GenesisSource {
    Derived(Vec<u8>),
    Provided(u64),
}

pub struct LedgerStreamBuilder {
    key: AleKey,
    ring_capacity: usize,
    queue_capacity: usize,
    genesis: GenesisSource,
    writer_config: WriterConfig,
    idle_sleep: Duration,
    thread_name: Option<String>,
}

impl LedgerStreamBuilder {
    pub fn new(key: AleKey) -> Self {
        Self {
            key,
            ring_capacity: 1 << 12,
            queue_capacity: 1 << 14,
            genesis: GenesisSource::Derived(b"ALE|day|stream|boot".to_vec()),
            writer_config: WriterConfig::default(),
            idle_sleep: Duration::from_micros(50),
            thread_name: None,
        }
    }

    pub fn ring_capacity(mut self, capacity: usize) -> Self {
        self.ring_capacity = capacity;
        self
    }

    pub fn queue_capacity(mut self, capacity: usize) -> Self {
        self.queue_capacity = capacity;
        self
    }

    pub fn idle_sleep(mut self, duration: Duration) -> Self {
        self.idle_sleep = duration;
        self
    }

    pub fn genesis_context(mut self, context: impl Into<Vec<u8>>) -> Self {
        self.genesis = GenesisSource::Derived(context.into());
        self
    }

    pub fn genesis_hash(mut self, hash: u64) -> Self {
        self.genesis = GenesisSource::Provided(hash);
        self
    }

    pub fn writer_config(mut self, config: WriterConfig) -> Self {
        self.writer_config = config;
        self
    }

    pub fn thread_name(mut self, name: impl Into<String>) -> Self {
        self.thread_name = Some(name.into());
        self
    }

    pub fn spawn(self) -> Result<LedgerStream, StreamBuildError> {
        let LedgerStreamBuilder {
            key,
            ring_capacity,
            queue_capacity,
            genesis,
            writer_config,
            idle_sleep,
            thread_name,
        } = self;

        if !ring_capacity.is_power_of_two() {
            return Err(StreamBuildError::RingCapacityNotPowerOfTwo {
                capacity: ring_capacity,
            });
        }
        if queue_capacity == 0 {
            return Err(StreamBuildError::QueueCapacityZero);
        }
        let ring = Arc::new(AleRing::with_capacity_pow2(ring_capacity));
        let queue = Arc::new(ArrayQueue::new(queue_capacity));
        let running = Arc::new(AtomicBool::new(true));
        let metrics = Arc::new(Metrics::default());

        let key_for_thread = key.clone();
        let genesis = match genesis {
            GenesisSource::Provided(hash) => hash,
            GenesisSource::Derived(context) => derive_genesis_hash(&key, &context),
        };
        let mut writer_config = writer_config;
        writer_config.genesis_prev_hash = genesis;

        let ring_thread = ring.clone();
        let queue_thread = queue.clone();
        let running_thread = running.clone();
        let metrics_thread = metrics.clone();

        let mut thread_builder = thread::Builder::new();
        if let Some(name) = thread_name {
            thread_builder = thread_builder.name(name);
        }

        let join = thread_builder
            .spawn(move || {
                let mut writer = Writer::new(&ring_thread, &key_for_thread, writer_config);
                while running_thread.load(Ordering::Acquire) || !queue_thread.is_empty() {
                    match queue_thread.pop() {
                        Some(event) => match writer.append(event) {
                            Ok(_) => {
                                metrics_thread.appended.fetch_add(1, Ordering::Relaxed);
                            }
                            Err(_) => {
                                metrics_thread.meta_errors.fetch_add(1, Ordering::Relaxed);
                            }
                        },
                        None => thread::sleep(idle_sleep),
                    }
                }
            })
            .map_err(StreamBuildError::ThreadSpawn)?;

        Ok(LedgerStream {
            ring,
            queue,
            running,
            metrics,
            join: Some(join),
        })
    }
}

pub struct LedgerStream {
    ring: Arc<AleRing>,
    queue: Arc<ArrayQueue<AleEvent>>,
    running: Arc<AtomicBool>,
    metrics: Arc<Metrics>,
    join: Option<JoinHandle<()>>,
}

impl LedgerStream {
    pub fn producer(&self) -> LedgerProducer {
        LedgerProducer {
            queue: self.queue.clone(),
            running: self.running.clone(),
        }
    }

    pub fn ring(&self) -> &Arc<AleRing> {
        &self.ring
    }

    pub fn stats(&self) -> StreamStats {
        StreamStats {
            appended: self.metrics.appended.load(Ordering::Relaxed),
            meta_errors: self.metrics.meta_errors.load(Ordering::Relaxed),
        }
    }

    pub fn shutdown(mut self) -> Result<StreamStats, StreamJoinError> {
        self.running.store(false, Ordering::Release);
        if let Some(handle) = self.join.take() {
            match handle.join() {
                Ok(()) => Ok(self.stats()),
                Err(err) => Err(StreamJoinError(err)),
            }
        } else {
            Ok(self.stats())
        }
    }
}

impl Drop for LedgerStream {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Release);
        if let Some(handle) = self.join.take() {
            let _ = handle.join();
        }
    }
}
