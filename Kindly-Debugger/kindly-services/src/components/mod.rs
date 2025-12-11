//! UI Components Module
//!
//! Premium Leptos components with glassmorphism styling.

pub mod hero;
pub mod navbar;
pub mod features;
pub mod pricing;
pub mod footer;
pub mod cta;
pub mod docs;
pub mod privacy;
pub mod terms;
pub mod license;
pub mod verified;
pub mod signup;
pub mod oauth_success;
pub mod script_generator;
pub mod dashboard;

pub use hero::Hero;
pub use navbar::Navbar;
pub use features::Features;
pub use pricing::Pricing;
pub use footer::Footer;
pub use cta::Cta;
pub use docs::Docs;
pub use privacy::PrivacyPage;
pub use terms::TermsPage;
pub use license::LicensePage;
pub use verified::Verified;
pub use signup::Signup;
pub use oauth_success::OAuthSuccess;
pub use script_generator::{
    Platform,
    ScriptOptions,
    generate_setup_script,
    generate_enhanced_setup_script,
    download_setup_script,
    download_setup_script_for_platform,
    download_enhanced_setup_script,
};
pub use dashboard::Dashboard;
