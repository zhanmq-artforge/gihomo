use std::cell::Cell;
use std::rc::Rc;

use adw::prelude::*;
use gihomo_app::AppService;
use gtk4::{Button, Label, Orientation, ScrolledWindow, StringList};

use crate::i18n::{
    current_language_config, current_theme_config, set_language, set_theme, tr, Language, ThemeMode,
};
use crate::APPLICATION_ID;

pub struct SettingsView {
    pub page: adw::NavigationPage,
    general_group: adw::PreferencesGroup,
    autostart_row: adw::SwitchRow,
    close_to_tray_row: adw::SwitchRow,
    auto_start_kernel_row: adw::SwitchRow,
    auto_restore_proxy_row: adw::SwitchRow,
    appearance_group: adw::PreferencesGroup,
    theme_row: adw::ComboRow,
    is_updating_theme: Rc<Cell<bool>>,
    lang_row: adw::ComboRow,
    net_group: adw::PreferencesGroup,
    mixed_port_row: adw::ActionRow,
    controller_row: adw::ActionRow,
    kernel_group: adw::PreferencesGroup,
    kernel_path_row: adw::ActionRow,
    tun_cap_row: adw::ActionRow,
    tun_cap_badge: Label,
    btn_auth_tun: Button,
    open_config_btn: Button,
    open_logs_btn: Button,
    geo_group: adw::PreferencesGroup,
    geoip_row: adw::ActionRow,
    geosite_row: adw::ActionRow,
    update_geo_btn: Button,
    about_group: adw::PreferencesGroup,
    app_id_row: adw::ActionRow,
    version_row: adw::ActionRow,
    service: AppService,
}

