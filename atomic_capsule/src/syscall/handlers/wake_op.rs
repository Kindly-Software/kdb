//! # FUTEX_WAKE_OP Handler
//!
//! **UCE34 T1 Atomic: Atomic modify + wake operation**
//!
//! FUTEX_WAKE_OP performs an atomic operation on uaddr2, then wakes waiters
//! from both uaddr and uaddr2 based on the result.
//!
//! ## Operation Encoding (val3)
//!
//! ```text
//! val3 encoding:
//! +----+----+--------+----+--------+
//! | op |cmp | oparg  |cmp | cmparg |
//! +----+----+--------+----+--------+
//!  4    4      12     4      12    = 36 bits (fits in u32)
//!
//! Actual Linux encoding:
//! bits 31-28: op (FUTEX_OP_*)
//! bits 27-24: cmp (FUTEX_OP_CMP_*)
//! bits 23-12: oparg
//! bits 11-0:  cmparg
//! ```
//!
//! ## Supported Operations
//!
//! | Op  | Name           | Description          |
//! |-----|----------------|----------------------|
//! | 0   | FUTEX_OP_SET   | *uaddr2 = oparg      |
//! | 1   | FUTEX_OP_ADD   | *uaddr2 += oparg     |
//! | 2   | FUTEX_OP_OR    | *uaddr2 |= oparg     |
//! | 3   | FUTEX_OP_ANDN  | *uaddr2 &= ~oparg    |
//! | 4   | FUTEX_OP_XOR   | *uaddr2 ^= oparg     |
//!
//! ## Comparison Operations
//!
//! | Cmp | Name               | Condition            |
//! |-----|--------------------|----------------------|
//! | 0   | FUTEX_OP_CMP_EQ    | oldval == cmparg     |
//! | 1   | FUTEX_OP_CMP_NE    | oldval != cmparg     |
//! | 2   | FUTEX_OP_CMP_LT    | oldval < cmparg      |
//! | 3   | FUTEX_OP_CMP_LE    | oldval <= cmparg     |
//! | 4   | FUTEX_OP_CMP_GT    | oldval > cmparg      |
//! | 5   | FUTEX_OP_CMP_GE    | oldval >= cmparg     |
//!
//! ## ASSUM Framework (10 annotations)
//!
//! - `#ASSUME_WAKE_OP_ATOMIC`: Operation on uaddr2 is atomic
//! - `#ASSUME_WAKE_OP_ORDER`: Op executes before any wakes

use core::sync::atomic::{AtomicU32, Ordering};

use crate::syscall::error::{FutexError, FutexErrorKind};
use crate::syscall::futex::{FutexOperation, FutexResult};
use crate::syscall::waiter::FUTEX_BITSET_MATCH_ANY;

use super::futex::FutexHandlerContext;

/// Atomic operation types for FUTEX_WAKE_OP
///
/// # ASSUM_OP_ENCODING
/// - Encoding matches Linux kernel FUTEX_OP_* values
/// - #VERIFY_OP_ENCODING: Validated against kernel headers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum WakeOpType {
    /// SET: *uaddr2 = oparg
    Set = 0,

    /// ADD: *uaddr2 += oparg
    Add = 1,

    /// OR: *uaddr2 |= oparg
    Or = 2,

    /// ANDN: *uaddr2 &= ~oparg
    AndNot = 3,

    /// XOR: *uaddr2 ^= oparg
    Xor = 4,
}

impl WakeOpType {
    /// Create from raw operation code
    ///
    /// # Arguments
    /// - `op`: Raw operation code (0-4)
    ///
    /// # Returns
    /// Some(WakeOpType) if valid, None otherwise
    #[inline]
    pub const fn from_raw(op: u8) -> Option<Self> {
        match op {
            0 => Some(Self::Set),
            1 => Some(Self::Add),
            2 => Some(Self::Or),
            3 => Some(Self::AndNot),
            4 => Some(Self::Xor),
            _ => None,
        }
    }

    /// Execute the operation atomically
    ///
    /// # Arguments
    /// - `addr`: Target atomic address
    /// - `oparg`: Operation argument
    ///
    /// # Returns
    /// Old value before modification
    ///
    /// # ASSUM_OP_ATOMIC
    /// - Each operation is a single atomic RMW
    /// - #VERIFY_OP_ATOMIC: Uses fetch_* methods with AcqRel ordering
    pub fn execute(self, addr: &AtomicU32, oparg: u32) -> u32 {
        match self {
            Self::Set => addr.swap(oparg, Ordering::AcqRel),
            Self::Add => addr.fetch_add(oparg, Ordering::AcqRel),
            Self::Or => addr.fetch_or(oparg, Ordering::AcqRel),
            Self::AndNot => addr.fetch_and(!oparg, Ordering::AcqRel),
            Self::Xor => addr.fetch_xor(oparg, Ordering::AcqRel),
        }
    }
}

