//! Kindly-AV1 CLI Module
//!
//! [TRADE SECRET] - Proprietary Kindly branding and CLI implementation.
//! This module contains the command-line interface for the Kindly-AV1 encoder.
//!
//! # Architecture
//!
//! The CLI is structured as:
//! - `branding` - Brand constants, colors, emojis, and display functions
//! - `args` - Command-line argument parsing (lockfree, no mutex)
//! - `commands` - Command handlers and execution logic
//! - `legacy` - Legacy argument structures (for backwards compatibility)
//!
//! # Chaos Compliance
//!
//! - UCE34 Q33: 100% lockfree argument parsing
//! - No mutex, no RwLock, pure functional design
//! - All state passed explicitly, no global mutable state

// New branded modules
pub mod branding;
pub mod args;
pub mod commands;
pub mod encode;
pub mod wizard;
pub mod license_cmd;
pub mod friendly_errors;

// Legacy module for backwards compatibility
pub mod legacy;

// Re-exports from new modules
pub use branding::{print_header, print_success, print_error, print_progress};
pub use args::{
    parse_args, Command, GlobalOptions, EncodeOptions, Preset, ParsedArgs, CliError,
    WizardMode, determine_wizard_mode,
};
pub use commands::{cmd_encode, cmd_info, cmd_benchmark, cmd_help, execute, CommandError, CommandResult};
pub use encode::{run_encode, EncodeArgs as NewEncodeArgs};
pub use wizard::{
    UserPreferences, RecentFiles, RecentFile, PreferencesError, RecentFilesError,
    QualityGoal, SpeedChoice, EncodingOptions as WizardEncodingOptions,
    map_to_encoding_options, estimate_output_size, estimate_time, format_size, format_time,
};
pub use license_cmd::{
    cmd_license_activate, cmd_license_status, cmd_license_deactivate,
    LicenseCommandError, LicenseCommandResult,
};
pub use friendly_errors::{
    FriendlyError, format_friendly_error, cli_error_to_friendly,
};

// Re-exports from legacy module for backwards compatibility
pub use legacy::{
    EncodeArgs, EncodingPreset, GpuBackend,
    parse_encode_args,
};
