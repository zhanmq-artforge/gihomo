use adw::prelude::*;
use gihomo_app::AppService;
use gihomo_core::ProxyGroup;
use gtk4::{Button, Label, Orientation, ScrolledWindow};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::i18n::tr;

struct NodeRowHandle {
    title_vbox: gtk4::Box,
    suffix_box: gtk4::Box,
    meta_box: gtk4::Box,
    is_narrow: Cell<bool>,
}

impl NodeRowHandle {
    fn set_narrow(&self, narrow: bool) {
        if self.is_narrow.get() == narrow {
            return;
        }
        self.is_narrow.set(narrow);
        self.meta_box.unparent();
        if narrow {
            self.title_vbox.append(&self.meta_box);
        } else {
            self.suffix_box.prepend(&self.meta_box);
        }
    }
}

pub struct ProxiesView {
    pub page: adw::NavigationPage,
    groups_box: gtk4::Box,
    status_page: adw::StatusPage,
    mode_label: Label,
    btn_rule_mode: Button,
    btn_global_mode: Button,
    btn_direct_mode: Button,
    service: AppService,
    is_updating_mode: Rc<Cell<bool>>,
    groups_cache: Rc<RefCell<Vec<ProxyGroup>>>,
    current_mode: Rc<RefCell<String>>,
    row_handles: Rc<RefCell<Vec<NodeRowHandle>>>,
    is_narrow: Rc<Cell<bool>>,
}

impl ProxiesView {
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

        // Top Control Bar: Mode selection only
        let top_bar = gtk4::Box::new(Orientation::Horizontal, 12);
        top_bar.set_hexpand(true);

        let mode_label = Label::builder()
            .label(tr("proxies_mode_label"))
            .css_classes(["dim-label"])
            .valign(gtk4::Align::Center)
            .build();
        top_bar.append(&mode_label);

        // Segmented buttons for Rule, Global, Direct
        let mode_box = gtk4::Box::new(Orientation::Horizontal, 0);
        mode_box.add_css_class("linked");

        let btn_rule_mode = Button::builder()
            .label(tr("mode_rule"))
            .tooltip_text(tr("mode_rule_tooltip"))
            .css_classes(["suggested-action"])
            .build();
        let btn_global_mode = Button::builder()
            .label(tr("mode_global"))
            .tooltip_text(tr("mode_global_tooltip"))
            .build();
        let btn_direct_mode = Button::builder()
            .label(tr("mode_direct"))
            .tooltip_text(tr("mode_direct_tooltip"))
            .build();

        mode_box.append(&btn_rule_mode);
        mode_box.append(&btn_global_mode);
        mode_box.append(&btn_direct_mode);
        top_bar.append(&mode_box);

        // Status page for empty state
        let status_page = adw::StatusPage::builder()
            .icon_name("network-server-symbolic")
            .title(tr("empty_proxies_title"))
            .description(tr("empty_proxies_desc"))
            .vexpand(true)
            .build();

        // Groups container
        let groups_box = gtk4::Box::new(Orientation::Vertical, 16);
        groups_box.set_visible(false);

        content_box.append(&top_bar);
        content_box.append(&status_page);
        content_box.append(&groups_box);

        clamp.set_child(Some(&content_box));

        let scrolled = ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .vexpand(true)
            .hexpand(true)
            .child(&clamp)
            .build();

        let is_updating_mode = Rc::new(Cell::new(false));
        let groups_cache: Rc<RefCell<Vec<ProxyGroup>>> = Rc::new(RefCell::new(Vec::new()));
        let current_mode = Rc::new(RefCell::new("rule".to_string()));
        let row_handles: Rc<RefCell<Vec<NodeRowHandle>>> = Rc::new(RefCell::new(Vec::new()));
        let is_narrow = Rc::new(Cell::new(false));

        let breakpoint_bin = adw::BreakpointBin::new();
        breakpoint_bin.set_child(Some(&scrolled));

        let breakpoint = adw::Breakpoint::new(
            adw::BreakpointCondition::parse("max-width: 520px")
                .expect("valid breakpoint condition"),
        );
        let is_narrow_apply = is_narrow.clone();
        let row_handles_apply = row_handles.clone();
        breakpoint.connect_apply(move |_| {
            if !is_narrow_apply.get() {
                is_narrow_apply.set(true);
                for h in row_handles_apply.borrow().iter() {
                    h.set_narrow(true);
                }
            }
        });