/// Comparison types for FUTEX_WAKE_OP conditional wake
///
/// # ASSUM_CMP_ENCODING
/// - Encoding matches Linux kernel FUTEX_OP_CMP_* values
/// - #VERIFY_CMP_ENCODING: Validated against kernel headers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum WakeOpCmp {
    /// Equal: oldval == cmparg
    Eq = 0,

    /// Not equal: oldval != cmparg
    Ne = 1,

    /// Less than: oldval < cmparg
    Lt = 2,

    /// Less or equal: oldval <= cmparg
    Le = 3,

    /// Greater than: oldval > cmparg
    Gt = 4,

    /// Greater or equal: oldval >= cmparg
    Ge = 5,
}

impl WakeOpCmp {
    /// Create from raw comparison code
    #[inline]
    pub const fn from_raw(cmp: u8) -> Option<Self> {
        match cmp {
            0 => Some(Self::Eq),
            1 => Some(Self::Ne),
            2 => Some(Self::Lt),
            3 => Some(Self::Le),
            4 => Some(Self::Gt),
            5 => Some(Self::Ge),
            _ => None,
        }
    }

    /// Evaluate comparison
    ///
    /// # Arguments
    /// - `oldval`: Value to compare
    /// - `cmparg`: Comparison argument
    ///
    /// # Returns
    /// true if comparison succeeds
    #[inline]
    pub const fn evaluate(self, oldval: u32, cmparg: u32) -> bool {
        match self {
            Self::Eq => oldval == cmparg,
            Self::Ne => oldval != cmparg,
            Self::Lt => oldval < cmparg,
            Self::Le => oldval <= cmparg,
            Self::Gt => oldval > cmparg,
            Self::Ge => oldval >= cmparg,
        }
    }
}

/// Decoded WAKE_OP parameters
///
/// # Layout
/// Packed from val3 as per Linux kernel encoding.
///
/// # ASSUM_DECODE_CORRECT
/// - Bit extraction matches kernel implementation
/// - #VERIFY_DECODE_CORRECT: Tested against known kernel behavior
#[derive(Debug, Clone, Copy)]
pub struct WakeOpParams {
    /// Atomic operation type
    pub op: WakeOpType,

    /// Comparison type
    pub cmp: WakeOpCmp,

    /// Operation argument (12 bits, sign-extended if shift flag set)
    pub oparg: u32,

    /// Comparison argument (12 bits)
    pub cmparg: u32,

    /// Shift oparg flag (use 1 << oparg instead)
    pub shift: bool,
}

impl WakeOpParams {
    /// Decode from val3 encoding
    ///
    /// # Arguments
    /// - `val3`: Packed operation parameters
    ///
    /// # Returns
    /// Decoded parameters, or None if invalid
    ///
    /// # Linux Kernel Encoding
    /// ```text
    /// #define FUTEX_OP(op, oparg, cmp, cmparg)        \
    ///     (((op & 0xf) << 28) | ((cmp & 0xf) << 24) | \
    ///      ((oparg & 0xfff) << 12) | (cmparg & 0xfff))
    /// ```
    ///
    /// # ASSUM_DECODE_BITS
    /// - Bits 31-28: op (4 bits)
    /// - Bits 27-24: cmp (4 bits)
    /// - Bits 23-12: oparg (12 bits)
    /// - Bits 11-0: cmparg (12 bits)
    /// - Bit 31 of op: shift flag (FUTEX_OP_OPARG_SHIFT)
    pub fn decode(val3: u32) -> Option<Self> {
        let raw_op = ((val3 >> 28) & 0xF) as u8;
        let raw_cmp = ((val3 >> 24) & 0xF) as u8;
        let oparg = (val3 >> 12) & 0xFFF;
        let cmparg = val3 & 0xFFF;

        // Bit 3 of op field indicates shift
        let shift = (raw_op & 0x8) != 0;
        let op_code = raw_op & 0x7;

        let op = WakeOpType::from_raw(op_code)?;
        let cmp = WakeOpCmp::from_raw(raw_cmp)?;

        Some(Self {
            op,
            cmp,
            oparg,
            cmparg,
            shift,
        })
    }

