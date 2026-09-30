use adw::prelude::*;
use gihomo_app::AppService;
use gtk4::subclass::prelude::*;
use std::cell::{Cell, RefCell};
use tracing::{info, warn};

use crate::window::MainWindow;
use crate::APPLICATION_ID;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct GihomoApplication {
        pub service: RefCell<Option<AppService>>,
        pub start_minimized: Cell<bool>,
        pub hold_guard: RefCell<Option<gio::ApplicationHoldGuard>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for GihomoApplication {
        const NAME: &'static str = "GihomoApplication";
        type Type = super::GihomoApplication;
        type ParentType = adw::Application;
    }

    impl ObjectImpl for GihomoApplication {}

    impl ApplicationImpl for GihomoApplication {
        fn handle_local_options(&self, options: &glib::VariantDict) -> glib::ExitCode {
            if options.contains("minimized") {
                self.start_minimized.set(true);
            }
            self.parent_handle_local_options(options)
        }

        fn activate(&self) {
            let app = self.obj();
            let is_minimized = self.start_minimized.replace(false);

            if let Some(window) = app.windows().first() {
                if !is_minimized {
                    window.set_visible(true);
                    window.present();
                }
            } else {
                let service = self
                    .service
                    .borrow()
                    .as_ref()
                    .expect("AppService must be initialized before activate")
                    .clone();

                let window = MainWindow::new(&*app, service);
                if !is_minimized {
                    window.present();
                }
            }
        }

        fn startup(&self) {
            self.parent_startup();
            let app = self.obj();

            // Keep application process running in background even when windows are closed/hidden
            let guard = app.hold();
            *self.hold_guard.borrow_mut() = Some(guard);

            // Register local icon search path
            if let Some(display) = gdk4::Display::default() {
                let icon_theme = gtk4::IconTheme::for_display(&display);
                icon_theme.add_search_path("data/icons");
                icon_theme.add_search_path("data/icons/hicolor/scalable/apps");
            }

            // Launch background system tray
            if let Some(service) = self.service.borrow().clone() {
                let svc_tray = service.clone();
                tokio::spawn(async move {
                    crate::tray::start_tray(svc_tray).await;
                });

                // Auto-start kernel and restore proxy state if configured
                let svc_auto = service.clone();
                tokio::spawn(async move {
                    let config = crate::i18n::load_config();
                    if config.auto_start_kernel {
                        if svc_auto.can_start_kernel().await {
                            info!("Auto-start: Valid configuration found, starting Mihomo kernel in background...");
                            if let Err(e) = svc_auto.start_kernel().await {
                                warn!("Failed to auto-start Mihomo kernel: {}", e);
                            } else {
                                info!("Mihomo kernel auto-started successfully");
                                if config.auto_restore_proxy {
                                    if config.last_proxy_enabled {
                                        info!("Auto-restore: Re-enabling system proxy...");
                                        let _ = svc_auto.toggle_proxy(true);
                                    }
                                    if config.last_tun_enabled {
                                        info!("Auto-restore: Re-enabling TUN mode...");
                                        let _ = svc_auto.set_tun(true).await;
                                    }
                                }
                            }
                        } else {
                            info!("Auto-start: No active subscription or config found; skipping auto-start");
                        }
                    }
                });

                // Launch subscription auto-update background scheduler
                let svc_scheduler = service.clone();
                tokio::spawn(async move {
                    start_auto_update_scheduler(svc_scheduler).await;
                });
            }

            info!("Starting up GihomoApplication (id: {})", APPLICATION_ID);
        }

        fn shutdown(&self) {
            info!("GihomoApplication shutting down, performing system cleanup");
            if let Some(service) = self.service.borrow().clone() {
                // Persist current active proxy/tun state before shutdown resets them
                let proxy_on = service.is_proxy_enabled();
                let tun_on = service.is_tun_enabled();
                crate::i18n::update_last_proxy_state(proxy_on, Some(tun_on));
                crate::i18n::flush_config_writes();

                if let Ok(handle) = tokio::runtime::Handle::try_current() {
                    let (tx, rx) = std::sync::mpsc::channel();
                    handle.spawn(async move {
                        service.shutdown().await;
                        let _ = tx.send(());
                    });
                    let _ = rx.recv_timeout(std::time::Duration::from_secs(2));
                }
            }
            self.parent_shutdown();
        }
    }

    impl GtkApplicationImpl for GihomoApplication {}
    impl adw::subclass::prelude::AdwApplicationImpl for GihomoApplication {}
}

