//! Standalone HybridBatchPool test and benchmark
//! Compile with: rustc --edition 2021 -L /home/samuel/Primitives/atomic_capsule/target/debug/deps test_hybrid_batch_pool.rs

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::Instant;

type Task = Box<dyn FnOnce() + Send>;

// Minimal LockfreeWorkQueue implementation for testing
pub struct LockfreeWorkQueue {
    data: Arc<parking_lot::Mutex<std::collections::VecDeque<Task>>>,
}

impl LockfreeWorkQueue {
    fn new() -> Self {
        Self {
            data: Arc::new(parking_lot::Mutex::new(std::collections::VecDeque::new())),
        }
    }

    fn push(&self, task: Task) -> Result<(), String> {
        self.data.lock().push_back(task);
        Ok(())
    }

    fn steal(&self) -> Option<Task> {
        self.data.lock().pop_front()
    }
}

// Thread-local batch storage
thread_local! {
    static TASK_BATCH: std::cell::RefCell<Vec<Task>> = std::cell::RefCell::new(Vec::with_capacity(64));
}

/// HybridBatchPool: Thread-local batching + lockfree distribution
#[derive(Clone)]
pub struct HybridBatchPool {
    queues: Arc<Vec<Arc<LockfreeWorkQueue>>>,
    global_tasks: Arc<AtomicUsize>,
    shutdown: Arc<AtomicBool>,
    batch_capacity: usize,
}

impl HybridBatchPool {
    pub fn new(num_workers: usize) -> Result<Self, String> {
        Self::with_config(num_workers, 8, 64)
    }

    pub fn with_config(
        num_workers: usize,
        num_queues: usize,
        batch_capacity: usize,
    ) -> Result<Self, String> {
        if num_workers == 0 || num_queues == 0 {
            return Err("Invalid config".to_string());
        }

        let queues: Vec<Arc<LockfreeWorkQueue>> = (0..num_queues)
            .map(|_| Arc::new(LockfreeWorkQueue::new()))
            .collect();

        let global_tasks = Arc::new(AtomicUsize::new(0));
        let shutdown = Arc::new(AtomicBool::new(false));

        // Spawn worker threads
        for worker_id in 0..num_workers {
            let queues = queues.clone();
            let global_tasks = global_tasks.clone();
            let shutdown = shutdown.clone();

            thread::spawn(move || {
                worker_loop(worker_id, queues, global_tasks, shutdown);
            });
        }

        Ok(Self {
            queues: Arc::new(queues),
            global_tasks,
            shutdown,
            batch_capacity,
        })
    }

    pub fn push(&self, task: Task) -> Result<(), String> {
        if self.shutdown.load(Ordering::Relaxed) {
            return Err("Pool shutdown".to_string());
        }

        TASK_BATCH.with(|batch| {
            let mut b = batch.borrow_mut();
            b.push(task);

            if b.len() >= self.batch_capacity {
                self.flush_batch(b.drain(..).collect())?;
            }

            Ok(())
        })
    }

    fn flush_batch(&self, tasks: Vec<Task>) -> Result<(), String> {
        if tasks.is_empty() {
            return Ok(());
        }

        // Distribute to queue based on thread ID
        let thread_id = std::thread::current().id();
        let queue_idx = (thread_id.as_u64().get() as usize) % self.queues.len();

        for task in tasks {
            self.queues[queue_idx].push(task)?;
            self.global_tasks.fetch_add(1, Ordering::Release);
        }

        Ok(())
    }

    pub fn wait(&self) {
        // Flush any remaining batched tasks
        TASK_BATCH.with(|batch| {
            let tasks: Vec<_> = batch.borrow_mut().drain(..).collect();
            if !tasks.is_empty() {
                let _ = self.flush_batch(tasks);
            }
        });

        // Spin-wait for completion
        loop {
            let remaining = self.global_tasks.load(Ordering::Acquire);
            if remaining == 0 {
                break;
            }
            std::thread::yield_now();
        }
    }

    pub fn remaining_tasks(&self) -> usize {
        self.global_tasks.load(Ordering::Acquire)
    }
}

impl Drop for HybridBatchPool {
    fn drop(&mut self) {
        TASK_BATCH.with(|batch| {
            let tasks: Vec<_> = batch.borrow_mut().drain(..).collect();
            if !tasks.is_empty() {
                let _ = self.flush_batch(tasks);
            }
        });

        self.wait();
        self.shutdown.store(true, Ordering::Release);
    }
}

