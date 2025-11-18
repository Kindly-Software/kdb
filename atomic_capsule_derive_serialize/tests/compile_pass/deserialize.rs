//! Compile-pass test for #[derive(CapsuleDeserialize)]
//!
//! This test verifies that the macro generates valid code for:
//! - Named fields struct
//! - Tuple struct
//! - Unit struct (empty)

use atomic_capsule_derive_serialize::CapsuleDeserialize;

// Test 1: Named fields struct
#[derive(CapsuleDeserialize)]
#[repr(C, align(128))]
struct NamedFieldsStruct {
    field1: i64,
    field2: i64,
    field3: u64,
}

// Test 2: Tuple struct with multiple fields
#[derive(CapsuleDeserialize)]
#[repr(C, align(64))]
struct TupleStruct(i64, u64, i32);

// Test 3: Unit struct
#[derive(CapsuleDeserialize)]
#[repr(C, align(64))]
struct UnitStruct;

// Test 4: Single field struct
#[derive(CapsuleDeserialize)]
#[repr(C, align(256))]
struct SingleFieldStruct {
    value: i64,
}

fn main() {
    // Dummy usage to ensure structs are valid
    let _ = NamedFieldsStruct {
        field1: 0,
        field2: 0,
        field3: 0,
    };
    let _ = TupleStruct(0, 0, 0);
    let _ = UnitStruct;
    let _ = SingleFieldStruct { value: 0 };
}
