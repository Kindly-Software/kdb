//! QuantumGateCapsule - Standard quantum gates

use crate::quantum_pure::error::{QuantumPureError, QuantumPureResult};
use crate::quantum_pure::state_vector::Complex;
use std::sync::atomic::{AtomicU8, AtomicU32, Ordering};

/// Gate types supported in Phase 1
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GateType {
    Hadamard = 0,
    PauliX = 1,
    PauliY = 2,
    PauliZ = 3,
    SGate = 4,
    TGate = 5,
    CNOT = 6, // Phase 2: Multi-qubit gates
    Custom = 7,
}

/// T1 Atomic: Quantum Gate Capsule (128B cache-aligned)
///
/// # Architecture
///
/// - **Metadata** (12 bytes): Gate type, target qubit, control qubit
/// - **Matrix** (64 bytes): 2×2 unitary matrix (4 complex = 8 f64)
/// - **Padding** (52 bytes): Cache alignment to 128 bytes
///
/// # Standard Gates
///
/// - **Hadamard**: H = (1/√2) [[1,  1], [1, -1]]
/// - **Pauli-X**: X = [[0, 1], [1, 0]]
/// - **Pauli-Y**: Y = [[0, -i], [i, 0]]
/// - **Pauli-Z**: Z = [[1, 0], [0, -1]]
/// - **S Gate**: S = [[1, 0], [0, i]]
/// - **T Gate**: T = [[1, 0], [0, e^(iπ/4)]]
#[repr(C, align(128))]
pub struct QuantumGateCapsule {
    /// Gate type (0-6, 7=custom)
    gate_type: AtomicU8,

    /// Target qubit index
    target_qubit: AtomicU32,

    /// Control qubit index (u32::MAX if not controlled)
    control_qubit: AtomicU32,

    /// 2×2 unitary matrix (stored as [a, b, c, d] row-major)
    /// [[a, b],
    ///  [c, d]]
    matrix: [[Complex; 2]; 2],

    /// Padding to 128 bytes
    _padding: [u8; 48],
}

// Manual verification
impl QuantumGateCapsule {
    const _VERIFY: () = {
        assert!(
            std::mem::size_of::<Self>() == 128,
            "QuantumGateCapsule must be 128 bytes"
        );
        assert!(
            std::mem::align_of::<Self>() == 128,
            "QuantumGateCapsule must be 128-byte aligned"
        );
    };
}

// Manual Clone implementation (AtomicU8/U32 don't implement Clone)
impl Clone for QuantumGateCapsule {
    fn clone(&self) -> Self {
        Self {
            gate_type: AtomicU8::new(self.gate_type.load(Ordering::Relaxed)),
            target_qubit: AtomicU32::new(self.target_qubit.load(Ordering::Relaxed)),
            control_qubit: AtomicU32::new(self.control_qubit.load(Ordering::Relaxed)),
            matrix: self.matrix,
            _padding: self._padding,
        }
    }
}

impl QuantumGateCapsule {
    /// Create Hadamard gate: H = (1/√2) [[1, 1], [1, -1]]
    ///
    /// Creates uniform superposition: H|0⟩ = (|0⟩+|1⟩)/√2
    pub fn hadamard(target: usize) -> Self {
        let inv_sqrt2 = std::f64::consts::FRAC_1_SQRT_2;
        let matrix = [
            [Complex::real(inv_sqrt2), Complex::real(inv_sqrt2)],
            [Complex::real(inv_sqrt2), Complex::real(-inv_sqrt2)],
        ];

        Self {
            gate_type: AtomicU8::new(GateType::Hadamard as u8),
            target_qubit: AtomicU32::new(target as u32),
            control_qubit: AtomicU32::new(u32::MAX),
            matrix,
            _padding: [0; 48],
        }
    }

    /// Create Pauli-X gate: X = [[0, 1], [1, 0]]
    ///
    /// Quantum NOT (bit-flip): X|0⟩ = |1⟩, X|1⟩ = |0⟩
    pub fn pauli_x(target: usize) -> Self {
        let matrix = [
            [Complex::real(0.0), Complex::real(1.0)],
            [Complex::real(1.0), Complex::real(0.0)],
        ];

        Self {
            gate_type: AtomicU8::new(GateType::PauliX as u8),
            target_qubit: AtomicU32::new(target as u32),
            control_qubit: AtomicU32::new(u32::MAX),
            matrix,
            _padding: [0; 48],
        }
    }

