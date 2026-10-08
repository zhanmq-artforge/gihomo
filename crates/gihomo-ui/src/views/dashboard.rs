use adw::prelude::*;
use gihomo_app::AppService;
use gihomo_core::{KernelStatus, Subscription, SubscriptionUserInfo, TrafficStats};
use gtk4::{Button, Label, Orientation, ProgressBar, ScrolledWindow};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::i18n::tr;

pub struct DashboardView {
    pub page: adw::NavigationPage,
    control_group: adw::PreferencesGroup,
    proxy_switch: adw::SwitchRow,
    tun_switch: adw::SwitchRow,
    kernel_row: adw::ActionRow,
    kernel_badge: Label,
    kernel_spinner: gtk4::Spinner,
    btn_start_kernel: Button,
    btn_restart_kernel: Button,
    btn_stop_kernel: Button,
    stats_group: adw::PreferencesGroup,
    up_title: Label,
    down_title: Label,
    upload_speed_label: Label,
    download_speed_label: Label,
    upload_total_label: Label,
    download_total_label: Label,
    active_sub_group: adw::PreferencesGroup,
    active_sub_row: adw::ActionRow,
    btn_refresh_sub: Button,
    sub_refresh_spinner: gtk4::Spinner,
    sub_dropdown: gtk4::DropDown,
    package_traffic_row: adw::ActionRow,
    package_pbar: ProgressBar,
    last_traffic: Rc<RefCell<(u64, u64)>>, // (last_up_total, last_down_total)
    subs_cache: Rc<RefCell<Vec<Subscription>>>,
    is_updating_dropdown: Rc<Cell<bool>>,
    is_updating_proxy: Rc<Cell<bool>>,
    is_updating_tun: Rc<Cell<bool>>,
    current_status: Rc<RefCell<KernelStatus>>,
    current_active_sub: Rc<RefCell<Option<Subscription>>>,
}

impl DashboardView {
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

        // Group 1: Quick Controls
        let control_group = adw::PreferencesGroup::builder()
            .title(tr("ctrl_group_title"))
            .tooltip_text(tr("ctrl_group_desc"))
            .build();

        // System Proxy Switch
        let proxy_switch = adw::SwitchRow::builder()
            .title(tr("sys_proxy_title"))
            .subtitle(tr("sys_proxy_sub"))
            .title_lines(1)
            .subtitle_lines(2)
            .active(service.is_proxy_enabled())
            .build();

        // TUN Switch
        let tun_switch = adw::SwitchRow::builder()
            .title(tr("tun_title"))
            .subtitle(tr("tun_sub"))
            .title_lines(1)
            .subtitle_lines(2)
            .active(service.is_tun_enabled())
            .build();

        // Mihomo Kernel Row
        let kernel_row = adw::ActionRow::builder()
            .title(tr("kernel_status_title"))
            .subtitle(tr("kernel_sub_not_found"))
            .title_lines(1)
            .subtitle_lines(2)
            .build();

        let kernel_badge = Label::builder()
            .label("...")
            .css_classes(["pill", "caption", "dim-label"])
            .valign(gtk4::Align::Center)
            .build();

        let kernel_spinner = gtk4::Spinner::builder()
            .valign(gtk4::Align::Center)
            .visible(false)
            .build();

        let btn_start_kernel = Button::builder()
            .icon_name("media-playback-start-symbolic")
            .tooltip_text(tr("btn_start_kernel_tooltip"))
            .valign(gtk4::Align::Center)
            .css_classes(["flat"])
            .build();

        let btn_restart_kernel = Button::builder()
            .icon_name("view-refresh-symbolic")
            .tooltip_text(tr("btn_restart_kernel_tooltip"))
            .valign(gtk4::Align::Center)
            .css_classes(["flat"])
            .build();

        let btn_stop_kernel = Button::builder()
            .icon_name("media-playback-stop-symbolic")
            .tooltip_text(tr("btn_stop_kernel_tooltip"))
            .valign(gtk4::Align::Center)
            .css_classes(["flat"])
            .build();

