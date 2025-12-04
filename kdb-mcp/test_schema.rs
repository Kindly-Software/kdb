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
}