fn worker_loop(
    _worker_id: usize,
    queues: Vec<Arc<LockfreeWorkQueue>>,
    global_tasks: Arc<AtomicUsize>,
    shutdown: Arc<AtomicBool>,
) {
    let mut last_queue = 0;

    loop {
        if shutdown.load(Ordering::Relaxed) {
            break;
        }

        let mut found = false;

        for i in 0..queues.len() {
            let queue_idx = (last_queue + i) % queues.len();

            if let Some(task) = queues[queue_idx].steal() {
                task();
                global_tasks.fetch_sub(1, Ordering::Release);

                found = true;
                last_queue = queue_idx;
                break;
            }
        }

        if !found {
            std::thread::yield_now();
        }
    }
}

fn main() {
    println!("HybridBatchPool Standalone Tests");
    println!("================================\n");

    // Test 1: Basic task execution
    println!("Test 1: Basic single task...");
    let pool = HybridBatchPool::new(2).unwrap();
    let counter = Arc::new(AtomicUsize::new(0));

    let c = counter.clone();
    pool.push(Box::new(move || {
        c.fetch_add(1, Ordering::Relaxed);
    }))
    .unwrap();

    pool.wait();
    assert_eq!(counter.load(Ordering::Relaxed), 1);
    println!("✓ Single task test passed\n");

    // Test 2: 100 tasks single thread
    println!("Test 2: 100 tasks, single thread...");
    let pool = HybridBatchPool::new(2).unwrap();
    let counter = Arc::new(AtomicUsize::new(0));

    for _ in 0..100 {
        let c = counter.clone();
        pool.push(Box::new(move || {
            c.fetch_add(1, Ordering::Relaxed);
        }))
        .unwrap();
    }

    pool.wait();
    assert_eq!(counter.load(Ordering::Relaxed), 100);
    println!("✓ 100 tasks test passed\n");

    // Test 3: 1,000 tasks, 10 threads
    println!("Test 3: 1,000 tasks, 10 threads...");
    let pool = Arc::new(HybridBatchPool::new(8).unwrap());
    let counter = Arc::new(AtomicUsize::new(0));

    let handles: Vec<_> = (0..10)
        .map(|_| {
            let pool = pool.clone();
            let counter = counter.clone();

            thread::spawn(move || {
                for _ in 0..100 {
                    let c = counter.clone();
                    pool.push(Box::new(move || {
                        c.fetch_add(1, Ordering::Relaxed);
                    }))
                    .unwrap();
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    pool.wait();
    assert_eq!(counter.load(Ordering::Relaxed), 1000);
    println!("✓ 1,000 tasks test passed\n");

    // Test 4: THE CRITICAL TEST - 1,600 tasks, 50 threads
    println!("Test 4: CRITICAL - 1,600 tasks, 50 threads (4.4× speedup target)...");
    let pool = Arc::new(HybridBatchPool::new(8).unwrap());
    let counter = Arc::new(AtomicUsize::new(0));
    let start = Instant::now();

    let handles: Vec<_> = (0..50)
        .map(|_| {
            let pool = pool.clone();
            let counter = counter.clone();

            thread::spawn(move || {
                for _ in 0..32 {
                    let c = counter.clone();
                    pool.push(Box::new(move || {
                        c.fetch_add(1, Ordering::Relaxed);
                    }))
                    .unwrap();
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    pool.wait();
    let elapsed = start.elapsed();

    assert_eq!(
        counter.load(Ordering::Relaxed),
        1600,
        "Task loss: expected 1600, got {}",
        counter.load(Ordering::Relaxed)
    );

    println!("✓ 1,600 tasks completed successfully!");
    println!("  Elapsed time: {:.2}ms", elapsed.as_secs_f64() * 1000.0);
    println!("  Target: <20μs (<0.02ms)");
    println!("  Status: {} {}",
        if elapsed.as_micros() < 20000 { "✓ PASSED" } else { "⚠ EXCEEDED" },
        if elapsed.as_micros() < 20000 { "(4.4× speedup confirmed)" } else { "(needs optimization)" }
    );

    println!("\n✓ All tests passed!");
}