impl SettingsView {
    pub fn new(service: AppService) -> Self {
        let header_bar = adw::HeaderBar::builder()
            .show_title(true)
            .centering_policy(adw::CenteringPolicy::Strict)
            .build();
        let toolbar_view = adw::ToolbarView::new();
        toolbar_view.add_top_bar(&header_bar);

        let clamp = adw::Clamp::builder().maximum_size(700).build();
        let content_box = gtk4::Box::new(Orientation::Vertical, 16);
        content_box.set_margin_top(16);
        content_box.set_margin_bottom(24);
        content_box.set_margin_start(12);
        content_box.set_margin_end(12);

        // Group 1: General (Autostart & Tray)
        let general_group = adw::PreferencesGroup::builder()
            .title(tr("settings_general"))
            .description(tr("settings_general_desc"))
            .build();

        let autostart_row = adw::SwitchRow::builder()
            .title(tr("settings_autostart"))
            .subtitle(tr("settings_autostart_sub"))
            .title_lines(1)
            .subtitle_lines(1)
            .active(false)
            .build();

        let service_for_autostart = service.clone();
        let row_for_autostart = autostart_row.clone();
        glib::MainContext::default().spawn_local(async move {
            if let Ok(enabled) = service_for_autostart.autostart_enabled().await {
                row_for_autostart.set_active(enabled);
            }
        });
        let service_for_autostart = service.clone();
        autostart_row.connect_active_notify(move |row| {
            let active = row.is_active();
            let service = service_for_autostart.clone();
            glib::MainContext::default().spawn_local(async move {
                if let Err(error) = service.set_autostart(active).await {
                    tracing::warn!("Failed to update autostart setting: {}", error);
                }
            });
        });

        let close_to_tray_row = adw::SwitchRow::builder()
            .title(tr("settings_close_to_tray"))
            .subtitle(tr("settings_close_to_tray_sub"))
            .title_lines(1)
            .subtitle_lines(1)
            .active(crate::i18n::close_to_tray_config())
            .build();
        close_to_tray_row.connect_active_notify(|row| {
            crate::i18n::set_close_to_tray(row.is_active());
        });

        let auto_start_kernel_row = adw::SwitchRow::builder()
            .title(tr("settings_auto_start_kernel"))
            .subtitle(tr("settings_auto_start_kernel_sub"))
            .title_lines(1)
            .subtitle_lines(2)
            .active(crate::i18n::auto_start_kernel_config())
            .build();
        auto_start_kernel_row.connect_active_notify(|row| {
            crate::i18n::set_auto_start_kernel(row.is_active());
        });

        let auto_restore_proxy_row = adw::SwitchRow::builder()
            .title(tr("settings_auto_restore_proxy"))
            .subtitle(tr("settings_auto_restore_proxy_sub"))
            .title_lines(1)
            .subtitle_lines(2)
            .active(crate::i18n::auto_restore_proxy_config())
            .build();
        auto_restore_proxy_row.connect_active_notify(|row| {
            crate::i18n::set_auto_restore_proxy(row.is_active());
        });

        general_group.add(&autostart_row);
        general_group.add(&close_to_tray_row);
        general_group.add(&auto_start_kernel_row);
        general_group.add(&auto_restore_proxy_row);

        // Group 2: Appearance & Language
        let appearance_group = adw::PreferencesGroup::builder()
            .title(tr("settings_appearance"))
            .description(tr("settings_appearance_desc"))
            .build();

        // Theme ComboRow
        let theme_row = adw::ComboRow::builder().title(tr("settings_theme")).build();

        let theme_model =
            StringList::new(&[tr("theme_system"), tr("theme_light"), tr("theme_dark")]);
        theme_row.set_model(Some(&theme_model));
        theme_row.set_selected(current_theme_config().to_index());

        let is_updating_theme = Rc::new(Cell::new(false));
        let is_updating_theme_clone = is_updating_theme.clone();
        theme_row.connect_selected_notify(move |row| {
            if is_updating_theme_clone.get() {
                return;
            }
            let selected = row.selected();
            let theme = ThemeMode::from_index(selected);
            set_theme(theme);
        });

        // Language ComboRow
        let lang_row = adw::ComboRow::builder()
            .title(tr("settings_language"))
            .build();

        let lang_model = StringList::new(&[
            "跟随系统 (System Default)",
            "简体中文 (Simplified Chinese)",
            "English",
        ]);
        lang_row.set_model(Some(&lang_model));
        lang_row.set_selected(current_language_config().to_index());

        lang_row.connect_selected_notify(|row| {
            let selected = row.selected();
            let lang = Language::from_index(selected);
            set_language(lang);
        });

        appearance_group.add(&theme_row);
        appearance_group.add(&lang_row);

        // Group 2: Network & Ports
        let net_group = adw::PreferencesGroup::builder()
            .title(tr("settings_net_ports"))
            .description(tr("settings_net_ports_desc"))
            .build();

        let mixed_port_row = adw::ActionRow::builder()
            .title(tr("settings_mixed_port"))
            .subtitle(tr("settings_mixed_port_sub"))
            .title_lines(1)
            .subtitle_lines(1)
            .build();

        let controller_row = adw::ActionRow::builder()
            .title(tr("settings_controller"))
            .subtitle(tr("settings_controller_sub"))
            .title_lines(1)
            .subtitle_lines(1)
            .build();

        net_group.add(&mixed_port_row);
        net_group.add(&controller_row);

        // Group 3: Mihomo Kernel & Privileges
        let kernel_group = adw::PreferencesGroup::builder()
            .title(tr("settings_kernel"))
            .description(tr("settings_kernel_desc"))
            .build();

        let kernel_bin_path = service
            .find_kernel_binary()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| tr("kernel_not_found").to_string());

        let kernel_path_row = adw::ActionRow::builder()
            .title(tr("settings_kernel_path"))
            .subtitle(&kernel_bin_path)
            .title_lines(1)
            .subtitle_lines(1)
            .build();

        let open_config_btn = Button::builder()
            .label(tr("btn_open_config_dir"))
            .valign(gtk4::Align::Center)
            .css_classes(["flat"])
            .build();

        let mihomo_dir = service.mihomo_dir();
        open_config_btn.connect_clicked(move |_| {
            let uri = format!("file://{}", mihomo_dir.display());
            let _ = gio::AppInfo::launch_default_for_uri(&uri, None::<&gio::AppLaunchContext>);
        });

