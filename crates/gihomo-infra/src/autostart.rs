//! System login autostart entry configuration (`.desktop` in autostart directory).

use std::fs;
use std::io;
use std::path::PathBuf;

const AUTOSTART_DESKTOP_ENTRY: &str = "[Desktop Entry]\n\
Type=Application\n\
Name=Gihomo\n\
Comment=Modern GTK4 & Libadwaita Mihomo Proxy Client\n\
Exec=gihomo --minimized\n\
Icon=art.artforge.Gihomo\n\
Terminal=false\n\
Categories=Network;Proxy;\n\
X-GNOME-Autostart-enabled=true\n";

fn autostart_file_path() -> Option<PathBuf> {
    dirs::config_dir().map(|p| p.join("autostart/art.artforge.Gihomo.desktop"))
}

/// Check if autostart at system login is currently enabled.
pub fn is_autostart_enabled() -> bool {
    let Some(path) = autostart_file_path() else {
        return false;
    };
    if !path.exists() {
        return false;
    }
    fs::read_to_string(&path)
        .map(|content| !content.contains("X-GNOME-Autostart-enabled=false"))
        .unwrap_or(false)
}

/// Enable or disable autostart at system login.
pub fn set_autostart(enabled: bool) -> io::Result<()> {
    let Some(path) = autostart_file_path() else {
        return Ok(());
    };
    if enabled {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, AUTOSTART_DESKTOP_ENTRY)?;
    } else if path.exists() {
        let _ = fs::remove_file(path);
    }
    Ok(())
}
