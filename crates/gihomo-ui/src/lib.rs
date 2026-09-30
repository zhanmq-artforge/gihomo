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

/// Re-exports of common GTK / libadwaita traits for convenience.
#[allow(unused_imports)]
pub mod prelude {
    pub use adw::prelude::*;
    pub use gio::prelude::*;
    pub use glib::prelude::*;
    pub use gtk4::prelude::*;
}

/// The official application identifier.
pub const APPLICATION_ID: &str = "art.artforge.Gihomo";