        kernel_path_row.add_suffix(&open_config_btn);
        kernel_group.add(&kernel_path_row);

        // TUN Capabilities Row
        let tun_cap_row = adw::ActionRow::builder()
            .title(tr("settings_tun_cap"))
            .title_lines(1)
            .subtitle_lines(1)
            .build();

        let tun_cap_badge = Label::builder()
            .label("...")
            .css_classes(["pill", "caption"])
            .valign(gtk4::Align::Center)
            .build();

        let btn_auth_tun = Button::builder()
            .label(tr("btn_authorize_tun"))
            .valign(gtk4::Align::Center)
            .css_classes(["suggested-action"])
            .visible(false)
            .build();

        let open_logs_btn = Button::builder()
            .label(tr("btn_open_log_dir"))
            .valign(gtk4::Align::Center)
            .css_classes(["flat"])
            .build();

        let logs_dir = service.logs_dir();
        open_logs_btn.connect_clicked(move |_| {
            let uri = format!("file://{}", logs_dir.display());
            let _ = gio::AppInfo::launch_default_for_uri(&uri, None::<&gio::AppLaunchContext>);
        });

        // Wire TUN Auth Button
        {
            let service_clone = service.clone();
            let badge_clone = tun_cap_badge.clone();
            let btn_clone = btn_auth_tun.clone();

            btn_auth_tun.connect_clicked(move |_| {
                let service = service_clone.clone();
                let badge = badge_clone.clone();
                let btn = btn_clone.clone();

                glib::MainContext::default().spawn_local(async move {
                    let _ = service.request_tun_permissions().await;
                    let cap_ok = service.check_tun_capabilities().await;
                    if cap_ok {
                        badge.set_label(tr("settings_tun_cap_granted"));
                        badge.set_css_classes(&["pill", "caption", "success"]);
                        btn.set_visible(false);
                    } else {
                        badge.set_label(tr("settings_tun_cap_missing"));
                        badge.set_css_classes(&["pill", "caption", "error"]);
                        btn.set_visible(true);
                    }
                });
            });
        }

        // Initial TUN check
        {
            let service_clone = service.clone();
            let badge_clone = tun_cap_badge.clone();
            let btn_clone = btn_auth_tun.clone();

            glib::MainContext::default().spawn_local(async move {
                let cap_ok = service_clone.check_tun_capabilities().await;
                if cap_ok {
                    badge_clone.set_label(tr("settings_tun_cap_granted"));
                    badge_clone.set_css_classes(&["pill", "caption", "success"]);
                    btn_clone.set_visible(false);
                } else {
                    badge_clone.set_label(tr("settings_tun_cap_missing"));
                    badge_clone.set_css_classes(&["pill", "caption", "error"]);
                    btn_clone.set_visible(true);
                }
            });
        }

        tun_cap_row.add_suffix(&tun_cap_badge);
        tun_cap_row.add_suffix(&btn_auth_tun);
        tun_cap_row.add_suffix(&open_logs_btn);
        kernel_group.add(&tun_cap_row);

        // Group 4: Geo Database Management
        let geo_group = adw::PreferencesGroup::builder()
            .title(tr("settings_geo"))
            .description(tr("settings_geo_desc"))
            .build();

        let geoip_row = adw::ActionRow::builder()
            .title("GeoIP (geoip.metadb)")
            .subtitle(tr("geo_checking"))
            .build();

        let geosite_row = adw::ActionRow::builder()
            .title("GeoSite (geosite.dat)")
            .subtitle(tr("geo_checking"))
            .build();

        let update_geo_btn = Button::builder()
            .label(tr("btn_update_geo"))
            .valign(gtk4::Align::Center)
            .css_classes(["suggested-action"])
            .build();

        geo_group.set_header_suffix(Some(&update_geo_btn));
        geo_group.add(&geoip_row);
        geo_group.add(&geosite_row);

