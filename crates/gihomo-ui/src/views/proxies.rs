use adw::prelude::*;
use gihomo_app::AppService;
use gihomo_core::{ProxyGroup, ProxyNode};
use gtk4::{Button, Label, Orientation, ScrolledWindow, SearchEntry};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use crate::i18n::tr;

const INITIAL_VISIBLE_COUNT: usize = 150;
const LOAD_MORE_STEP: usize = 150;

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

struct NodeWidgets {
    name: String,
    delay_label: Label,
    suffix_box: gtk4::Box,
    prow: adw::PreferencesRow,
    is_selectable: bool,
    is_selected: bool,
    delay: Option<u32>,
}

fn format_delay(delay: Option<u32>) -> (String, &'static str) {
    match delay {
        Some(0) => (tr("timeout_badge").to_string(), "error"),
        Some(d) if d < 400 => (format!("{} ms", d), "success"),
        Some(d) if d < 1000 => (format!("{} ms", d), "warning"),
        Some(d) => (format!("{} ms", d), "error"),
        None => ("-- ms".to_string(), "dim-label"),
    }
}

fn setup_node_suffix(
    suffix_box: &gtk4::Box,
    prow: &adw::PreferencesRow,
    service: &AppService,
    group_name: &str,
    node_name: &str,
    is_selected: bool,
    is_selectable: bool,
) {
    while let Some(child) = suffix_box.first_child() {
        suffix_box.remove(&child);
    }

    if is_selected {
        let active_badge = Label::builder()
            .label(tr("selected_badge"))
            .css_classes(["pill", "caption", "accent"])
            .valign(gtk4::Align::Center)
            .build();
        suffix_box.append(&active_badge);
        prow.set_activatable(false);
    } else if is_selectable {
        let select_btn = Button::builder()
            .label(tr("btn_select"))
            .css_classes(["flat"])
            .valign(gtk4::Align::Center)
            .build();

        let service_btn = service.clone();
        let group_btn = group_name.to_string();
        let node_btn = node_name.to_string();
        let select_btn_clone = select_btn.clone();
        let prow_clone = prow.clone();
        select_btn.connect_clicked(move |_| {
            select_btn_clone.set_sensitive(false);
            prow_clone.set_sensitive(false);
            let service = service_btn.clone();
            let group_name = group_btn.clone();
            let node_name = node_btn.clone();
            glib::MainContext::default().spawn_local(async move {
                let _ = service.select_proxy_node(&group_name, &node_name).await;
            });
        });
        suffix_box.append(&select_btn);

        let service_row = service.clone();
        let group_row = group_name.to_string();
        let node_row = node_name.to_string();
        let select_btn_row = select_btn.clone();
        let prow_row = prow.clone();
        prow.connect_activate(move |_| {
            select_btn_row.set_sensitive(false);
            prow_row.set_sensitive(false);
            let service = service_row.clone();
            let group_name = group_row.clone();
            let node_name = node_row.clone();
            glib::MainContext::default().spawn_local(async move {
                let _ = service.select_proxy_node(&group_name, &node_name).await;
            });
        });
        prow.set_activatable(true);
    } else {
        prow.set_activatable(false);
    }
}

#[derive(Clone)]
pub struct ProxiesView {
    pub page: adw::NavigationPage,
    status_page: adw::StatusPage,
    main_box: gtk4::Box,
    mode_label: Label,
    btn_rule_mode: Button,
    btn_global_mode: Button,
    btn_direct_mode: Button,
    dropdown_label: Label,
    group_dropdown: gtk4::DropDown,
    group_title_label: Label,
    group_subtitle_label: Label,
    ping_btn: Button,
    refresh_btn: Button,
    search_entry: SearchEntry,
    nodes_list: gtk4::ListBox,
    empty_nodes_label: Label,
    load_more_btn: Button,

