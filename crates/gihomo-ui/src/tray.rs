use gihomo_app::{AppEvent, AppService};
use gihomo_core::Subscription;
use ksni::menu::{CheckmarkItem, MenuItem, StandardItem, SubMenu};
use ksni::{Handle, ToolTip, Tray, TrayMethods};
use tracing::{error, info};

pub struct GihomoTray {
    pub proxy_enabled: bool,
    pub tun_enabled: bool,
    pub proxy_mode: String,
    pub kernel_error: bool,
    pub subscriptions: Vec<Subscription>,
    pub active_sub_id: Option<String>,
    service: AppService,
}

impl GihomoTray {
    pub fn resolve_icon_name(
        proxy_enabled: bool,
        tun_enabled: bool,
        kernel_error: bool,
    ) -> &'static str {
        if kernel_error {
            "art.artforge.Gihomo-red"
        } else if tun_enabled {
            "art.artforge.Gihomo-green"
        } else if proxy_enabled {
            "art.artforge.Gihomo-blue"
        } else {
            "art.artforge.Gihomo-gray"
        }
    }
}

impl Tray for GihomoTray {
    fn id(&self) -> String {
        "art.artforge.Gihomo".to_string()
    }

    fn title(&self) -> String {
        crate::i18n::tr("app_name").to_string()
    }

    fn icon_name(&self) -> String {
        Self::resolve_icon_name(self.proxy_enabled, self.tun_enabled, self.kernel_error).to_string()
    }

    fn icon_theme_path(&self) -> String {
        let local_path = std::path::PathBuf::from("data/icons/hicolor/scalable/apps");
        if local_path.join("art.artforge.Gihomo.svg").exists() {
            if let Ok(abs) = local_path.canonicalize() {
                return abs.to_string_lossy().to_string();
            }
        }
        if let Some(user_data) = dirs::data_dir() {
            let user_icon_path = user_data.join("icons/hicolor/scalable/apps");
            if user_icon_path.join("art.artforge.Gihomo.svg").exists() {
                return user_icon_path.to_string_lossy().to_string();
            }
        }
        "/usr/share/icons/hicolor/scalable/apps".to_string()
    }

