//! Container widgets for terminal UI
//!
//! Container widgets provide visual grouping and layout structure:
//! - `PanelCapsule`: Visual container with borders, shadows, and collapsible functionality
//! - `ModalContainerCapsule`: Modal dialog with backdrop, focus trap, and dismiss handling
//! - `SplitPaneCapsule`: Resizable split pane with draggable divider (T1+T3)
//! - `GridContainerCapsule`: CSS Grid layout container with track sizing and auto-placement (T4+T6)
//! - `FlexContainerCapsule`: CSS Flexbox layout container with flex grow/shrink and alignment (T4+T6)
//! - `ScrollCapsule`: Scrollable container

pub mod panel;
pub mod modal;
pub mod split;
pub mod grid;
pub mod flex;
pub mod scroll;

pub use panel::{PanelCapsule, BorderStyle, ShadowDirection, PanelState};
pub use modal::{ModalContainerCapsule, ModalState, ModalPosition};
pub use split::{SplitPaneCapsule, SplitOrientation, SplitState};
pub use grid::{GridContainerCapsule, GridContainerState, GridTrack, GridItem, AutoFlow, Alignment, TrackSizeType};
pub use flex::{FlexContainerCapsule, FlexDirection, FlexWrap, JustifyContent, AlignItems, FlexChild};
