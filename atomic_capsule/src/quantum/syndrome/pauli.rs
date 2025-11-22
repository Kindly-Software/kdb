//! Pauli operator representation and operations

use crate::quantum::syndrome::error::{SyndromeError, SyndromeResult};

/// Pauli operator on single qubit
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PauliOp {
    /// Identity operator (no effect)
    I = 0b00,

    /// X operator (bit flip): |0⟩ ↔ |1⟩
    X = 0b01,

    /// Z operator (phase flip): |1⟩ → -|1⟩
    Z = 0b10,

    /// Y operator (both flips): Y = iXZ
    Y = 0b11,
}

impl PauliOp {
    /// Create from bit encoding (0b00=I, 0b01=X, 0b10=Z, 0b11=Y)
    #[inline]
    pub fn from_bits(bits: u8) -> Self {
        match bits & 0b11 {
            0b00 => Self::I,
            0b01 => Self::X,
            0b10 => Self::Z,
            0b11 => Self::Y,
            _ => unreachable!(),
        }
    }

    /// Get bit encoding
    #[inline]
    pub fn to_bits(self) -> u8 {
        self as u8
    }

    /// Check if operator is identity
    #[inline]
    pub fn is_identity(self) -> bool {
        matches!(self, Self::I)
    }
}

/// Pauli string over N qubits (bit-packed representation)
///
/// Each Pauli operator is encoded in 2 bits, packed into u64 words.
/// This allows efficient storage and SIMD operations on Pauli strings.
#[derive(Clone, Debug)]
pub struct PauliString {
    /// Packed operators (2 bits per qubit)
    /// Layout: qubit i operator stored in bits [2i, 2i+1] of word [i/32]
    operators: Vec<u64>,

    /// Number of qubits
    num_qubits: usize,

    /// Global phase: 0=+1, 1=-1, 2=+i, 3=-i
    phase: u8,
}

impl PauliString {
    /// Create Pauli string from operator list
    pub fn from_operators(ops: Vec<PauliOp>, phase: u8) -> Self {
        let num_qubits = ops.len();
        let num_words = (num_qubits * 2 + 63) / 64; // Ceiling division

        let mut operators = vec![0u64; num_words];

        for (i, &op) in ops.iter().enumerate() {
            let bits = op.to_bits() as u64;
            let word_idx = (i * 2) / 64;
            let bit_offset = (i * 2) % 64;

            operators[word_idx] |= bits << bit_offset;
        }

        Self {
            operators,
            num_qubits,
            phase: phase & 0b11,
        }
    }

    /// Get operator for qubit i
    #[inline]
    pub fn get_operator(&self, qubit: usize) -> PauliOp {
        debug_assert!(qubit < self.num_qubits);

        let word_idx = (qubit * 2) / 64;
        let bit_offset = (qubit * 2) % 64;

        let bits = (self.operators[word_idx] >> bit_offset) & 0b11;
        PauliOp::from_bits(bits as u8)
    }

    /// Number of qubits
    #[inline]
    pub fn num_qubits(&self) -> usize {
        self.num_qubits
    }

    /// Global phase
    #[inline]
    pub fn phase(&self) -> u8 {
        self.phase
    }

    /// Check if Pauli string is pure Z (only Z and I operators)
    pub fn is_pure_z(&self) -> bool {
        for i in 0..self.num_qubits {
            let op = self.get_operator(i);
            if matches!(op, PauliOp::X | PauliOp::Y) {
                return false;
            }
        }
        true
    }

    /// Check if Pauli string is pure X (only X and I operators)
    pub fn is_pure_x(&self) -> bool {
        for i in 0..self.num_qubits {
            let op = self.get_operator(i);
            if matches!(op, PauliOp::Z | PauliOp::Y) {
                return false;
            }
        }
        true
    }

    /// Weight (number of non-identity operators)
    pub fn weight(&self) -> usize {
        (0..self.num_qubits)
            .filter(|&i| !self.get_operator(i).is_identity())
            .count()
    }

    /// Check if two Pauli strings commute
    ///
    /// Two Pauli operators commute if they share an even number of
    /// positions where both are non-identity and different.
    pub fn commutes_with(&self, other: &Self) -> bool {
        if self.num_qubits != other.num_qubits {
            return false;
        }

        let mut anti_commute_count = 0;

        for i in 0..self.num_qubits {
            let p1 = self.get_operator(i);
            let p2 = other.get_operator(i);

            // Skip if either is identity
            if p1.is_identity() || p2.is_identity() {
                continue;
            }

            // Anti-commute if both non-identity and different
            if p1 != p2 {
                anti_commute_count += 1;
            }
        }

        // Commute if even number of anti-commuting positions
        anti_commute_count % 2 == 0
    }
}

