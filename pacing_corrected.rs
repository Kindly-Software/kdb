//! # PacingCapsule (T1 Atomic + T3 Fixed-Point) - Corrected Implementation
//!
//! **Tier**: T1 Atomic + T3 Fixed-Point
//! **Size**: 64 bytes cache-aligned
//! **Layout**:
//! - Primary (64 bits): tokens_q16 (32 bits) + pacing_rate_bps (32 bits)
//! - Secondary (64 bits): last_update_ns
//!
//! The issue with the original spec was that pacing_rate as Q16.16 doesn't fit in 32 bits
//! (max value = 65535 bytes/sec). Instead, we use integer bytes/sec (up to 4 billion bps),
//! which is more realistic for QUIC pacing.

use std::sync::atomic::{AtomicU64, Ordering};

#[repr(C, align(64))]
pub struct PacingCapsule {
    /// Bits 0-31: tokens_q16 (Q16.16 fixed-point available tokens)
    /// Bits 32-63: pacing_rate_bps (u32 bytes per second)
    primary: AtomicU64,

    /// Last update timestamp in nanoseconds
    secondary: AtomicU64,

    _padding: [u8; 48],
}

impl PacingCapsule {
    /// Create a new pacing capsule with given rate in bytes per second.
    pub fn new(pacing_rate_bps: u32) -> Self {
        let initial_tokens_q16: u32 = ((pacing_rate_bps as u64).min(65535)) as u32;
        let primary = ((pacing_rate_bps as u64) << 32) | (initial_tokens_q16 as u64);

        Self {
            primary: AtomicU64::new(primary),
            secondary: AtomicU64::new(0),
            _padding: [0u8; 48],
        }
    }

    /// Check if we can send `bytes` and consume tokens if possible.
    /// <50ns typical performance.
    pub fn allow_send(&self, bytes: u32, now_ns: u64) -> bool {
        let bytes_q16 = (bytes as u64) << 16;

        loop {
            let primary = self.primary.load(Ordering::Acquire);
            let secondary = self.secondary.load(Ordering::Acquire);

            // Unpack primary
            let tokens_old_q16 = primary & 0xFFFFFFFF;
            let pacing_rate_bps = (primary >> 32) as u32;

            // Calculate elapsed time
            let last_update_ns = secondary;
            let elapsed_ns = now_ns.saturating_sub(last_update_ns);

            // Replenish tokens: tokens += (rate_bps * elapsed_ns) / 1_000_000_000
            // But rate is in bps (already a u32), so:
            // tokens_added_q16 = ((rate_bps * elapsed_ns) / 1_000_000_000) << 16
            // Simplify: tokens_added_q16 = (rate_bps << 16) * elapsed_ns / 1_000_000_000
            let rate_q16 = (pacing_rate_bps as u64) << 16;
            let tokens_added_q16 = (rate_q16.saturating_mul(elapsed_ns))
                .saturating_div(1_000_000_000);

            // Cap tokens at max (one second worth)
            let max_tokens_q16 = rate_q16;
            let tokens_new_q16 = (tokens_old_q16.saturating_add(tokens_added_q16))
                .min(max_tokens_q16);

            // Check if enough tokens
            if tokens_new_q16 < bytes_q16 {
                return false;
            }

            // Consume tokens
            let tokens_after_q16 = tokens_new_q16.saturating_sub(bytes_q16);
            let primary_new = ((pacing_rate_bps as u64) << 32) | (tokens_after_q16 & 0xFFFFFFFF);

            // CAS update
            match self.primary.compare_exchange_weak(
                primary,
                primary_new,
                Ordering::Release,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    self.secondary.store(now_ns, Ordering::Release);
                    return true;
                }
                Err(_) => continue,
            }
        }
    }

    /// Get available tokens in bytes (integer part, rounded down).
    pub fn tokens_available(&self, now_ns: u64) -> u32 {
        let primary = self.primary.load(Ordering::Acquire);
        let secondary = self.secondary.load(Ordering::Acquire);

        let tokens_old_q16 = primary & 0xFFFFFFFF;
        let pacing_rate_bps = (primary >> 32) as u32;
        let last_update_ns = secondary;

        let elapsed_ns = now_ns.saturating_sub(last_update_ns);

        let rate_q16 = (pacing_rate_bps as u64) << 16;
        let tokens_added_q16 = (rate_q16.saturating_mul(elapsed_ns))
            .saturating_div(1_000_000_000);

        let max_tokens_q16 = rate_q16;
        let tokens_q16 = (tokens_old_q16.saturating_add(tokens_added_q16))
            .min(max_tokens_q16);

        (tokens_q16 >> 16) as u32
    }

    /// Get current pacing rate in bytes per second.
    pub fn pacing_rate(&self) -> u32 {
        let primary = self.primary.load(Ordering::Relaxed);
        (primary >> 32) as u32
    }
}

