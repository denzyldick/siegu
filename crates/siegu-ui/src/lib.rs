//! Shared GPUI + Ely component layer for Siegu.
//!
//! This is the only crate that names the pinned GPUI/Ely packages. Keep the
//! exact versions in the workspace manifest so every consumer compiles against
//! the same GPUI types.

pub use app_shell::SieguAppShell;
pub use ely_gpui_component as ely;
pub use gpui;
pub use gpui_platform;
pub use nav::siegu_nav_items;
pub use photo_card::PhotoCard;
pub use photo_grid::PhotoGrid;

mod app_shell;
mod nav;
mod photo_card;
mod photo_grid;
