//! Form Builder Component - Multi-page Forms with inquire
//!
//! # UCE34 Framework
//! - Q1-Q9: Interactive multi-page forms with validation
//! - Q10: N/A (uses inquire library for terminal prompts, no capsule state)
//! - Q11-Q21: Form widgets (sliders, multi-select, text input, radio buttons)
//! - Q31: Simplicity - Builder pattern for form construction
//! - Q33: Validation - Input validation at prompt time
//! - Q34: Auditability N/A (ephemeral UI, results returned as struct)
//!
//! # Widgets
//! - Slider: Continuous value selection (0.0-1.0)
//! - MultiSelect: Multiple checkboxes (tier selection)
//! - TextInput: Single-line text with validation
//! - RadioButtons: Single choice from list
//! - Confirm: Yes/No question
//!
//! # Usage
//! ```rust
//! let form = FormBuilder::new("Configuration")
//!     .add_slider("threshold", "Jaccard threshold", 0.0, 1.0, 0.85)
//!     .add_multi_select("tiers", "Select tiers", vec!["T1", "T2", "T3"])
//!     .add_text_input("output", "Output file", "output.json")
//!     .build();
//!
//! let results = form.run()?;
//! ```

use inquire::{
    validator::{ErrorMessage, Validation},
    Confirm, CustomType, MultiSelect, Select, Text,
};
use std::collections::HashMap;

/// Form field value
#[derive(Debug, Clone)]
pub enum FieldValue {
    Float(f64),
    String(String),
    StringVec(Vec<String>),
    Bool(bool),
}

impl FieldValue {
    /// Get as float
    pub fn as_float(&self) -> Option<f64> {
        match self {
            FieldValue::Float(v) => Some(*v),
            _ => None,
        }
    }

    /// Get as string
    pub fn as_string(&self) -> Option<&str> {
        match self {
            FieldValue::String(s) => Some(s),
            _ => None,
        }
    }

    /// Get as string vector
    pub fn as_string_vec(&self) -> Option<&[String]> {
        match self {
            FieldValue::StringVec(v) => Some(v),
            _ => None,
        }
    }

    /// Get as bool
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            FieldValue::Bool(b) => Some(*b),
            _ => None,
        }
    }
}

/// Form field definition
#[derive(Debug, Clone)]
enum FormField {
    Slider {
        key: String,
        label: String,
        min: f64,
        max: f64,
        default: f64,
    },
    MultiSelect {
        key: String,
        label: String,
        options: Vec<String>,
        defaults: Vec<String>,
    },
    TextInput {
        key: String,
        label: String,
        default: String,
        validator: Option<fn(&str) -> Result<(), String>>,
    },
    RadioButtons {
        key: String,
        label: String,
        options: Vec<String>,
        default: usize,
    },
    Confirm {
        key: String,
        label: String,
        default: bool,
    },
}

/// Form builder
pub struct FormBuilder {
    title: String,
    fields: Vec<FormField>,
}

impl FormBuilder {
    /// Create new form builder
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            fields: Vec::new(),
        }
    }

    /// Add slider field (continuous value 0.0-1.0 or custom range)
    pub fn add_slider(
        mut self,
        key: impl Into<String>,
        label: impl Into<String>,
        min: f64,
        max: f64,
        default: f64,
    ) -> Self {
        self.fields.push(FormField::Slider {
            key: key.into(),
            label: label.into(),
            min,
            max,
            default,
        });
        self
    }

    /// Add multi-select field (checkboxes)
    pub fn add_multi_select(self, key: impl Into<String>, label: impl Into<String>, options: Vec<String>) -> Self {
        self.add_multi_select_with_defaults(key, label, options, Vec::new())
    }

    /// Add multi-select field with defaults
    pub fn add_multi_select_with_defaults(
        mut self,
        key: impl Into<String>,
        label: impl Into<String>,
        options: Vec<String>,
        defaults: Vec<String>,
    ) -> Self {
        self.fields.push(FormField::MultiSelect {
            key: key.into(),
            label: label.into(),
            options,
            defaults,
        });
        self
    }

    /// Add text input field
    pub fn add_text_input(self, key: impl Into<String>, label: impl Into<String>, default: impl Into<String>) -> Self {
        self.add_text_input_with_validator(key, label, default, None)
    }

    /// Add text input field with validator
    pub fn add_text_input_with_validator(
        mut self,
        key: impl Into<String>,
        label: impl Into<String>,
        default: impl Into<String>,
        validator: Option<fn(&str) -> Result<(), String>>,
    ) -> Self {
        self.fields.push(FormField::TextInput {
            key: key.into(),
            label: label.into(),
            default: default.into(),
            validator,
        });
        self
    }

    /// Add radio buttons field (single choice)
    pub fn add_radio_buttons(
        mut self,
        key: impl Into<String>,
        label: impl Into<String>,
        options: Vec<String>,
        default: usize,
    ) -> Self {
        self.fields.push(FormField::RadioButtons {
            key: key.into(),
            label: label.into(),
            options,
            default,
        });
        self
    }

    /// Add confirm field (Yes/No)
    pub fn add_confirm(mut self, key: impl Into<String>, label: impl Into<String>, default: bool) -> Self {
        self.fields.push(FormField::Confirm {
            key: key.into(),
            label: label.into(),
            default,
        });
        self
    }

    /// Build form
    pub fn build(self) -> Form {
        Form {
            title: self.title,
            fields: self.fields,
        }
    }
}

/// Interactive form
pub struct Form {
    title: String,
    fields: Vec<FormField>,
}

