//! Complex Widgets
//!
//! Advanced composite widgets built on foundation primitives.
//!
//! ## Widgets
//!
//! - `DropdownCapsule` - T1+T5 dropdown with search and keyboard navigation
//! - `TreeCapsule` - T4+T5 hierarchical tree view with expand/collapse
//! - `ListCapsule` - T4+T5 virtualized scrollable list (100K+ items)

pub mod dropdown;
pub mod tree;
pub mod list;

pub use dropdown::DropdownCapsule;
pub use tree::{TreeCapsule, TreeNodeState};
pub use list::{ListCapsule, SelectionMode, ListItemState};
