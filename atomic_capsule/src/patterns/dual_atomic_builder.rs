//! Builder pattern for DualAtomicU64 field layouts
//!
//! # Overview
//!
//! Provides runtime field layout definition with validation for DualAtomicU64.
//! Unlike typed_field.rs (compile-time), this allows dynamic layout creation.
//!
//! # Use Cases
//!
//! - Configuration-driven field layouts
//! - Dynamic protocol parsing
//! - Testing different layouts
//!
//! # Framework Compliance
//!
//! - **UCE34**: T0 Auditable tier (validation + documentation)
//! - **Chaos**: Zero allocations after build, const-friendly
//! - **ASSUM**: Runtime validation with clear error messages

use core::fmt;

/// Error during field layout construction
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuilderError {
    /// Field exceeds 64-bit boundary
    FieldOverflow {
        field_name: &'static str,
        offset: u8,
        width: u8,
    },
    /// Field width is zero
    ZeroWidth { field_name: &'static str },
    /// Field width exceeds 64 bits
    ExcessiveWidth {
        field_name: &'static str,
        width: u8,
    },
    /// Too many primary fields (max 8)
    TooManyPrimaryFields,
    /// Too many secondary fields (max 8)
    TooManySecondaryFields,
    /// Duplicate field name
    DuplicateFieldName { field_name: &'static str },
}

impl fmt::Display for BuilderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BuilderError::FieldOverflow { field_name, offset, width } => {
                write!(f, "Field '{}' at offset {} with width {} exceeds 64-bit boundary",
                       field_name, offset, width)
            }
            BuilderError::ZeroWidth { field_name } => {
                write!(f, "Field '{}' has zero width", field_name)
            }
            BuilderError::ExcessiveWidth { field_name, width } => {
                write!(f, "Field '{}' has width {} > 64", field_name, width)
            }
            BuilderError::TooManyPrimaryFields => {
                write!(f, "Too many primary fields (max 8)")
            }
            BuilderError::TooManySecondaryFields => {
                write!(f, "Too many secondary fields (max 8)")
            }
            BuilderError::DuplicateFieldName { field_name } => {
                write!(f, "Duplicate field name: '{}'", field_name)
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for BuilderError {}

/// Field definition in a layout
#[derive(Debug, Clone, Copy)]
pub struct FieldDef {
    /// Field name (for debugging)
    pub name: &'static str,
    /// Bit offset from LSB
    pub offset: u8,
    /// Width in bits
    pub width: u8,
}

impl FieldDef {
    /// Get value from packed u64
    #[inline]
    pub const fn get(&self, packed: u64) -> u64 {
        let mask = if self.width == 64 {
            u64::MAX
        } else {
            ((1u64 << self.width) - 1) << self.offset
        };
        (packed & mask) >> self.offset
    }

    /// Set value in packed u64
    #[inline]
    pub const fn set(&self, packed: u64, value: u64) -> u64 {
        let max_value = if self.width == 64 {
            u64::MAX
        } else {
            (1u64 << self.width) - 1
        };
        let masked_value = value & max_value;
        let shifted_value = masked_value << self.offset;

        let field_mask = if self.width == 64 {
            u64::MAX
        } else {
            ((1u64 << self.width) - 1) << self.offset
        };

        (packed & !field_mask) | shifted_value
    }
}

/// Builder for DualAtomicU64 field layouts
#[derive(Debug, Clone)]
pub struct DualAtomicBuilder {
    primary_fields: [Option<FieldDef>; 8],
    primary_count: usize,
    primary_next_offset: u8,

    secondary_fields: [Option<FieldDef>; 8],
    secondary_count: usize,
    secondary_next_offset: u8,

    secondary_is_generation: bool,
}

impl DualAtomicBuilder {
    /// Create a new builder
    pub const fn new() -> Self {
        Self {
            primary_fields: [None; 8],
            primary_count: 0,
            primary_next_offset: 0,
            secondary_fields: [None; 8],
            secondary_count: 0,
            secondary_next_offset: 0,
            secondary_is_generation: false,
        }
    }

    /// Add a field to the primary channel
    pub fn primary_field(mut self, name: &'static str, width: u8) -> Self {
        // Store for validation in build()
        if self.primary_count < 8 {
            self.primary_fields[self.primary_count] = Some(FieldDef {
                name,
                offset: self.primary_next_offset,
                width,
            });
            self.primary_count += 1;
            self.primary_next_offset = self.primary_next_offset.saturating_add(width);
        }
        self
    }

    /// Add a field to the secondary channel
    pub fn secondary_field(mut self, name: &'static str, width: u8) -> Self {
        if self.secondary_count < 8 {
            self.secondary_fields[self.secondary_count] = Some(FieldDef {
                name,
                offset: self.secondary_next_offset,
                width,
            });
            self.secondary_count += 1;
            self.secondary_next_offset = self.secondary_next_offset.saturating_add(width);
        }
        self
    }

    /// Mark secondary as a generation counter (full 64 bits)
    pub fn secondary_as_generation(mut self) -> Self {
        self.secondary_is_generation = true;
        self.secondary_fields[0] = Some(FieldDef {
            name: "generation",
            offset: 0,
            width: 64,
        });
        self.secondary_count = 1;
        self
    }

    /// Build and validate the layout
    pub fn build(self) -> Result<DualAtomicLayout, BuilderError> {
        // Validate primary fields
        for i in 0..self.primary_count {
            if let Some(field) = &self.primary_fields[i] {
                if field.width == 0 {
                    return Err(BuilderError::ZeroWidth { field_name: field.name });
                }
                if field.width > 64 {
                    return Err(BuilderError::ExcessiveWidth {
                        field_name: field.name,
                        width: field.width
                    });
                }
                if field.offset as u16 + field.width as u16 > 64 {
                    return Err(BuilderError::FieldOverflow {
                        field_name: field.name,
                        offset: field.offset,
                        width: field.width,
                    });
                }
            }
        }

        // Validate secondary fields
        for i in 0..self.secondary_count {
            if let Some(field) = &self.secondary_fields[i] {
                if field.width == 0 {
                    return Err(BuilderError::ZeroWidth { field_name: field.name });
                }
                if field.width > 64 {
                    return Err(BuilderError::ExcessiveWidth {
                        field_name: field.name,
                        width: field.width
                    });
                }
                if field.offset as u16 + field.width as u16 > 64 {
                    return Err(BuilderError::FieldOverflow {
                        field_name: field.name,
                        offset: field.offset,
                        width: field.width,
                    });
                }
            }
        }

        Ok(DualAtomicLayout {
            primary_fields: self.primary_fields,
            primary_count: self.primary_count,
            secondary_fields: self.secondary_fields,
            secondary_count: self.secondary_count,
            secondary_is_generation: self.secondary_is_generation,
        })
    }
}

impl Default for DualAtomicBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Validated field layout for DualAtomicU64
#[derive(Debug, Clone)]
pub struct DualAtomicLayout {
    primary_fields: [Option<FieldDef>; 8],
    primary_count: usize,
    secondary_fields: [Option<FieldDef>; 8],
    secondary_count: usize,
    secondary_is_generation: bool,
}

impl DualAtomicLayout {
    /// Get a primary field by name
    pub fn primary_field(&self, name: &str) -> Option<&FieldDef> {
        self.primary_fields[..self.primary_count]
            .iter()
            .filter_map(|f| f.as_ref())
            .find(|f| f.name == name)
    }

    /// Get a secondary field by name
    pub fn secondary_field(&self, name: &str) -> Option<&FieldDef> {
        self.secondary_fields[..self.secondary_count]
            .iter()
            .filter_map(|f| f.as_ref())
            .find(|f| f.name == name)
    }

    /// Is secondary channel a generation counter?
    pub fn is_generation_counter(&self) -> bool {
        self.secondary_is_generation
    }

    /// Get number of primary fields
    pub fn primary_field_count(&self) -> usize {
        self.primary_count
    }

    /// Get number of secondary fields
    pub fn secondary_field_count(&self) -> usize {
        self.secondary_count
    }

    /// Total bits used in primary channel
    pub fn primary_bits_used(&self) -> u8 {
        self.primary_fields[..self.primary_count]
            .iter()
            .filter_map(|f| f.as_ref())
            .map(|f| f.offset + f.width)
            .max()
            .unwrap_or(0)
    }

    /// Total bits used in secondary channel
    pub fn secondary_bits_used(&self) -> u8 {
        self.secondary_fields[..self.secondary_count]
            .iter()
            .filter_map(|f| f.as_ref())
            .map(|f| f.offset + f.width)
            .max()
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_layout() {
        let layout = DualAtomicBuilder::new()
            .primary_field("state", 3)
            .primary_field("version", 2)
            .primary_field("counter", 16)
            .secondary_as_generation()
            .build()
            .unwrap();

        assert_eq!(layout.primary_field_count(), 3);
        assert_eq!(layout.secondary_field_count(), 1);
        assert!(layout.is_generation_counter());
    }

    #[test]
    fn test_field_get_set() {
        let layout = DualAtomicBuilder::new()
            .primary_field("state", 3)
            .primary_field("version", 2)
            .build()
            .unwrap();

        let state = layout.primary_field("state").unwrap();
        let version = layout.primary_field("version").unwrap();

        let mut packed = 0u64;
        packed = state.set(packed, 5);
        packed = version.set(packed, 2);

        assert_eq!(state.get(packed), 5);
        assert_eq!(version.get(packed), 2);
    }

    #[test]
    fn test_overflow_error() {
        let result = DualAtomicBuilder::new()
            .primary_field("big", 60)
            .primary_field("overflow", 10)
            .build();

        assert!(matches!(result, Err(BuilderError::FieldOverflow { .. })));
    }

    #[test]
    fn test_zero_width_error() {
        let result = DualAtomicBuilder::new()
            .primary_field("empty", 0)
            .build();

        assert!(matches!(result, Err(BuilderError::ZeroWidth { .. })));
    }

    #[test]
    fn test_bits_used() {
        let layout = DualAtomicBuilder::new()
            .primary_field("a", 8)
            .primary_field("b", 16)
            .build()
            .unwrap();

        assert_eq!(layout.primary_bits_used(), 24);
    }
}
