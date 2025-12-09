// Quick validation test for generation counter fixes
use atomic_capsule::encoder::temporal_rdo::TemporalRDOCapsule;

fn main() {
    println!("Testing Generation Counter Fixes...\n");

    let capsule = TemporalRDOCapsule::new(24);
    
    // Test 1: Initial generation should be 1 (from new())
    let gen1 = capsule.get_generation();
    println!("✓ Initial generation: {} (expected: 1)", gen1);
    assert_eq!(gen1, 1, "Initial generation should be 1");

    // Test 2: Update lambda, generation should increment
    capsule.update_lambda(32);
    let gen2 = capsule.get_generation();
    println!("✓ After 1st update: {} (expected: 2)", gen2);
    assert_eq!(gen2, 2, "Generation should increment to 2");

    // Test 3: Multiple updates should increment properly
    for i in 3..=10 {
        capsule.update_lambda(24 + (i as u8));
        let gen = capsule.get_generation();
        assert_eq!(gen, i as u32, "Generation should be {}", i);
    }
    println!("✓ Multiple updates: generations 3-10 correct");

    // Test 4: Verify generation wraps at 256
    for _ in 0..246 {
        capsule.update_lambda(24);
    }
    let gen_before_wrap = capsule.get_generation();
    capsule.update_lambda(24);
    let gen_after_wrap = capsule.get_generation();
    println!("✓ Wrap test: {} -> {} (expected: 255 -> 0)", gen_before_wrap, gen_after_wrap);
    assert_eq!(gen_before_wrap, 255, "Should be 255 before wrap");
    assert_eq!(gen_after_wrap, 0, "Should wrap to 0");

    // Test 5: Verify lambda values are still correct (Q16.16)
    let lambda_q16 = capsule.get_lambda_q16(24);
    println!("✓ Lambda Q16.16 for QP=24: {} (expected: 891289)", lambda_q16);
    assert_eq!(lambda_q16, 891289, "Lambda should be correct Q16.16 value");

    println!("\n✅ All generation counter tests PASSED!");
}