        // Initial Geo DB status fetch
        {
            let svc = service.clone();
            let gip = geoip_row.clone();
            let gst = geosite_row.clone();
            glib::MainContext::default().spawn_local(async move {
                let infos = svc.get_geo_databases().await;
                for info in &infos {
                    let sub = format_geo_info(info);
                    if info.name == "GeoIP" {
                        gip.set_subtitle(&sub);
                    } else if info.name == "GeoSite" {
                        gst.set_subtitle(&sub);
                    }
                }
            });
        }

        // Wire Update Geo Button
        {
            let service_clone = service.clone();
            let btn_clone = update_geo_btn.clone();
            let gip_clone = geoip_row.clone();
            let gst_clone = geosite_row.clone();
            update_geo_btn.connect_clicked(move |_| {
                btn_clone.set_sensitive(false);
                btn_clone.set_label(tr("sub_refreshing"));
                let service = service_clone.clone();
                let btn = btn_clone.clone();
                let gip = gip_clone.clone();
                let gst = gst_clone.clone();
                glib::MainContext::default().spawn_local(async move {
                    let _ = service.update_geo_databases().await;
                    let infos = service.get_geo_databases().await;
                    for info in &infos {
                        let sub = format_geo_info(info);
                        if info.name == "GeoIP" {
                            gip.set_subtitle(&sub);
                        } else if info.name == "GeoSite" {
                            gst.set_subtitle(&sub);
                        }
                    }
                    btn.set_label(tr("btn_update_geo"));
                    btn.set_sensitive(true);
                });
            });
        }

        // Group 5: About
        let about_group = adw::PreferencesGroup::builder()
            .title(tr("settings_about"))
            .build();

        let app_id_row = adw::ActionRow::builder()
            .title(tr("settings_app_id"))
            .subtitle(APPLICATION_ID)
            .title_lines(1)
            .subtitle_lines(1)
            .build();

        let version_row = adw::ActionRow::builder()
            .title(tr("settings_version"))
            .subtitle(env!("CARGO_PKG_VERSION"))
            .title_lines(1)
            .subtitle_lines(1)
            .build();

        about_group.add(&app_id_row);
        about_group.add(&version_row);

        // Assemble
        content_box.append(&general_group);
        content_box.append(&appearance_group);
        content_box.append(&net_group);
        content_box.append(&kernel_group);
        content_box.append(&geo_group);
        content_box.append(&about_group);

        clamp.set_child(Some(&content_box));

        let scrolled = ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .vexpand(true)
            .hexpand(true)
            .child(&clamp)
            .build();

        toolbar_view.set_content(Some(&scrolled));

        let page = adw::NavigationPage::builder()
            .title(tr("tab_settings"))
            .tag("settings")
            .child(&toolbar_view)
            .build();

