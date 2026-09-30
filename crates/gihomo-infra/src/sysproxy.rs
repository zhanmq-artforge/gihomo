use crate::error::InfraError;
use gio::prelude::*;
use glib::GString;
use std::sync::mpsc::{self, Receiver, Sender, SyncSender};
use std::time::Duration;
use tracing::{debug, info, warn};

enum ProxyCommand {
    IsEnabled(SyncSender<bool>),
    Enable(String, u16, SyncSender<Result<(), String>>),
    Disable(SyncSender<Result<(), String>>),
    Connect(Box<dyn Fn(bool) + Send>),
}

pub struct SystemProxyManager {
    commands: Sender<ProxyCommand>,
}

impl SystemProxyManager {
    pub fn new() -> Result<Self, InfraError> {
        let (commands, receiver) = mpsc::channel();
        std::thread::Builder::new()
            .name("gihomo-system-proxy".to_string())
            .spawn(move || run_proxy_worker(receiver))
            .map_err(InfraError::Io)?;
        Ok(Self { commands })
    }

    /// Check if GNOME system proxy mode is 'manual'
    pub fn is_enabled(&self) -> bool {
        let (reply, response) = mpsc::sync_channel(1);
        if self.commands.send(ProxyCommand::IsEnabled(reply)).is_err() {
            return false;
        }
        match response.recv() {
            Ok(enabled) => enabled,
            Err(error) => {
                warn!("System proxy worker did not respond: {}", error);
                false
            }
        }
    }

    /// Enable system proxy pointing to Mihomo's mixed port
    pub fn enable(&self, host: &str, port: u16) -> Result<(), InfraError> {
        info!("Enabling GNOME system proxy -> {}:{}", host, port);
        let (reply, response) = mpsc::sync_channel(1);
        self.commands
            .send(ProxyCommand::Enable(host.to_string(), port, reply))
            .map_err(|_| InfraError::GSettings("Proxy worker is unavailable".to_string()))?;
        response
            .recv()
            .map_err(|_| InfraError::GSettings("Proxy worker stopped unexpectedly".to_string()))?
            .map_err(InfraError::GSettings)
    }

    /// Disable system proxy by setting mode to 'none'
    pub fn disable(&self) -> Result<(), InfraError> {
        info!("Disabling GNOME system proxy (mode -> none)");
        let (reply, response) = mpsc::sync_channel(1);
        self.commands
            .send(ProxyCommand::Disable(reply))
            .map_err(|_| InfraError::GSettings("Proxy worker is unavailable".to_string()))?;
        response
            .recv()
            .map_err(|_| InfraError::GSettings("Proxy worker stopped unexpectedly".to_string()))?
            .map_err(InfraError::GSettings)
    }

    /// Connect to GNOME proxy mode change signal to update UI reactively
    pub fn connect_mode_changed<F: Fn(bool) + Send + 'static>(&self, callback: F) {
        let _ = self
            .commands
            .send(ProxyCommand::Connect(Box::new(callback)));
    }
}

struct GnomeProxySettings {
    proxy_settings: gio::Settings,
    http_settings: gio::Settings,
    https_settings: gio::Settings,
    socks_settings: gio::Settings,
}

impl GnomeProxySettings {
    fn new() -> Self {
        Self {
            proxy_settings: gio::Settings::new("org.gnome.system.proxy"),
            http_settings: gio::Settings::new("org.gnome.system.proxy.http"),
            https_settings: gio::Settings::new("org.gnome.system.proxy.https"),
            socks_settings: gio::Settings::new("org.gnome.system.proxy.socks"),
        }
    }

    fn is_enabled(&self) -> bool {
        let mode: GString = self.proxy_settings.string("mode");
        mode.as_str() == "manual"
    }

    fn enable(&self, host: &str, port: u16) -> Result<(), String> {
        self.http_settings
            .set_string("host", host)
            .map_err(|e| format!("Failed to set http host: {}", e))?;
        self.http_settings
            .set_int("port", port as i32)
            .map_err(|e| format!("Failed to set http port: {}", e))?;
        self.http_settings
            .set_boolean("enabled", true)
            .map_err(|e| format!("Failed to enable http: {}", e))?;
        self.https_settings
            .set_string("host", host)
            .map_err(|e| format!("Failed to set https host: {}", e))?;
        self.https_settings
            .set_int("port", port as i32)
            .map_err(|e| format!("Failed to set https port: {}", e))?;
        self.socks_settings
            .set_string("host", host)
            .map_err(|e| format!("Failed to set socks host: {}", e))?;
        self.socks_settings
            .set_int("port", port as i32)
            .map_err(|e| format!("Failed to set socks port: {}", e))?;
        self.proxy_settings
            .set_string("mode", "manual")
            .map_err(|e| format!("Failed to set proxy mode to manual: {}", e))?;
        self.proxy_settings
            .set_boolean("use-same-proxy", true)
            .map_err(|e| format!("Failed to set use-same-proxy: {}", e))?;

        gio::Settings::sync();
        Ok(())
    }

    fn disable(&self) -> Result<(), String> {
        self.proxy_settings
            .set_string("mode", "none")
            .map_err(|e| format!("Failed to set proxy mode to none: {}", e))?;
        gio::Settings::sync();
        Ok(())
    }

    fn connect_mode_changed<F: Fn(bool) + 'static>(&self, callback: F) {
        self.proxy_settings
            .connect_changed(Some("mode"), move |settings, _| {
                let mode: GString = settings.string("mode");
                let enabled = mode.as_str() == "manual";
                debug!("System proxy mode changed externally to: {}", mode);
                callback(enabled);
            });
    }
}

fn run_proxy_worker(receiver: Receiver<ProxyCommand>) {
    let context = glib::MainContext::new();
    let result = context.with_thread_default(|| {
        let settings = GnomeProxySettings::new();
        loop {
            match receiver.recv_timeout(Duration::from_millis(250)) {
                Ok(ProxyCommand::IsEnabled(reply)) => {
                    let _ = reply.send(settings.is_enabled());
                }
                Ok(ProxyCommand::Enable(host, port, reply)) => {
                    let _ = reply.send(settings.enable(&host, port));
                }
                Ok(ProxyCommand::Disable(reply)) => {
                    let _ = reply.send(settings.disable());
                }
                Ok(ProxyCommand::Connect(callback)) => settings.connect_mode_changed(callback),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
            while context.pending() {
                context.iteration(false);
            }
        }
    });
    if let Err(error) = result {
        tracing::error!("Failed to initialize proxy worker context: {}", error);
    }
}
