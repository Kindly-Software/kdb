//! Debug Level Selection Logic

fn compute_level_index(width: u16, height: u16) -> u8 {
    let pic_size = (width as u32) * (height as u32);

    println!("{}×{} = {} pixels", width, height, pic_size);

    if pic_size <= 110_592 {
        println!("  → Level 2.0 (≤ 110,592)");
        0
    } else if pic_size <= 278_784 {
        println!("  → Level 2.1 (≤ 278,784)");
        1
    } else if pic_size <= 665_856 {
        println!("  → Level 3.0 (≤ 665,856)");
        4
    } else if pic_size <= 1_065_024 {
        println!("  → Level 3.1 (≤ 1,065,024)");
        5
    } else if pic_size <= 2_359_296 {
        println!("  → Level 4.0 (≤ 2,359,296)");
        8
    } else if pic_size <= 8_912_896 {
        println!("  → Level 5.0 (≤ 8,912,896)");
        12
    } else {
        println!("  → Level 6.0 (> 8,912,896)");
        16
    }
}

fn main() {
    println!("=== Level Selection Debug ===\n");

    let test_cases = vec![
        (64, 64),
        (384, 288),
        (480, 360),
        (768, 576),
        (1024, 576),
        (1920, 1080),
        (3840, 2160),
    ];

    for (width, height) in test_cases {
        let level = compute_level_index(width, height);
        println!("  Result: seq_level_idx = {}\n", level);
    }
}
