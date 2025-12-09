use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

type Task = Box<dyn FnOnce() + Send>;

struct SimpleQueue {
    data: Mutex<std::collections::VecDeque<Task>>,
    pushed: AtomicUsize,
    stolen: AtomicUsize,
}

impl SimpleQueue {
    fn new() -> Self {
        Self {
            data: Mutex::new(std::collections::VecDeque::new()),
            pushed: AtomicUsize::new(0),
            stolen: AtomicUsize::new(0),
        }
    }

    fn push(&self, task: Task) -> Result<(), String> {
        self.data.lock().unwrap().push_back(task);
        self.pushed.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    fn steal(&self) -> Option<Task> {
        if let Some(task) = self.data.lock().unwrap().pop_front() {
            self.stolen.fetch_add(1, Ordering::Relaxed);
            Some(task)
        } else {
            None
        }
    }
}

thread_local! {
    static TASK_BATCH: std::cell::RefCell<Vec<Task>> = std::cell::RefCell::new(Vec::with_capacity(64));
}

#[derive(Clone)]
pub struct HybridBatchPool {
    queues: Arc<Vec<Arc<SimpleQueue>>>,
    global_tasks: Arc<AtomicUsize>,
    shutdown: Arc<AtomicBool>,
    batch_capacity: usize,
}

impl HybridBatchPool {
    pub fn with_config(num_workers: usize, num_queues: usize, batch_capacity: usize) -> Result<Self, String> {
        if num_workers == 0 || num_queues == 0 {
            return Err("Invalid config".to_string());
        }

        let queues: Vec<Arc<SimpleQueue>> = (0..num_queues)
            .map(|_| Arc::new(SimpleQueue::new()))
            .collect();

        let global_tasks = Arc::new(AtomicUsize::new(0));
        let shutdown = Arc::new(AtomicBool::new(false));

        for _worker_id in 0..num_workers {
            let queues = queues.clone();
            let global_tasks = global_tasks.clone();
            let shutdown = shutdown.clone();

            thread::spawn(move || {
                worker_loop(queues, global_tasks, shutdown);
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

        static QUEUE_COUNTER: AtomicUsize = AtomicUsize::new(0);
        let queue_idx = QUEUE_COUNTER.fetch_add(1, Ordering::Relaxed) % self.queues.len();

        for task in tasks {
            self.queues[queue_idx].push(task)?;
            self.global_tasks.fetch_add(1, Ordering::Release);
        }

        Ok(())
    }

    pub fn wait(&self) {
        TASK_BATCH.with(|batch| {
            let tasks: Vec<_> = batch.borrow_mut().drain(..).collect();
            if !tasks.is_empty() {
                let _ = self.flush_batch(tasks);
            }
        });

        let mut spins = 0;
        loop {
            let remaining = self.global_tasks.load(Ordering::Acquire);
            if remaining == 0 {
                std::thread::yield_now();
                if self.global_tasks.load(Ordering::Acquire) == 0 {
                    break;
                }
            }

            spins += 1;
            if spins % 1000 == 0 {
                let r = self.global_tasks.load(Ordering::Acquire);
                eprintln!("  waiting... {} tasks remaining", r);
                std::thread::sleep(std::time::Duration::from_millis(1));
            } else {
                std::thread::yield_now();
            }
        }
    }

    pub fn stats(&self) {
        eprintln!("\nQueue statistics:");
        let mut total_pushed = 0;
        let mut total_stolen = 0;
        for (i, q) in self.queues.iter().enumerate() {
            let p = q.pushed.load(Ordering::Relaxed);
            let s = q.stolen.load(Ordering::Relaxed);
            eprintln!("  Queue {}: pushed={}, stolen={}, diff={}", i, p, s, p - s);
            total_pushed += p;
            total_stolen += s;
        }
        eprintln!("  Total: pushed={}, stolen={}, diff={}", total_pushed, total_stolen, total_pushed - total_stolen);
        eprintln!("  Global counter: {}", self.global_tasks.load(Ordering::Relaxed));
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

fn worker_loop(queues: Vec<Arc<SimpleQueue>>, global_tasks: Arc<AtomicUsize>, shutdown: Arc<AtomicBool>) {
    let mut last_queue = 0;

    loop {
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
            if shutdown.load(Ordering::Relaxed) {
                break;
            }
            std::thread::yield_now();
        }
    }
}

fn main() {
    println!("Debug HybridBatchPool - Task Loss Investigation\n");

    println!("Test: 1,000 tasks, 10 producer threads");
    let pool = Arc::new(HybridBatchPool::with_config(8, 8, 64).unwrap());
    let counter = Arc::new(AtomicUsize::new(0));

    let handles: Vec<_> = (0..10)
        .map(|tid| {
            let pool = pool.clone();
            let counter = counter.clone();

            thread::spawn(move || {
                eprintln!("[P{}] Starting producer", tid);
                for i in 0..100 {
                    let c = counter.clone();
                    pool.push(Box::new(move || {
                        c.fetch_add(1, Ordering::Relaxed);
                    }))
                    .unwrap();
                }
                eprintln!("[P{}] Finished pushing 100 tasks", tid);
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    eprintln!("\nAll producers finished. Waiting for workers to drain...");
    pool.wait();

    let final_count = counter.load(Ordering::Relaxed);
    eprintln!("\nFinal counter: {}", final_count);

    pool.stats();

    if final_count == 1000 {
        println!("\n✓ Test PASSED - all 1000 tasks executed");
    } else {
        println!("\n✗ Test FAILED - {} tasks lost (expected 1000)", 1000 - final_count);
    }
}
