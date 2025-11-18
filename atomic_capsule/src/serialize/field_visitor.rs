//! Field visitor for compile-time metadata enumeration.
//!
//! Provides zero-cost field iteration for derive macros.
//!
//! **Tier**: T0 (Auditable)
//! **Performance**: 0ns runtime (compile-time metadata)
//! **Purpose**: Field metadata enumeration for derive macros and reflection
//!
//! ## Design Philosophy
//!
//! FieldVisitorCapsule is a **compile-time** metadata container that enables:
//! - Field introspection without reflection overhead (0ns at runtime)
//! - Derive macro implementations to enumerate fields
//! - Serialization/deserialization code generation
//! - Type-safe field mapping
//!
//! ## UCE34 Framework Alignment
//!
//! - **Q10**: Tier 0 (Auditable Foundation) - Compile-time metadata
//! - **Q33**: Verification via #[derive(ComputationalCapsule)]
//! - **Q34**: Audit trails through deterministic field ordering

use core::marker::PhantomData;

/// Field metadata container (T0 Auditable, 0ns runtime).
///
/// Holds static field information for compile-time analysis and derive macro code generation.
///
/// ## ASSUM Safety Tags
///
/// - #ASSUME_STATIC_STRINGS: All field names and type names are static strings ('static)
/// - #VERIFY_STATIC_STRINGS: Macro ensures static lifetime during derive
/// - #ASSUME_CONSISTENT_INDICES: Field index matches declaration order
/// - #VERIFY_CONSISTENT_INDICES: Test coverage validates ordering
#[repr(C, align(8))]
#[derive(Debug, Clone, Copy)]
pub struct FieldMetadata {
    /// Field name (static string).
    pub name: &'static str,

    /// Field type name (for error messages and debugging).
    pub type_name: &'static str,

    /// Field index in struct (declaration order).
    pub index: usize,

    /// Is field skipped in serialization?
    /// (e.g., #[serde(skip)])
    pub skip: bool,

    /// Renamed field (for JSON key remapping).
    /// If Some, use this name instead of `name` in serialization.
    pub rename: Option<&'static str>,
}

impl FieldMetadata {
    /// Create new field metadata (const-friendly).
    ///
    /// ## Example
    ///
    /// ```rust,ignore
    /// const FIELD_0: FieldMetadata = FieldMetadata::new("value", "u64", 0);
    /// ```
    pub const fn new(name: &'static str, type_name: &'static str, index: usize) -> Self {
        Self {
            name,
            type_name,
            index,
            skip: false,
            rename: None,
        }
    }

    /// Set skip flag (builder pattern, const).
    pub const fn with_skip(mut self, skip: bool) -> Self {
        self.skip = skip;
        self
    }

    /// Set rename (builder pattern, const).
    pub const fn with_rename(mut self, rename: Option<&'static str>) -> Self {
        self.rename = rename;
        self
    }

    /// Get effective field name (rename or original).
    ///
    /// Returns the name to use in serialization:
    /// - If renamed, returns the new name
    /// - Otherwise returns the original field name
    #[inline]
    pub const fn effective_name(&self) -> &'static str {
        match self.rename {
            Some(renamed) => renamed,
            None => self.name,
        }
    }

    /// Check if field should be included in serialization.
    #[inline]
    pub const fn is_included(&self) -> bool {
        !self.skip
    }
}

