// Analyze what width_bits libaom uses for 64x64 frame

fn main() {
    // Our calculation
    let width: u32 = 64;
    let our_bits = 32 - width.leading_zeros();
    println!("Our bits_needed(64) = {}", our_bits);
    println!("Our frame_width_bits_minus_1 = {}", our_bits - 1);
    
    // Alternative: bits needed for width-1
    let alt_bits = 32 - (width - 1).leading_zeros();
    println!("Alternative bits_needed(63) = {}", alt_bits);
    println!("Alternative frame_width_bits_minus_1 = {}", alt_bits - 1);
    
    // Reference byte 3 = 0x02 = 0b00000010
    // Bits 30-31 = 10 (binary)
    // If frame_width_bits_minus_1 starts with 10..., it's ≥ 8
    
    // Let's see what bits produce 0x02:
    // Bit 24: 0 (op_idc last bit)
    // Bits 25-29: 00000 (level 0)
    // Bits 30-31: first 2 bits of frame_width_bits_minus_1
    
    // 0x02 = 0b00000010 means bits 30-31 = 0b10
    // So frame_width_bits_minus_1 = 0b10xx = 8..11
    
    // Maybe libaom uses minimum 8 bits (frame_width_bits_minus_1 = 7)?
    // Or 9 bits (frame_width_bits_minus_1 = 8)?
    
    // If frame_width_bits_minus_1 = 8 = 0b1000, first 2 bits = 10 ✓
    println!("\nIf frame_width_bits_minus_1 = 8 (9 bits):");
    println!("  Binary: {:04b}", 8);
    println!("  First 2 bits: {:02b}", 8 >> 2);
    
    // Check byte 4 to narrow down
    // Reference byte 4 = 0xAF = 0b10101111
    
    // Continuing from byte 3:
    // If frame_width_bits_minus_1 = 8 = 0b1000:
    //   Bits 30-33: 1000
    // frame_height_bits_minus_1 should also be 8 for 64x64 square
    //   Bits 34-37: 1000
    // max_frame_width_minus_1 = 63 (in 9 bits)
    //   Bits 38-46: 000111111
    // max_frame_height_minus_1 = 63 (in 9 bits)
    //   Bits 47-55: 000111111
    
    // Let's reconstruct bytes 3-5 with frame_width_bits_minus_1 = 8:
    // Byte 3 (bits 24-31):
    //   24: 0 (op_idc)
    //   25-29: 00000 (level)
    //   30-31: 10 (first 2 of 0b1000)
    //   = 0b00000010 = 0x02 ✓
    
    // Byte 4 (bits 32-39):
    //   32-33: 00 (last 2 of frame_width_bits_minus_1)
    //   34-37: 1000 (frame_height_bits_minus_1 = 8)
    //   38-39: 00 (first 2 of max_frame_width_minus_1 = 63)
    //   = 0b00100000 = 0x20
    
    // But reference byte 4 = 0xAF = 0b10101111
    // That doesn't match...
    
    // Let me try frame_width_bits_minus_1 = 4 (5 bits):
    println!("\nIf frame_width_bits_minus_1 = 4 (5 bits):");
    println!("  Binary: {:04b}", 4);
    
    // Hmm, 4 = 0b0100, first 2 bits = 01
    // That gives byte 3 = 0b00000001 = 0x01, which is what WE have!
    
    // So the question is why libaom has 0x02 instead of 0x01
    
    // Wait - maybe the issue is in the AV1 spec interpretation
    // Let me check if there's padding or something after seq_level_idx
}
