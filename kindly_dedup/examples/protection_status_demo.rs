//! Protection Status Visualization Demo
//!
//! Demonstrates real-time 4-layer protection status display for Phase 6.
//!
//! **Usage**:
//! ```bash
//! cargo run --example protection_status_demo --features "meta-capsule,interactive"
//! ```
//!
//! **Output**: Real-time protection layer status with Byzantine purple + gold styling

use kindly_dedup::tui::components::{
    LayerStatus, ProtectionStatusCapsule, ProtectionStatusViewer,
};
use std::thread;
use std::time::Duration;

fn main() {
    println!("\n4-Layer Protection Status Demo\n");
    println!("Press Ctrl+C to exit\n");

    // Create protection status capsule
    let capsule = ProtectionStatusCapsule::new();

    // Simulate protection layer status updates
    println!("Initializing protection layers...\n");
    thread::sleep(Duration::from_secs(1));

    // Layer 2: Circuit Breaker (8 detection methods)
    capsule.update_layer2(LayerStatus::Secure);
    println!("✓ Circuit Breaker initialized (8/8 checks passing)\n");
    thread::sleep(Duration::from_millis(500));

    // Layer 2.5: Hardware Binding (PUF + Hardware ID)
    capsule.update_layer2_5(LayerStatus::Secure);
    println!("✓ Hardware Binding complete (PUF 99.7% stable)\n");
    thread::sleep(Duration::from_millis(500));

    // Layer 3: License Management
    capsule.update_layer3(LayerStatus::Secure);
    println!("✓ License validated (24hr cache active)\n");
    thread::sleep(Duration::from_millis(500));

    // Layer 4: Audit Trail
    capsule.update_audit_metrics(0, true);
    println!("✓ Audit trail initialized (hash chain intact)\n");
    thread::sleep(Duration::from_secs(1));

    // Render initial status
    println!("{}", ProtectionStatusViewer::render(&capsule));
    println!("\nSimulating security events...\n");

    // Simulate audit events
    for i in 1..=10 {
        thread::sleep(Duration::from_millis(500));

        // Update audit metrics
        let events = i * 25;
        capsule.update_audit_metrics(events, true);

        // Clear screen and re-render
        print!("\x1B[2J\x1B[H"); // Clear screen, move cursor to top
        println!("4-Layer Protection Status Demo (Event {})\n", events);
        println!("{}", ProtectionStatusViewer::render(&capsule));

        // Show compact status
        println!("\n{}\n", ProtectionStatusViewer::render_compact(&capsule));
    }

    // Simulate warning state
    println!("\nSimulating PUF drift (thermal variation)...\n");
    thread::sleep(Duration::from_secs(1));
    capsule.update_layer2_5(LayerStatus::Warning);

    print!("\x1B[2J\x1B[H");
    println!("4-Layer Protection Status Demo (PUF Drift Detected)\n");
    println!("{}", ProtectionStatusViewer::render(&capsule));
    println!("\n{}\n", ProtectionStatusViewer::render_compact(&capsule));

    thread::sleep(Duration::from_secs(2));

    // Restore secure state
    println!("\nPUF recalibrated (stable)\n");
    capsule.update_layer2_5(LayerStatus::Secure);

    print!("\x1B[2J\x1B[H");
    println!("4-Layer Protection Status Demo (All Secure)\n");
    println!("{}", ProtectionStatusViewer::render(&capsule));
    println!("\n{}\n", ProtectionStatusViewer::render_compact(&capsule));

    // Final stats
    println!("\nDemo Complete!");
    println!("  Total Events: {}", capsule.get_events_logged());
    println!("  Hash Chain: {}", if capsule.is_chain_intact() { "INTACT" } else { "BROKEN" });
    println!("  Active Layers: {}/5", capsule.active_layer_count());
    println!("  Overall Status: {:?}\n", capsule.overall_status());
}
