//! Parsing utilities for extracting capsule definitions from Rust source code.

use anyhow::{anyhow, Result};
use quote::ToTokens;
use regex::Regex;
use syn::{parse_file, Item, ItemStruct};

use crate::utils::{extract_named_fields, estimate_type_size, is_padding_field};

/// Represents a field in a capsule structure.
#[derive(Debug, Clone)]
pub struct FieldInfo {
    pub name: String,
    pub ty: String,
    pub size_bytes: usize,
}

/// Represents a padding field in a capsule.
#[derive(Debug, Clone)]
pub struct PaddingFieldInfo {
    pub name: String,
    pub size_bytes: usize,
}

/// Represents a computational capsule definition.
#[derive(Debug, Clone)]
pub struct CapsuleInfo {
    pub name: String,
    pub alignment: usize,
    pub total_size: usize,
    /// All padding fields found (may be multiple: _padding1, _padding2, etc.)
    pub padding_fields: Vec<PaddingFieldInfo>,
    /// Total size of all padding fields combined
    pub total_padding_size: usize,
    /// User fields (non-padding)
    pub fields: Vec<FieldInfo>,
}

impl CapsuleInfo {
    /// Legacy compatibility: get single padding size if only one field exists.
    ///
    /// # ASSUME_BACKWARD_COMPATIBILITY
    /// Existing code may expect Option<usize> for single padding
    ///
    /// # VERIFY
    /// Tests confirm backward compatibility
    pub fn padding_size(&self) -> Option<usize> {
        if self.padding_fields.len() == 1 {
            Some(self.padding_fields[0].size_bytes)
        } else if self.padding_fields.is_empty() {
            None
        } else {
            // Multiple padding fields: return total
            Some(self.total_padding_size)
        }
    }

    /// Check if this capsule has multiple padding fields that need consolidation.
    pub fn needs_consolidation(&self) -> bool {
        self.padding_fields.len() > 1
    }
}

/// Extract all computational capsules from Rust source code.
///
/// # Arguments
///
/// * `content` - Rust source code as a string
///
/// # Returns
///
/// A vector of `CapsuleInfo` for each `#[derive(ComputationalCapsule)]` struct
pub fn extract_capsules(content: &str) -> Result<Vec<CapsuleInfo>> {
    let file = parse_file(content).map_err(|e| anyhow!("Failed to parse Rust file: {}", e))?;

    let mut capsules = Vec::new();

    for item in file.items {
        if let Item::Struct(item_struct) = item {
            // Check for ComputationalCapsule derive
            if has_computational_capsule_derive(&item_struct) {
                if let Ok(capsule) = extract_capsule_info(&item_struct) {
                    capsules.push(capsule);
                }
            }
        }
    }

    Ok(capsules)
}

/// Check if a struct has the ComputationalCapsule derive macro.
fn has_computational_capsule_derive(item_struct: &ItemStruct) -> bool {
    item_struct.attrs.iter().any(|attr| {
        attr.path().is_ident("derive")
            && attr
                .to_token_stream()
                .to_string()
                .contains("ComputationalCapsule")
    })
}

/// Extract capsule information from an ItemStruct.
///
/// # ASSUME_FIELD_EXTRACTION
/// All named fields can be extracted and sized accurately
///
/// # VERIFY
/// Tests confirm field extraction for single and multiple padding fields
fn extract_capsule_info(item_struct: &ItemStruct) -> Result<CapsuleInfo> {
    let name = item_struct.ident.to_string();
    let mut alignment = 64; // Default alignment
    let mut total_size = 64; // Default size
    let mut fields = Vec::new();
    let mut padding_fields = Vec::new();

    // Extract alignment and size from #[capsule(...)] attributes
    for attr in &item_struct.attrs {
        if attr.path().is_ident("capsule") {
            let attr_content = attr.to_token_stream().to_string();
            if let Some(align_val) = extract_number(&attr_content, "alignment") {
                alignment = align_val;
                total_size = align_val; // Usually alignment == size
            }
            if let Some(size_val) = extract_number(&attr_content, "size") {
                total_size = size_val;
            }
        }
    }

    // Extract all fields using utils
    let all_fields = extract_named_fields(item_struct);

    for field in all_fields {
        if let Some(ident) = &field.ident {
            let field_name = ident.to_string();
            let field_type = field.ty.to_token_stream().to_string();

            // Use utils for size estimation
            let size = estimate_type_size(&field_type);

            if is_padding_field(&field_name) {
                // Collect all padding fields
                padding_fields.push(PaddingFieldInfo {
                    name: field_name,
                    size_bytes: size,
                });
            } else {
                // User field
                fields.push(FieldInfo {
                    name: field_name,
                    ty: field_type,
                    size_bytes: size,
                });
            }
        }
    }

    // Calculate total padding size
    let total_padding_size = padding_fields.iter().map(|pf| pf.size_bytes).sum();

    Ok(CapsuleInfo {
        name,
        alignment,
        total_size,
        padding_fields,
        total_padding_size,
        fields,
    })
}

/// Extract numeric value from attribute string.
fn extract_number(attr_str: &str, param_name: &str) -> Option<usize> {
    let pattern = format!(r#"{}\s*=\s*(\d+)"#, param_name);
    if let Ok(re) = Regex::new(&pattern) {
        if let Some(caps) = re.captures(attr_str) {
            if let Ok(num) = caps[1].parse::<usize>() {
                return Some(num);
            }
        }
    }
    None
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_number() {
        assert_eq!(extract_number("alignment = 64", "alignment"), Some(64));
        assert_eq!(extract_number("size = 128", "size"), Some(128));
        assert_eq!(extract_number("alignment=64", "alignment"), Some(64));
    }

    #[test]
    fn test_capsule_info_padding_size_single() {
        let capsule = CapsuleInfo {
            name: "Test".to_string(),
            alignment: 64,
            total_size: 64,
            padding_fields: vec![PaddingFieldInfo {
                name: "_padding".to_string(),
                size_bytes: 56,
            }],
            total_padding_size: 56,
            fields: vec![],
        };
        assert_eq!(capsule.padding_size(), Some(56));
        assert!(!capsule.needs_consolidation());
    }

    #[test]
    fn test_capsule_info_padding_size_multiple() {
        let capsule = CapsuleInfo {
            name: "Test".to_string(),
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
            fields: vec![],
        };
        assert_eq!(capsule.padding_size(), Some(56));
        assert!(capsule.needs_consolidation());
    }

    #[test]
    fn test_capsule_info_no_padding() {
        let capsule = CapsuleInfo {
            name: "Test".to_string(),
            alignment: 64,
            total_size: 64,
            padding_fields: vec![],
            total_padding_size: 0,
            fields: vec![],
        };
        assert_eq!(capsule.padding_size(), None);
        assert!(!capsule.needs_consolidation());
    }
}