        let kernel_actions_box = gtk4::Box::new(Orientation::Horizontal, 4);
        kernel_actions_box.set_valign(gtk4::Align::Center);
        kernel_actions_box.append(&btn_start_kernel);
        kernel_actions_box.append(&btn_restart_kernel);
        kernel_actions_box.append(&btn_stop_kernel);

        kernel_row.add_suffix(&kernel_spinner);
        kernel_row.add_suffix(&kernel_badge);
        kernel_row.add_suffix(&kernel_actions_box);

        control_group.add(&proxy_switch);
        control_group.add(&tun_switch);
        control_group.add(&kernel_row);

        // Group 2: Traffic Stats
        let stats_group = adw::PreferencesGroup::builder()
            .title(tr("traffic_title"))
            .tooltip_text(tr("traffic_desc"))
            .build();

        let stats_row = adw::ActionRow::builder().build();

        let stats_box = gtk4::Box::new(Orientation::Horizontal, 24);
        stats_box.set_hexpand(true);
        stats_box.set_halign(gtk4::Align::Fill);
        stats_box.set_margin_top(8);
        stats_box.set_margin_bottom(8);

        // Upload Item
        let up_box = gtk4::Box::new(Orientation::Vertical, 4);
        up_box.set_hexpand(true);
        let up_title = Label::builder()
            .label(tr("realtime_up"))
            .css_classes(["dim-label", "caption"])
            .build();
        let upload_speed_label = Label::builder()
            .label("0.00 KB/s")
            .css_classes(["title-3"])
            .build();
        let upload_total_label = Label::builder()
            .label(format!("{}0 B", tr("total_prefix")))
            .css_classes(["caption", "dim-label"])
            .build();
        up_box.append(&up_title);
        up_box.append(&upload_speed_label);
        up_box.append(&upload_total_label);

        // Download Item
        let down_box = gtk4::Box::new(Orientation::Vertical, 4);
        down_box.set_hexpand(true);
        let down_title = Label::builder()
            .label(tr("realtime_down"))
            .css_classes(["dim-label", "caption"])
            .build();
        let download_speed_label = Label::builder()
            .label("0.00 KB/s")
            .css_classes(["title-3"])
            .build();
        let download_total_label = Label::builder()
            .label(format!("{}0 B", tr("total_prefix")))
            .css_classes(["caption", "dim-label"])
            .build();
        down_box.append(&down_title);
        down_box.append(&download_speed_label);
        down_box.append(&download_total_label);

        stats_box.append(&up_box);
        stats_box.append(&down_box);
        stats_row.set_child(Some(&stats_box));
        stats_group.add(&stats_row);

        // Group 3: Active Subscription & Quick Switch
        let active_sub_group = adw::PreferencesGroup::builder()
            .title(tr("active_sub_title"))
            .tooltip_text(tr("active_sub_desc"))
            .build();

        let active_sub_row = adw::ActionRow::builder()
            .title(tr("active_sub_row"))
            .subtitle(tr("no_active_sub"))
            .title_lines(1)
            .subtitle_lines(1)
            .build();

        let sub_refresh_spinner = gtk4::Spinner::builder()
            .valign(gtk4::Align::Center)
            .visible(false)
            .build();

        let btn_refresh_sub = Button::builder()
            .icon_name("view-refresh-symbolic")
            .tooltip_text(tr("tooltip_refresh_sub"))
            .valign(gtk4::Align::Center)
            .css_classes(["flat", "circular"])
            .sensitive(false)
            .build();

        let sub_dropdown = gtk4::DropDown::builder()
            .valign(gtk4::Align::Center)
            .build();

        active_sub_row.add_suffix(&sub_refresh_spinner);
        active_sub_row.add_suffix(&btn_refresh_sub);
        active_sub_row.add_suffix(&sub_dropdown);

