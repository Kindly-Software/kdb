// Copyright (C) 2025 Kindly Platform, Inc.
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! T28 Integration Tests for Text Widgets
//!
//! # Test Coverage
//!
//! - Q1-Q7: Unit tests (correctness, bounds, UTF-8 handling)
//! - Q8-Q14: Property tests (invariants, concurrency)
//! - Q15-Q21: Integration tests (widget composition)

#![cfg(feature = "gui")]
#![allow(unused_imports)]

use atomic_capsule::gui::widgets::text::{FontWeight, LabelCapsule, TextAlign, TextCapsule};
use atomic_capsule::gui::Rect;
use std::sync::Arc;
use std::thread;

/// Q1: Label creation and basic operations
#[test]
fn q1_label_creation() {
    let bounds = Rect::new(10, 10, 200, 30).unwrap();
    let label = LabelCapsule::new(1, "Hello, World!", bounds);

    assert_eq!(label.id(), 1);
    assert_eq!(label.text(), "Hello, World!");
    assert_eq!(label.generation(), 0);
    assert_eq!(label.bounds().x.to_int(), 10);
    assert_eq!(label.bounds().y.to_int(), 10);
}

/// Q2: Label text updates
#[test]
fn q2_label_set_text() {
    let bounds = Rect::new(0, 0, 100, 20).unwrap();
    let label = LabelCapsule::new(1, "Initial", bounds);

    assert_eq!(label.text(), "Initial");
    assert_eq!(label.generation(), 0);

    label.set_text("Updated");
    assert_eq!(label.text(), "Updated");
    assert_eq!(label.generation(), 1);

    label.set_text("Final");
    assert_eq!(label.text(), "Final");
    assert_eq!(label.generation(), 2);
}

/// Q3: Label truncates long text
#[test]
fn q3_label_truncate_long_text() {
    let bounds = Rect::new(0, 0, 500, 50).unwrap();
    let long_text = "A".repeat(200);
    let label = LabelCapsule::new(1, &long_text, bounds);

    // Should truncate to 128 bytes
    assert_eq!(label.text().len(), LabelCapsule::MAX_TEXT_LEN);
    assert_eq!(label.text(), "A".repeat(128));
}

/// Q4: Label style updates
#[test]
fn q4_label_style() {
    let bounds = Rect::new(0, 0, 100, 20).unwrap();
    let label = LabelCapsule::new(1, "Styled", bounds);

    // Default style
    assert_eq!(label.font_size(), 12);
    assert_eq!(label.font_weight(), FontWeight::Normal);
    assert_eq!(label.text_align(), TextAlign::Left);
    assert_eq!(label.color_index(), 0);

    // Update style
    label.set_style(16, FontWeight::Bold, TextAlign::Center, 5);
    assert_eq!(label.font_size(), 16);
    assert_eq!(label.font_weight(), FontWeight::Bold);
    assert_eq!(label.text_align(), TextAlign::Center);
    assert_eq!(label.color_index(), 5);
    assert_eq!(label.generation(), 1);
}

/// Q5: Label UTF-8 support
#[test]
fn q5_label_utf8() {
    let bounds = Rect::new(0, 0, 200, 40).unwrap();
    let label = LabelCapsule::new(1, "Hello 世界 🌍", bounds);
    assert_eq!(label.text(), "Hello 世界 🌍");

    label.set_text("こんにちは");
    assert_eq!(label.text(), "こんにちは");

    label.set_text("Привет мир");
    assert_eq!(label.text(), "Привет мир");
}

/// Q6: TextCapsule creation
#[test]
fn q6_text_capsule_creation() {
    let bounds = Rect::new(10, 10, 300, 50).unwrap();
    let text = TextCapsule::new(1, bounds);

    assert_eq!(text.id(), 1);
    assert_eq!(text.run_count(), 0);
    assert_eq!(text.generation(), 0);
}

/// Q7: TextCapsule add runs
#[test]
fn q7_text_capsule_add_run() {
    let bounds = Rect::new(0, 0, 200, 30).unwrap();
    let mut text = TextCapsule::new(1, bounds);

    assert!(text.add_run("Bold ", FontWeight::Bold, 12));
    assert_eq!(text.run_count(), 1);
    assert_eq!(text.generation(), 1);

    assert!(text.add_run("Normal", FontWeight::Normal, 12));
    assert_eq!(text.run_count(), 2);
    assert_eq!(text.generation(), 2);

    let run0 = text.get_run(0).unwrap();
    assert_eq!(run0.text(), "Bold ");

    let run1 = text.get_run(1).unwrap();
    assert_eq!(run1.text(), "Normal");
}

