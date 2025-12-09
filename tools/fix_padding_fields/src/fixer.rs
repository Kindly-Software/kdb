//! Padding field fixing and code transformation using AST manipulation.
//!
//! This module uses syn/quote for accurate AST-based fixing instead of regex,
//! following Q28 simplicity and Q31 Rust transformation principles.

use crate::calculator::PaddingCalculator;
use crate::parser::CapsuleInfo;
use crate::utils::is_padding_field;
use anyhow::{Context, Result};
use syn::{parse_file, Field, Fields, File, Item, ItemStruct};

/// Handles applying padding fixes to Rust source code.
///
/// # ASSUME_AST_MANIPULATION
/// syn::parse_file can parse Rust source correctly
/// quote! generates valid Rust code
///
/// # VERIFY
/// Tests confirm AST round-trip correctness
pub struct PaddingFixer {
    content: String,
}

impl PaddingFixer {
    /// Create a new padding fixer from source code.
    pub fn new(content: String) -> Self {
        Self { content }
    }

    /// Apply padding fix to a capsule using AST manipulation.
    ///
    /// # Arguments
    ///
    /// * `capsule` - The capsule to fix
    ///
    /// # Returns
    ///
    /// `true` if changes were made, `false` if no changes needed
    pub fn apply_padding_fix(&mut self, capsule: &CapsuleInfo) -> Result<bool> {
        let calculator = PaddingCalculator::new(capsule)?;

        if !calculator.needs_fixing() {
            return Ok(false);
        }

        // Parse the entire file as AST
        let mut ast: File = parse_file(&self.content)
            .context("Failed to parse Rust source as AST")?;

        let mut modified = false;

        // Find and fix the target struct
        for item in &mut ast.items {
            if let Item::Struct(item_struct) = item {
                if item_struct.ident == capsule.name {
                    fix_struct_padding(item_struct, &calculator)?;
                    modified = true;
                    break;
                }
            }
        }

        if modified {
            // Generate new source code from AST
            let new_content = prettyplease::unparse(&ast);
            self.content = new_content;
        }

        Ok(modified)
    }

    /// Get the modified content.
    pub fn content(&self) -> &str {
        &self.content
    }
}