        let package_traffic_row = adw::ActionRow::builder()
            .title(tr("package_traffic_title"))
            .subtitle("...")
            .title_lines(1)
            .subtitle_lines(1)
            .visible(false)
            .build();

        let package_pbar = ProgressBar::builder()
            .valign(gtk4::Align::Center)
            .hexpand(true)
            .build();
        package_traffic_row.add_suffix(&package_pbar);

        active_sub_group.add(&active_sub_row);
        active_sub_group.add(&package_traffic_row);

        // Assemble
        content_box.append(&control_group);
        content_box.append(&stats_group);
        content_box.append(&active_sub_group);

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
            .title(tr("tab_dashboard"))
            .tag("dashboard")
            .child(&toolbar_view)
            .build();

        let subs_cache: Rc<RefCell<Vec<Subscription>>> = Rc::new(RefCell::new(Vec::new()));
        let is_updating_dropdown = Rc::new(Cell::new(false));
        let is_updating_proxy = Rc::new(Cell::new(false));
        let is_updating_tun = Rc::new(Cell::new(false));
        let current_status = Rc::new(RefCell::new(KernelStatus::NotFound));
        let current_active_sub: Rc<RefCell<Option<Subscription>>> = Rc::new(RefCell::new(None));

        // Wire Interactions: Proxy
        {
            let service = service.clone();
            let is_updating = is_updating_proxy.clone();
            proxy_switch.connect_active_notify(move |switch| {
                if is_updating.get() {
                    return;
                }
                let active = switch.is_active();
                let _ = service.toggle_proxy(active);
            });
        }

        // Wire Interactions: TUN
        {
            let service = service.clone();
            let is_updating = is_updating_tun.clone();
            tun_switch.connect_active_notify(move |switch| {
                if is_updating.get() {
                    return;
                }
                let active = switch.is_active();
                let service = service.clone();
                glib::MainContext::default().spawn_local(async move {
                    if let Err(e) = service.set_tun(active).await {
                        tracing::error!("Failed to toggle TUN: {}", e);
                    }
                });
            });
        }

        // Wire Interactions: Kernel buttons
        {
            let service = service.clone();
            let spinner = kernel_spinner.clone();
            let badge = kernel_badge.clone();
            let btn_start = btn_start_kernel.clone();
            let btn_restart = btn_restart_kernel.clone();
            let btn_stop = btn_stop_kernel.clone();
            btn_start_kernel.connect_clicked(move |_| {
                btn_start.set_sensitive(false);
                btn_restart.set_sensitive(false);
                btn_stop.set_sensitive(false);
                spinner.set_visible(true);
                spinner.start();
                badge.set_label(tr("kernel_starting"));
                badge.set_css_classes(&["pill", "caption", "warning"]);

                let service = service.clone();
                let spinner = spinner.clone();
                glib::MainContext::default().spawn_local(async move {
                    let _ = service.start_kernel().await;
                    spinner.stop();
                    spinner.set_visible(false);
                });
            });
        }

        {
            let service = service.clone();
            let spinner = kernel_spinner.clone();
            let badge = kernel_badge.clone();
            let btn_start = btn_start_kernel.clone();
            let btn_restart = btn_restart_kernel.clone();
            let btn_stop = btn_stop_kernel.clone();
            btn_restart_kernel.connect_clicked(move |_| {
                btn_start.set_sensitive(false);
                btn_restart.set_sensitive(false);
                btn_stop.set_sensitive(false);
                spinner.set_visible(true);
                spinner.start();
                badge.set_label(tr("kernel_restarting"));
                badge.set_css_classes(&["pill", "caption", "warning"]);

                let service = service.clone();
                let spinner = spinner.clone();
                glib::MainContext::default().spawn_local(async move {
                    let _ = service.restart_kernel().await;
                    spinner.stop();
                    spinner.set_visible(false);
                });
            });
        }