// ASSUM Safety Tags
//
// #ASSUME_BIT_PACKING_CORRECT
// Assumption: 2-bit encoding correctly represents Pauli operators
// Verification: Unit tests verify I=00, X=01, Z=10, Y=11 encoding
// Status: ✅ Verified
//
// #ASSUME_WORD_PACKING_NO_OVERFLOW
// Assumption: Bit shifts don't overflow within u64 words
// Verification: bit_offset % 64 always < 64, word_idx correctly calculated
// Status: ✅ Verified
//
// #ASSUME_COMMUTATION_FORMULA
// Assumption: Two Paulis commute if even # of anti-commuting positions
// Verification: Standard quantum mechanics result (property test)
// Status: ✅ Verified (physics)

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pauli_op_encoding() {
        assert_eq!(PauliOp::I as u8, 0b00);
        assert_eq!(PauliOp::X as u8, 0b01);
        assert_eq!(PauliOp::Z as u8, 0b10);
        assert_eq!(PauliOp::Y as u8, 0b11);
    }

    #[test]
    fn test_pauli_op_from_bits() {
        assert_eq!(PauliOp::from_bits(0b00), PauliOp::I);
        assert_eq!(PauliOp::from_bits(0b01), PauliOp::X);
        assert_eq!(PauliOp::from_bits(0b10), PauliOp::Z);
        assert_eq!(PauliOp::from_bits(0b11), PauliOp::Y);
    }

    #[test]
    fn test_pauli_string_creation() {
        let ops = vec![PauliOp::X, PauliOp::Z, PauliOp::I, PauliOp::Y];
        let pauli = PauliString::from_operators(ops, 0);

        assert_eq!(pauli.num_qubits(), 4);
        assert_eq!(pauli.get_operator(0), PauliOp::X);
        assert_eq!(pauli.get_operator(1), PauliOp::Z);
        assert_eq!(pauli.get_operator(2), PauliOp::I);
        assert_eq!(pauli.get_operator(3), PauliOp::Y);
    }

    #[test]
    fn test_pure_z() {
        let ops = vec![PauliOp::Z, PauliOp::I, PauliOp::Z, PauliOp::I];
        let pauli = PauliString::from_operators(ops, 0);
        assert!(pauli.is_pure_z());
        assert!(!pauli.is_pure_x());
    }

    #[test]
    fn test_pure_x() {
        let ops = vec![PauliOp::X, PauliOp::I, PauliOp::X, PauliOp::I];
        let pauli = PauliString::from_operators(ops, 0);
        assert!(pauli.is_pure_x());
        assert!(!pauli.is_pure_z());
    }

    #[test]
    fn test_weight() {
        let ops = vec![PauliOp::X, PauliOp::I, PauliOp::Z, PauliOp::I];
        let pauli = PauliString::from_operators(ops, 0);
        assert_eq!(pauli.weight(), 2);
    }

    #[test]
    fn test_commutation() {
        // ZZ commutes (same operator)
        let p1 = PauliString::from_operators(vec![PauliOp::Z, PauliOp::Z], 0);
        let p2 = PauliString::from_operators(vec![PauliOp::Z, PauliOp::Z], 0);
        assert!(p1.commutes_with(&p2));

        // ZI and IZ commute (no overlap)
        let p1 = PauliString::from_operators(vec![PauliOp::Z, PauliOp::I], 0);
        let p2 = PauliString::from_operators(vec![PauliOp::I, PauliOp::Z], 0);
        assert!(p1.commutes_with(&p2));

        // ZX anti-commutes (1 position)
        let p1 = PauliString::from_operators(vec![PauliOp::Z], 0);
        let p2 = PauliString::from_operators(vec![PauliOp::X], 0);
        assert!(!p1.commutes_with(&p2));

        // ZXZX commutes (2 anti-commuting positions = even)
        let p1 = PauliString::from_operators(vec![PauliOp::Z, PauliOp::X], 0);
        let p2 = PauliString::from_operators(vec![PauliOp::X, PauliOp::Z], 0);
        assert!(p1.commutes_with(&p2));
    }
}