        let is_narrow_unapply = is_narrow.clone();
        let row_handles_unapply = row_handles.clone();
        breakpoint.connect_unapply(move |_| {
            if is_narrow_unapply.get() {
                is_narrow_unapply.set(false);
                for h in row_handles_unapply.borrow().iter() {
                    h.set_narrow(false);
                }
            }
        });
        breakpoint_bin.add_breakpoint(breakpoint);

        toolbar_view.set_content(Some(&breakpoint_bin));

        let page = adw::NavigationPage::builder()
            .title(tr("tab_proxies"))
            .tag("proxies")
            .child(&toolbar_view)
            .build();

        // Wire Mode Buttons
        {
            let service = service.clone();
            let is_updating = is_updating_mode.clone();
            btn_rule_mode.connect_clicked(move |_| {
                if is_updating.get() {
                    return;
                }
                let service = service.clone();
                glib::MainContext::default().spawn_local(async move {
                    let _ = service.set_proxy_mode("rule").await;
                });
            });
        }

        {
            let service = service.clone();
            let is_updating = is_updating_mode.clone();
            btn_global_mode.connect_clicked(move |_| {
                if is_updating.get() {
                    return;
                }
                let service = service.clone();
                glib::MainContext::default().spawn_local(async move {
                    let _ = service.set_proxy_mode("global").await;
                });
            });
        }

        {
            let service = service.clone();
            let is_updating = is_updating_mode.clone();
            btn_direct_mode.connect_clicked(move |_| {
                if is_updating.get() {
                    return;
                }
                let service = service.clone();
                glib::MainContext::default().spawn_local(async move {
                    let _ = service.set_proxy_mode("direct").await;
                });
            });
        }

