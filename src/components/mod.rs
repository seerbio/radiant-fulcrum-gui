//! The components module contains all shared components for our app.

mod cli_form;
#[cfg(any(feature = "web", feature = "server"))]
mod file_browser;
pub(crate) mod form_primitives;
mod theme_toggle;

pub use cli_form::CliForm;
pub use theme_toggle::ThemeToggle;
pub mod label;
pub mod button;
pub mod input;
pub mod radio_group;
pub mod collapsible;
