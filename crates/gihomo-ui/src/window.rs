use adw::prelude::*;
use gihomo_app::{AppEvent, AppService};
use gtk4::subclass::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;
use tracing::info;

use crate::views::{
    ConnectionsView, DashboardView, LogsView, ProxiesView, RulesView, SettingsView,
    SubscriptionsView,
};

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct MainWindow {
        pub toast_overlay: adw::ToastOverlay,
        pub split_view: adw::NavigationSplitView,
        pub sidebar_page: adw::NavigationPage,
        pub sidebar_header: adw::HeaderBar,
        pub sidebar_list: gtk4::ListBox,
        pub dashboard: RefCell<Option<Rc<DashboardView>>>,
        pub service: RefCell<Option<AppService>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for MainWindow {
        const NAME: &'static str = "GihomoMainWindow";
        type Type = super::MainWindow;
        type ParentType = adw::ApplicationWindow;
    }

    impl ObjectImpl for MainWindow {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();

            crate::i18n::init_i18n_and_theme();

            obj.set_default_size(980, 680);
            obj.set_title(Some(crate::i18n::tr("app_name")));

            // Configure Adaptive NavigationSplitView
            self.split_view.set_min_sidebar_width(200.0);
            self.split_view.set_max_sidebar_width(260.0);

            // 1. Sidebar View Assembly
            self.sidebar_header.set_show_title(true);
            let sidebar_toolbar = adw::ToolbarView::new();
            sidebar_toolbar.add_top_bar(&self.sidebar_header);

            self.sidebar_list.add_css_class("navigation-sidebar");
            self.sidebar_list
                .set_selection_mode(gtk4::SelectionMode::Browse);

            let scrolled_sidebar = gtk4::ScrolledWindow::builder()
                .hscrollbar_policy(gtk4::PolicyType::Never)
                .vscrollbar_policy(gtk4::PolicyType::Automatic)
                .child(&self.sidebar_list)
                .build();
            sidebar_toolbar.set_content(Some(&scrolled_sidebar));

            self.sidebar_page.set_title(crate::i18n::tr("app_name"));
            self.sidebar_page.set_tag(Some("sidebar"));
            self.sidebar_page.set_child(Some(&sidebar_toolbar));

            self.split_view.set_sidebar(Some(&self.sidebar_page));

            // 2. Configure Adaptive Breakpoint (Ubuntu Settings / GNOME HIG standard: 720px)
            let breakpoint = adw::Breakpoint::new(
                adw::BreakpointCondition::parse("max-width: 720px")
                    .expect("valid breakpoint condition"),
            );
            let val = true.to_value();
            breakpoint.add_setter(&self.split_view, "collapsed", Some(&val));
            obj.add_breakpoint(breakpoint);

            self.toast_overlay.set_child(Some(&self.split_view));
            obj.set_content(Some(&self.toast_overlay));
        }
    }

    impl WidgetImpl for MainWindow {}
    impl WindowImpl for MainWindow {}
    impl ApplicationWindowImpl for MainWindow {}
    impl adw::subclass::prelude::AdwApplicationWindowImpl for MainWindow {}
}

glib::wrapper! {
    pub struct MainWindow(ObjectSubclass<imp::MainWindow>)
        @extends gtk4::Widget, gtk4::Window, gtk4::ApplicationWindow, adw::ApplicationWindow,
        @implements gio::ActionGroup, gio::ActionMap, gtk4::Accessible, gtk4::Buildable,
                    gtk4::ConstraintTarget, gtk4::Native, gtk4::Root, gtk4::ShortcutManager;
}

