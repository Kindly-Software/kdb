//! Verify Sequence Header Level Fix
//!
//! This example verifies that the sequence header now uses Level 2.0 for 64×64.

use atomic_capsule::encoder::ObuBitstreamWriterCapsule;

fn main() {
    println!("=== AV1 Sequence Header Level Fix Verification ===\n");

    let writer = ObuBitstreamWriterCapsule::new();

    // Test 64×64 (should use Level 2.0, seq_level_idx=0)
    let obu_64 = writer.write_sequence_header_spec_compliant(64, 64);
    println!("64×64 Sequence Header:");
    println!("  Total: {} bytes", obu_64.len());
    println!("  Hex: {}", obu_64.iter().map(|b| format!("{:02x}", b)).collect::<Vec<_>>().join(" "));

    // Extract payload (skip OBU header and size)
    let payload_64 = &obu_64[2..];
    println!("  Payload: {}", payload_64.iter().map(|b| format!("{:02x}", b)).collect::<Vec<_>>().join(" "));

    // Decode seq_level_idx (bits 25-29)
    struct BitReader<'a> {
        data: &'a [u8],
        bit_pos: usize,
    }

    impl<'a> BitReader<'a> {
        fn new(data: &'a [u8]) -> Self {
            Self { data, bit_pos: 0 }
        }

        fn read_bits(&mut self, n: u8) -> u64 {
            let mut value = 0u64;
            for _ in 0..n {
                let byte_index = self.bit_pos / 8;
                let bit_index = 7 - (self.bit_pos % 8);
                if byte_index >= self.data.len() {
                    break;
                }
                let bit = (self.data[byte_index] >> bit_index) & 1;
                value = (value << 1) | (bit as u64);
                self.bit_pos += 1;
            }
            value
        }
    }

    let mut reader = BitReader::new(payload_64);
    reader.read_bits(24); // Skip to seq_level_idx
    let level_idx = reader.read_bits(5);

    println!("  seq_level_idx: {} (Level {})",
             level_idx,
             match level_idx {
                 0 => "2.0",
                 1 => "2.1",
                 2 => "2.2",
                 3 => "2.3",
                 4 => "3.0",
                 5 => "3.1",
                 8 => "4.0",
                 12 => "5.0",
                 16 => "6.0",
                 _ => "Unknown",
             });

    if level_idx == 0 {
        println!("  ✓ CORRECT: Using Level 2.0 for 64×64");
    } else {
        println!("  ✗ WRONG: Should use Level 2.0 (seq_level_idx=0), got {}", level_idx);
    }

    println!();

    // Test other resolutions
    let test_cases = vec![
        (384, 288, 0, "2.0"),
        (480, 360, 1, "2.1"),
        (1920, 1080, 8, "4.0"),
    ];

    for (width, height, expected_idx, expected_level) in test_cases {
        let obu = writer.write_sequence_header_spec_compliant(width, height);
        let payload = &obu[2..];

        let mut reader = BitReader::new(payload);
        reader.read_bits(24);
        let level_idx = reader.read_bits(5);

        let status = if level_idx as u8 == expected_idx { "✓" } else { "✗" };

        println!("{}×{}: seq_level_idx={} (Level {}) {}",
                 width, height, level_idx,
                 match level_idx {
                     0 => "2.0",
                     1 => "2.1",
                     4 => "3.0",
                     5 => "3.1",
                     8 => "4.0",
                     12 => "5.0",
                     16 => "6.0",
                     _ => "?",
                 },
                 status);
    }

    println!("\n=== Verification Complete ===");
}
