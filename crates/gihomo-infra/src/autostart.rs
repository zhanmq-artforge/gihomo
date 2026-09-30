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
    if let Some(path) = autostart_file_path() {
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                return !content.contains("X-GNOME-Autostart-enabled=false");
            }
        }
    }
    false
}

/// Enable or disable autostart at system login.
pub fn set_autostart(enabled: bool) -> io::Result<()> {
    if let Some(path) = autostart_file_path() {
        if enabled {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(path, AUTOSTART_DESKTOP_ENTRY)?;
        } else if path.exists() {
            let _ = fs::remove_file(path);
        }
    }
    Ok(())
}