    /// Create Pauli-Y gate: Y = [[0, -i], [i, 0]]
    ///
    /// Combined bit-flip and phase-flip
    pub fn pauli_y(target: usize) -> Self {
        let matrix = [
            [Complex::real(0.0), Complex::new(0.0, -1.0)],
            [Complex::new(0.0, 1.0), Complex::real(0.0)],
        ];

        Self {
            gate_type: AtomicU8::new(GateType::PauliY as u8),
            target_qubit: AtomicU32::new(target as u32),
            control_qubit: AtomicU32::new(u32::MAX),
            matrix,
            _padding: [0; 48],
        }
    }

    /// Create Pauli-Z gate: Z = [[1, 0], [0, -1]]
    ///
    /// Phase-flip: Z|0⟩ = |0⟩, Z|1⟩ = -|1⟩
    pub fn pauli_z(target: usize) -> Self {
        let matrix = [
            [Complex::real(1.0), Complex::real(0.0)],
            [Complex::real(0.0), Complex::real(-1.0)],
        ];

        Self {
            gate_type: AtomicU8::new(GateType::PauliZ as u8),
            target_qubit: AtomicU32::new(target as u32),
            control_qubit: AtomicU32::new(u32::MAX),
            matrix,
            _padding: [0; 48],
        }
    }

    /// Create S gate: S = [[1, 0], [0, i]]
    ///
    /// π/2 phase rotation (√Z gate)
    pub fn s_gate(target: usize) -> Self {
        let matrix = [
            [Complex::real(1.0), Complex::real(0.0)],
            [Complex::real(0.0), Complex::i()],
        ];

        Self {
            gate_type: AtomicU8::new(GateType::SGate as u8),
            target_qubit: AtomicU32::new(target as u32),
            control_qubit: AtomicU32::new(u32::MAX),
            matrix,
            _padding: [0; 48],
        }
    }

    /// Create T gate: T = [[1, 0], [0, e^(iπ/4)]]
    ///
    /// π/4 phase rotation (Clifford+T universal gate set)
    pub fn t_gate(target: usize) -> Self {
        let phase = std::f64::consts::FRAC_PI_4;
        let t_element = Complex::new(phase.cos(), phase.sin());

        let matrix = [
            [Complex::real(1.0), Complex::real(0.0)],
            [Complex::real(0.0), t_element],
        ];

        Self {
            gate_type: AtomicU8::new(GateType::TGate as u8),
            target_qubit: AtomicU32::new(target as u32),
            control_qubit: AtomicU32::new(u32::MAX),
            matrix,
            _padding: [0; 48],
        }
    }

    /// Create custom gate with arbitrary 2×2 unitary matrix
    ///
    /// # Arguments
    ///
    /// * `target` - Target qubit index
    /// * `matrix` - 2×2 unitary matrix
    ///
    /// # Errors
    ///
    /// - `NonUnitaryMatrix`: Matrix does not satisfy U†U = I
    pub fn custom(target: usize, matrix: [[Complex; 2]; 2]) -> QuantumPureResult<Self> {
        // Verify unitarity: U†U = I
        Self::verify_unitary(&matrix)?;

        Ok(Self {
            gate_type: AtomicU8::new(GateType::Custom as u8),
            target_qubit: AtomicU32::new(target as u32),
            control_qubit: AtomicU32::new(u32::MAX),
            matrix,
            _padding: [0; 48],
        })
    }

