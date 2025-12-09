//! BudgetMetaCapsule Demo - Large-scale budget management
//!
//! Demonstrates:
//! - Slot allocation (1M capacity)
//! - Concurrent budget deductions
//! - Budget queries and statistics
//! - Arc-based sharing for async/concurrent access

use clapi_core::{BudgetMetaCapsule, MAX_BUDGET_SLOTS};
use std::sync::Arc;
use std::thread;

fn main() {
    println!("=== BudgetMetaCapsule Demo ===\n");

    // Part 1: Basic allocation
    demo_basic_allocation();

    // Part 2: Concurrent operations
    demo_concurrent_operations();

    // Part 3: Statistics and capacity
    demo_statistics();
}

/// Demo 1: Basic slot allocation and budget management
fn demo_basic_allocation() {
    println!("--- Demo 1: Basic Allocation ---");

    let mut meta = BudgetMetaCapsule::new();

    // Allocate budget for user 1
    let (slot1, capsule1) = meta.allocate(1000_00).unwrap(); // $1000.00
    println!("Allocated slot {} with $1000.00 budget", slot1);
    println!("  Budget: ${:.2}", capsule1.budget() as f64 / 100.0);

    // Deduct from budget
    let result = capsule1.try_deduct(50_00); // $50.00
    println!("Deducted $50.00: {:?}", result);
    println!("  Remaining: ${:.2}", capsule1.budget() as f64 / 100.0);

    // Allocate more slots
    let (slot2, capsule2) = meta.allocate(500_00).unwrap(); // $500.00
    let (slot3, capsule3) = meta.allocate(2000_00).unwrap(); // $2000.00

    println!("\nAllocated 3 slots total:");
    println!("  Slot {}: ${:.2}", slot1, capsule1.budget() as f64 / 100.0);
    println!("  Slot {}: ${:.2}", slot2, capsule2.budget() as f64 / 100.0);
    println!("  Slot {}: ${:.2}", slot3, capsule3.budget() as f64 / 100.0);
    println!();
}

/// Demo 2: Concurrent budget operations
fn demo_concurrent_operations() {
    println!("--- Demo 2: Concurrent Operations ---");

    let mut meta = Arc::new(std::sync::Mutex::new(BudgetMetaCapsule::new()));

    // Allocate 10 budgets
    let mut capsules = Vec::new();
    for i in 0..10 {
        let mut m = meta.lock().unwrap();
        let (_slot_id, capsule) = m.allocate(10000_00).unwrap(); // $10,000.00 each
        capsules.push(capsule);
    }

    println!("Allocated 10 budgets of $10,000.00 each");

    // Spawn 10 threads, each deducting $1.00 × 100 times from its budget
    let mut handles = Vec::new();
    for (i, capsule) in capsules.into_iter().enumerate() {
        handles.push(thread::spawn(move || {
            let mut success = 0;
            let mut failed = 0;

            for _ in 0..100 {
                match capsule.try_deduct(1_00) {
                    Ok(_) => success += 1,
                    Err(_) => failed += 1,
                }
            }

            (i, success, failed, capsule.budget(), capsule.total_spent())
        }));
    }

    // Collect results
    println!("\nConcurrent deductions (100 × $1.00 per budget):");
    let mut total_spent = 0i64;
    let mut total_remaining = 0i64;

    for h in handles {
        let (budget_id, success, failed, remaining, spent) = h.join().unwrap();
        total_spent += spent;
        total_remaining += remaining;

        println!(
            "  Budget {}: {} success, {} failed | Remaining: ${:.2} | Spent: ${:.2}",
            budget_id,
            success,
            failed,
            remaining as f64 / 100.0,
            spent as f64 / 100.0
        );
    }

    println!("\nTotals:");
    println!("  Spent: ${:.2}", total_spent as f64 / 100.0);
    println!("  Remaining: ${:.2}", total_remaining as f64 / 100.0);
    println!(
        "  Total: ${:.2}",
        (total_spent + total_remaining) as f64 / 100.0
    );
    println!();
}

/// Demo 3: Statistics and capacity
fn demo_statistics() {
    println!("--- Demo 3: Statistics & Capacity ---");

    let mut meta = BudgetMetaCapsule::new();

    // Show initial state
    let stats = meta.get_stats();
    println!("Initial state:");
    println!("  Capacity: {} slots", stats.max_slots);
    println!("  Active: {} slots", stats.slot_count);
    println!("  Generation: {}", stats.generation);

    // Allocate some budgets
    println!("\nAllocating 100 budgets...");
    for i in 0..100 {
        meta.allocate((i * 10 + 100) * 100).unwrap();
    }

    let stats = meta.get_stats();
    println!("After allocation:");
    println!("  Active: {} slots", stats.slot_count);
    println!("  Total allocations: {}", stats.total_allocations);
    println!("  Generation: {}", stats.generation);

    // Deallocate some
    println!("\nDeallocating 50 budgets...");
    for i in 0..50 {
        meta.deallocate(i).unwrap();
    }

    let stats = meta.get_stats();
    println!("After deallocation:");
    println!("  Active: {} slots", stats.slot_count);
    println!("  Total deallocations: {}", stats.total_deallocations);
    println!("  Generation: {}", stats.generation);

    // Show capacity
    println!("\nCapacity information:");
    println!("  Maximum slots: {}", MAX_BUDGET_SLOTS);
    println!(
        "  Current usage: {:.2}%",
        (stats.slot_count as f64 / MAX_BUDGET_SLOTS as f64) * 100.0
    );
    println!(
        "  Available: {} slots",
        MAX_BUDGET_SLOTS - stats.slot_count
    );

    // Memory footprint
    let header_size = std::mem::size_of::<clapi_core::BudgetMetaCapsuleHeader>();
    let slot_size = std::mem::size_of::<std::option::Option<std::sync::Arc<clapi_core::RequestCapsule128>>>();
    let total_size = header_size + (slot_size * MAX_BUDGET_SLOTS);

    println!("\nMemory footprint:");
    println!("  Header: {} bytes", header_size);
    println!("  Slots: {} bytes per slot", slot_size);
    println!(
        "  Total (empty): {} MB",
        total_size / 1024 / 1024
    );
    println!(
        "  Total (full): ~{} MB",
        (header_size + (128 * MAX_BUDGET_SLOTS)) / 1024 / 1024
    );
}