        {
            let service = service.clone();
            let spinner = kernel_spinner.clone();
            let badge = kernel_badge.clone();
            let btn_start = btn_start_kernel.clone();
            let btn_restart = btn_restart_kernel.clone();
            let btn_stop = btn_stop_kernel.clone();
            btn_stop_kernel.connect_clicked(move |_| {
                btn_start.set_sensitive(false);
                btn_restart.set_sensitive(false);
                btn_stop.set_sensitive(false);
                spinner.set_visible(true);
                spinner.start();
                badge.set_label(tr("kernel_stopping"));
                badge.set_css_classes(&["pill", "caption", "warning"]);

                let service = service.clone();
                let spinner = spinner.clone();
                glib::MainContext::default().spawn_local(async move {
                    let _ = service.stop_kernel().await;
                    spinner.stop();
                    spinner.set_visible(false);
                });
            });
        }

        // Wire Interactions: Dropdown Subscription Switch
        {
            let service = service.clone();
            let subs_cache = subs_cache.clone();
            let is_updating = is_updating_dropdown.clone();

            sub_dropdown.connect_selected_notify(move |dropdown| {
                if is_updating.get() {
                    return;
                }
                let idx = dropdown.selected() as usize;
                let subs = subs_cache.borrow();
                if let Some(target) = subs.get(idx) {
                    if !target.is_active {
                        let sub_id = target.id.clone();
                        let service = service.clone();
                        glib::MainContext::default().spawn_local(async move {
                            let _ = service.activate_subscription(&sub_id).await;
                        });
                    }
                }
            });
        }

        // Wire Interactions: Refresh Active Subscription
        {
            let service = service.clone();
            let current_active_sub = current_active_sub.clone();
            let btn_refresh = btn_refresh_sub.clone();
            let spinner = sub_refresh_spinner.clone();
            let dropdown = sub_dropdown.clone();

            btn_refresh_sub.connect_clicked(move |_| {
                let sub_opt = current_active_sub.borrow().clone();
                if let Some(sub) = sub_opt {
                    btn_refresh.set_sensitive(false);
                    dropdown.set_sensitive(false);
                    spinner.set_visible(true);
                    spinner.start();

                    let service = service.clone();
                    let btn_refresh = btn_refresh.clone();
                    let dropdown = dropdown.clone();
                    let spinner = spinner.clone();
                    let sub_id = sub.id.clone();
                    glib::MainContext::default().spawn_local(async move {
                        if let Err(err) = service.update_subscription(&sub_id).await {
                            service.emit_event(gihomo_app::AppEvent::ErrorOccurred(format!(
                                "{}: {}",
                                tr("toast_sub_failed"),
                                err
                            )));
                        }
                        spinner.stop();
                        spinner.set_visible(false);
                        btn_refresh.set_sensitive(true);
                        dropdown.set_sensitive(true);
                    });
                }
            });
        }

