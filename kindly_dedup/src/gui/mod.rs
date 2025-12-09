//! GUI module for kindly_dedup (iced-based)

mod animation;
mod app;
pub mod depth;
mod messages;
mod spring_animation;
mod styles;
mod theme;
mod utils;
pub mod widgets;

pub use app::KindlyDedupApp;
pub use messages::Message;
