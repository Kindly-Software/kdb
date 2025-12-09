use kindly_av1::encoder::sub_capsules::EncoderSubCapsules;

fn main() {
    let sub_capsules = EncoderSubCapsules::new();

    // Test 64x64
    let seq_header = sub_capsules.bitstream().write_sequence_header_v2(64, 64);

    println!("=== Generated Sequence Header (64x64) ===");
    println!("Length: {} bytes", seq_header.len());
    print!("Hex: ");
    for b in &seq_header {
        print!("{:02x} ", b);
    }
    println!();

    // Reference from libaom for 64x64 (extracted from IVF)
    // Full sequence including temporal delimiter: 12 00 0a 0a 00 00 00 02 af ff 9b 5f 20 08
    // Just sequence header OBU (type=1): 0a 0a 00 00 00 02 af ff 9b 5f 20 08
    let reference = [
        0x0a, 0x0a, 0x00, 0x00, 0x00, 0x02, 0xaf, 0xff, 0x9b, 0x5f, 0x20, 0x08,
    ];

    println!("\n=== Reference (libaom 64x64) ===");
    println!("Length: {} bytes", reference.len());
    print!("Hex: ");
    for b in &reference {
        print!("{:02x} ", b);
    }
    println!();

    println!("\n=== Byte-by-byte comparison ===");
    let min_len = seq_header.len().min(reference.len());
    for i in 0..min_len {
        let gen = seq_header[i];
        let ref_b = reference[i];
        let match_str = if gen == ref_b { "✓" } else { "✗ DIFFER" };
        println!(
            "Byte {}: gen={:02x} ref={:02x} {}",
            i, gen, ref_b, match_str
        );
    }

    if seq_header.len() != reference.len() {
        println!(
            "\nLength mismatch: gen={} ref={}",
            seq_header.len(),
            reference.len()
        );
    }
}