    fn tool_tip(&self) -> ToolTip {
        let proxy_status = if self.proxy_enabled {
            crate::i18n::tr("kernel_running")
        } else {
            crate::i18n::tr("kernel_stopped")
        };
        let tun_status = if self.tun_enabled {
            crate::i18n::tr("kernel_running")
        } else {
            crate::i18n::tr("kernel_stopped")
        };
        let active_sub_name = self
            .subscriptions
            .iter()
            .find(|s| self.active_sub_id.as_deref() == Some(&s.id) || s.is_active)
            .map(|s| s.name.as_str())
            .unwrap_or("-");
        let desc = format!(
            "{}: {} | {}: {} | {}: {} | {}: {}",
            crate::i18n::tr("tray_system_proxy"),
            proxy_status,
            crate::i18n::tr("tray_tun_mode"),
            tun_status,
            crate::i18n::tr("tray_proxy_mode"),
            self.proxy_mode,
            crate::i18n::tr("tray_active_subscription"),
            active_sub_name
        );
        ToolTip {
            title: crate::i18n::tr("app_name").to_string(),
            description: desc,
            icon_name: self.icon_name(),
            icon_pixmap: Vec::new(),
        }
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        glib::idle_add_once(|| {
            if let Some(app) = gio::Application::default() {
                if let Some(gtk_app) = app.downcast_ref::<gtk4::Application>() {
                    use gtk4::prelude::*;
                    if let Some(win) = gtk_app.windows().first() {
                        win.set_visible(true);
                        win.present();
                        if let Some(main_win) = win.downcast_ref::<crate::window::MainWindow>() {
                            main_win.resync_runtime_state();
                        }
                        return;
                    }
                }
                use gio::prelude::*;
                app.activate();
            }
        });
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let mut items = Vec::new();

        // 1. Show Main Window
        items.push(
            StandardItem::<Self> {
                label: crate::i18n::tr("tray_show_window").to_string(),
                activate: Box::new(|_| {
                    glib::idle_add_once(|| {
                        if let Some(app) = gio::Application::default() {
                            if let Some(gtk_app) = app.downcast_ref::<gtk4::Application>() {
                                use gtk4::prelude::*;
                                if let Some(win) = gtk_app.windows().first() {
                                    win.set_visible(true);
                                    win.present();
                                    if let Some(main_win) =
                                        win.downcast_ref::<crate::window::MainWindow>()
                                    {
                                        main_win.resync_runtime_state();
                                    }
                                    return;
                                }
                            }
                            use gio::prelude::*;
                            app.activate();
                        }
                    });
                }),
                ..Default::default()
            }
            .into(),
        );

        items.push(MenuItem::Separator);

        // 2. System Proxy Toggle
        let current_proxy = self.proxy_enabled;
        items.push(
            CheckmarkItem::<Self> {
                label: crate::i18n::tr("tray_system_proxy").to_string(),
                checked: current_proxy,
                activate: Box::new(move |tray: &mut Self| {
                    let next = !current_proxy;
                    tray.proxy_enabled = next;
                    tray.kernel_error = false;
                    let svc = tray.service.clone();
                    glib::idle_add_once(move || {
                        let _ = svc.toggle_proxy(next);
                    });
                }),
                ..Default::default()
            }
            .into(),
        );

        // 3. TUN Mode Toggle
        let current_tun = self.tun_enabled;
        items.push(
            CheckmarkItem::<Self> {
                label: crate::i18n::tr("tray_tun_mode").to_string(),
                checked: current_tun,
                activate: Box::new(move |tray: &mut Self| {
                    let next = !current_tun;
                    tray.tun_enabled = next;
                    tray.kernel_error = false;
                    let svc = tray.service.clone();
                    tokio::spawn(async move {
                        let _ = svc.set_tun(next).await;
                    });
                }),
                ..Default::default()
            }
            .into(),
        );

        // 4. Proxy Mode Submenu
        let current_mode = self.proxy_mode.to_lowercase();
        let modes = [
            ("rule", "tray_mode_rule"),
            ("global", "tray_mode_global"),
            ("direct", "tray_mode_direct"),
        ];

        let mode_items: Vec<MenuItem<Self>> = modes
            .iter()
            .map(|(mode_val, label_key)| {
                let m = mode_val.to_string();
                let is_selected = current_mode == *mode_val;
                CheckmarkItem::<Self> {
                    label: crate::i18n::tr(label_key).to_string(),
                    checked: is_selected,
                    activate: Box::new(move |tray: &mut Self| {
                        tray.proxy_mode = m.clone();
                        let svc = tray.service.clone();
                        let target = m.clone();
                        tokio::spawn(async move {
                            let _ = svc.set_proxy_mode(&target).await;
                        });
                    }),
                    ..Default::default()
                }
                .into()
            })
            .collect();

        items.push(
            SubMenu::<Self> {
                label: crate::i18n::tr("tray_proxy_mode").to_string(),
                submenu: mode_items,
                ..Default::default()
            }
            .into(),
        );

        // 5. Subscription Switch Submenu
        let active_id = self.active_sub_id.clone();
        let sub_items: Vec<MenuItem<Self>> = if self.subscriptions.is_empty() {
            vec![StandardItem::<Self> {
                label: crate::i18n::tr("tray_no_subscriptions").to_string(),
                enabled: false,
                ..Default::default()
            }
            .into()]
        } else {
            self.subscriptions
                .iter()
                .map(|sub| {
                    let sub_id = sub.id.clone();
                    let is_active = active_id.as_deref() == Some(&sub.id) || sub.is_active;
                    CheckmarkItem::<Self> {
                        label: sub.name.clone(),
                        checked: is_active,
                        activate: Box::new(move |tray: &mut Self| {
                            let target_id = sub_id.clone();
                            tray.active_sub_id = Some(target_id.clone());
                            let svc = tray.service.clone();
                            tokio::spawn(async move {
                                let _ = svc.activate_subscription(&target_id).await;
                            });
                        }),
                        ..Default::default()
                    }
                    .into()
                })
                .collect()
        };

        items.push(
            SubMenu::<Self> {
                label: crate::i18n::tr("tray_subscriptions").to_string(),
                submenu: sub_items,
                ..Default::default()
            }
            .into(),
        );

        items.push(MenuItem::Separator);

        // 6. Quit
        items.push(
            StandardItem::<Self> {
                label: crate::i18n::tr("tray_quit").to_string(),
                activate: Box::new(|_| {
                    glib::idle_add_once(|| {
                        if let Some(app) = gio::Application::default() {
                            use gio::prelude::*;
                            app.quit();
                        }
                    });
                }),
                ..Default::default()
            }
            .into(),
        );

        items
    }
}