    service: AppService,
    is_updating_mode: Rc<Cell<bool>>,
    is_updating_dropdown: Rc<Cell<bool>>,
    groups_cache: Rc<RefCell<Vec<ProxyGroup>>>,
    current_mode: Rc<RefCell<String>>,
    selected_group_name: Rc<RefCell<Option<String>>>,
    search_query: Rc<RefCell<String>>,
    search_debounce_source: Rc<RefCell<Option<glib::SourceId>>>,
    visible_count: Rc<RefCell<usize>>,
    active_rendered_nodes: Rc<RefCell<Vec<NodeWidgets>>>,
    rendered_group_name: Rc<RefCell<Option<String>>>,
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

        let clamp = adw::Clamp::builder()
            .maximum_size(860)
            .tightening_threshold(650)
            .build();

        let content_box = gtk4::Box::new(Orientation::Vertical, 16);
        content_box.set_margin_top(16);
        content_box.set_margin_bottom(24);
        content_box.set_margin_start(12);
        content_box.set_margin_end(12);

        // 1. Top Control Bar: Mode selection (Rule / Global / Direct)
        let top_bar = gtk4::Box::new(Orientation::Horizontal, 12);
        top_bar.set_hexpand(true);

        let mode_label = Label::builder()
            .label(tr("proxies_mode_label"))
            .css_classes(["dim-label"])
            .valign(gtk4::Align::Center)
            .build();
        top_bar.append(&mode_label);

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

        // 2. Status page for empty state
        let status_page = adw::StatusPage::builder()
            .icon_name("network-server-symbolic")
            .title(tr("empty_proxies_title"))
            .description(tr("empty_proxies_desc"))
            .vexpand(true)
            .build();

        // 3. Main content container for group selector & node list
        let main_box = gtk4::Box::new(Orientation::Vertical, 14);
        main_box.set_visible(false);

        // 3.1 Native Group DropDown Selector
        let dropdown_box = gtk4::Box::new(Orientation::Horizontal, 10);
        dropdown_box.set_hexpand(true);

        let dropdown_label = Label::builder()
            .label(tr("proxy_group_label"))
            .css_classes(["dim-label"])
            .valign(gtk4::Align::Center)
            .build();

        let group_dropdown = gtk4::DropDown::builder()
            .hexpand(true)
            .valign(gtk4::Align::Center)
            .build();

        dropdown_box.append(&dropdown_label);
        dropdown_box.append(&group_dropdown);
        main_box.append(&dropdown_box);

        // 3.2 Active Group Header Box (Left: Title + Multiline Subtitle, Right: Ping + Refresh)
        let group_header_box = gtk4::Box::new(Orientation::Horizontal, 12);
        group_header_box.set_valign(gtk4::Align::Center);
        group_header_box.set_margin_top(4);

        let title_vbox = gtk4::Box::new(Orientation::Vertical, 4);
        title_vbox.set_hexpand(true);
        title_vbox.set_valign(gtk4::Align::Center);

        let group_title_label = Label::builder()
            .label("")
            .halign(gtk4::Align::Start)
            .css_classes(["title-2"])
            .build();

        let group_subtitle_label = Label::builder()
            .label("")
            .halign(gtk4::Align::Start)
            .wrap(true)
            .wrap_mode(gtk4::pango::WrapMode::WordChar)
            .css_classes(["caption", "dim-label"])
            .build();

        title_vbox.append(&group_title_label);
        title_vbox.append(&group_subtitle_label);

        let header_actions_box = gtk4::Box::new(Orientation::Horizontal, 6);
        header_actions_box.set_valign(gtk4::Align::Center);

        let ping_btn = Button::builder()
            .icon_name("network-transmit-receive-symbolic")
            .tooltip_text(tr("btn_ping"))
            .valign(gtk4::Align::Center)
            .css_classes(["flat"])
            .build();

        let refresh_btn = Button::builder()
            .icon_name("view-refresh-symbolic")
            .tooltip_text(tr("tooltip_refresh_proxies"))
            .valign(gtk4::Align::Center)
            .css_classes(["flat"])
            .build();

        header_actions_box.append(&ping_btn);
        header_actions_box.append(&refresh_btn);