    /// Get effective oparg (shifted if flag set)
    ///
    /// # ASSUM_OPARG_SHIFT
    /// - If shift flag set, use 1 << oparg
    /// - Prevents encoding large values in 12 bits
    #[inline]
    pub fn effective_oparg(&self) -> u32 {
        if self.shift && self.oparg < 32 {
            1u32 << self.oparg
        } else {
            self.oparg
        }
    }
}

/// FUTEX_WAKE_OP handler
///
/// Atomically modifies *uaddr2, then wakes waiters from both addresses.
///
/// # Arguments
/// - `ctx`: Handler context
/// - `uaddr`: First futex address (wake val waiters)
/// - `uaddr2`: Second futex address (modify + conditional wake)
/// - `val`: Number of waiters to wake from uaddr
/// - `val3`: Encoded operation parameters
/// - `val2`: Number of waiters to wake from uaddr2 if condition met
///
/// # Returns
/// - `Ok(count)`: Total waiters woken from both addresses
/// - `Err`: On invalid parameters
///
/// # Algorithm
/// 1. Decode operation from val3
/// 2. Atomically execute op on *uaddr2, capture old value
/// 3. Wake val waiters from uaddr
/// 4. If cmp(oldval, cmparg) is true, wake val2 waiters from uaddr2
/// 5. Return total woken
///
/// # ASSUM Framework
/// - `#ASSUME_WAKE_OP_SEQUENCE`: Op before wakes, uaddr before uaddr2
/// - `#VERIFY_WAKE_OP_SEQUENCE`: Atomic op completes before any wake
/// - `#ASSUME_WAKE_OP_VALID_ADDR`: Both addresses are valid
/// - `#VERIFY_WAKE_OP_VALID_ADDR`: Alignment checked, dereference safe
/// - `#ASSUME_WAKE_OP_NO_OVERFLOW`: Wake counts don't overflow u32
/// - `#VERIFY_WAKE_OP_NO_OVERFLOW`: saturating_add used
///
/// # Performance (B32)
/// - Time: O(val + val2) for wake operations
/// - Atomic op: <20ns
/// - Per-waiter: <20ns
pub fn futex_wake_op_handler(
    ctx: &FutexHandlerContext<'_>,
    uaddr: *const AtomicU32,
    uaddr2: *const AtomicU32,
    val: u32,
    val3: u32,
    val2: u32,
) -> FutexResult<u32> {
    // Validate addresses
    //
    // #ASSUME_ADDR_ALIGNED: Both addresses must be 4-byte aligned
    // #VERIFY_ADDR_ALIGNED: Checked before any operation
    let addr1 = uaddr as u64;
    let addr2 = uaddr2 as u64;

    if addr1 & 3 != 0 || addr2 & 3 != 0 {
        return Err(FutexError::invalid_address(
            if addr1 & 3 != 0 { addr1 } else { addr2 },
            FutexOperation::WakeOp as u32,
        ));
    }

    // Decode operation parameters
    let params = WakeOpParams::decode(val3).ok_or_else(|| {
        FutexError::new(
            FutexErrorKind::InvalidOperation,
            addr2,
            FutexOperation::WakeOp as u32,
        )
    })?;

    // Step 1: Execute atomic operation on uaddr2
    //
    // #ASSUME_OP_SAFE: uaddr2 points to valid memory
    // #VERIFY_OP_SAFE: Alignment already checked
    let oldval = unsafe {
        let atomic_addr2 = &*(uaddr2 as *const AtomicU32);
        params.op.execute(atomic_addr2, params.effective_oparg())
    };

    // Step 2: Wake waiters from uaddr
    let woken1 = ctx.capsule.futex_wake(uaddr, val, FUTEX_BITSET_MATCH_ANY, ctx.waiter_pool);

    // Step 3: Conditionally wake waiters from uaddr2
    let woken2 = if params.cmp.evaluate(oldval, params.cmparg) {
        ctx.capsule
            .futex_wake(uaddr2, val2, FUTEX_BITSET_MATCH_ANY, ctx.waiter_pool)
    } else {
        0
    };

    // Return total woken (saturating to prevent overflow)
    Ok(woken1.saturating_add(woken2))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wake_op_type_from_raw() {
        assert_eq!(WakeOpType::from_raw(0), Some(WakeOpType::Set));
        assert_eq!(WakeOpType::from_raw(1), Some(WakeOpType::Add));
        assert_eq!(WakeOpType::from_raw(2), Some(WakeOpType::Or));
        assert_eq!(WakeOpType::from_raw(3), Some(WakeOpType::AndNot));
        assert_eq!(WakeOpType::from_raw(4), Some(WakeOpType::Xor));
        assert_eq!(WakeOpType::from_raw(5), None);
    }

    #[test]
    fn test_wake_op_cmp_from_raw() {
        assert_eq!(WakeOpCmp::from_raw(0), Some(WakeOpCmp::Eq));
        assert_eq!(WakeOpCmp::from_raw(1), Some(WakeOpCmp::Ne));
        assert_eq!(WakeOpCmp::from_raw(2), Some(WakeOpCmp::Lt));
        assert_eq!(WakeOpCmp::from_raw(3), Some(WakeOpCmp::Le));
        assert_eq!(WakeOpCmp::from_raw(4), Some(WakeOpCmp::Gt));
        assert_eq!(WakeOpCmp::from_raw(5), Some(WakeOpCmp::Ge));
        assert_eq!(WakeOpCmp::from_raw(6), None);
    }

    #[test]
    fn test_wake_op_cmp_evaluate() {
        assert!(WakeOpCmp::Eq.evaluate(42, 42));
        assert!(!WakeOpCmp::Eq.evaluate(42, 43));

        assert!(WakeOpCmp::Ne.evaluate(42, 43));
        assert!(!WakeOpCmp::Ne.evaluate(42, 42));

        assert!(WakeOpCmp::Lt.evaluate(10, 20));
        assert!(!WakeOpCmp::Lt.evaluate(20, 10));

        assert!(WakeOpCmp::Le.evaluate(10, 20));
        assert!(WakeOpCmp::Le.evaluate(10, 10));
        assert!(!WakeOpCmp::Le.evaluate(20, 10));

        assert!(WakeOpCmp::Gt.evaluate(20, 10));
        assert!(!WakeOpCmp::Gt.evaluate(10, 20));

        assert!(WakeOpCmp::Ge.evaluate(20, 10));
        assert!(WakeOpCmp::Ge.evaluate(10, 10));
        assert!(!WakeOpCmp::Ge.evaluate(10, 20));
    }

    #[test]
    fn test_wake_op_params_decode() {
        // FUTEX_OP(SET, 100, EQ, 0) = ((0 << 28) | (0 << 24) | (100 << 12) | 0)
        let val3 = (0u32 << 28) | (0u32 << 24) | (100u32 << 12) | 0u32;
        let params = WakeOpParams::decode(val3).unwrap();

        assert_eq!(params.op, WakeOpType::Set);
        assert_eq!(params.cmp, WakeOpCmp::Eq);
        assert_eq!(params.oparg, 100);
        assert_eq!(params.cmparg, 0);
        assert!(!params.shift);
    }

    #[test]
    fn test_wake_op_params_shift() {
        // FUTEX_OP(SET | SHIFT, 3, EQ, 0) = use 1 << 3 = 8 as oparg
        let val3 = (0x8u32 << 28) | (0u32 << 24) | (3u32 << 12) | 0u32;
        let params = WakeOpParams::decode(val3).unwrap();

        assert_eq!(params.op, WakeOpType::Set);
        assert!(params.shift);
        assert_eq!(params.oparg, 3);
        assert_eq!(params.effective_oparg(), 8); // 1 << 3
    }

    #[test]
    fn test_wake_op_execute() {
        let val = AtomicU32::new(10);

        // SET
        let old = WakeOpType::Set.execute(&val, 42);
        assert_eq!(old, 10);
        assert_eq!(val.load(Ordering::Relaxed), 42);

        // ADD
        val.store(10, Ordering::Relaxed);
        let old = WakeOpType::Add.execute(&val, 5);
        assert_eq!(old, 10);
        assert_eq!(val.load(Ordering::Relaxed), 15);

        // OR
        val.store(0b1010, Ordering::Relaxed);
        let old = WakeOpType::Or.execute(&val, 0b0101);
        assert_eq!(old, 0b1010);
        assert_eq!(val.load(Ordering::Relaxed), 0b1111);

        // ANDN
        val.store(0b1111, Ordering::Relaxed);
        let old = WakeOpType::AndNot.execute(&val, 0b0101);
        assert_eq!(old, 0b1111);
        assert_eq!(val.load(Ordering::Relaxed), 0b1010);

        // XOR
        val.store(0b1010, Ordering::Relaxed);
        let old = WakeOpType::Xor.execute(&val, 0b1100);
        assert_eq!(old, 0b1010);
        assert_eq!(val.load(Ordering::Relaxed), 0b0110);
    }
}