pub async fn start_tray(service: AppService) -> Option<Handle<GihomoTray>> {
    let proxy_enabled = service.is_proxy_enabled();
    let tun_enabled = service.is_tun_enabled();
    let proxy_mode = service
        .get_proxy_mode()
        .await
        .unwrap_or_else(|_| "rule".to_string());

    let subscriptions = service.get_subscriptions().await.unwrap_or_default();
    let active_sub_id = service.get_active_subscription().await.map(|s| s.id);

    let tray = GihomoTray {
        proxy_enabled,
        tun_enabled,
        proxy_mode,
        kernel_error: false,
        subscriptions,
        active_sub_id,
        service: service.clone(),
    };

    match tray.assume_sni_available(true).spawn().await {
        Ok(handle) => {
            info!("System tray service registered successfully via D-Bus SNI");

            // Background event sync for Proxy/TUN/Mode/Kernel/Subscription changes
            let mut rx = service.event_receiver();
            let handle_clone = handle.clone();
            tokio::spawn(async move {
                loop {
                    match rx.recv().await {
                        Ok(event) => match event {
                            AppEvent::ProxyStatusChanged(enabled) => {
                                let _ = handle_clone
                                    .update(|t| {
                                        t.proxy_enabled = enabled;
                                        t.kernel_error = false;
                                    })
                                    .await;
                            }
                            AppEvent::TunStatusChanged(enabled) => {
                                let _ = handle_clone
                                    .update(|t| {
                                        t.tun_enabled = enabled;
                                        t.kernel_error = false;
                                    })
                                    .await;
                            }
                            AppEvent::ProxyModeChanged(mode) => {
                                let _ = handle_clone.update(|t| t.proxy_mode = mode).await;
                            }
                            AppEvent::KernelStatusChanged(status) => {
                                let is_err = matches!(status, gihomo_core::KernelStatus::Error(_));
                                let _ = handle_clone.update(|t| t.kernel_error = is_err).await;
                            }
                            AppEvent::ErrorOccurred(_) => {
                                let _ = handle_clone.update(|t| t.kernel_error = true).await;
                            }
                            AppEvent::SubscriptionsChanged(subs) => {
                                let _ = handle_clone
                                    .update(|t| {
                                        t.subscriptions = subs;
                                    })
                                    .await;
                            }
                            AppEvent::ActiveSubscriptionChanged(sub_opt) => {
                                let id = sub_opt.map(|s| s.id);
                                let _ = handle_clone
                                    .update(|t| {
                                        t.active_sub_id = id;
                                    })
                                    .await;
                            }
                            _ => {}
                        },
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
            });

            // Re-render tray menu when application language changes
            let mut lang_rx = crate::i18n::language_change_receiver();
            let handle_for_lang = handle.clone();
            tokio::spawn(async move {
                while let Ok(()) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) =
                    lang_rx.recv().await
                {
                    let _ = handle_for_lang.update(|_| ()).await;
                }
            });

            Some(handle)
        }
        Err(e) => {
            error!("Failed to initialize system tray: {}", e);
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_tray_icon_name_resolution() {
        // 1. Idle / Off / Direct -> Gray
        assert_eq!(
            GihomoTray::resolve_icon_name(false, false, false),
            "art.artforge.Gihomo-gray"
        );

        // 2. System Proxy only -> Blue
        assert_eq!(
            GihomoTray::resolve_icon_name(true, false, false),
            "art.artforge.Gihomo-blue"
        );

        // 3. TUN Mode only -> Green
        assert_eq!(
            GihomoTray::resolve_icon_name(false, true, false),
            "art.artforge.Gihomo-green"
        );

        // 4. Both Proxy & TUN active -> Green (TUN takes precedence)
        assert_eq!(
            GihomoTray::resolve_icon_name(true, true, false),
            "art.artforge.Gihomo-green"
        );

        // 5. Error occurred -> Red (Highest priority)
        assert_eq!(
            GihomoTray::resolve_icon_name(true, true, true),
            "art.artforge.Gihomo-red"
        );
        assert_eq!(
            GihomoTray::resolve_icon_name(false, false, true),
            "art.artforge.Gihomo-red"
        );
    }

    #[test]
    fn test_tray_icon_assets_exist() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let icon_dir = manifest_dir.join("../../data/icons/hicolor/scalable/apps");

        let expected_icons = [
            "art.artforge.Gihomo.svg",
            "art.artforge.Gihomo-blue.svg",
            "art.artforge.Gihomo-gray.svg",
            "art.artforge.Gihomo-green.svg",
            "art.artforge.Gihomo-red.svg",
        ];

        for icon in expected_icons {
            let path = icon_dir.join(icon);
            assert!(
                path.exists(),
                "Required tray icon asset does not exist: {:?}",
                path
            );
        }
    }
}