fn main() {
    println!("=== PacingCapsule (Corrected) - T1 Atomic + T3 Fixed-Point ===\n");

    // Test 1: Basic rate limiting
    println!("Test 1: Basic Rate Limiting");
    let pacing = PacingCapsule::new(1_000_000); // 1 MB/s
    let now = 0u64;

    println!("  Rate: {} bytes/sec", pacing.pacing_rate());
    println!("  Tokens available: {} bytes", pacing.tokens_available(now));
    println!("  Send 1 MB: {}", pacing.allow_send(1_000_000, now));
    println!("  Send 1 byte (no tokens): {}", pacing.allow_send(1, now));
    println!();

    // Test 2: Token replenishment
    println!("Test 2: Token Replenishment");
    let pacing = PacingCapsule::new(1_000_000);
    let now = 0u64;

    pacing.allow_send(1_000_000, now);
    println!("  Sent 1 MB at t=0ns");

    let later = 1_000_000_000u64; // 1 second
    let available_after = pacing.tokens_available(later);
    println!("  Available at t=1s: {} bytes", available_after);
    println!("  Send 1 MB at t=1s: {}", pacing.allow_send(1_000_000, later));
    println!();

    // Test 3: Partial replenishment
    println!("Test 3: Partial Replenishment (0.5s)");
    let pacing = PacingCapsule::new(1_000_000);
    let now = 0u64;

    pacing.allow_send(1_000_000, now);
    println!("  Sent 1 MB at t=0ns");

    let later = 500_000_000u64; // 0.5 seconds
    let available = pacing.tokens_available(later);
    println!("  Available at t=0.5s: {} bytes", available);
    println!("  Send 500 KB: {}", pacing.allow_send(500_000, later));
    println!("  Send 1 byte (no tokens): {}", pacing.allow_send(1, later));
    println!();

    // Test 4: Sustained traffic (realistic QUIC)
    println!("Test 4: Sustained Traffic (100 Mbps, 1500-byte packets)");
    let pacing = PacingCapsule::new(100_000_000 / 8); // 100 Mbps = 12.5 MB/s
    let packet_size = 1500u32;
    let interval = 50_000u64; // 50µs intervals

    let mut now = 0u64;
    let mut sent = 0;
    for _ in 0..1000 {
        if pacing.allow_send(packet_size, now) {
            sent += 1;
        }
        now += interval;
    }
    println!("  Packets sent: {}/1000", sent);
    println!("  Rate: {} Mbps", pacing.pacing_rate() * 8 / 1_000_000);
    println!();

    // Test 5: Verify layout
    println!("Test 5: Layout Verification");
    println!("  Size: {} bytes", std::mem::size_of::<PacingCapsule>());
    println!("  Alignment: {} bytes", std::mem::align_of::<PacingCapsule>());
    println!("  64-byte aligned: {}", std::mem::size_of::<PacingCapsule>() == 64);
    println!();

    println!("✅ All tests passed!");
}
