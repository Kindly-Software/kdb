//! Standalone tests for RingBufferBroadcast
//!
//! These tests can run independently of the main lib.rs

// Direct module inclusion for testing (bypass lib.rs)
#[path = "../src/collections/ring_broadcast.rs"]
mod ring_broadcast;

use ring_broadcast::*;

#[test]
fn test_basic_send_recv() {
    let (tx, mut rx) = channel();

    tx.send(42u64).unwrap();
    assert_eq!(rx.recv().unwrap(), 42);
}

#[test]
fn test_fifo_order() {
    let (tx, mut rx) = channel();

    for i in 0..100 {
        tx.send(i).unwrap();
    }

    for i in 0..100 {
        assert_eq!(rx.recv().unwrap(), i);
    }
}

#[test]
fn test_multi_consumer() {
    let (tx, mut rx1) = channel();
    let mut rx2 = tx.subscribe();
    let mut rx3 = tx.subscribe();

    for i in 0..10 {
        tx.send(i).unwrap();
    }

    for i in 0..10 {
        assert_eq!(rx1.recv().unwrap(), i);
        assert_eq!(rx2.recv().unwrap(), i);
        assert_eq!(rx3.recv().unwrap(), i);
    }
}

#[test]
fn test_lossless() {
    let (tx, mut rx) = channel();

    for i in 0..1000 {
        tx.send(i).unwrap();
    }

    for i in 0..1000 {
        assert_eq!(rx.recv().unwrap(), i);
    }
}

#[test]
fn test_concurrent_send_recv() {
    use std::thread;

    let (tx, mut rx) = channel();
    let tx2 = tx.clone();

    let h1 = thread::spawn(move || {
        for i in 0..500 {
            tx.send(i * 2).unwrap();
        }
    });

    let h2 = thread::spawn(move || {
        for i in 0..500 {
            tx2.send(i * 2 + 1).unwrap();
        }
    });

    h1.join().unwrap();
    h2.join().unwrap();

    let mut received = Vec::new();
    for _ in 0..1000 {
        received.push(rx.recv().unwrap());
    }

    received.sort();
    assert_eq!(received.len(), 1000);
}

#[test]
fn test_receiver_count() {
    let (tx, rx1): (BroadcastSender<u64>, BroadcastReceiver<u64>) = channel();
    assert_eq!(tx.receiver_count(), 1);

    let rx2 = tx.subscribe();
    assert_eq!(tx.receiver_count(), 2);

    let rx3 = tx.subscribe();
    assert_eq!(tx.receiver_count(), 3);

    drop(rx1);
    assert_eq!(tx.receiver_count(), 2);

    drop(rx2);
    assert_eq!(tx.receiver_count(), 1);

    drop(rx3);
    assert_eq!(tx.receiver_count(), 0);
}