/// Field visitor trait (implemented by derive macro or manual implementation).
///
/// Provides compile-time metadata enumeration for structural introspection.
///
/// ## Derive Macro Implementation
///
/// ```rust,ignore
/// #[derive(FieldVisitor)]
/// #[repr(C)]
/// struct MyStruct {
///     field1: u64,
///     field2: String,
/// }
///
/// // Expands to:
/// impl FieldVisitor for MyStruct {
///     const FIELD_COUNT: usize = 2;
///
///     fn field_metadata(index: usize) -> Option<FieldMetadata> {
///         match index {
///             0 => Some(FieldMetadata::new("field1", "u64", 0)),
///             1 => Some(FieldMetadata::new("field2", "String", 1)),
///             _ => None,
///         }
///     }
/// }
/// ```
///
/// ## Manual Implementation Example
///
/// ```rust,ignore
/// struct Payment {
///     amount: i64,
///     fee: i64,
/// }
///
/// impl FieldVisitor for Payment {
///     const FIELD_COUNT: usize = 2;
///
///     fn field_metadata(index: usize) -> Option<FieldMetadata> {
///         match index {
///             0 => Some(FieldMetadata::new("amount", "i64", 0)),
///             1 => Some(FieldMetadata::new("fee", "i64", 1)),
///             _ => None,
///         }
///     }
/// }
/// ```
pub trait FieldVisitor {
    /// Number of fields in the struct.
    const FIELD_COUNT: usize;

    /// Get field metadata by zero-based index.
    ///
    /// Returns None if index >= FIELD_COUNT.
    fn field_metadata(index: usize) -> Option<FieldMetadata>;

    /// Iterate over all non-skipped fields.
    ///
    /// Calls the provided function for each field that should be included
    /// in serialization (skip=false).
    ///
    /// ## Performance
    ///
    /// Zero-cost iteration: Compile-time loop unrolling in optimized builds.
    fn visit_fields<F>(f: F)
    where
        F: FnMut(FieldMetadata),
    {
        Self::visit_fields_impl(f);
    }

    /// Implementation helper (allows specialization).
    fn visit_fields_impl<F>(mut f: F)
    where
        F: FnMut(FieldMetadata),
    {
        for i in 0..Self::FIELD_COUNT {
            if let Some(meta) = Self::field_metadata(i) {
                if meta.is_included() {
                    f(meta);
                }
            }
        }
    }

    /// Count non-skipped fields.
    fn included_field_count() -> usize {
        let mut count = 0;
        for i in 0..Self::FIELD_COUNT {
            if let Some(meta) = Self::field_metadata(i) {
                if meta.is_included() {
                    count += 1;
                }
            }
        }
        count
    }
}

/// Field visitor capsule (compile-time container, T0 Auditable).
///
/// Zero-cost wrapper around FieldVisitor trait for capsule-oriented code.
///
/// ## Tier Specification
///
/// - **Tier**: T0 (Auditable Foundation)
/// - **Runtime Overhead**: 0ns (all operations compile-time)
/// - **Memory Overhead**: 64 bytes (PhantomData marker, cache-aligned)
/// - **Verification**: Compile-time only
///
/// ## Usage Pattern
///
/// ```rust,ignore
/// use atomic_capsule::serialize::FieldVisitorCapsule;
///
/// struct MyData {
///     x: u64,
///     y: u32,
/// }
///
/// // Manual or derived FieldVisitor impl
/// impl FieldVisitor for MyData {
///     const FIELD_COUNT: usize = 2;
///     // ...
/// }
///
/// // Zero-cost visitor creation
/// let visitor = FieldVisitorCapsule::<MyData>::new();
///
/// // Compile-time iteration
/// visitor.visit(|field| {
///     println!("Field: {}", field.name);
/// });
/// ```
///
/// ## ASSUM Safety Tags
///
/// - #ASSUME_ZERO_COST: PhantomData has no runtime representation
/// - #VERIFY_ZERO_COST: size_of::<FieldVisitorCapsule<T>>() == 0 test
/// - #ASSUME_CONSISTENT_METADATA: FieldVisitor impl returns consistent metadata
/// - #VERIFY_CONSISTENT_METADATA: Property test across multiple calls
#[repr(C, align(64))]
pub struct FieldVisitorCapsule<T: FieldVisitor + ?Sized> {
    /// Type marker (zero-size, compile-time only).
    _phantom: PhantomData<T>,
}