/// Fix padding in a struct by consolidating all padding fields into one.
///
/// # ASSUME_STRUCT_FIELDS
/// Struct has named fields (not tuple or unit struct)
///
/// # VERIFY
/// Tests confirm field consolidation for all padding patterns
fn fix_struct_padding(
    item_struct: &mut ItemStruct,
    calculator: &PaddingCalculator,
) -> Result<()> {
    let required_padding = calculator.required_padding();

    // Only process structs with named fields
    if let Fields::Named(ref mut fields_named) = item_struct.fields {
        // Extract all non-padding fields
        let user_fields: Vec<Field> = fields_named
            .named
            .iter()
            .filter(|field| {
                field
                    .ident
                    .as_ref()
                    .map(|ident| !is_padding_field(&ident.to_string()))
                    .unwrap_or(false)
            })
            .cloned()
            .collect();

        // Build new fields list: user_fields + single _padding
        let mut new_fields = user_fields;

        // Add consolidated padding field if needed
        if required_padding > 0 {
            let padding_field: Field = syn::parse_quote! {
                _padding: [u8; #required_padding]
            };
            new_fields.push(padding_field);
        }

        // Replace fields in the struct
        fields_named.named = new_fields.into_iter().collect();
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{FieldInfo, PaddingFieldInfo};

    #[test]
    fn test_fix_single_padding() {
        let source = r#"
#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
struct TestCapsule {
    state: AtomicU64,
    _padding: [u8; 50],
}
"#;

        let capsule = CapsuleInfo {
            name: "TestCapsule".to_string(),
            alignment: 64,
            total_size: 64,
            padding_fields: vec![PaddingFieldInfo {
                name: "_padding".to_string(),
                size_bytes: 50,
            }],
            total_padding_size: 50,
            fields: vec![FieldInfo {
                name: "state".to_string(),
                ty: "AtomicU64".to_string(),
                size_bytes: 8,
            }],
        };

        let mut fixer = PaddingFixer::new(source.to_string());
        let result = fixer.apply_padding_fix(&capsule).unwrap();

        assert!(result);
        let fixed_content = fixer.content();
        // prettyplease formats as: [u8; 56usize]
        assert!(fixed_content.contains("_padding") && fixed_content.contains("[u8") && fixed_content.contains("56"));
        assert!(!fixed_content.contains("50"));
    }

    #[test]
    fn test_fix_multiple_padding() {
        let source = r#"
#[derive(ComputationalCapsule)]
#[capsule(alignment = 128, size = 128)]
#[repr(C, align(128))]
struct TestCapsule {
    state: AtomicU64,
    counter: AtomicU64,
    _padding1: [u8; 8],
    _padding2: [u8; 48],
}
"#;

        let capsule = CapsuleInfo {
            name: "TestCapsule".to_string(),
            alignment: 128,
            total_size: 128,
            padding_fields: vec![
                PaddingFieldInfo {
                    name: "_padding1".to_string(),
                    size_bytes: 8,
                },
                PaddingFieldInfo {
                    name: "_padding2".to_string(),
                    size_bytes: 48,
                },
            ],
            total_padding_size: 56,
            fields: vec![
                FieldInfo {
                    name: "state".to_string(),
                    ty: "AtomicU64".to_string(),
                    size_bytes: 8,
                },
                FieldInfo {
                    name: "counter".to_string(),
                    ty: "AtomicU64".to_string(),
                    size_bytes: 8,
                },
            ],
        };

        let mut fixer = PaddingFixer::new(source.to_string());
        let result = fixer.apply_padding_fix(&capsule).unwrap();

        assert!(result);
        let fixed_content = fixer.content();

        // Should have single consolidated padding (prettyplease formats as: [u8; 112usize])
        assert!(fixed_content.contains("_padding") && fixed_content.contains("[u8") && fixed_content.contains("112"));
        // Should not have old padding fields
        assert!(!fixed_content.contains("_padding1"));
        assert!(!fixed_content.contains("_padding2"));
    }

    #[test]
    fn test_fix_no_padding() {
        let source = r#"
#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
struct TestCapsule {
    state: AtomicU64,
}
"#;

        let capsule = CapsuleInfo {
            name: "TestCapsule".to_string(),
            alignment: 64,
            total_size: 64,
            padding_fields: vec![],
            total_padding_size: 0,
            fields: vec![FieldInfo {
                name: "state".to_string(),
                ty: "AtomicU64".to_string(),
                size_bytes: 8,
            }],
        };

        let mut fixer = PaddingFixer::new(source.to_string());
        let result = fixer.apply_padding_fix(&capsule).unwrap();

        assert!(result);
        let fixed_content = fixer.content();
        // prettyplease formats as: [u8; 56usize]
        assert!(fixed_content.contains("_padding") && fixed_content.contains("[u8") && fixed_content.contains("56"));
    }

    #[test]
    fn test_no_fix_needed() {
        let source = r#"
#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
struct TestCapsule {
    state: AtomicU64,
    _padding: [u8; 56],
}
"#;

        let capsule = CapsuleInfo {
            name: "TestCapsule".to_string(),
            alignment: 64,
            total_size: 64,
            padding_fields: vec![PaddingFieldInfo {
                name: "_padding".to_string(),
                size_bytes: 56,
            }],
            total_padding_size: 56,
            fields: vec![FieldInfo {
                name: "state".to_string(),
                ty: "AtomicU64".to_string(),
                size_bytes: 8,
            }],
        };

        let mut fixer = PaddingFixer::new(source.to_string());
        let result = fixer.apply_padding_fix(&capsule).unwrap();

        assert!(!result); // No changes needed
    }
}