/// Q8: Property test - Label text length invariant
#[test]
fn q8_label_text_length_invariant() {
    let bounds = Rect::new(0, 0, 100, 20).unwrap();

    // Test various text lengths
    for len in [0, 1, 10, 50, 127, 128, 200] {
        let text = "X".repeat(len);
        let label = LabelCapsule::new(1, &text, bounds);

        // Invariant: text length never exceeds MAX_TEXT_LEN
        assert!(label.text().len() <= LabelCapsule::MAX_TEXT_LEN);

        // If input ≤ 128, no truncation
        if len <= 128 {
            assert_eq!(label.text().len(), len);
        } else {
            assert_eq!(label.text().len(), 128);
        }
    }
}

/// Q9: Property test - Generation monotonically increases
#[test]
fn q9_label_generation_monotonic() {
    let bounds = Rect::new(0, 0, 100, 20).unwrap();
    let label = LabelCapsule::new(1, "Test", bounds);

    let gen0 = label.generation();

    label.set_text("Update 1");
    let gen1 = label.generation();
    assert!(gen1 > gen0);

    label.set_text("Update 2");
    let gen2 = label.generation();
    assert!(gen2 > gen1);

    label.set_style(14, FontWeight::Bold, TextAlign::Center, 0);
    let gen3 = label.generation();
    assert!(gen3 > gen2);
}

/// Q10: Property test - TextCapsule run count bounded
#[test]
fn q10_text_capsule_run_count_bounded() {
    let bounds = Rect::new(0, 0, 200, 50).unwrap();
    let mut text = TextCapsule::new(1, bounds);

    // Add more runs than capacity
    for i in 0..20 {
        text.add_run(&format!("Run {}", i), FontWeight::Normal, 12);
    }

    // Invariant: run count never exceeds MAX_RUNS
    assert!(text.run_count() <= TextCapsule::MAX_RUNS);
    assert_eq!(text.run_count(), 8);
}

/// Q11: Property test - TextCapsule total length
#[test]
fn q11_text_capsule_total_length_property() {
    let bounds = Rect::new(0, 0, 200, 30).unwrap();
    let mut text = TextCapsule::new(1, bounds);

    text.add_run("ABC", FontWeight::Normal, 12); // 3 bytes
    text.add_run("12345", FontWeight::Bold, 14); // 5 bytes
    text.add_run("XY", FontWeight::Normal, 12); // 2 bytes

    // Total length = sum of run lengths
    let mut expected_len = 0;
    for i in 0..text.run_count() {
        expected_len += text.get_run(i).unwrap().text().len();
    }
    assert_eq!(text.total_text_len(), expected_len);
    assert_eq!(text.total_text_len(), 10); // 3 + 5 + 2
}

/// Q12: Concurrency test - Label concurrent updates
#[test]
fn q12_label_concurrent_update() {
    let bounds = Rect::new(0, 0, 100, 20).unwrap();
    let label: Arc<LabelCapsule> = Arc::new(LabelCapsule::new(1, "Initial", bounds));

    let label_clone: Arc<LabelCapsule> = Arc::clone(&label);
    let handle = thread::spawn(move || {
        for i in 0..50 {
            label_clone.set_text(&format!("Update {}", i));
        }
    });

    for i in 0..50 {
        label.set_text(&format!("Main {}", i));
    }

    handle.join().unwrap();

    // Final generation should be 100 (50 updates per thread)
    assert_eq!(label.generation(), 100);
}

