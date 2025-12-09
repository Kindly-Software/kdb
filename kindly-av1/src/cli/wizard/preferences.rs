//! User preferences persistence
//!
//! Stores user preferences to `~/.kindly-av1/preferences.json`
//!
//! ## JSON Format
//! ```json
//! {"skip_wizard": false, "default_quality": 2, "default_speed": 2}
//! ```
//!
//! ## Framework Compliance
//! - **Chaos**: Simple file I/O, no coordination needed
//! - **ASSUM**: File I/O assumptions documented below
//! - **Zero Dependencies**: Hand-written JSON parsing

use std::fs;
use std::io;
use std::path::PathBuf;

/// User preferences that persist across sessions
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserPreferences {
    /// Skip wizard prompt on startup
    pub skip_wizard: bool,
    /// Default quality preset (1=Smallest, 2=Balanced, 3=Best)
    pub default_quality: u8,
    /// Default speed preset (1=Quick, 2=Normal, 3=Thorough)
    pub default_speed: u8,
}

impl Default for UserPreferences {
    fn default() -> Self {
        Self {
            skip_wizard: false,
            default_quality: 2,  // Balanced
            default_speed: 2,    // Normal
        }
    }
}

#[derive(Debug)]
pub enum PreferencesError {
    IoError(io::Error),
    ParseError(String),
}

impl From<io::Error> for PreferencesError {
    fn from(err: io::Error) -> Self {
        PreferencesError::IoError(err)
    }
}

impl UserPreferences {
    /// Load preferences from disk (returns default if not found)
    ///
    /// #ASSUME: $HOME environment variable is set (verified via std::env::var)
    /// #VERIFY: Returns default preferences if file doesn't exist (graceful fallback)
    pub fn load() -> Self {
        // #ASSUME: File read succeeds or doesn't exist
        // #VERIFY: Return default on any error (graceful degradation)
        match fs::read_to_string(Self::preferences_path()) {
            Ok(contents) => Self::parse_json(&contents).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Save preferences to disk
    ///
    /// #ASSUME: User has write permissions to home directory
    /// #VERIFY: Create config directory if it doesn't exist
    pub fn save(&self) -> Result<(), PreferencesError> {
        // Ensure config directory exists
        let config_dir = Self::config_dir();
        fs::create_dir_all(&config_dir)?;

        // Serialize to JSON (hand-written)
        let json = self.to_json();
        fs::write(Self::preferences_path(), json)?;
        Ok(())
    }

    /// Get config directory path (~/.kindly-av1/)
    ///
    /// #ASSUME: $HOME environment variable is set
    /// #VERIFY: Falls back to current directory if $HOME not set
    pub fn config_dir() -> PathBuf {
        match std::env::var("HOME") {
            Ok(home) => PathBuf::from(home).join(".kindly-av1"),
            Err(_) => PathBuf::from(".kindly-av1"),
        }
    }

    /// Get preferences file path
    fn preferences_path() -> PathBuf {
        Self::config_dir().join("preferences.json")
    }

    /// Serialize to JSON (hand-written, no dependencies)
    fn to_json(&self) -> String {
        format!(
            r#"{{"skip_wizard": {}, "default_quality": {}, "default_speed": {}}}"#,
            self.skip_wizard, self.default_quality, self.default_speed
        )
    }

    /// Parse JSON (hand-written, no dependencies)
    ///
    /// #ASSUME: JSON is well-formed
    /// #VERIFY: Returns error on malformed JSON
    fn parse_json(json: &str) -> Result<Self, PreferencesError> {
        let json = json.trim();

        // Extract boolean skip_wizard
        let skip_wizard = Self::extract_bool(json, "skip_wizard")
            .ok_or_else(|| PreferencesError::ParseError("Missing skip_wizard".to_string()))?;

        // Extract u8 default_quality
        let default_quality = Self::extract_u8(json, "default_quality")
            .ok_or_else(|| PreferencesError::ParseError("Missing default_quality".to_string()))?;

        // Extract u8 default_speed
        let default_speed = Self::extract_u8(json, "default_speed")
            .ok_or_else(|| PreferencesError::ParseError("Missing default_speed".to_string()))?;

        Ok(Self {
            skip_wizard,
            default_quality,
            default_speed,
        })
    }

    /// Extract boolean value from JSON string
    fn extract_bool(json: &str, key: &str) -> Option<bool> {
        let pattern = format!(r#""{}":"#, key);
        let start = json.find(&pattern)? + pattern.len();
        let rest = &json[start..].trim_start();

        if rest.starts_with("true") {
            Some(true)
        } else if rest.starts_with("false") {
            Some(false)
        } else {
            None
        }
    }

    /// Extract u8 value from JSON string
    fn extract_u8(json: &str, key: &str) -> Option<u8> {
        let pattern = format!(r#""{}":"#, key);
        let start = json.find(&pattern)? + pattern.len();
        let rest = &json[start..].trim_start();

        let end = rest.find(|c: char| !c.is_ascii_digit())?;
        rest[..end].parse::<u8>().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_preferences() {
        let prefs = UserPreferences::default();
        assert!(!prefs.skip_wizard);
        assert_eq!(prefs.default_quality, 2);
        assert_eq!(prefs.default_speed, 2);
    }

    #[test]
    fn test_json_serialization() {
        let prefs = UserPreferences {
            skip_wizard: true,
            default_quality: 3,
            default_speed: 1,
        };
        let json = prefs.to_json();
        assert_eq!(json, r#"{"skip_wizard": true, "default_quality": 3, "default_speed": 1}"#);
    }

    #[test]
    fn test_json_deserialization() {
        let json = r#"{"skip_wizard": true, "default_quality": 3, "default_speed": 1}"#;
        let prefs = UserPreferences::parse_json(json).unwrap();
        assert!(prefs.skip_wizard);
        assert_eq!(prefs.default_quality, 3);
        assert_eq!(prefs.default_speed, 1);
    }

    #[test]
    fn test_json_deserialization_default_values() {
        let json = r#"{"skip_wizard": false, "default_quality": 2, "default_speed": 2}"#;
        let prefs = UserPreferences::parse_json(json).unwrap();
        assert_eq!(prefs, UserPreferences::default());
    }

    #[test]
    fn test_json_roundtrip() {
        let original = UserPreferences {
            skip_wizard: true,
            default_quality: 1,
            default_speed: 3,
        };
        let json = original.to_json();
        let parsed = UserPreferences::parse_json(&json).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn test_config_dir() {
        let config_dir = UserPreferences::config_dir();
        assert!(config_dir.ends_with(".kindly-av1"));
    }

    #[test]
    fn test_extract_bool() {
        let json = r#"{"key": true, "other": false}"#;
        assert_eq!(UserPreferences::extract_bool(json, "key"), Some(true));
        assert_eq!(UserPreferences::extract_bool(json, "other"), Some(false));
        assert_eq!(UserPreferences::extract_bool(json, "missing"), None);
    }

    #[test]
    fn test_extract_u8() {
        let json = r#"{"count": 42, "value": 7}"#;
        assert_eq!(UserPreferences::extract_u8(json, "count"), Some(42));
        assert_eq!(UserPreferences::extract_u8(json, "value"), Some(7));
        assert_eq!(UserPreferences::extract_u8(json, "missing"), None);
    }

    #[test]
    fn test_malformed_json() {
        let json = r#"{"skip_wizard": true"#; // Missing closing brace
        assert!(UserPreferences::parse_json(json).is_err());
    }
}