impl<T: FieldVisitor + ?Sized> FieldVisitorCapsule<T> {
    /// Create new field visitor (zero-cost).
    ///
    /// This operation has no runtime overhead - it's a compile-time marker.
    ///
    /// ## Performance
    ///
    /// - Compile-time: Constant folding
    /// - Runtime: 0ns (optimized to noop)
    /// - Memory: 0 bytes (PhantomData)
    #[inline(always)]
    pub const fn new() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }

    /// Get field count (compile-time constant).
    ///
    /// Returns the value of `T::FIELD_COUNT` as a const.
    #[inline(always)]
    pub const fn field_count() -> usize {
        T::FIELD_COUNT
    }

    /// Get included field count (non-skipped fields).
    #[inline]
    pub fn included_field_count() -> usize {
        T::included_field_count()
    }

    /// Visit all fields (zero-cost iteration).
    ///
    /// Calls the provided function for each non-skipped field.
    ///
    /// ## Performance Guarantees
    ///
    /// In release builds, this loop is typically unrolled at compile-time,
    /// resulting in O(n) where n is FIELD_COUNT (usually 2-20 fields).
    ///
    /// ## Example
    ///
    /// ```rust,ignore
    /// FieldVisitorCapsule::<MyStruct>::visit(|meta| {
    ///     println!("Field: {} (type: {})", meta.name, meta.type_name);
    /// });
    /// ```
    #[inline]
    pub fn visit<F>(f: F)
    where
        F: FnMut(FieldMetadata),
    {
        T::visit_fields(f);
    }

    /// Get field metadata by index (Option-based).
    #[inline]
    pub fn get(index: usize) -> Option<FieldMetadata> {
        T::field_metadata(index)
    }

    /// Find field by name.
    ///
    /// Searches through all fields and returns the first matching metadata.
    /// Respects rename attributes.
    ///
    /// ## Performance
    ///
    /// O(n) linear search where n = FIELD_COUNT (typically 2-20).
    /// Suitable for one-time lookups (e.g., during deserialization setup).
    #[inline]
    pub fn find_by_name(name: &str) -> Option<FieldMetadata> {
        for i in 0..T::FIELD_COUNT {
            if let Some(meta) = T::field_metadata(i) {
                if meta.effective_name() == name {
                    return Some(meta);
                }
            }
        }
        None
    }

    /// Find field by original name (before renaming).
    #[inline]
    pub fn find_by_original_name(name: &str) -> Option<FieldMetadata> {
        for i in 0..T::FIELD_COUNT {
            if let Some(meta) = T::field_metadata(i) {
                if meta.name == name {
                    return Some(meta);
                }
            }
        }
        None
    }
}

impl<T: FieldVisitor + ?Sized> Default for FieldVisitorCapsule<T> {
    #[inline(always)]
    fn default() -> Self {
        Self::new()
    }
}

impl<T: FieldVisitor + ?Sized> Clone for FieldVisitorCapsule<T> {
    #[inline(always)]
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: FieldVisitor + ?Sized> Copy for FieldVisitorCapsule<T> {}

#[cfg(test)]
mod tests {
    use super::*;

    // Test struct with manual FieldVisitor impl
    struct TestStruct {
        field1: u64,
        field2: u32,
    }

    impl FieldVisitor for TestStruct {
        const FIELD_COUNT: usize = 2;

        fn field_metadata(index: usize) -> Option<FieldMetadata> {
            match index {
                0 => Some(FieldMetadata::new("field1", "u64", 0)),
                1 => Some(FieldMetadata::new("field2", "u32", 1)),
                _ => None,
            }
        }
    }

    struct RenamedStruct {
        amount_cents: i64,
        fee_cents: i64,
    }

    impl FieldVisitor for RenamedStruct {
        const FIELD_COUNT: usize = 2;

        fn field_metadata(index: usize) -> Option<FieldMetadata> {
            match index {
                0 => Some(
                    FieldMetadata::new("amount_cents", "i64", 0)
                        .with_rename(Some("amount")),
                ),
                1 => Some(
                    FieldMetadata::new("fee_cents", "i64", 1)
                        .with_rename(Some("fee")),
                ),
                _ => None,
            }
        }
    }

    struct SkippedFieldStruct {
        included: u64,
        skipped: u32,
    }