/// Q13: Concurrency test - TextCapsule concurrent reads
#[test]
fn q13_text_capsule_concurrent_read() {
    let bounds = Rect::new(0, 0, 300, 50).unwrap();
    let mut text = TextCapsule::new(1, bounds);

    text.add_run("Hello ", FontWeight::Normal, 12);
    text.add_run("World", FontWeight::Bold, 14);
    text.add_run("!", FontWeight::Normal, 12);

    let text_arc: Arc<TextCapsule> = Arc::new(text);
    let mut handles = vec![];

    // Spawn 10 reader threads
    for _ in 0..10 {
        let text_clone: Arc<TextCapsule> = Arc::clone(&text_arc);
        let handle = thread::spawn(move || {
            for _ in 0..100 {
                let count = text_clone.run_count();
                assert_eq!(count, 3);

                let rendered = text_clone.render_to_string().unwrap();
                assert_eq!(rendered, "Hello World!");
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }
}

/// Q14: Integration test - TextCapsule clear operation
#[test]
fn q14_text_capsule_clear() {
    let bounds = Rect::new(0, 0, 200, 30).unwrap();
    let mut text = TextCapsule::new(1, bounds);

    text.add_run("Run 1", FontWeight::Normal, 12);
    text.add_run("Run 2", FontWeight::Bold, 14);
    text.add_run("Run 3", FontWeight::Normal, 12);
    assert_eq!(text.run_count(), 3);
    assert_eq!(text.generation(), 3);

    text.clear();
    assert_eq!(text.run_count(), 0);
    assert_eq!(text.generation(), 4); // 3 adds + 1 clear

    // Can add runs after clear
    text.add_run("New Run", FontWeight::Normal, 12);
    assert_eq!(text.run_count(), 1);
    assert_eq!(text.generation(), 5);
}

/// Q15: Integration test - TextCapsule render to string
#[test]
fn q15_text_capsule_render() {
    let bounds = Rect::new(0, 0, 300, 50).unwrap();
    let mut text = TextCapsule::new(1, bounds);

    text.add_run("Hello ", FontWeight::Normal, 12);
    text.add_run("World", FontWeight::Bold, 14);
    text.add_run("!", FontWeight::Normal, 12);

    let rendered = text.render_to_string().unwrap();
    assert_eq!(rendered, "Hello World!");
}

/// Q16: Integration test - TextCapsule max runs
#[test]
fn q16_text_capsule_max_runs() {
    let bounds = Rect::new(0, 0, 400, 100).unwrap();
    let mut text = TextCapsule::new(1, bounds);

    // Add 8 runs (max capacity)
    for i in 0..8 {
        assert!(text.add_run(&format!("Run{}", i), FontWeight::Normal, 12));
    }
    assert_eq!(text.run_count(), 8);

    // Try to add 9th run (should fail)
    assert!(!text.add_run("Overflow", FontWeight::Normal, 12));
    assert_eq!(text.run_count(), 8);

    // Generation should be 8 (8 successful adds, 1 failed add doesn't increment)
    assert_eq!(text.generation(), 8);
}

/// Q17: Integration test - FontWeight CSS conversions
#[test]
fn q17_font_weight_conversions() {
    assert_eq!(FontWeight::Normal.to_css_value(), 400);
    assert_eq!(FontWeight::Bold.to_css_value(), 700);
    assert_eq!(FontWeight::Thin.to_css_value(), 100);
    assert_eq!(FontWeight::Black.to_css_value(), 900);

    assert_eq!(FontWeight::from_css_value(400), FontWeight::Normal);
    assert_eq!(FontWeight::from_css_value(700), FontWeight::Bold);
    assert_eq!(FontWeight::from_css_value(100), FontWeight::Thin);
    assert_eq!(FontWeight::from_css_value(999), FontWeight::Black);
}

/// Q18: Stress test - Many label updates
#[test]
fn q18_label_stress_test() {
    let bounds = Rect::new(0, 0, 100, 20).unwrap();
    let label = LabelCapsule::new(1, "Start", bounds);

    for i in 0..1000 {
        label.set_text(&format!("Update {}", i));
    }

    assert_eq!(label.generation(), 1000);
    assert_eq!(label.text(), "Update 999");
}

/// Q19: Stress test - Many TextCapsule operations
#[test]
fn q19_text_capsule_stress_test() {
    let bounds = Rect::new(0, 0, 200, 50).unwrap();
    let mut text = TextCapsule::new(1, bounds);

    for _ in 0..100 {
        // Fill up runs
        for i in 0..8 {
            text.add_run(&format!("R{}", i), FontWeight::Normal, 12);
        }

        // Clear
        text.clear();
    }

    // Should be empty after 100 cycles
    assert_eq!(text.run_count(), 0);

    // Generation: 100 cycles × (8 adds + 1 clear) = 900
    assert_eq!(text.generation(), 900);
}

/// Q20: Edge case - Empty text
#[test]
fn q20_edge_case_empty_text() {
    let bounds = Rect::new(0, 0, 100, 20).unwrap();
    let label = LabelCapsule::new(1, "", bounds);

    assert_eq!(label.text(), "");
    assert_eq!(label.text().len(), 0);

    label.set_text("Non-empty");
    assert_eq!(label.text(), "Non-empty");

    label.set_text("");
    assert_eq!(label.text(), "");
}

/// Q21: Edge case - TextCapsule empty render
#[test]
fn q21_text_capsule_empty_render() {
    let bounds = Rect::new(0, 0, 200, 30).unwrap();
    let text = TextCapsule::new(1, bounds);

    let rendered = text.render_to_string().unwrap();
    assert_eq!(rendered, "");
    assert_eq!(text.total_text_len(), 0);
}
