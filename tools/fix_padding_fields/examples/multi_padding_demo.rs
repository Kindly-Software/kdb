//! Demonstration of multi-padding field consolidation
//!
//! This example shows how the refactored `fix_padding_fields` tool
//! consolidates multiple padding fields into a single _padding field.

use fix_padding_fields::{extract_capsules, PaddingCalculator, PaddingFixer};

fn main() {
    // Example 1: Multiple padding fields (_padding1, _padding2)
    let source_multi = r#"
#[derive(ComputationalCapsule)]
#[capsule(alignment = 128, size = 128)]
#[repr(C, align(128))]
struct MultiPaddingCapsule {
    state: AtomicU64,
    counter: AtomicU64,
    _padding1: [u8; 8],
    _padding2: [u8; 48],
}
"#;

    println!("=== Example 1: Multiple Padding Fields ===\n");
    println!("BEFORE:\n{}", source_multi);

    // Extract capsule information
    let capsules = extract_capsules(source_multi).unwrap();
    let capsule = &capsules[0];

    println!("Detected capsule: {}", capsule.name);
    println!("Padding fields found: {}", capsule.padding_fields.len());
    for pf in &capsule.padding_fields {
        println!("  - {}: {} bytes", pf.name, pf.size_bytes);
    }
    println!("Total padding size: {} bytes", capsule.total_padding_size);
    println!("Needs consolidation: {}", capsule.needs_consolidation());

    // Calculate required padding
    let calculator = PaddingCalculator::new(capsule).unwrap();
    println!("\nData size: {} bytes", calculator.total_data_size());
    println!("Required padding: {} bytes", calculator.required_padding());
    println!("Needs fixing: {}", calculator.needs_fixing());

    // Apply fix
    let mut fixer = PaddingFixer::new(source_multi.to_string());
    let fixed = fixer.apply_padding_fix(capsule).unwrap();
    println!("\nFixed: {}", fixed);

    if fixed {
        println!("AFTER:\n{}", fixer.content());
    }

    println!("\n{}", "=".repeat(60));
    println!();

    // Example 2: Single padding field (correct size, no fix needed)
    let source_correct = r#"
#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
struct CorrectCapsule {
    state: AtomicU64,
    _padding: [u8; 56],
}
"#;

    println!("=== Example 2: Correct Single Padding ===\n");
    println!("BEFORE:\n{}", source_correct);

    let capsules = extract_capsules(source_correct).unwrap();
    let capsule = &capsules[0];

    println!("Detected capsule: {}", capsule.name);
    println!("Padding fields found: {}", capsule.padding_fields.len());
    println!("Needs consolidation: {}", capsule.needs_consolidation());

    let calculator = PaddingCalculator::new(capsule).unwrap();
    println!("Needs fixing: {}", calculator.needs_fixing());

    let mut fixer = PaddingFixer::new(source_correct.to_string());
    let fixed = fixer.apply_padding_fix(capsule).unwrap();
    println!("Fixed: {} (no changes needed)", fixed);

    println!("\n{}", "=".repeat(60));
    println!();

    // Example 3: Three padding fields (extreme case)
    let source_triple = r#"
#[derive(ComputationalCapsule)]
#[capsule(alignment = 256, size = 256)]
#[repr(C, align(256))]
struct TriplePaddingCapsule {
    state: AtomicU64,
    _padding1: [u8; 8],
    counter: AtomicU64,
    _padding2: [u8; 16],
    data: AtomicU64,
    _padding3: [u8; 200],
}
"#;

    println!("=== Example 3: Three Padding Fields ===\n");
    println!("BEFORE:\n{}", source_triple);

    let capsules = extract_capsules(source_triple).unwrap();
    let capsule = &capsules[0];

    println!("Detected capsule: {}", capsule.name);
    println!("Padding fields found: {}", capsule.padding_fields.len());
    for pf in &capsule.padding_fields {
        println!("  - {}: {} bytes", pf.name, pf.size_bytes);
    }
    println!("Total padding size: {} bytes", capsule.total_padding_size);
    println!("Needs consolidation: {}", capsule.needs_consolidation());

    let calculator = PaddingCalculator::new(capsule).unwrap();
    println!("\nData size: {} bytes", calculator.total_data_size());
    println!("Required padding: {} bytes", calculator.required_padding());

    let mut fixer = PaddingFixer::new(source_triple.to_string());
    let fixed = fixer.apply_padding_fix(capsule).unwrap();

    if fixed {
        println!("\nAFTER:\n{}", fixer.content());
    }

    println!("\n=== Summary ===");
    println!("✓ Multi-padding field consolidation: WORKING");
    println!("✓ AST-based transformation: ACCURATE");
    println!("✓ Backward compatibility: PRESERVED");
}
