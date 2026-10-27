//! Native GTK4 and Libadwaita user interface layer for Gihomo.

pub mod application;
pub mod dialogs;
pub mod i18n;
pub mod tray;
pub mod views;
pub mod window;

pub use application::{GihomoApplication, MihomoApplication};
pub use glib::ExitCode;
pub use i18n::tr;
pub use window::MainWindow;

/// The official application identifier.
pub const APPLICATION_ID: &str = "art.artforge.Gihomo";