        group_header_box.append(&title_vbox);
        group_header_box.append(&header_actions_box);
        main_box.append(&group_header_box);

        // 3.3 Search Entry for quick filtering of nodes in the active group
        let search_entry = SearchEntry::builder()
            .placeholder_text(tr("proxy_search_placeholder"))
            .hexpand(true)
            .build();
        main_box.append(&search_entry);

        // 3.4 Nodes List (boxed-list)
        let nodes_list = gtk4::ListBox::new();
        nodes_list.set_selection_mode(gtk4::SelectionMode::None);
        nodes_list.add_css_class("boxed-list");
        main_box.append(&nodes_list);

        // 3.5 Empty nodes label (when search returns 0)
        let empty_nodes_label = Label::builder()
            .label(tr("empty_group_nodes"))
            .css_classes(["dim-label", "caption"])
            .margin_top(16)
            .margin_bottom(16)
            .halign(gtk4::Align::Center)
            .build();
        empty_nodes_label.set_visible(false);
        main_box.append(&empty_nodes_label);

        // 3.6 Load more button
        let load_more_btn = Button::builder()
            .label(tr("btn_load_more_nodes"))
            .css_classes(["suggested-action", "pill"])
            .halign(gtk4::Align::Center)
            .margin_top(8)
            .margin_bottom(12)
            .build();
        load_more_btn.set_visible(false);
        main_box.append(&load_more_btn);

        content_box.append(&top_bar);
        content_box.append(&status_page);
        content_box.append(&main_box);

        clamp.set_child(Some(&content_box));

        let scrolled = ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .vexpand(true)
            .hexpand(true)
            .child(&clamp)
            .build();

        let is_updating_mode = Rc::new(Cell::new(false));
        let is_updating_dropdown = Rc::new(Cell::new(false));
        let groups_cache: Rc<RefCell<Vec<ProxyGroup>>> = Rc::new(RefCell::new(Vec::new()));
        let current_mode = Rc::new(RefCell::new("rule".to_string()));
        let selected_group_name: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
        let search_query = Rc::new(RefCell::new(String::new()));
        let search_debounce_source: Rc<RefCell<Option<glib::SourceId>>> =
            Rc::new(RefCell::new(None));
        let visible_count = Rc::new(RefCell::new(INITIAL_VISIBLE_COUNT));
        let active_rendered_nodes = Rc::new(RefCell::new(Vec::new()));
        let rendered_group_name = Rc::new(RefCell::new(None));
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

        let view = Self {
            page,
            status_page,
            main_box,
            mode_label,
            btn_rule_mode,
            btn_global_mode,
            btn_direct_mode,
            dropdown_label,
            group_dropdown,
            group_title_label,
            group_subtitle_label,
            ping_btn,
            refresh_btn,
            search_entry,
            nodes_list,
            empty_nodes_label,
            load_more_btn,
            service,
            is_updating_mode,
            is_updating_dropdown,
            groups_cache,
            current_mode,
            selected_group_name,
            search_query,
            search_debounce_source,
            visible_count,
            active_rendered_nodes,
            rendered_group_name,
            row_handles,
            is_narrow,
        };

