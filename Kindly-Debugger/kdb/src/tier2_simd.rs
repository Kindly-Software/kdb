//! T2 SIMD Tier Components
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

#[repr(C, align(64))]
pub struct StackFrame {
    pub rip: AtomicU64,
    pub rbp: AtomicU64,
    pub rsp: AtomicU64,
    pub symbol_id: AtomicU32,
    pub depth: AtomicU32,
    pub frame_size: AtomicU32,
    _padding: [u8; 256 - 3 * 8 - 3 * 4 - 4],
}

impl StackFrame {
    pub const fn empty() -> Self {
        Self {
            rip: AtomicU64::new(0),
            rbp: AtomicU64::new(0),
            rsp: AtomicU64::new(0),
            symbol_id: AtomicU32::new(0),
            depth: AtomicU32::new(0),
            frame_size: AtomicU32::new(0),
            _padding: [0; 256 - 3 * 8 - 3 * 4 - 4],
        }
    }

    pub fn set(&self, rip: u64, rbp: u64, rsp: u64, depth: u32) {
        self.rip.store(rip, Ordering::Release);
        self.rbp.store(rbp, Ordering::Release);
        self.rsp.store(rsp, Ordering::Release);
        self.depth.store(depth, Ordering::Release);
    }
}

#[repr(C, align(256))]
pub struct SimdStackFrameCapsule {
    pub depth: AtomicU32,
    pub max_depth: AtomicU32,
    pub frames: [StackFrame; 256],
}

impl SimdStackFrameCapsule {
    pub fn new() -> Self {
        const EMPTY: StackFrame = StackFrame::empty();
        Self {
            depth: AtomicU32::new(0),
            max_depth: AtomicU32::new(0),
            frames: [EMPTY; 256],
        }
    }

    pub fn push_frame(&self, rip: u64, rbp: u64, rsp: u64) -> Result<(), &'static str> {
        let depth = self.depth.load(Ordering::Acquire);
        if depth >= 256 {
            return Err("Stack overflow");
        }

        self.frames[depth as usize].set(rip, rbp, rsp, depth);
        self.depth.store(depth + 1, Ordering::Release);

        let max = self.max_depth.load(Ordering::Relaxed);
        if depth + 1 > max {
            self.max_depth.store(depth + 1, Ordering::Relaxed);
        }

        Ok(())
    }

    pub fn pop_frame(&self) -> Result<(), &'static str> {
        let depth = self.depth.load(Ordering::Acquire);
        if depth == 0 {
            return Err("Stack underflow");
        }
        self.depth.store(depth - 1, Ordering::Release);
        Ok(())
    }

    pub fn get_depth(&self) -> u32 {
        self.depth.load(Ordering::Acquire)
    }

    pub fn collect_trace_simd(&self) -> Vec<u64> {
        let depth = self.depth.load(Ordering::Acquire) as usize;
        let mut trace = Vec::with_capacity(depth);

        for i in 0..depth {
            trace.push(self.frames[i].rip.load(Ordering::Relaxed));
        }

        trace
    }
}

#[repr(C, align(64))]
pub struct SymbolEntry {
    pub start_addr: AtomicU64,
    pub end_addr: AtomicU64,
    pub name_hash: AtomicU64,
    pub symbol_id: AtomicU32,
    pub symbol_type: AtomicU32,
    pub file_id: AtomicU32,
    pub line_number: AtomicU32,
    _padding: [u8; 256 - 3 * 8 - 4 * 4 - 8],
}

impl SymbolEntry {
    pub const fn empty() -> Self {
        Self {
            start_addr: AtomicU64::new(0),
            end_addr: AtomicU64::new(0),
            name_hash: AtomicU64::new(0),
            symbol_id: AtomicU32::new(0),
            symbol_type: AtomicU32::new(0),
            file_id: AtomicU32::new(0),
            line_number: AtomicU32::new(0),
            _padding: [0; 256 - 3 * 8 - 4 * 4 - 8],
        }
    }

    pub fn set(&self, start_addr: u64, end_addr: u64, name_hash: u64, symbol_id: u32) {
        self.start_addr.store(start_addr, Ordering::Release);
        self.end_addr.store(end_addr, Ordering::Release);
        self.name_hash.store(name_hash, Ordering::Release);
        self.symbol_id.store(symbol_id, Ordering::Release);
    }

    pub fn contains(&self, addr: u64) -> bool {
        let start = self.start_addr.load(Ordering::Relaxed);
        let end = self.end_addr.load(Ordering::Relaxed);
        addr >= start && addr < end
    }
}

#[repr(C, align(256))]
pub struct SimdSymbolTableCapsule {
    pub count: AtomicU32,
    _align_padding: [u8; 256 - 4 - 4],
    pub symbols: [SymbolEntry; 256],
}

impl SimdSymbolTableCapsule {
    pub fn new() -> Self {
        const EMPTY: SymbolEntry = SymbolEntry::empty();
        Self {
            count: AtomicU32::new(0),
            _align_padding: [0; 256 - 4 - 4],
            symbols: [EMPTY; 256],
        }
    }

    pub fn add_symbol(
        &self,
        start_addr: u64,
        end_addr: u64,
        name_hash: u64,
    ) -> Result<u32, &'static str> {
        let count = self.count.load(Ordering::Acquire);
        if count >= 256 {
            return Err("Symbol table full");
        }

        let symbol_id = count;
        self.symbols[count as usize].set(start_addr, end_addr, name_hash, symbol_id);
        self.count.store(count + 1, Ordering::Release);

        Ok(symbol_id)
    }

    pub fn lookup_symbol_simd(&self, addr: u64) -> Option<u32> {
        let count = self.count.load(Ordering::Acquire) as usize;

        for i in 0..count {
            if self.symbols[i].contains(addr) {
                return Some(self.symbols[i].symbol_id.load(Ordering::Relaxed));
            }
        }

        None
    }
}