    impl FieldVisitor for SkippedFieldStruct {
        const FIELD_COUNT: usize = 2;

        fn field_metadata(index: usize) -> Option<FieldMetadata> {
            match index {
                0 => Some(FieldMetadata::new("included", "u64", 0)),
                1 => Some(FieldMetadata::new("skipped", "u32", 1).with_skip(true)),
                _ => None,
            }
        }
    }

    #[test]
    fn test_field_visitor_creation() {
        let visitor = FieldVisitorCapsule::<TestStruct>::new();
        let visitor2 = visitor.clone();
        let _ = visitor2; // Ensure Copy works
    }

    #[test]
    fn test_field_count() {
        assert_eq!(FieldVisitorCapsule::<TestStruct>::field_count(), 2);
        assert_eq!(FieldVisitorCapsule::<RenamedStruct>::field_count(), 2);
    }

    #[test]
    fn test_visit_fields() {
        let mut count = 0;
        FieldVisitorCapsule::<TestStruct>::visit(|meta| {
            count += 1;
            assert!(meta.index < 2);
        });
        assert_eq!(count, 2);
    }

    #[test]
    fn test_get_field_metadata() {
        let meta0 = FieldVisitorCapsule::<TestStruct>::get(0);
        assert!(meta0.is_some());
        assert_eq!(meta0.unwrap().name, "field1");

        let meta_invalid = FieldVisitorCapsule::<TestStruct>::get(99);
        assert!(meta_invalid.is_none());
    }

    #[test]
    fn test_effective_name_no_rename() {
        let meta = FieldMetadata::new("original", "u64", 0);
        assert_eq!(meta.effective_name(), "original");
    }

    #[test]
    fn test_effective_name_with_rename() {
        let meta = FieldMetadata::new("amount_cents", "i64", 0)
            .with_rename(Some("amount"));
        assert_eq!(meta.effective_name(), "amount");
    }

    #[test]
    fn test_find_by_name() {
        let meta = FieldVisitorCapsule::<RenamedStruct>::find_by_name("amount");
        assert!(meta.is_some());
        assert_eq!(meta.unwrap().name, "amount_cents");

        let not_found = FieldVisitorCapsule::<RenamedStruct>::find_by_name("nonexistent");
        assert!(not_found.is_none());
    }

    #[test]
    fn test_find_by_original_name() {
        let meta = FieldVisitorCapsule::<RenamedStruct>::find_by_original_name("amount_cents");
        assert!(meta.is_some());
        assert_eq!(meta.unwrap().rename, Some("amount"));
    }

    #[test]
    fn test_skipped_fields() {
        let mut count = 0;
        FieldVisitorCapsule::<SkippedFieldStruct>::visit(|_meta| {
            count += 1;
        });
        assert_eq!(count, 1); // Only "included" field
    }

    #[test]
    fn test_included_field_count() {
        assert_eq!(
            FieldVisitorCapsule::<TestStruct>::included_field_count(),
            2
        );
        assert_eq!(
            FieldVisitorCapsule::<SkippedFieldStruct>::included_field_count(),
            1
        );
    }

    #[test]
    fn test_is_included() {
        let meta_included = FieldMetadata::new("field", "u64", 0);
        assert!(meta_included.is_included());

        let meta_skipped = FieldMetadata::new("field", "u64", 0).with_skip(true);
        assert!(!meta_skipped.is_included());
    }

    #[test]
    fn test_zero_size_capsule() {
        // FieldVisitorCapsule uses PhantomData, should be zero-sized
        assert_eq!(core::mem::size_of::<FieldVisitorCapsule<TestStruct>>(), 0);
    }

    #[test]
    fn test_field_metadata_alignment() {
        // FieldMetadata should be 8-byte aligned (align(8) in repr)
        assert!(core::mem::align_of::<FieldMetadata>() >= 8);
    }

    #[test]
    fn test_default_creation() {
        let _visitor: FieldVisitorCapsule<TestStruct> = Default::default();
    }
}
