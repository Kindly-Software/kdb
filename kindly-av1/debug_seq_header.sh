#!/bin/bash
# Debug sequence header generation to hex dump

cd /home/samuel/Primitives/kindly-av1

cat > /tmp/debug_seq.rs << 'EOF'
use atomic_capsule::encoder::ObuBitstreamWriterCapsule;

fn main() {
    let writer = ObuBitstreamWriterCapsule::new();

    // Test both methods
    println!("=== write_sequence_header_dav1d_compatible (64x64) ===");
    let dav1d_header = writer.write_sequence_header_dav1d_compatible(64, 64);
    println!("Size: {} bytes", dav1d_header.len());
    print!("Hex: ");
    for (i, byte) in dav1d_header.iter().enumerate() {
        if i > 0 && i % 16 == 0 { print!("\n     "); }
        print!("{:02x} ", byte);
    }
    println!("\n");

    println!("=== write_sequence_header_spec_compliant (64x64) ===");
    let spec_header = writer.write_sequence_header_spec_compliant(64, 64);
    println!("Size: {} bytes", spec_header.len());
    print!("Hex: ");
    for (i, byte) in spec_header.iter().enumerate() {
        if i > 0 && i % 16 == 0 { print!("\n     "); }
        print!("{:02x} ", byte);
    }
    println!("\n");
}
EOF

rustc --edition 2021 \
    --extern atomic_capsule=/home/samuel/Primitives/atomic_capsule/target/debug/libatomic_capsule.rlib \
    -L /home/samuel/Primitives/atomic_capsule/target/debug/deps \
    /tmp/debug_seq.rs -o /tmp/debug_seq 2>&1 | head -50

if [ -f /tmp/debug_seq ]; then
    /tmp/debug_seq
else
    echo "Compilation failed, trying simpler approach..."
    cargo run --example test_seq_header
fi