        Self {
            page,
            general_group,
            autostart_row,
            close_to_tray_row,
            auto_start_kernel_row,
            auto_restore_proxy_row,
            appearance_group,
            theme_row,
            is_updating_theme,
            lang_row,
            net_group,
            mixed_port_row,
            controller_row,
            kernel_group,
            kernel_path_row,
            tun_cap_row,
            tun_cap_badge,
            btn_auth_tun,
            open_config_btn,
            open_logs_btn,
            geo_group,
            geoip_row,
            geosite_row,
            update_geo_btn,
            about_group,
            app_id_row,
            version_row,
            service,
        }
    }

    pub fn update_ui_text(&self) {
        self.page.set_title(tr("tab_settings"));
        self.general_group.set_title(tr("settings_general"));
        self.general_group
            .set_description(Some(tr("settings_general_desc")));
        self.autostart_row.set_title(tr("settings_autostart"));
        self.autostart_row
            .set_subtitle(tr("settings_autostart_sub"));
        self.close_to_tray_row
            .set_title(tr("settings_close_to_tray"));
        self.close_to_tray_row
            .set_subtitle(tr("settings_close_to_tray_sub"));
        self.auto_start_kernel_row
            .set_title(tr("settings_auto_start_kernel"));
        self.auto_start_kernel_row
            .set_subtitle(tr("settings_auto_start_kernel_sub"));
        self.auto_restore_proxy_row
            .set_title(tr("settings_auto_restore_proxy"));
        self.auto_restore_proxy_row
            .set_subtitle(tr("settings_auto_restore_proxy_sub"));

        self.appearance_group.set_title(tr("settings_appearance"));
        self.appearance_group
            .set_description(Some(tr("settings_appearance_desc")));
        self.theme_row.set_title(tr("settings_theme"));
        let selected_theme = current_theme_config().to_index();
        self.is_updating_theme.set(true);
        let theme_model =
            StringList::new(&[tr("theme_system"), tr("theme_light"), tr("theme_dark")]);
        self.theme_row.set_model(Some(&theme_model));
        self.theme_row.set_selected(selected_theme);
        self.is_updating_theme.set(false);

        self.lang_row.set_title(tr("settings_language"));

        self.net_group.set_title(tr("settings_net_ports"));
        self.net_group
            .set_description(Some(tr("settings_net_ports_desc")));
        self.mixed_port_row.set_title(tr("settings_mixed_port"));
        self.mixed_port_row
            .set_subtitle(tr("settings_mixed_port_sub"));
        self.controller_row.set_title(tr("settings_controller"));
        self.controller_row
            .set_subtitle(tr("settings_controller_sub"));

        self.kernel_group.set_title(tr("settings_kernel"));
        self.kernel_group
            .set_description(Some(tr("settings_kernel_desc")));
        self.kernel_path_row.set_title(tr("settings_kernel_path"));
        self.tun_cap_row.set_title(tr("settings_tun_cap"));
        self.btn_auth_tun.set_label(tr("btn_authorize_tun"));
        self.open_config_btn.set_label(tr("btn_open_config_dir"));
        self.open_logs_btn.set_label(tr("btn_open_log_dir"));

        let svc = self.service.clone();
        let badge = self.tun_cap_badge.clone();
        let btn = self.btn_auth_tun.clone();
        glib::MainContext::default().spawn_local(async move {
            let cap_ok = svc.check_tun_capabilities().await;
            if cap_ok {
                badge.set_label(tr("settings_tun_cap_granted"));
                badge.set_css_classes(&["pill", "caption", "success"]);
                btn.set_visible(false);
            } else {
                badge.set_label(tr("settings_tun_cap_missing"));
                badge.set_css_classes(&["pill", "caption", "error"]);
                btn.set_visible(true);
            }
        });

        self.geo_group.set_title(tr("settings_geo"));
        self.geo_group
            .set_description(Some(tr("settings_geo_desc")));
        self.update_geo_btn.set_label(tr("btn_update_geo"));
        self.update_geo_status();

        self.about_group.set_title(tr("settings_about"));
        self.app_id_row.set_title(tr("settings_app_id"));
        self.version_row.set_title(tr("settings_version"));
    }

    pub fn update_geo_status(&self) {
        let service = self.service.clone();
        let gip = self.geoip_row.clone();
        let gst = self.geosite_row.clone();
        glib::MainContext::default().spawn_local(async move {
            let infos = service.get_geo_databases().await;
            for info in &infos {
                let sub = format_geo_info(info);
                if info.name == "GeoIP" {
                    gip.set_subtitle(&sub);
                } else if info.name == "GeoSite" {
                    gst.set_subtitle(&sub);
                }
            }
        });
    }
}

fn format_geo_info(info: &gihomo_core::GeoDatabaseInfo) -> String {
    if info.exists {
        let size_mb = info.size_bytes as f64 / (1024.0 * 1024.0);
        let mtime = info
            .updated_at
            .map(|t| {
                t.with_timezone(&chrono::Local)
                    .format("%Y-%m-%d %H:%M")
                    .to_string()
            })
            .unwrap_or_else(|| tr("geo_db_unknown").to_string());
        tr("geo_db_info")
            .replace("{size}", &format!("{:.2}", size_mb))
            .replace("{time}", &mtime)
    } else {
        tr("geo_not_installed").to_string()
    }
}
