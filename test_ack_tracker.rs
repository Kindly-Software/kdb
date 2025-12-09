#[cfg(test)]
mod tests {
    use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

    const MAX_SENT_PACKETS: usize = 256;
    const MAX_ACK_RANGES: usize = 64;
    const ACK_TRACKER_SIZE: usize = 4096;

    #[repr(C, align(16))]
    pub struct SentPacket {
        pub packet_number: AtomicU64,
        pub time_sent_ns: AtomicU64,
    }

    impl SentPacket {
        pub fn new(packet_number: u64, time_sent_ns: u64) -> Self {
            SentPacket {
                packet_number: AtomicU64::new(packet_number),
                time_sent_ns: AtomicU64::new(time_sent_ns),
            }
        }

        pub fn mark_acked(&self) {
            self.packet_number.store(0, Ordering::Release);
        }

        pub fn is_acked(&self) -> bool {
            self.packet_number.load(Ordering::Acquire) == 0
        }
    }

    #[repr(C)]
    #[derive(Clone, Copy, Debug)]
    pub struct AckRange {
        pub smallest: u64,
        pub largest: u64,
    }

    impl AckRange {
        pub fn new(smallest: u64, largest: u64) -> Self {
            AckRange { smallest, largest }
        }

        pub fn contains(&self, pn: u64) -> bool {
            pn >= self.smallest && pn <= self.largest
        }

        pub fn len(&self) -> u64 {
            self.largest - self.smallest + 1
        }
    }

    #[repr(C, align(256))]
    pub struct AckTrackerCapsule {
        sent_packets: [SentPacket; MAX_SENT_PACKETS],
        head: AtomicU32,
        tail: AtomicU32,
        ack_ranges: [AckRange; MAX_ACK_RANGES],
        ack_range_count: AtomicU32,
        lost_packets: AtomicU32,
        _padding: [u8; 3300],
    }

    impl AckTrackerCapsule {
        pub fn new() -> Self {
            AckTrackerCapsule {
                sent_packets: [SentPacket::new(0, 0); MAX_SENT_PACKETS],
                head: AtomicU32::new(0),
                tail: AtomicU32::new(0),
                ack_ranges: [AckRange::new(0, 0); MAX_ACK_RANGES],
                ack_range_count: AtomicU32::new(0),
                lost_packets: AtomicU32::new(0),
                _padding: [0u8; 3300],
            }
        }

        pub fn record_sent(&self, packet_number: u64, time_sent_ns: u64) -> Result<(), &'static str> {
            let tail = self.tail.load(Ordering::Acquire);
            let head = self.head.load(Ordering::Acquire);
            let next_tail = (tail + 1) % MAX_SENT_PACKETS as u32;
            if next_tail == head {
                return Err("ACK tracker ring buffer full");
            }
            let idx = tail as usize;
            self.sent_packets[idx]
                .packet_number
                .store(packet_number, Ordering::Release);
            self.sent_packets[idx]
                .time_sent_ns
                .store(time_sent_ns, Ordering::Release);
            self.tail.store(next_tail, Ordering::Release);
            Ok(())
        }
    }

    #[test]
    fn test_size() {
        assert_eq!(std::mem::size_of::<AckTrackerCapsule>(), ACK_TRACKER_SIZE);
        println!("✅ AckTrackerCapsule is exactly 4096 bytes");
    }

    #[test]
    fn test_alignment() {
        let tracker = AckTrackerCapsule::new();
        let addr = &tracker as *const _ as usize;
        assert_eq!(addr % 256, 0, "Tracker must be 256B-aligned");
        println!("✅ AckTrackerCapsule is 256B-aligned");
    }

    #[test]
    fn test_record_sent_basic() {
        let tracker = AckTrackerCapsule::new();
        assert!(tracker.record_sent(1, 0).is_ok());
        assert!(tracker.record_sent(2, 1000).is_ok());
        println!("✅ record_sent works correctly");
    }

    #[test]
    fn test_ack_range_contains() {
        let range = AckRange::new(10, 20);
        assert!(range.contains(10));
        assert!(range.contains(15));
        assert!(!range.contains(9));
        println!("✅ AckRange::contains works correctly");
    }
}