        Self {
            page,
            groups_box,
            status_page,
            mode_label,
            btn_rule_mode,
            btn_global_mode,
            btn_direct_mode,
            service,
            is_updating_mode,
            groups_cache,
            current_mode,
            row_handles,
            is_narrow,
        }
    }

    pub fn set_narrow(&self, narrow: bool) {
        if self.is_narrow.get() == narrow {
            return;
        }
        self.is_narrow.set(narrow);
        for handle in self.row_handles.borrow().iter() {
            handle.set_narrow(narrow);
        }
    }

    pub fn update_ui_text(&self) {
        self.page.set_title(tr("tab_proxies"));
        self.mode_label.set_label(tr("proxies_mode_label"));
        self.btn_rule_mode.set_label(tr("mode_rule"));
        self.btn_rule_mode
            .set_tooltip_text(Some(tr("mode_rule_tooltip")));
        self.btn_global_mode.set_label(tr("mode_global"));
        self.btn_global_mode
            .set_tooltip_text(Some(tr("mode_global_tooltip")));
        self.btn_direct_mode.set_label(tr("mode_direct"));
        self.btn_direct_mode
            .set_tooltip_text(Some(tr("mode_direct_tooltip")));

        self.status_page.set_title(tr("empty_proxies_title"));
        self.status_page
            .set_description(Some(tr("empty_proxies_desc")));

        let groups = self.groups_cache.borrow().clone();
        self.render_groups(&groups);
    }

    pub fn update_proxy_mode(&self, mode: &str) {
        if *self.current_mode.borrow() == mode {
            return;
        }
        *self.current_mode.borrow_mut() = mode.to_string();
        self.is_updating_mode.set(true);

        self.btn_rule_mode.remove_css_class("suggested-action");
        self.btn_global_mode.remove_css_class("suggested-action");
        self.btn_direct_mode.remove_css_class("suggested-action");

        match mode.to_lowercase().as_str() {
            "global" => self.btn_global_mode.add_css_class("suggested-action"),
            "direct" => self.btn_direct_mode.add_css_class("suggested-action"),
            _ => self.btn_rule_mode.add_css_class("suggested-action"),
        }

        self.is_updating_mode.set(false);
    }

    pub fn update_groups(&self, groups: &[ProxyGroup]) {
        if *self.groups_cache.borrow() == groups {
            return;
        }
        *self.groups_cache.borrow_mut() = groups.to_vec();
        self.render_groups(groups);
    }

    fn render_groups(&self, groups: &[ProxyGroup]) {
        self.row_handles.borrow_mut().clear();

        // Clear existing groups
        while let Some(child) = self.groups_box.first_child() {
            self.groups_box.remove(&child);
        }

        if groups.is_empty() {
            self.status_page.set_visible(true);
            self.groups_box.set_visible(false);
            return;
        }

        self.status_page.set_visible(false);
        self.groups_box.set_visible(true);

        for group in groups {
            let is_selectable = matches!(group.group_type.as_str(), "Selector" | "Fallback")
                || group.name == "GLOBAL";

            let group_widget = adw::PreferencesGroup::new();

            // Header ActionRow: Title + Subtitle (wraps) + Actions [ ⚡ 测速 ] [ 🔄 刷新 ]
            let header_row = adw::ActionRow::builder()
                .title(&group.name)
                .subtitle(format!(
                    "{}{}  |  {}{}",
                    tr("group_type_prefix"),
                    group.group_type,
                    tr("current_node_prefix"),
                    group.now
                ))
                .title_lines(1)
                .subtitle_lines(0)
                .activatable(false)
                .selectable(false)
                .build();

            let actions_box = gtk4::Box::new(Orientation::Horizontal, 4);
            actions_box.set_valign(gtk4::Align::Center);

            // Group Ping Button (⚡)
            let ping_btn = Button::builder()
                .icon_name("network-transmit-receive-symbolic")
                .tooltip_text(tr("btn_ping"))
                .valign(gtk4::Align::Center)
                .css_classes(["flat"])
                .build();

            {
                let service = self.service.clone();
                let group_name = group.name.clone();
                let ping_btn_clone = ping_btn.clone();
                ping_btn.connect_clicked(move |_| {
                    ping_btn_clone.set_sensitive(false);
                    let service = service.clone();
                    let group_name = group_name.clone();
                    let ping_btn = ping_btn_clone.clone();
                    glib::MainContext::default().spawn_local(async move {
                        let _ = service.test_group_delay(&group_name).await;
                        ping_btn.set_sensitive(true);
                    });
                });
            }

            // Refresh Button (🔄)
            let refresh_btn = Button::builder()
                .icon_name("view-refresh-symbolic")
                .tooltip_text(tr("tooltip_refresh_proxies"))
                .valign(gtk4::Align::Center)
                .css_classes(["flat"])
                .build();

            {
                let service = self.service.clone();
                let refresh_btn_clone = refresh_btn.clone();
                refresh_btn.connect_clicked(move |_| {
                    refresh_btn_clone.set_sensitive(false);
                    let service = service.clone();
                    let refresh_btn = refresh_btn_clone.clone();
                    glib::MainContext::default().spawn_local(async move {
                        let _ = service.fetch_proxies().await;
                        refresh_btn.set_sensitive(true);
                    });
                });
            }

            actions_box.append(&ping_btn);
            actions_box.append(&refresh_btn);
            header_row.add_suffix(&actions_box);

            group_widget.add(&header_row);

            for node in &group.nodes {
                let is_selected = node.name == group.now;

                let prow = adw::PreferencesRow::builder()
                    .activatable(is_selectable && !is_selected)
                    .selectable(false)
                    .build();

                let hbox = gtk4::Box::new(Orientation::Horizontal, 12);
                hbox.set_margin_top(8);
                hbox.set_margin_bottom(8);
                hbox.set_margin_start(12);
                hbox.set_margin_end(12);

                let title_vbox = gtk4::Box::new(Orientation::Vertical, 4);
                title_vbox.set_hexpand(true);
                title_vbox.set_valign(gtk4::Align::Center);

                let title_label = Label::builder()
                    .label(&node.name)
                    .halign(gtk4::Align::Start)
                    .ellipsize(gtk4::pango::EllipsizeMode::End)
                    .lines(1)
                    .hexpand(true)
                    .build();
                title_vbox.append(&title_label);

                // Meta Box: Type badge + Delay badge/ping button
                let meta_box = gtk4::Box::new(Orientation::Horizontal, 6);
                meta_box.set_valign(gtk4::Align::Center);
                meta_box.set_halign(gtk4::Align::Start);

                let type_badge = Label::builder()
                    .label(&node.node_type)
                    .css_classes(["pill", "caption", "dim-label"])
                    .valign(gtk4::Align::Center)
                    .build();
                meta_box.append(&type_badge);

                let node_ping_btn = Button::builder()
                    .valign(gtk4::Align::Center)
                    .css_classes(["flat"])
                    .tooltip_text(tr("tooltip_ping_node"))
                    .build();

                let (delay_text, delay_class) = match node.delay {
                    Some(0) => (tr("timeout_badge").to_string(), "error"),
                    Some(d) if d < 400 => (format!("{} ms", d), "success"),
                    Some(d) if d < 1000 => (format!("{} ms", d), "warning"),
                    Some(d) => (format!("{} ms", d), "error"),
                    None => ("-- ms".to_string(), "dim-label"),
                };

                let delay_label = Label::builder()
                    .label(&delay_text)
                    .css_classes(["pill", "caption", delay_class])
                    .build();
                node_ping_btn.set_child(Some(&delay_label));

                {
                    let service = self.service.clone();
                    let group_name = group.name.clone();
                    let node_name = node.name.clone();
                    let btn_clone = node_ping_btn.clone();
                    let lbl_clone = delay_label.clone();
                    node_ping_btn.connect_clicked(move |_| {
                        btn_clone.set_sensitive(false);
                        lbl_clone.set_label("...");
                        let service = service.clone();
                        let group_name = group_name.clone();
                        let node_name = node_name.clone();
                        let btn = btn_clone.clone();
                        let lbl = lbl_clone.clone();
                        glib::MainContext::default().spawn_local(async move {
                            match service.test_node_delay(&group_name, &node_name).await {
                                Ok(d) => {
                                    lbl.remove_css_class("dim-label");
                                    lbl.remove_css_class("error");
                                    lbl.remove_css_class("warning");
                                    lbl.remove_css_class("success");
                                    if d == 0 {
                                        lbl.set_label(tr("timeout_badge"));
                                        lbl.add_css_class("error");
                                    } else if d < 400 {
                                        lbl.set_label(&format!("{} ms", d));
                                        lbl.add_css_class("success");
                                    } else if d < 1000 {
                                        lbl.set_label(&format!("{} ms", d));
                                        lbl.add_css_class("warning");
                                    } else {
                                        lbl.set_label(&format!("{} ms", d));
                                        lbl.add_css_class("error");
                                    }
                                }
                                Err(_) => {
                                    lbl.remove_css_class("success");
                                    lbl.remove_css_class("warning");
                                    lbl.add_css_class("error");
                                    lbl.set_label(tr("timeout_badge"));
                                }
                            }
                            btn.set_sensitive(true);
                        });
                    });
                }
                meta_box.append(&node_ping_btn);

                // Suffix Box: Status badge or Select button
                let suffix_box = gtk4::Box::new(Orientation::Horizontal, 8);
                suffix_box.set_valign(gtk4::Align::Center);

                if is_selected {
                    let active_badge = Label::builder()
                        .label(tr("selected_badge"))
                        .css_classes(["pill", "caption", "accent"])
                        .valign(gtk4::Align::Center)
                        .build();
                    suffix_box.append(&active_badge);
                } else if is_selectable {
                    let select_btn = Button::builder()
                        .label(tr("btn_select"))
                        .css_classes(["flat"])
                        .valign(gtk4::Align::Center)
                        .build();

                    let service = self.service.clone();
                    let group_name = group.name.clone();
                    let node_name = node.name.clone();
                    let select_btn_clone = select_btn.clone();
                    let prow_clone = prow.clone();
                    select_btn.connect_clicked(move |_| {
                        select_btn_clone.set_sensitive(false);
                        prow_clone.set_sensitive(false);
                        let service = service.clone();
                        let group_name = group_name.clone();
                        let node_name = node_name.clone();
                        glib::MainContext::default().spawn_local(async move {
                            let _ = service.select_proxy_node(&group_name, &node_name).await;
                        });
                    });
                    suffix_box.append(&select_btn);

                    // Row clickable for quick selection
                    let service = self.service.clone();
                    let group_name = group.name.clone();
                    let node_name = node.name.clone();
                    let select_btn_clone = select_btn.clone();
                    let prow_clone = prow.clone();
                    prow.connect_activate(move |_| {
                        select_btn_clone.set_sensitive(false);
                        prow_clone.set_sensitive(false);
                        let service = service.clone();
                        let group_name = group_name.clone();
                        let node_name = node_name.clone();
                        glib::MainContext::default().spawn_local(async move {
                            let _ = service.select_proxy_node(&group_name, &node_name).await;
                        });
                    });
                }

                let is_narrow = self.is_narrow.get();
                if is_narrow {
                    title_vbox.append(&meta_box);
                } else {
                    suffix_box.prepend(&meta_box);
                }

                hbox.append(&title_vbox);
                hbox.append(&suffix_box);
                prow.set_child(Some(&hbox));

                let handle = NodeRowHandle {
                    title_vbox,
                    suffix_box,
                    meta_box,
                    is_narrow: Cell::new(is_narrow),
                };
                self.row_handles.borrow_mut().push(handle);

                group_widget.add(&prow);
            }

            self.groups_box.append(&group_widget);
        }
    }
}