impl MainWindow {
    fn init_custom_styles() {
        let provider = gtk4::CssProvider::new();
        provider.load_from_string(
            r#"
            list.navigation-sidebar > row.sidebar-separator {
                padding: 0px;
                margin-top: 6px;
                margin-bottom: 6px;
                margin-left: 12px;
                margin-right: 12px;
                min-height: 1px;
                max-height: 1px;
                background: none;
                border: none;
                border-radius: 0px;
                box-shadow: none;
                outline: none;
            }
            list.navigation-sidebar > row.sidebar-separator:hover {
                background: none;
            }
            .sidebar-separator-line {
                min-height: 1px;
                max-height: 1px;
                background-color: alpha(currentColor, 0.15);
            }
            "#,
        );
        if let Some(display) = gtk4::gdk::Display::default() {
            gtk4::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
    }

    pub fn new(app: &impl IsA<gtk4::Application>, service: AppService) -> Self {
        Self::init_custom_styles();
        let win: Self = glib::Object::builder().property("application", app).build();

        win.connect_close_request(|w| {
            if crate::i18n::close_to_tray_config() {
                w.set_visible(false);
                glib::Propagation::Stop
            } else {
                if let Some(app) = gio::Application::default() {
                    app.quit();
                }
                glib::Propagation::Proceed
            }
        });

        win.setup_views_and_events(service);
        win.connect_notify_local(Some("visible"), |w, _| {
            if w.is_visible() {
                w.resync_runtime_state();
            }
        });
        win
    }

    pub fn resync_runtime_state(&self) {
        let imp = self.imp();
        if let (Some(db), Some(service)) = (
            imp.dashboard.borrow().clone(),
            imp.service.borrow().clone(),
        ) {
            glib::MainContext::default().spawn_local(async move {
                if let Ok(status) = service.check_kernel_status().await {
                    db.update_kernel_status(&status);
                }
                db.update_proxy_status(service.is_proxy_enabled());
                db.update_tun_status(service.is_tun_enabled());
                if let Some(active) = service.get_active_subscription().await {
                    db.update_active_subscription(Some(&active));
                }
                let _ = service.fetch_proxies().await;
            });
        }
    }

    fn create_sidebar_item(
        title_key: &'static str,
        icon_name: &'static str,
    ) -> (gtk4::ListBoxRow, gtk4::Label) {
        let row = gtk4::ListBoxRow::new();
        let hbox = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
        hbox.set_margin_top(8);
        hbox.set_margin_bottom(8);
        hbox.set_margin_start(12);
        hbox.set_margin_end(12);

        let icon = gtk4::Image::from_icon_name(icon_name);
        icon.set_pixel_size(18);

        let label = gtk4::Label::builder()
            .label(crate::i18n::tr(title_key))
            .halign(gtk4::Align::Start)
            .hexpand(true)
            .ellipsize(gtk4::pango::EllipsizeMode::End)
            .build();

        hbox.append(&icon);
        hbox.append(&label);
        row.set_child(Some(&hbox));
        (row, label)
    }

    fn create_sidebar_separator() -> gtk4::ListBoxRow {
        let row = gtk4::ListBoxRow::new();
        row.set_selectable(false);
        row.set_activatable(false);
        row.set_focusable(false);
        row.add_css_class("sidebar-separator");

        let line = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
        line.set_height_request(1);
        line.add_css_class("sidebar-separator-line");
        row.set_child(Some(&line));
        row
    }

    fn setup_views_and_events(&self, service: AppService) {
        let imp = self.imp();

        let dashboard = Rc::new(DashboardView::new(service.clone()));
        imp.dashboard.replace(Some(dashboard.clone()));
        imp.service.replace(Some(service.clone()));

        let proxies = Rc::new(ProxiesView::new(service.clone()));
        let rules = Rc::new(RulesView::new(service.clone()));
        let connections = Rc::new(ConnectionsView::new(service.clone()));
        let logs = Rc::new(LogsView::new(service.clone()));
        let subscriptions = Rc::new(SubscriptionsView::new(service.clone()));
        let settings = Rc::new(SettingsView::new(service.clone()));

        subscriptions.set_parent_window(self.clone().upcast::<gtk4::Window>());
        connections.set_parent_window(self.clone().upcast::<gtk4::Window>());

        // Initial content
        imp.split_view.set_content(Some(&dashboard.page));

        // Build Navigation Sidebar items
        let (row_dash, lbl_dash) =
            Self::create_sidebar_item("tab_dashboard", "speedometer-symbolic");
        let (row_proxies, lbl_proxies) =
            Self::create_sidebar_item("tab_proxies", "network-server-symbolic");
        let (row_subs, lbl_subs) =
            Self::create_sidebar_item("tab_subscriptions", "emblem-documents-symbolic");
        let (row_conns, lbl_conns) =
            Self::create_sidebar_item("tab_connections", "network-transmit-receive-symbolic");
        let (row_rules, lbl_rules) =
            Self::create_sidebar_item("tab_rules", "view-list-bullet-symbolic");
        let (row_logs, lbl_logs) =
            Self::create_sidebar_item("tab_logs", "utilities-terminal-symbolic");
        let (row_settings, lbl_settings) =
            Self::create_sidebar_item("tab_settings", "preferences-system-symbolic");

        // Visual separators between functional tiers
        let sep1 = Self::create_sidebar_separator();
        let sep2 = Self::create_sidebar_separator();

        imp.sidebar_list.append(&row_dash);
        imp.sidebar_list.append(&row_proxies);
        imp.sidebar_list.append(&row_subs);
        imp.sidebar_list.append(&sep1);
        imp.sidebar_list.append(&row_conns);
        imp.sidebar_list.append(&row_rules);
        imp.sidebar_list.append(&row_logs);
        imp.sidebar_list.append(&sep2);
        imp.sidebar_list.append(&row_settings);

        // Sidebar Navigation Logic
        let navigate_to = {
            let split_view = imp.split_view.clone();
            let dashboard = dashboard.clone();
            let proxies = proxies.clone();
            let subscriptions = subscriptions.clone();
            let connections = connections.clone();
            let rules = rules.clone();
            let logs = logs.clone();
            let settings = settings.clone();
            let service = service.clone();

            let row_dash = row_dash.clone();
            let row_proxies = row_proxies.clone();
            let row_subs = row_subs.clone();
            let row_conns = row_conns.clone();
            let row_rules = row_rules.clone();
            let row_logs = row_logs.clone();
            let row_settings = row_settings.clone();

            Rc::new(move |selected_row: &gtk4::ListBoxRow| {
                let page: &adw::NavigationPage = if selected_row == &row_dash {
                    &dashboard.page
                } else if selected_row == &row_proxies {
                    let service = service.clone();
                    glib::MainContext::default().spawn_local(async move {
                        let _ = service.fetch_proxies().await;
                    });
                    &proxies.page
                } else if selected_row == &row_subs {
                    let service = service.clone();
                    let sub_clone = subscriptions.clone();
                    glib::MainContext::default().spawn_local(async move {
                        if let Ok(subs) = service.get_subscriptions().await {
                            sub_clone.update_subscriptions(&subs);
                        }
                    });
                    &subscriptions.page
                } else if selected_row == &row_conns {
                    connections.fetch_connections();
                    &connections.page
                } else if selected_row == &row_rules {
                    let service = service.clone();
                    glib::MainContext::default().spawn_local(async move {
                        let _ = service.fetch_rules().await;
                        let _ = service.fetch_rule_providers().await;
                    });
                    &rules.page
                } else if selected_row == &row_logs {
                    &logs.page
                } else if selected_row == &row_settings {
                    &settings.page
                } else {
                    &dashboard.page
                };
                split_view.set_content(Some(page));
                split_view.set_show_content(true);
            })
        };

        {
            let nav = navigate_to.clone();
            imp.sidebar_list.connect_row_selected(move |_, row_opt| {
                if let Some(row) = row_opt {
                    nav(row);
                }
            });
        }
        {
            let nav = navigate_to.clone();
            imp.sidebar_list.connect_row_activated(move |_, row| {
                nav(row);
            });
        }

        // Ensure ProxiesView and RulesView adapt their row layout when collapsed/uncollapsed
        {
            let proxies_clone = proxies.clone();
            let rules_clone = rules.clone();
            let split_view = imp.split_view.clone();
            imp.split_view.connect_collapsed_notify(move |_| {
                let collapsed = split_view.is_collapsed();
                proxies_clone.set_narrow(collapsed);
                rules_clone.set_narrow(collapsed);
            });
        }

        // Default select Dashboard
        imp.sidebar_list.select_row(Some(&row_dash));

        // Listen for Language Changes
        let mut lang_rx = crate::i18n::language_change_receiver();
        let db_for_lang = dashboard.clone();
        let proxies_for_lang = proxies.clone();
        let rules_for_lang = rules.clone();
        let conns_for_lang = connections.clone();
        let logs_for_lang = logs.clone();
        let subs_for_lang = subscriptions.clone();
        let settings_for_lang = settings.clone();
        let sidebar_page_clone = imp.sidebar_page.clone();

        glib::MainContext::default().spawn_local(async move {
            while let Ok(()) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) =
                lang_rx.recv().await
            {
                sidebar_page_clone.set_title(crate::i18n::tr("app_name"));
                lbl_dash.set_label(crate::i18n::tr("tab_dashboard"));
                lbl_proxies.set_label(crate::i18n::tr("tab_proxies"));
                lbl_subs.set_label(crate::i18n::tr("tab_subscriptions"));
                lbl_conns.set_label(crate::i18n::tr("tab_connections"));
                lbl_rules.set_label(crate::i18n::tr("tab_rules"));
                lbl_logs.set_label(crate::i18n::tr("tab_logs"));
                lbl_settings.set_label(crate::i18n::tr("tab_settings"));

                db_for_lang.update_ui_text();
                proxies_for_lang.update_ui_text();
                rules_for_lang.update_ui_text();
                conns_for_lang.update_ui_text();
                logs_for_lang.update_ui_text();
                subs_for_lang.update_ui_text();
                settings_for_lang.update_ui_text();
            }
        });

        // Proactive initial population of subscriptions, active sub, proxies, and rules
        {
            let service = service.clone();
            let sub_clone = subscriptions.clone();
            let db_clone = dashboard.clone();
            let rules_clone = rules.clone();
            glib::MainContext::default().spawn_local(async move {
                if let Ok(subs) = service.get_subscriptions().await {
                    sub_clone.update_subscriptions(&subs);
                    db_clone.update_subscriptions_list(&subs);
                }
                if let Some(active) = service.get_active_subscription().await {
                    db_clone.update_active_subscription(Some(&active));
                }
                let _ = service.fetch_proxies().await;
                if let Ok(rule_list) = service.fetch_rules().await {
                    rules_clone.update_rules(&rule_list);
                }
                if let Ok(providers) = service.fetch_rule_providers().await {
                    rules_clone.update_providers(&providers);
                }
            });
        }

        // Listen for AppEvent via broadcast channel
        let mut rx = service.event_receiver();
        let toast_overlay = imp.toast_overlay.clone();
        let db_clone = dashboard.clone();
        let sub_clone = subscriptions.clone();
        let proxies_clone = proxies.clone();
        let rules_clone = rules.clone();
        let settings_clone = settings.clone();

        glib::MainContext::default().spawn_local(async move {
            loop {
                match rx.recv().await {
                    Ok(event) => match event {
                        AppEvent::ProxyStatusChanged(enabled) => {
                            db_clone.update_proxy_status(enabled);
                            crate::i18n::update_last_proxy_state(enabled, None);
                        }
                        AppEvent::TunStatusChanged(enabled) => {
                            db_clone.update_tun_status(enabled);
                            let proxy_cur = crate::i18n::last_proxy_enabled_config();
                            crate::i18n::update_last_proxy_state(proxy_cur, Some(enabled));
                        }
                        AppEvent::KernelStatusChanged(status) => {
                            db_clone.update_kernel_status(&status);
                        }
                        AppEvent::TrafficUpdated(stats) => {
                            db_clone.update_traffic(&stats);
                        }
                        AppEvent::SubscriptionsChanged(subs) => {
                            sub_clone.update_subscriptions(&subs);
                            db_clone.update_subscriptions_list(&subs);
                        }
                        AppEvent::ActiveSubscriptionChanged(sub_opt) => {
                            db_clone.update_active_subscription(sub_opt.as_ref());
                        }
                        AppEvent::ProxyGroupsUpdated(groups) => {
                            proxies_clone.update_groups(&groups);
                        }
                        AppEvent::ProxyModeChanged(mode) => {
                            proxies_clone.update_proxy_mode(&mode);
                        }
                        AppEvent::RulesUpdated(rule_items) => {
                            rules_clone.update_rules(&rule_items);
                        }
                        AppEvent::GeoUpdated => {
                            settings_clone.update_geo_status();
                        }
                        AppEvent::Notification(msg) => {
                            info!("Notification: {}", msg);
                            let localized_msg = crate::i18n::localize_notification(&msg);
                            toast_overlay.add_toast(adw::Toast::new(&localized_msg));
                        }
                        AppEvent::ErrorOccurred(err) => {
                            let toast = adw::Toast::new(&format!(
                                "{}{}",
                                crate::i18n::tr("error_prefix"),
                                err
                            ));
                            toast.set_priority(adw::ToastPriority::High);
                            toast_overlay.add_toast(toast);
                        }
                    },
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        });
    }
}