        view.wire_handlers();
        view
    }

    fn wire_handlers(&self) {
        // Wire Mode Buttons
        {
            let service = self.service.clone();
            let is_updating = self.is_updating_mode.clone();
            self.btn_rule_mode.connect_clicked(move |_| {
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
            let service = self.service.clone();
            let is_updating = self.is_updating_mode.clone();
            self.btn_global_mode.connect_clicked(move |_| {
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
            let service = self.service.clone();
            let is_updating = self.is_updating_mode.clone();
            self.btn_direct_mode.connect_clicked(move |_| {
                if is_updating.get() {
                    return;
                }
                let service = service.clone();
                glib::MainContext::default().spawn_local(async move {
                    let _ = service.set_proxy_mode("direct").await;
                });
            });
        }

        // Wire Group DropDown Selector
        {
            let view_clone = self.clone();
            let is_updating = self.is_updating_dropdown.clone();
            let groups_cache = self.groups_cache.clone();
            self.group_dropdown.connect_selected_notify(move |dd| {
                if is_updating.get() {
                    return;
                }
                let idx = dd.selected() as usize;
                let groups = groups_cache.borrow();
                if let Some(group) = groups.get(idx) {
                    view_clone.switch_group(&group.name);
                }
            });
        }

        // Wire Group Ping Button
        {
            let service = self.service.clone();
            let selected_group = self.selected_group_name.clone();
            let ping_btn_clone = self.ping_btn.clone();
            self.ping_btn.connect_clicked(move |_| {
                if let Some(group_name) = selected_group.borrow().clone() {
                    ping_btn_clone.set_sensitive(false);
                    let service = service.clone();
                    let btn = ping_btn_clone.clone();
                    glib::MainContext::default().spawn_local(async move {
                        let _ = service.test_group_delay(&group_name).await;
                        btn.set_sensitive(true);
                    });
                }
            });
        }

        // Wire Refresh Button
        {
            let service = self.service.clone();
            let refresh_btn_clone = self.refresh_btn.clone();
            self.refresh_btn.connect_clicked(move |_| {
                refresh_btn_clone.set_sensitive(false);
                let service = service.clone();
                let btn = refresh_btn_clone.clone();
                glib::MainContext::default().spawn_local(async move {
                    let _ = service.fetch_proxies().await;
                    btn.set_sensitive(true);
                });
            });
        }

        // Wire Search Entry with 200ms debounce
        {
            let view_clone = self.clone();
            let debounce_source = self.search_debounce_source.clone();
            let search_query = self.search_query.clone();
            let visible_count = self.visible_count.clone();
            self.search_entry.connect_search_changed(move |entry| {
                if let Some(source) = debounce_source.borrow_mut().take() {
                    source.remove();
                }
                let text = entry.text().to_string();
                let view = view_clone.clone();
                let sq = search_query.clone();
                let vc = visible_count.clone();
                let debounce_holder = debounce_source.clone();
                let source_id = glib::timeout_add_local_once(Duration::from_millis(200), move || {
                    *debounce_holder.borrow_mut() = None;
                    *sq.borrow_mut() = text;
                    *vc.borrow_mut() = INITIAL_VISIBLE_COUNT;
                    view.render_active_group();
                });
                *debounce_source.borrow_mut() = Some(source_id);
            });
        }

        // Wire Load More Button
        {
            let view_clone = self.clone();
            let visible_count = self.visible_count.clone();
            self.load_more_btn.connect_clicked(move |_| {
                *visible_count.borrow_mut() += LOAD_MORE_STEP;
                view_clone.render_active_group();
            });
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

        self.dropdown_label.set_label(tr("proxy_group_label"));
        self.search_entry
            .set_placeholder_text(Some(tr("proxy_search_placeholder")));
        self.ping_btn.set_tooltip_text(Some(tr("btn_ping")));
        self.refresh_btn
            .set_tooltip_text(Some(tr("tooltip_refresh_proxies")));
        self.empty_nodes_label.set_label(tr("empty_group_nodes"));
        self.load_more_btn.set_label(tr("btn_load_more_nodes"));

        let groups = self.groups_cache.borrow().clone();
        if !groups.is_empty() {
            self.update_dropdown_model(&groups);
        }

        self.render_active_group();
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
        *self.groups_cache.borrow_mut() = groups.to_vec();

        if groups.is_empty() {
            self.status_page.set_visible(true);
            self.main_box.set_visible(false);
            return;
        }

        self.status_page.set_visible(false);
        self.main_box.set_visible(true);

        // Ensure selected group is valid
        let has_selected = {
            let sel = self.selected_group_name.borrow();
            sel.as_ref()
                .map_or(false, |name| groups.iter().any(|g| &g.name == name))
        };
        if !has_selected {
            let default_name = groups
                .iter()
                .find(|g| g.name == "Proxy" || g.name == "GLOBAL")
                .map(|g| g.name.clone())
                .unwrap_or_else(|| groups[0].name.clone());
            *self.selected_group_name.borrow_mut() = Some(default_name);
        }

        self.update_dropdown_model(groups);

        let sel_name = self.selected_group_name.borrow().clone();
        if let Some(active_group) = groups.iter().find(|g| sel_name.as_deref() == Some(&g.name)) {
            if self.try_incremental_update(active_group) {
                return;
            }
        }

        self.render_active_group();
    }

    pub fn switch_group(&self, group_name: &str) {
        if self.selected_group_name.borrow().as_deref() == Some(group_name) {
            return;
        }
        *self.selected_group_name.borrow_mut() = Some(group_name.to_string());
        *self.visible_count.borrow_mut() = INITIAL_VISIBLE_COUNT;
        self.search_entry.set_text("");
        *self.search_query.borrow_mut() = String::new();

        self.sync_dropdown_selected();
        self.render_active_group();
    }

    fn sync_dropdown_selected(&self) {
        let groups = self.groups_cache.borrow();
        let sel = self.selected_group_name.borrow();
        if let Some(pos) = groups
            .iter()
            .position(|g| Some(&g.name) == sel.as_ref())
        {
            self.is_updating_dropdown.set(true);
            self.group_dropdown.set_selected(pos as u32);
            self.is_updating_dropdown.set(false);
        }
    }

    fn update_dropdown_model(&self, groups: &[ProxyGroup]) {
        self.is_updating_dropdown.set(true);
        let labels: Vec<String> = groups
            .iter()
            .map(|g| format!("{} ({})", g.name, g.group_type))
            .collect();
        let str_labels: Vec<&str> = labels.iter().map(|s| s.as_str()).collect();
        let model = gtk4::StringList::new(&str_labels);
        self.group_dropdown.set_model(Some(&model));

        let sel = self.selected_group_name.borrow();
        if let Some(pos) = groups.iter().position(|g| Some(&g.name) == sel.as_ref()) {
            self.group_dropdown.set_selected(pos as u32);
        }
        self.is_updating_dropdown.set(false);
    }

    fn try_incremental_update(&self, active_group: &ProxyGroup) -> bool {
        if self.rendered_group_name.borrow().as_deref() != Some(&active_group.name) {
            return false;
        }

        let query = self.search_query.borrow().to_lowercase().trim().to_string();
        let matching_nodes: Vec<&ProxyNode> = active_group
            .nodes
            .iter()
            .filter(|n| {
                if query.is_empty() {
                    true
                } else {
                    n.name.to_lowercase().contains(&query)
                        || n.node_type.to_lowercase().contains(&query)
                }
            })
            .collect();

        let limit = *self.visible_count.borrow();
        let expected_len = matching_nodes.len().min(limit);

        let mut rendered = self.active_rendered_nodes.borrow_mut();
        if rendered.len() != expected_len {
            return false;
        }

        for (node_w, node) in rendered.iter().zip(matching_nodes.iter()) {
            if node_w.name != node.name {
                return false;
            }
        }

        // Structural match: perform in-place update!
        self.group_subtitle_label.set_label(&format!(
            "{}{}  |  {}{}",
            tr("group_type_prefix"),
            active_group.group_type,
            tr("current_node_prefix"),
            active_group.now
        ));

        for (node_w, node) in rendered.iter_mut().zip(matching_nodes.iter()) {
            if node_w.delay != node.delay {
                node_w.delay = node.delay;
                let (delay_text, delay_class) = format_delay(node.delay);
                node_w.delay_label.set_label(&delay_text);
                node_w
                    .delay_label
                    .set_css_classes(&["pill", "caption", delay_class]);
            }

            let is_selected = node.name == active_group.now;
            if node_w.is_selected != is_selected {
                node_w.is_selected = is_selected;
                setup_node_suffix(
                    &node_w.suffix_box,
                    &node_w.prow,
                    &self.service,
                    &active_group.name,
                    &node.name,
                    is_selected,
                    node_w.is_selectable,
                );
            }
            node_w.prow.set_sensitive(true);
        }

        true
    }

    fn render_active_group(&self) {
        let groups = self.groups_cache.borrow();
        if groups.is_empty() {
            self.status_page.set_visible(true);
            self.main_box.set_visible(false);
            return;
        }

        let selected_name = self.selected_group_name.borrow().clone();
        let active_group = groups
            .iter()
            .find(|g| selected_name.as_deref() == Some(&g.name))
            .or_else(|| groups.first());

        let active_group = match active_group {
            Some(g) => g,
            None => return,
        };

        // Update Group Header: Title and Multiline Subtitle
        self.group_title_label.set_label(&active_group.name);
        self.group_subtitle_label.set_label(&format!(
            "{}{}  |  {}{}",
            tr("group_type_prefix"),
            active_group.group_type,
            tr("current_node_prefix"),
            active_group.now
        ));

        *self.rendered_group_name.borrow_mut() = Some(active_group.name.clone());

        // Filter nodes by search query
        let query = self.search_query.borrow().to_lowercase().trim().to_string();
        let matching_nodes: Vec<&ProxyNode> = active_group
            .nodes
            .iter()
            .filter(|n| {
                if query.is_empty() {
                    true
                } else {
                    n.name.to_lowercase().contains(&query)
                        || n.node_type.to_lowercase().contains(&query)
                }
            })
            .collect();

        // Clear existing rows
        while let Some(child) = self.nodes_list.first_child() {
            self.nodes_list.remove(&child);
        }
        self.row_handles.borrow_mut().clear();
        self.active_rendered_nodes.borrow_mut().clear();

        if matching_nodes.is_empty() {
            self.empty_nodes_label.set_visible(true);
            self.nodes_list.set_visible(false);
            self.load_more_btn.set_visible(false);
            return;
        }

        self.empty_nodes_label.set_visible(false);
        self.nodes_list.set_visible(true);

        let is_selectable = matches!(active_group.group_type.as_str(), "Selector" | "Fallback")
            || active_group.name == "GLOBAL";

        let limit = *self.visible_count.borrow();
        let display_nodes = &matching_nodes[..matching_nodes.len().min(limit)];

        let is_narrow = self.is_narrow.get();
        let mut new_rendered_nodes = Vec::new();

        for node in display_nodes {
            let is_selected = node.name == active_group.now;

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

            let (delay_text, delay_class) = format_delay(node.delay);

            let delay_label = Label::builder()
                .label(&delay_text)
                .css_classes(["pill", "caption", delay_class])
                .build();
            node_ping_btn.set_child(Some(&delay_label));

            {
                let service = self.service.clone();
                let group_name = active_group.name.clone();
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
                                let (text, cls) = format_delay(Some(d));
                                lbl.set_label(&text);
                                lbl.set_css_classes(&["pill", "caption", cls]);
                            }
                            Err(_) => {
                                lbl.set_label(&tr("timeout_badge"));
                                lbl.set_css_classes(&["pill", "caption", "error"]);
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
            setup_node_suffix(
                &suffix_box,
                &prow,
                &self.service,
                &active_group.name,
                &node.name,
                is_selected,
                is_selectable,
            );

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
                suffix_box: suffix_box.clone(),
                meta_box,
                is_narrow: Cell::new(is_narrow),
            };
            self.row_handles.borrow_mut().push(handle);

            new_rendered_nodes.push(NodeWidgets {
                name: node.name.clone(),
                delay_label,
                suffix_box,
                prow: prow.clone(),
                is_selectable,
                is_selected,
                delay: node.delay,
            });

            self.nodes_list.append(&prow);
        }

        *self.active_rendered_nodes.borrow_mut() = new_rendered_nodes;

        if matching_nodes.len() > limit {
            self.load_more_btn.set_label(&format!(
                "{} ({} / {})",
                tr("btn_load_more_nodes"),
                limit,
                matching_nodes.len()
            ));
            self.load_more_btn.set_visible(true);
        } else {
            self.load_more_btn.set_visible(false);
        }
    }
}
