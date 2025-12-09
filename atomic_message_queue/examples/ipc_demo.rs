use atomic_message_queue::{SPSCQueue, MessageBatch};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

/// Example message for IPC communication
#[derive(Debug, Clone)]
struct TradeMessage {
    symbol: String,
    price: f64,
    quantity: u64,
    timestamp: u64,
    side: TradeSide,
}

#[derive(Debug, Clone)]
enum TradeSide {
    Buy,
    Sell,
}

fn main() {
    println!("Atomic Message Queue IPC Demo");
    println!("=============================");

    // Demo 1: High-frequency trading scenario
    high_frequency_trading_demo();

    // Demo 2: Producer-consumer with batching
    batch_processing_demo();

    // Demo 3: Multiple data types
    multi_type_demo();
}

fn high_frequency_trading_demo() {
    println!("\n1. High-Frequency Trading Scenario:");
    println!("   Market data producer -> Trading engine consumer");

    let queue = Arc::new(SPSCQueue::<TradeMessage, 8192>::new());
    let producer_queue = Arc::clone(&queue);
    let consumer_queue = Arc::clone(&queue);

    let start_time = Instant::now();
    const TRADING_SESSION_MS: u64 = 1000; // 1 second trading session

    // Market data producer (simulating market feed)
    let producer = thread::spawn(move || {
        let symbols = ["BTC", "ETH", "SOL", "ADA", "DOT"];
        let mut message_count = 0;
        let session_start = Instant::now();

        while session_start.elapsed().as_millis() < TRADING_SESSION_MS as u128 {
            for (i, &symbol) in symbols.iter().enumerate() {
                let message = TradeMessage {
                    symbol: symbol.to_string(),
                    price: 50000.0 + (i as f64 * 1000.0) + (message_count as f64 * 0.01),
                    quantity: 100 + (message_count % 1000),
                    timestamp: session_start.elapsed().as_nanos() as u64,
                    side: if message_count % 2 == 0 { TradeSide::Buy } else { TradeSide::Sell },
                };

                match producer_queue.push(message) {
                    Ok(()) => {
                        message_count += 1;
                    }
                    Err(_) => {
                        // Queue full - in real HFT, this might trigger backpressure
                        thread::yield_now();
                    }
                }
            }
        }

        println!("   Producer sent {} messages", message_count);
        message_count
    });

    // Trading engine consumer
    let consumer = thread::spawn(move || {
        let mut processed_messages = 0;
        let mut total_volume = 0u64;
        let mut btc_count = 0;

        while start_time.elapsed().as_millis() < (TRADING_SESSION_MS + 100) as u128 {
            match consumer_queue.pop() {
                Ok(message) => {
                    // Simulate trading logic
                    total_volume += message.quantity;
                    if message.symbol == "BTC" {
                        btc_count += 1;
                    }
                    processed_messages += 1;

                    // Simulate processing time (in real HFT this would be microseconds)
                    if processed_messages % 10000 == 0 {
                        thread::yield_now();
                    }
                }
                Err(_) => {
                    // No messages available
                    thread::yield_now();
                }
            }
        }

        println!("   Consumer processed {} messages", processed_messages);
        println!("   Total volume: {} shares", total_volume);
        println!("   BTC messages: {}", btc_count);
        processed_messages
    });

    let sent = producer.join().unwrap();
    let processed = consumer.join().unwrap();
    let total_time = start_time.elapsed();

    println!("   Message throughput: {:.1}K msgs/sec",
        sent as f64 / total_time.as_secs_f64() / 1000.0);
    println!("   Processing efficiency: {:.1}%",
        (processed as f64 / sent as f64) * 100.0);
}