glib::wrapper! {
    pub struct GihomoApplication(ObjectSubclass<imp::GihomoApplication>)
        @extends gio::Application, gtk4::Application, adw::Application,
        @implements gio::ActionGroup, gio::ActionMap;
}

pub type MihomoApplication = GihomoApplication;

impl Default for GihomoApplication {
    fn default() -> Self {
        Self::new()
    }
}

impl GihomoApplication {
    pub fn new() -> Self {
        let app: Self = glib::Object::builder()
            .property("application-id", APPLICATION_ID)
            .property("flags", gio::ApplicationFlags::FLAGS_NONE)
            .build();

        app.add_main_option(
            "minimized",
            glib::Char::from(0),
            glib::OptionFlags::NONE,
            glib::OptionArg::None,
            "Start minimized to system tray",
            None,
        );

        app
    }

    pub fn set_service(&self, service: AppService) {
        *self.imp().service.borrow_mut() = Some(service);
    }
}

async fn start_auto_update_scheduler(service: AppService) {
    info!("Starting background subscription auto-update scheduler");
    let mut last_attempt: Option<std::time::Instant> = None;

    loop {
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;

        let interval_min = crate::i18n::auto_update_interval_minutes_config();
        if interval_min == 0 {
            continue;
        }

        let interval_duration = std::time::Duration::from_secs(interval_min as u64 * 60);

        if let Some(prev) = last_attempt {
            if prev.elapsed() < interval_duration {
                continue;
            }
        }

        let Ok(subs) = service.get_subscriptions().await else {
            continue;
        };

        let remote_subs: Vec<_> = subs
            .into_iter()
            .filter(|s| matches!(s.source, gihomo_core::SubscriptionSource::Url(_)))
            .collect();

        if remote_subs.is_empty() {
            continue;
        }

        let now = chrono::Utc::now();
        let mut due = false;
        for sub in &remote_subs {
            match sub.updated_at {
                None => {
                    due = true;
                    break;
                }
                Some(updated_at) => {
                    if let Ok(elapsed) = (now - updated_at).to_std() {
                        if elapsed >= interval_duration {
                            due = true;
                            break;
                        }
                    } else {
                        due = true;
                        break;
                    }
                }
            }
        }

        if !due {
            continue;
        }

        info!(
            interval_minutes = interval_min,
            "Auto-update: Triggering scheduled update for all subscriptions..."
        );
        last_attempt = Some(std::time::Instant::now());

        match service.update_all_subscriptions().await {
            Ok(summary) if summary.failures.is_empty() => {
                info!(
                    updated = summary.updated,
                    total = summary.total,
                    "Auto-update: All subscriptions updated successfully"
                );
                service.emit_event(gihomo_app::AppEvent::Notification(format!(
                    "{} ({}/{})",
                    crate::i18n::tr("sub_update_all_done"),
                    summary.updated,
                    summary.total
                )));
            }
            Ok(summary) => {
                let details = summary.failures.join("; ");
                warn!(
                    updated = summary.updated,
                    total = summary.total,
                    failures = %details,
                    "Auto-update: Subscriptions partially updated"
                );
                service.emit_event(gihomo_app::AppEvent::ErrorOccurred(format!(
                    "{} ({}/{}): {}",
                    crate::i18n::tr("sub_update_all_partial"),
                    summary.updated,
                    summary.total,
                    details
                )));
            }
            Err(err) => {
                warn!("Auto-update: Failed to update subscriptions: {}", err);
                service.emit_event(gihomo_app::AppEvent::ErrorOccurred(format!(
                    "{}: {}",
                    crate::i18n::tr("toast_sub_failed"),
                    err
                )));
            }
        }
    }
}
