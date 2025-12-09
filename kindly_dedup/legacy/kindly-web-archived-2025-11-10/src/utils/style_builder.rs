// Style builder that works with string slice tuples
// This is the primary function that all code should use

pub fn build_style(properties: &[(&str, &str)]) -> String {
    properties
        .iter()
        .map(|(key, value)| format!("{}: {};", key, value))
        .collect::<Vec<_>>()
        .join(" ")
}
