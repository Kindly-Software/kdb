use std::mem::{size_of, align_of};

#[repr(C, align(64))]
pub struct ResponseBuilderCapsule {
    pub status_code: u64,
    pub body_len: u32,
    pub latency_ns: u64,
    pub generation: u64,
    pub response_flags: u32,
    pub error_code: u32,
    pub _padding: [u8; 8],
}

#[repr(C, align(64))]
pub struct SchemaValidatorToolCapsule {
    pub state: u64,
    pub generation: u64,
    pub response: ResponseBuilderCapsule,
    pub _reserved: [u8; 48],
}

fn main() {
    println!("ResponseBuilderCapsule: {} bytes, align {}", size_of::<ResponseBuilderCapsule>(), align_of::<ResponseBuilderCapsule>());
    println!("SchemaValidatorToolCapsule: {} bytes, align {}", size_of::<SchemaValidatorToolCapsule>(), align_of::<SchemaValidatorToolCapsule>());
    // Manual calc:
    // 0-7: state (u64)
    // 8-15: generation (u64)
    // 16-63: padding to align response to 64B boundary (48 bytes)
    // 64-127: response (ResponseBuilderCapsule, 64B)
    // 128-175: _reserved (48B)
    // Total: 176B
    
    // But wait, align(64) on the struct might force total size to be multiple of 64:
    // 176 % 64 = 48, so padded to 192
    println!("Expected: 16 + 48 (pad) + 64 + 48 = 176, padded to 192 with align(64)");
}