fn batch_processing_demo() {
    println!("\n2. Batch Processing Demo:");
    println!("   Log aggregator with configurable batch sizes");

    #[derive(Debug, Clone)]
    struct LogEntry {
        level: LogLevel,
        message: String,
        timestamp: u64,
    }

    #[derive(Debug, Clone)]
    enum LogLevel {
        Info,
        Warning,
        Error,
    }

    let queue = Arc::new(SPSCQueue::<LogEntry, 2048>::new());
    let producer_queue = Arc::clone(&queue);
    let consumer_queue = Arc::clone(&queue);

    // Log producer
    let producer = thread::spawn(move || {
        let log_messages = [
            "User login successful",
            "Database connection established",
            "Cache miss for key user:123",
            "API request completed",
            "Memory usage: 75%",
        ];

        for i in 0..10000 {
            let entry = LogEntry {
                level: match i % 10 {
                    0..=6 => LogLevel::Info,
                    7..=8 => LogLevel::Warning,
                    _ => LogLevel::Error,
                },
                message: log_messages[i % log_messages.len()].to_string(),
                timestamp: Instant::now().elapsed().as_nanos() as u64,
            };

            loop {
                match producer_queue.push(entry.clone()) {
                    Ok(()) => break,
                    Err(_) => thread::yield_now(),
                }
            }
        }
    });

    // Batch consumer (simulating log aggregation service)
    let consumer = thread::spawn(move || {
        let mut batch = MessageBatch::new(50); // Process in batches of 50
        let mut total_processed = 0;
        let mut error_count = 0;

        while total_processed < 10000 {
            let popped = batch.pop_from_queue(&consumer_queue);

            if popped > 0 {
                // Process batch
                for entry in batch.items() {
                    if matches!(entry.level, LogLevel::Error) {
                        error_count += 1;
                    }
                }

                total_processed += popped;
                batch.clear();

                // Simulate batch processing (e.g., writing to database)
                thread::sleep(Duration::from_micros(100));
            } else {
                thread::yield_now();
            }
        }

        println!("   Processed {} log entries", total_processed);
        println!("   Found {} errors", error_count);
        total_processed
    });

    producer.join().unwrap();
    let processed = consumer.join().unwrap();
    println!("   Batch processing completed: {} entries", processed);
}

fn multi_type_demo() {
    println!("\n3. Multi-Type Message Demo:");
    println!("   Different data types in separate queues");

    // Command queue for control messages
    let cmd_queue = Arc::new(SPSCQueue::<String, 256>::new());
    let cmd_producer = Arc::clone(&cmd_queue);
    let cmd_consumer = Arc::clone(&cmd_queue);

    // Data queue for metrics
    let data_queue = Arc::new(SPSCQueue::<(String, f64), 1024>::new());
    let data_producer = Arc::clone(&data_queue);
    let data_consumer = Arc::clone(&data_queue);

    // Command producer
    let cmd_thread = thread::spawn(move || {
        let commands = [
            "START_MONITORING",
            "SET_THRESHOLD_90",
            "ENABLE_ALERTS",
            "RESTART_SERVICE",
            "STOP_MONITORING",
        ];

        for cmd in commands {
            cmd_producer.push(cmd.to_string()).unwrap();
            thread::sleep(Duration::from_millis(200));
        }
    });

    // Data producer (metrics)
    let data_thread = thread::spawn(move || {
        let metrics = [
            ("cpu_usage", 65.5),
            ("memory_usage", 78.2),
            ("disk_io", 23.1),
            ("network_rx", 1024.0),
            ("network_tx", 512.0),
        ];

        let mut data_count = 0;
        for _ in 0..100 {
            for &(metric, base_value) in &metrics {
                let value = base_value + ((data_count as f64 * 0.1) % 10.0);
                data_producer.push((metric.to_string(), value)).unwrap();
                data_count += 1;
            }
            thread::sleep(Duration::from_millis(10));
        }
    });

    // Consumer for both queues
    let consumer_thread = thread::spawn(move || {
        let mut cmd_count = 0;
        let mut data_count = 0;
        let start = Instant::now();

        while start.elapsed() < Duration::from_secs(2) {
            // Check for commands
            if let Ok(cmd) = cmd_consumer.pop() {
                println!("   Received command: {}", cmd);
                cmd_count += 1;
            }

            // Process data
            if let Ok((metric, value)) = data_consumer.pop() {
                if data_count % 50 == 0 {
                    println!("   Metric: {} = {:.1}", metric, value);
                }
                data_count += 1;
            }

            thread::sleep(Duration::from_millis(1));
        }

        println!("   Commands processed: {}", cmd_count);
        println!("   Data points processed: {}", data_count);
    });

    cmd_thread.join().unwrap();
    data_thread.join().unwrap();
    consumer_thread.join().unwrap();
}