impl Form {
    /// Run form and collect results
    pub fn run(&self) -> Result<FormResults, inquire::InquireError> {
        println!("\n=== {} ===\n", self.title);

        let mut results = HashMap::new();

        for field in &self.fields {
            match field {
                FormField::Slider {
                    key,
                    label,
                    min,
                    max,
                    default,
                } => {
                    let value = CustomType::<f64>::new(label)
                        .with_default(*default)
                        .with_help_message(&format!("Range: {} to {}", min, max))
                        .with_error_message("Please enter a valid number")
                        .prompt()?;

                    // Clamp to range
                    let clamped = value.max(*min).min(*max);
                    results.insert(key.clone(), FieldValue::Float(clamped));
                }

                FormField::MultiSelect {
                    key,
                    label,
                    options,
                    defaults,
                } => {
                    // Build default indices before creating prompt
                    let default_indices: Vec<usize> = if !defaults.is_empty() {
                        options
                            .iter()
                            .enumerate()
                            .filter_map(|(idx, opt)| if defaults.contains(opt) { Some(idx) } else { None })
                            .collect()
                    } else {
                        Vec::new()
                    };

                    let mut prompt = MultiSelect::new(label, options.clone());

                    // Set defaults if provided
                    if !default_indices.is_empty() {
                        prompt = prompt.with_default(&default_indices);
                    }

                    let selected = prompt.prompt()?;
                    results.insert(key.clone(), FieldValue::StringVec(selected));
                }

                FormField::TextInput {
                    key,
                    label,
                    default,
                    validator,
                } => {
                    let mut prompt = Text::new(label).with_default(default);

                    // Add validator if provided
                    if let Some(validate_fn) = validator {
                        prompt = prompt.with_validator(move |input: &str| match validate_fn(input) {
                            Ok(()) => Ok(Validation::Valid),
                            Err(msg) => Ok(Validation::Invalid(ErrorMessage::Custom(msg))),
                        });
                    }

                    let value = prompt.prompt()?;
                    results.insert(key.clone(), FieldValue::String(value));
                }

                FormField::RadioButtons {
                    key,
                    label,
                    options,
                    default,
                } => {
                    let selected = Select::new(label, options.clone())
                        .with_starting_cursor(*default)
                        .prompt()?;
                    results.insert(key.clone(), FieldValue::String(selected));
                }

                FormField::Confirm { key, label, default } => {
                    let value = Confirm::new(label).with_default(*default).prompt()?;
                    results.insert(key.clone(), FieldValue::Bool(value));
                }
            }
        }

        Ok(FormResults { values: results })
    }
}

/// Form results
pub struct FormResults {
    values: HashMap<String, FieldValue>,
}

impl FormResults {
    /// Get float value
    pub fn get_float(&self, key: &str) -> Option<f64> {
        self.values.get(key)?.as_float()
    }

    /// Get string value
    pub fn get_string(&self, key: &str) -> Option<&str> {
        self.values.get(key)?.as_string()
    }

    /// Get string vector
    pub fn get_string_vec(&self, key: &str) -> Option<&[String]> {
        self.values.get(key)?.as_string_vec()
    }

    /// Get bool value
    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.values.get(key)?.as_bool()
    }

    /// Get all values
    pub fn all(&self) -> &HashMap<String, FieldValue> {
        &self.values
    }
}

/// Common validators
pub mod validators {
    /// Validate path exists
    pub fn path_exists(path: &str) -> Result<(), String> {
        if std::path::Path::new(path).exists() {
            Ok(())
        } else {
            Err(format!("Path does not exist: {}", path))
        }
    }

    /// Validate directory exists
    pub fn dir_exists(path: &str) -> Result<(), String> {
        let p = std::path::Path::new(path);
        if p.exists() && p.is_dir() {
            Ok(())
        } else {
            Err(format!("Directory does not exist: {}", path))
        }
    }

    /// Validate not empty
    pub fn not_empty(input: &str) -> Result<(), String> {
        if input.trim().is_empty() {
            Err("Input cannot be empty".to_string())
        } else {
            Ok(())
        }
    }

    /// Validate numeric range
    pub fn in_range(min: f64, max: f64) -> impl Fn(&str) -> Result<(), String> {
        move |input: &str| {
            let value: f64 = input.parse().map_err(|_| format!("Invalid number: {}", input))?;

            if value >= min && value <= max {
                Ok(())
            } else {
                Err(format!("Value must be between {} and {}", min, max))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_field_value() {
        let float_val = FieldValue::Float(0.85);
        assert_eq!(float_val.as_float(), Some(0.85));
        assert!(float_val.as_string().is_none());

        let string_val = FieldValue::String("test".to_string());
        assert_eq!(string_val.as_string(), Some("test"));
        assert!(string_val.as_float().is_none());

        let vec_val = FieldValue::StringVec(vec!["a".to_string(), "b".to_string()]);
        assert_eq!(vec_val.as_string_vec().unwrap().len(), 2);
    }

    #[test]
    fn test_validators() {
        assert!(validators::not_empty("hello").is_ok());
        assert!(validators::not_empty("").is_err());
        assert!(validators::not_empty("   ").is_err());

        let range_validator = validators::in_range(0.0, 1.0);
        assert!(range_validator("0.5").is_ok());
        assert!(range_validator("1.5").is_err());
        assert!(range_validator("-0.5").is_err());
        assert!(range_validator("abc").is_err());
    }

    #[test]
    fn test_form_builder() {
        let form = FormBuilder::new("Test Form")
            .add_slider("threshold", "Threshold", 0.0, 1.0, 0.85)
            .add_text_input("name", "Name", "default")
            .add_confirm("proceed", "Proceed?", true)
            .build();

        assert_eq!(form.title, "Test Form");
        assert_eq!(form.fields.len(), 3);
    }
}