    /// Verify matrix is unitary: U†U = I
    ///
    /// # Algorithm
    ///
    /// Compute M = U†U and check:
    /// - M[0][0] ≈ 1.0, M[1][1] ≈ 1.0
    /// - M[0][1] ≈ 0.0, M[1][0] ≈ 0.0
    fn verify_unitary(matrix: &[[Complex; 2]; 2]) -> QuantumPureResult<()> {
        // Compute U†U (hermitian conjugate × original)
        let [[a, b], [c, d]] = matrix;

        // U† = [[a*, c*], [b*, d*]]
        let a_conj = a.conj();
        let b_conj = b.conj();
        let c_conj = c.conj();
        let d_conj = d.conj();

        // M = U†U
        // M[0][0] = a*·a + c*·c
        let m00_re = a_conj.re * a.re - a_conj.im * a.im + c_conj.re * c.re - c_conj.im * c.im;
        let m00_im = a_conj.re * a.im + a_conj.im * a.re + c_conj.re * c.im + c_conj.im * c.re;

        // M[1][1] = b*·b + d*·d
        let m11_re = b_conj.re * b.re - b_conj.im * b.im + d_conj.re * d.re - d_conj.im * d.im;
        let m11_im = b_conj.re * b.im + b_conj.im * b.re + d_conj.re * d.im + d_conj.im * d.re;

        // M[0][1] = a*·b + c*·d
        let m01_re = a_conj.re * b.re - a_conj.im * b.im + c_conj.re * d.re - c_conj.im * d.im;
        let m01_im = a_conj.re * b.im + a_conj.im * b.re + c_conj.re * d.im + c_conj.im * d.re;

        const TOL: f64 = 1e-10;

        // Check diagonal = 1.0
        if (m00_re - 1.0).abs() > TOL || m00_im.abs() > TOL {
            return Err(QuantumPureError::NonUnitaryMatrix {
                row: 0,
                col: 0,
                value: m00_re,
                expected: 1.0,
            });
        }

        if (m11_re - 1.0).abs() > TOL || m11_im.abs() > TOL {
            return Err(QuantumPureError::NonUnitaryMatrix {
                row: 1,
                col: 1,
                value: m11_re,
                expected: 1.0,
            });
        }

        // Check off-diagonal = 0.0
        if m01_re.abs() > TOL || m01_im.abs() > TOL {
            return Err(QuantumPureError::NonUnitaryMatrix {
                row: 0,
                col: 1,
                value: m01_re,
                expected: 0.0,
            });
        }

        Ok(())
    }

    /// Get gate type
    #[inline]
    pub fn gate_type(&self) -> GateType {
        match self.gate_type.load(Ordering::Relaxed) {
            0 => GateType::Hadamard,
            1 => GateType::PauliX,
            2 => GateType::PauliY,
            3 => GateType::PauliZ,
            4 => GateType::SGate,
            5 => GateType::TGate,
            _ => GateType::Custom,
        }
    }

    /// Get target qubit
    #[inline]
    pub fn target(&self) -> usize {
        self.target_qubit.load(Ordering::Relaxed) as usize
    }

    /// Get matrix
    #[inline]
    pub fn matrix(&self) -> &[[Complex; 2]; 2] {
        &self.matrix
    }

    /// Check if the gate is unitary (U†U = I)
    ///
    /// All standard gates (Hadamard, Pauli, S, T) are guaranteed unitary.
    /// Custom gates are validated during construction.
    ///
    /// This method always returns `true` for gates created through the API,
    /// as they are pre-validated or constructed from known unitary matrices.
    #[inline]
    pub fn is_unitary(&self) -> bool {
        // All gates created through the API are unitary by construction
        // Custom gates are validated in custom(), standard gates are known unitary
        Self::verify_unitary(&self.matrix).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gate_layout() {
        assert_eq!(std::mem::size_of::<QuantumGateCapsule>(), 128);
        assert_eq!(std::mem::align_of::<QuantumGateCapsule>(), 128);
    }

    #[test]
    fn test_hadamard_gate() {
        let gate = QuantumGateCapsule::hadamard(0);
        assert_eq!(gate.gate_type(), GateType::Hadamard);
        assert_eq!(gate.target(), 0);
    }

    #[test]
    fn test_pauli_gates() {
        let x = QuantumGateCapsule::pauli_x(1);
        let y = QuantumGateCapsule::pauli_y(2);
        let z = QuantumGateCapsule::pauli_z(3);

        assert_eq!(x.gate_type(), GateType::PauliX);
        assert_eq!(y.gate_type(), GateType::PauliY);
        assert_eq!(z.gate_type(), GateType::PauliZ);
    }

    #[test]
    fn test_gate_unitarity() {
        // Hadamard is unitary
        let h = QuantumGateCapsule::hadamard(0);
        QuantumGateCapsule::verify_unitary(h.matrix()).unwrap();

        // Identity is unitary
        let identity = [
            [Complex::real(1.0), Complex::real(0.0)],
            [Complex::real(0.0), Complex::real(1.0)],
        ];
        QuantumGateCapsule::verify_unitary(&identity).unwrap();

        // Non-unitary matrix fails
        let non_unitary = [
            [Complex::real(2.0), Complex::real(0.0)],
            [Complex::real(0.0), Complex::real(1.0)],
        ];
        assert!(QuantumGateCapsule::verify_unitary(&non_unitary).is_err());
    }
}