        Self {
            page,
            control_group,
            proxy_switch,
            tun_switch,
            kernel_row,
            kernel_badge,
            kernel_spinner,
            btn_start_kernel,
            btn_restart_kernel,
            btn_stop_kernel,
            stats_group,
            up_title,
            down_title,
            upload_speed_label,
            download_speed_label,
            upload_total_label,
            download_total_label,
            active_sub_group,
            active_sub_row,
            btn_refresh_sub,
            sub_refresh_spinner,
            sub_dropdown,
            package_traffic_row,
            package_pbar,
            last_traffic: Rc::new(RefCell::new((0, 0))),
            subs_cache,
            is_updating_dropdown,
            is_updating_proxy,
            is_updating_tun,
            current_status,
            current_active_sub,
        }
    }

    pub fn update_ui_text(&self) {
        self.page.set_title(tr("tab_dashboard"));
        self.control_group.set_title(tr("ctrl_group_title"));
        self.control_group.set_description(None);
        self.control_group
            .set_tooltip_text(Some(tr("ctrl_group_desc")));
        self.proxy_switch.set_title(tr("sys_proxy_title"));
        self.proxy_switch.set_subtitle(tr("sys_proxy_sub"));
        self.tun_switch.set_title(tr("tun_title"));
        self.tun_switch.set_subtitle(tr("tun_sub"));
        self.kernel_row.set_title(tr("kernel_status_title"));
        self.btn_start_kernel
            .set_tooltip_text(Some(tr("btn_start_kernel_tooltip")));
        self.btn_restart_kernel
            .set_tooltip_text(Some(tr("btn_restart_kernel_tooltip")));
        self.btn_stop_kernel
            .set_tooltip_text(Some(tr("btn_stop_kernel_tooltip")));

        self.stats_group.set_title(tr("traffic_title"));
        self.stats_group.set_description(None);
        self.stats_group.set_tooltip_text(Some(tr("traffic_desc")));
        self.up_title.set_label(tr("realtime_up"));
        self.down_title.set_label(tr("realtime_down"));

        self.active_sub_group.set_title(tr("active_sub_title"));
        self.active_sub_group.set_description(None);
        self.active_sub_group
            .set_tooltip_text(Some(tr("active_sub_desc")));
        self.active_sub_row.set_title(tr("active_sub_row"));
        self.btn_refresh_sub
            .set_tooltip_text(Some(tr("tooltip_refresh_sub")));
        self.package_traffic_row
            .set_title(tr("package_traffic_title"));

        if self.subs_cache.borrow().is_empty() {
            let empty_list = gtk4::StringList::new(&[tr("no_available_sub")]);
            self.is_updating_dropdown.set(true);
            self.sub_dropdown.set_model(Some(&empty_list));
            self.is_updating_dropdown.set(false);
        }

        // Re-render status and active sub with translated strings
        let status = self.current_status.borrow().clone();
        self.update_kernel_status(&status);

        let active = self.current_active_sub.borrow().clone();
        self.update_active_subscription(active.as_ref());
    }

    pub fn update_proxy_status(&self, enabled: bool) {
        if self.proxy_switch.is_active() != enabled {
            self.is_updating_proxy.set(true);
            self.proxy_switch.set_active(enabled);
            self.is_updating_proxy.set(false);
        }
    }

    pub fn update_tun_status(&self, enabled: bool) {
        if self.tun_switch.is_active() != enabled {
            self.is_updating_tun.set(true);
            self.tun_switch.set_active(enabled);
            self.is_updating_tun.set(false);
        }
    }

    pub fn update_kernel_status(&self, status: &KernelStatus) {
        *self.current_status.borrow_mut() = status.clone();
        self.kernel_spinner.stop();
        self.kernel_spinner.set_visible(false);

        match status {
            KernelStatus::Running => {
                self.kernel_badge.set_label(tr("kernel_running"));
                self.kernel_badge
                    .set_css_classes(&["pill", "caption", "success"]);
                self.kernel_row.set_subtitle(tr("kernel_sub_running"));
                self.btn_start_kernel.set_sensitive(false);
                self.btn_stop_kernel.set_sensitive(true);
                self.btn_restart_kernel.set_sensitive(true);
            }
            KernelStatus::Stopped => {
                self.kernel_badge.set_label(tr("kernel_stopped"));
                self.kernel_badge
                    .set_css_classes(&["pill", "caption", "error"]);
                self.kernel_row.set_subtitle(tr("kernel_sub_stopped"));
                self.btn_start_kernel.set_sensitive(true);
                self.btn_stop_kernel.set_sensitive(false);
                self.btn_restart_kernel.set_sensitive(false);
            }
            KernelStatus::NotFound => {
                self.kernel_badge.set_label(tr("kernel_not_found"));
                self.kernel_badge
                    .set_css_classes(&["pill", "caption", "dim-label"]);
                self.kernel_row.set_subtitle(tr("kernel_sub_not_found"));
                self.btn_start_kernel.set_sensitive(false);
                self.btn_stop_kernel.set_sensitive(false);
                self.btn_restart_kernel.set_sensitive(false);
            }
            KernelStatus::Error(msg) => {
                self.kernel_badge.set_label(tr("kernel_error"));
                self.kernel_badge
                    .set_css_classes(&["pill", "caption", "warning"]);
                self.kernel_row.set_subtitle(msg);
                self.btn_start_kernel.set_sensitive(true);
                self.btn_stop_kernel.set_sensitive(false);
                self.btn_restart_kernel.set_sensitive(false);
            }
        }
    }

    pub fn update_traffic(&self, stats: &TrafficStats) {
        *self.last_traffic.borrow_mut() = (stats.up_total, stats.down_total);

        self.upload_speed_label
            .set_label(&format!("{}/s", format_speed(stats.up)));
        self.download_speed_label
            .set_label(&format!("{}/s", format_speed(stats.down)));

        self.upload_total_label.set_label(&format!(
            "{}{}",
            tr("total_prefix"),
            SubscriptionUserInfo::format_bytes(stats.up_total)
        ));
        self.download_total_label.set_label(&format!(
            "{}{}",
            tr("total_prefix"),
            SubscriptionUserInfo::format_bytes(stats.down_total)
        ));
    }

    pub fn update_subscriptions_list(&self, subs: &[Subscription]) {
        *self.subs_cache.borrow_mut() = subs.to_vec();

        self.is_updating_dropdown.set(true);
        if subs.is_empty() {
            let empty_list = gtk4::StringList::new(&[tr("no_available_sub")]);
            self.sub_dropdown.set_model(Some(&empty_list));
            self.sub_dropdown.set_sensitive(false);
        } else {
            let names: Vec<&str> = subs.iter().map(|s| s.name.as_str()).collect();
            let model = gtk4::StringList::new(&names);
            self.sub_dropdown.set_model(Some(&model));
            self.sub_dropdown.set_sensitive(true);

            if let Some(pos) = subs.iter().position(|s| s.is_active) {
                self.sub_dropdown.set_selected(pos as u32);
            }
        }
        self.is_updating_dropdown.set(false);
    }

    pub fn update_active_subscription(&self, sub_opt: Option<&Subscription>) {
        *self.current_active_sub.borrow_mut() = sub_opt.cloned();
        self.sub_refresh_spinner.stop();
        self.sub_refresh_spinner.set_visible(false);

        if let Some(sub) = sub_opt {
            self.btn_refresh_sub.set_sensitive(true);

            let updated_time_str = sub
                .updated_at
                .map(|t| {
                    t.with_timezone(&chrono::Local)
                        .format("%Y-%m-%d %H:%M:%S")
                        .to_string()
                })
                .unwrap_or_else(|| tr("sub_never_updated").to_string());
            self.active_sub_row.set_subtitle(&format!(
                "{}: {}",
                tr("sub_updated_time"),
                updated_time_str
            ));

            // Select in dropdown without triggering listener
            let subs = self.subs_cache.borrow();
            if let Some(pos) = subs.iter().position(|s| s.id == sub.id) {
                self.is_updating_dropdown.set(true);
                self.sub_dropdown.set_selected(pos as u32);
                self.is_updating_dropdown.set(false);
            }

            // Update package traffic row
            if let Some(info) = &sub.user_info {
                self.package_traffic_row.set_visible(true);
                let used_str = SubscriptionUserInfo::format_bytes(info.upload + info.download);
                let total_str = SubscriptionUserInfo::format_bytes(info.total);
                let percentage = (info.usage_percentage() * 100.0) as u32;

                self.package_traffic_row
                    .set_subtitle(&format!("{} / {} ({}%)", used_str, total_str, percentage));
                self.package_pbar.set_fraction(info.usage_percentage());
            } else {
                self.package_traffic_row.set_visible(false);
            }
        } else {
            self.btn_refresh_sub.set_sensitive(false);
            self.active_sub_row.set_subtitle(tr("no_active_sub"));
            self.package_traffic_row.set_visible(false);
        }
    }
}

fn format_speed(bytes_per_sec: u64) -> String {
    SubscriptionUserInfo::format_bytes(bytes_per_sec)
}
