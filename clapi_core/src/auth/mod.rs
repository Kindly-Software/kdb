//! Authentication Module - OAuth2 PKCE and Security Features
//!
//! **Purpose**: OAuth2 PKCE flow implementation with computational capsule architecture
//! **Architecture**: 100% lockfree, zero dependencies on external OAuth libraries
//!
//! # Components
//! - **OAuthStateCapsule**: PKCE state management (128B, Tier 1 Atomic)
//! - **OAuth2Client**: OAuth2 authorization code flow with PKCE
//!
//! # UCE34 Compliance
//! - **Q10 (Tier Selection)**: Tier 1 Atomic for lockfree PKCE state coordination
//! - **Q11 (Rust Transform)**: Base64URL encoding, SHA-256 hashing, CSPRNG
//! - **Q12 (Nightly)**: None required (stable Rust)
//! - **Q33 (Validation)**: #[derive(ComputationalCapsule)] automatic verification
//!
//! # ASSUM Safety
//! - #ASSUME: CSPRNG provides cryptographic randomness
//! - #VERIFY: getrandom crate uses platform CSPRNG (Linux: getrandom syscall)
//! - #ASSUME: SHA-256 prevents code_challenge brute force
//! - #VERIFY: NIST FIPS 180-4 validated algorithm
//! - #ASSUME: State nonce prevents CSRF attacks
//! - #VERIFY: Security audit validates state verification logic
//!
//! # Security Properties
//! - **CSRF Protection**: State nonce validation (64-bit CSPRNG)
//! - **PKCE Security**: Code challenge prevents authorization code interception
//! - **Replay Prevention**: 10-minute state expiry + generation counters
//! - **Thread Safety**: 100% lockfree atomic operations (zero mutex/RwLock)

pub mod oauth_state;
pub mod oauth_client;

pub use oauth_state::{OAuthStateCapsule, OAuthStateSnapshot, PKCEChallenge};
pub use oauth_client::{OAuth2Client, OAuth2Config, TokenResponse, OAuth2Error};
