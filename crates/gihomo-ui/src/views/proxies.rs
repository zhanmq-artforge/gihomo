use adw::prelude::*;
use gihomo_app::AppService;
use gihomo_core::{ProxyGroup, ProxyNode};
use gtk4::{Button, Label, Orientation, ScrolledWindow, SearchEntry};
use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::Rc;
use std::time::Duration;

use crate::i18n::tr;

const INITIAL_VISIBLE_COUNT: usize = 60;
const LOAD_MORE_STEP: usize = 60;

// --- Helper Types & Functions for Node Rows ---

struct NodeRowHandle {
    title_vbox: gtk4::Box,
    right_box: gtk4::Box,
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
            self.right_box.prepend(&self.meta_box);
        }
    }
}

struct NodeWidgets {
    name: String,
    delay_label: Label,
    action_box: gtk4::Box,
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

#[allow(clippy::too_many_arguments)]
fn setup_node_actions(
    action_box: &gtk4::Box,
    delay_label: &Label,
    prow: &adw::PreferencesRow,
    service: &AppService,
    group_name: &str,
    node_name: &str,
    is_selected: bool,
    is_selectable: bool,
) {
    while let Some(child) = action_box.first_child() {
        action_box.remove(&child);
    }

    // 1. Single Node Ping Icon Button (always available for each node)
    let node_ping_btn = Button::builder()
        .icon_name("network-transmit-receive-symbolic")
        .valign(gtk4::Align::Center)
        .css_classes(["flat", "circular"])
        .tooltip_text(tr("tooltip_ping_node"))
        .build();

    {
        let service = service.clone();
        let group_name = group_name.to_string();
        let node_name = node_name.to_string();
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
                        lbl.set_label(tr("timeout_badge"));
                        lbl.set_css_classes(&["pill", "caption", "error"]);
                    }
                }
                btn.set_sensitive(true);
            });
        });
    }
    action_box.append(&node_ping_btn);

    // 2. Select Button or Active In-Use Badge
    if is_selected {
        let active_badge = Label::builder()
            .label(tr("selected_badge"))
            .css_classes(["pill", "caption", "accent"])
            .valign(gtk4::Align::Center)
            .build();
        action_box.append(&active_badge);
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
        action_box.append(&select_btn);

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

// --- Group Card: Stacked Expander Card for Each Proxy Group ---

struct GroupCard {
    group_name: String,
    card_box: gtk4::Box,
    header_event_box: gtk4::Box,
    title_vbox: gtk4::Box,
    spacer: gtk4::Box,
    now_box: gtk4::Box,
    arrow_icon: gtk4::Image,
    now_node_badge: Label,
    now_delay_badge: Label,
    node_count_label: Label,
    revealer: gtk4::Revealer,
    nodes_list: gtk4::ListBox,
    empty_nodes_label: Label,
    load_more_btn: Button,

    service: AppService,
    is_expanded: Rc<Cell<bool>>,
    visible_count: Rc<Cell<usize>>,
    active_rendered_nodes: Rc<RefCell<Vec<NodeWidgets>>>,
    row_handles: Rc<RefCell<Vec<NodeRowHandle>>>,
    group_cache: Rc<RefCell<ProxyGroup>>,
    is_narrow: Rc<Cell<bool>>,
}

impl GroupCard {
    fn new<F>(
        group: &ProxyGroup,
        is_expanded: bool,
        service: AppService,
        is_narrow: bool,
        on_toggle: F,
    ) -> Self
    where
        F: Fn(&str, bool) + 'static,
    {
        let card_box = gtk4::Box::new(Orientation::Vertical, 0);
        card_box.add_css_class("card");
        card_box.set_margin_bottom(8);

        // --- Card Header Bar (No group ping button needed) ---
        let header_event_box = gtk4::Box::new(Orientation::Horizontal, 8);
        header_event_box.set_margin_top(10);
        header_event_box.set_margin_bottom(10);
        header_event_box.set_margin_start(14);
        header_event_box.set_margin_end(14);
        header_event_box.set_valign(gtk4::Align::Center);

        // Left Container: title_vbox contains Title Row (arrow + title) and optionally now_box (on narrow screen)
        let title_vbox = gtk4::Box::new(Orientation::Vertical, 3);
        title_vbox.set_valign(gtk4::Align::Center);

        let title_hrow = gtk4::Box::new(Orientation::Horizontal, 8);
        title_hrow.set_valign(gtk4::Align::Center);

        // Expand/Collapse Arrow
        let arrow_icon = gtk4::Image::builder()
            .icon_name(if is_expanded {
                "pan-down-symbolic"
            } else {
                "pan-end-symbolic"
            })
            .css_classes(["dim-label"])
            .valign(gtk4::Align::Center)
            .build();
        title_hrow.append(&arrow_icon);

        // Group Title with localized type tooltip (clean interface without redundant badge)
        let type_desc = match group.group_type.as_str() {
            "Selector" => "手动选择 (Selector)",
            "URLTest" => "自动优选 (URLTest)",
            "Fallback" => "故障转移 (Fallback)",
            _ => &group.group_type,
        };
        let title_label = Label::builder()
            .label(&group.name)
            .tooltip_text(format!(
                "{}: {}",
                tr("group_type_prefix").trim_end_matches(": "),
                type_desc
            ))
            .css_classes(["heading"])
            .valign(gtk4::Align::Center)
            .halign(gtk4::Align::Start)
            .build();
        title_hrow.append(&title_label);
        title_vbox.append(&title_hrow);

        // Spacer to push right-hand metadata to the end
        let spacer = gtk4::Box::new(Orientation::Horizontal, 0);
        spacer.set_hexpand(true);

        // Current Active Node & Delay Preview
        let now_node = group.now.clone();
        let now_delay = group
            .nodes
            .iter()
            .find(|n| n.name == now_node)
            .and_then(|n| n.delay);

        let now_box = gtk4::Box::new(Orientation::Horizontal, 6);
        now_box.set_valign(gtk4::Align::Center);

        let now_node_badge = Label::builder()
            .label(format!(
                "{}: {}",
                tr("current_node_prefix").trim_end_matches(": "),
                now_node
            ))
            .css_classes(["caption", "heading"])
            .ellipsize(gtk4::pango::EllipsizeMode::End)
            .max_width_chars(24)
            .valign(gtk4::Align::Center)
            .build();

        let (delay_str, delay_cls) = format_delay(now_delay);
        let now_delay_badge = Label::builder()
            .label(&delay_str)
            .css_classes(["pill", "caption", delay_cls])
            .valign(gtk4::Align::Center)
            .build();

        now_box.append(&now_node_badge);
        now_box.append(&now_delay_badge);

        // Node Count Label
        let node_count_label = Label::builder()
            .label(format!(
                "{} {}",
                group.nodes.len(),
                tr("group_nodes_count_suffix")
            ))
            .css_classes(["caption", "dim-label"])
            .valign(if is_narrow {
                gtk4::Align::Start
            } else {
                gtk4::Align::Center
            })
            .margin_top(if is_narrow { 3 } else { 0 })
            .build();

        header_event_box.append(&title_vbox);
        header_event_box.append(&spacer);

        if is_narrow {
            now_box.set_margin_start(24);
            title_vbox.append(&now_box);
            header_event_box.append(&node_count_label);
        } else {
            now_box.set_margin_start(0);
            header_event_box.append(&now_box);
            header_event_box.append(&node_count_label);
        }

        // --- Card Body (Revealer) ---
        let revealer = gtk4::Revealer::builder()
            .transition_type(gtk4::RevealerTransitionType::SlideDown)
            .transition_duration(200)
            .reveal_child(is_expanded)
            .build();

        let body_box = gtk4::Box::new(Orientation::Vertical, 6);
        body_box.set_margin_bottom(10);
        body_box.set_margin_start(10);
        body_box.set_margin_end(10);

        let separator = gtk4::Separator::new(Orientation::Horizontal);
        separator.set_margin_bottom(6);
        body_box.append(&separator);

        let nodes_list = gtk4::ListBox::new();
        nodes_list.set_selection_mode(gtk4::SelectionMode::None);
        nodes_list.add_css_class("boxed-list");
        body_box.append(&nodes_list);

        let empty_nodes_label = Label::builder()
            .label(tr("empty_group_nodes"))
            .css_classes(["dim-label", "caption"])
            .margin_top(12)
            .margin_bottom(12)
            .halign(gtk4::Align::Center)
            .build();
        empty_nodes_label.set_visible(false);
        body_box.append(&empty_nodes_label);

        let load_more_btn = Button::builder()
            .label(tr("btn_load_more_nodes"))
            .css_classes(["suggested-action", "pill"])
            .halign(gtk4::Align::Center)
            .margin_top(6)
            .margin_bottom(6)
            .build();
        load_more_btn.set_visible(false);
        body_box.append(&load_more_btn);

        revealer.set_child(Some(&body_box));

        card_box.append(&header_event_box);
        card_box.append(&revealer);

        let is_expanded_cell = Rc::new(Cell::new(is_expanded));
        let visible_count = Rc::new(Cell::new(INITIAL_VISIBLE_COUNT));
        let active_rendered_nodes = Rc::new(RefCell::new(Vec::new()));
        let row_handles: Rc<RefCell<Vec<NodeRowHandle>>> = Rc::new(RefCell::new(Vec::new()));
        let group_cache = Rc::new(RefCell::new(group.clone()));
        let is_narrow_cell = Rc::new(Cell::new(is_narrow));

        // Connect Header Click Gesture for Expand / Collapse
        let gesture = gtk4::GestureClick::new();
        {
            let is_expanded = is_expanded_cell.clone();
            let revealer = revealer.clone();
            let arrow_icon = arrow_icon.clone();
            let group_name = group.name.clone();
            gesture.connect_released(move |_, _, _, _| {
                let next_state = !is_expanded.get();
                is_expanded.set(next_state);
                revealer.set_reveal_child(next_state);
                arrow_icon.set_icon_name(if next_state {
                    Some("pan-down-symbolic")
                } else {
                    Some("pan-end-symbolic")
                });
                on_toggle(&group_name, next_state);
            });
        }
        header_event_box.add_controller(gesture);

        let card = Self {
            group_name: group.name.clone(),
            card_box,
            header_event_box,
            title_vbox,
            spacer,
            now_box,
            arrow_icon,
            now_node_badge,
            now_delay_badge,
            node_count_label,
            revealer,
            nodes_list,
            empty_nodes_label,
            load_more_btn,
            service,
            is_expanded: is_expanded_cell,
            visible_count,
            active_rendered_nodes,
            row_handles,
            group_cache,
            is_narrow: is_narrow_cell,
        };

        // Wire Load More Button
        {
            let card_clone = card.clone();
            card.load_more_btn.connect_clicked(move |_| {
                card_clone
                    .visible_count
                    .set(card_clone.visible_count.get() + LOAD_MORE_STEP);
                card_clone.render_nodes("");
            });
        }

        // Initial render of nodes
        card.render_nodes("");
        card
    }

    fn clone(&self) -> Self {
        Self {
            group_name: self.group_name.clone(),
            card_box: self.card_box.clone(),
            header_event_box: self.header_event_box.clone(),
            title_vbox: self.title_vbox.clone(),
            spacer: self.spacer.clone(),
            now_box: self.now_box.clone(),
            arrow_icon: self.arrow_icon.clone(),
            now_node_badge: self.now_node_badge.clone(),
            now_delay_badge: self.now_delay_badge.clone(),
            node_count_label: self.node_count_label.clone(),
            revealer: self.revealer.clone(),
            nodes_list: self.nodes_list.clone(),
            empty_nodes_label: self.empty_nodes_label.clone(),
            load_more_btn: self.load_more_btn.clone(),
            service: self.service.clone(),
            is_expanded: self.is_expanded.clone(),
            visible_count: self.visible_count.clone(),
            active_rendered_nodes: self.active_rendered_nodes.clone(),
            row_handles: self.row_handles.clone(),
            group_cache: self.group_cache.clone(),
            is_narrow: self.is_narrow.clone(),
        }
    }

    fn update_data(&self, group: &ProxyGroup, search_query: &str) -> bool {
        *self.group_cache.borrow_mut() = group.clone();

        // Update header badges
        let now_node = &group.now;
        let now_delay = group
            .nodes
            .iter()
            .find(|n| &n.name == now_node)
            .and_then(|n| n.delay);

        self.now_node_badge.set_label(&format!(
            "{}: {}",
            tr("current_node_prefix").trim_end_matches(": "),
            now_node
        ));

        let (delay_str, delay_cls) = format_delay(now_delay);
        self.now_delay_badge.set_label(&delay_str);
        self.now_delay_badge
            .set_css_classes(&["pill", "caption", delay_cls]);

        self.node_count_label.set_label(&format!(
            "{} {}",
            group.nodes.len(),
            tr("group_nodes_count_suffix")
        ));

        // Attempt in-place diff update for nodes if structure matches
        if self.try_incremental_update(group, search_query) {
            return true;
        }

        // Structural difference -> full rebuild of this group's nodes
        self.render_nodes(search_query);
        true
    }

    fn try_incremental_update(&self, group: &ProxyGroup, search_query: &str) -> bool {
        let query = search_query.to_lowercase().trim().to_string();
        let matching_nodes: Vec<&ProxyNode> = group
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

        let limit = self.visible_count.get();
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

        // Structural match! Update node rows in place
        for (node_w, node) in rendered.iter_mut().zip(matching_nodes.iter()) {
            if node_w.delay != node.delay {
                node_w.delay = node.delay;
                let (delay_text, delay_class) = format_delay(node.delay);
                node_w.delay_label.set_label(&delay_text);
                node_w
                    .delay_label
                    .set_css_classes(&["pill", "caption", delay_class]);
            }

            let is_selected = node.name == group.now;
            if node_w.is_selected != is_selected {
                node_w.is_selected = is_selected;
                setup_node_actions(
                    &node_w.action_box,
                    &node_w.delay_label,
                    &node_w.prow,
                    &self.service,
                    &group.name,
                    &node.name,
                    is_selected,
                    node_w.is_selectable,
                );
            }
            node_w.prow.set_sensitive(true);
        }

        true
    }

    fn render_nodes(&self, search_query: &str) {
        let group = self.group_cache.borrow();
        let query = search_query.to_lowercase().trim().to_string();

        let matching_nodes: Vec<&ProxyNode> = group
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

        let is_selectable =
            matches!(group.group_type.as_str(), "Selector" | "Fallback") || group.name == "GLOBAL";

        let limit = self.visible_count.get();
        let display_nodes = &matching_nodes[..matching_nodes.len().min(limit)];

        let is_narrow = self.is_narrow.get();
        let mut new_rendered_nodes = Vec::new();

        for node in display_nodes {
            let is_selected = node.name == group.now;

            let prow = adw::PreferencesRow::builder()
                .activatable(is_selectable && !is_selected)
                .selectable(false)
                .build();

            let hbox = gtk4::Box::new(Orientation::Horizontal, 12);
            hbox.set_margin_top(6);
            hbox.set_margin_bottom(6);
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

            // Meta Box: Type Badge + Delay Badge
            let meta_box = gtk4::Box::new(Orientation::Horizontal, 6);
            meta_box.set_valign(gtk4::Align::Center);
            meta_box.set_halign(gtk4::Align::Start);

            let type_badge = Label::builder()
                .label(&node.node_type)
                .css_classes(["pill", "caption", "dim-label"])
                .valign(gtk4::Align::Center)
                .build();
            meta_box.append(&type_badge);

            let (delay_text, delay_class) = format_delay(node.delay);
            let delay_label = Label::builder()
                .label(&delay_text)
                .css_classes(["pill", "caption", delay_class])
                .valign(gtk4::Align::Center)
                .build();
            meta_box.append(&delay_label);

            // Action Box: Ping icon button + Select button / Active badge
            let action_box = gtk4::Box::new(Orientation::Horizontal, 6);
            action_box.set_valign(gtk4::Align::Center);

            setup_node_actions(
                &action_box,
                &delay_label,
                &prow,
                &self.service,
                &group.name,
                &node.name,
                is_selected,
                is_selectable,
            );

            // Right Box holds meta_box (on wide screen) and action_box
            let right_box = gtk4::Box::new(Orientation::Horizontal, 10);
            right_box.set_valign(gtk4::Align::Center);
            right_box.append(&action_box);

            if is_narrow {
                title_vbox.append(&meta_box);
            } else {
                right_box.prepend(&meta_box);
            }

            hbox.append(&title_vbox);
            hbox.append(&right_box);
            prow.set_child(Some(&hbox));

            let handle = NodeRowHandle {
                title_vbox,
                right_box,
                meta_box,
                is_narrow: Cell::new(is_narrow),
            };
            self.row_handles.borrow_mut().push(handle);

            new_rendered_nodes.push(NodeWidgets {
                name: node.name.clone(),
                delay_label,
                action_box,
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

    fn set_narrow(&self, narrow: bool) {
        if self.is_narrow.get() == narrow {
            return;
        }
        self.is_narrow.set(narrow);

        if narrow {
            if self.now_box.parent().as_ref() == Some(self.header_event_box.upcast_ref()) {
                self.header_event_box.remove(&self.now_box);
            }
            self.now_box.set_margin_start(24);
            if self.now_box.parent().is_none() {
                self.title_vbox.append(&self.now_box);
            }
            self.node_count_label.set_valign(gtk4::Align::Start);
            self.node_count_label.set_margin_top(3);
        } else {
            if self.now_box.parent().as_ref() == Some(self.title_vbox.upcast_ref()) {
                self.title_vbox.remove(&self.now_box);
            }
            self.now_box.set_margin_start(0);
            if self.now_box.parent().is_none() {
                self.header_event_box
                    .insert_child_after(&self.now_box, Some(&self.spacer));
            }
            self.node_count_label.set_valign(gtk4::Align::Center);
            self.node_count_label.set_margin_top(0);
        }

        for handle in self.row_handles.borrow().iter() {
            handle.set_narrow(narrow);
        }
    }
}

// --- Main ProxiesView ---

#[derive(Clone)]
pub struct ProxiesView {
    pub page: adw::NavigationPage,
    status_page: adw::StatusPage,
    main_box: gtk4::Box,

    // Controls
    mode_label: Label,
    btn_rule_mode: Button,
    btn_global_mode: Button,
    btn_direct_mode: Button,
    search_entry: SearchEntry,
    ping_all_btn: Button,
    refresh_btn: Button,

    // Banners
    global_banner_box: gtk4::Box,
    direct_banner_box: gtk4::Box,

    // Cards list container
    cards_box: gtk4::Box,

    service: AppService,
    is_updating_mode: Rc<Cell<bool>>,
    groups_cache: Rc<RefCell<Vec<ProxyGroup>>>,
    current_mode: Rc<RefCell<String>>,
    expanded_groups: Rc<RefCell<HashSet<String>>>,
    group_cards: Rc<RefCell<Vec<GroupCard>>>,
    search_query: Rc<RefCell<String>>,
    search_debounce_source: Rc<RefCell<Option<glib::SourceId>>>,
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
            .maximum_size(880)
            .tightening_threshold(680)
            .build();

        let content_box = gtk4::Box::new(Orientation::Vertical, 14);
        content_box.set_margin_top(16);
        content_box.set_margin_bottom(24);
        content_box.set_margin_start(12);
        content_box.set_margin_end(12);

        // 1. Top Control Box (Adaptive single-row or two-row layout)
        let top_box = gtk4::Box::new(Orientation::Vertical, 8);
        top_box.set_hexpand(true);

        let mode_row = gtk4::Box::new(Orientation::Horizontal, 10);
        mode_row.set_hexpand(true);

        let mode_label = Label::builder()
            .label(tr("proxies_mode_label"))
            .css_classes(["dim-label"])
            .valign(gtk4::Align::Center)
            .build();
        mode_row.append(&mode_label);

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
        mode_row.append(&mode_box);

        // Action Row: Search + Batch Ping & Refresh (Always on its own full-width line)
        let action_row = gtk4::Box::new(Orientation::Horizontal, 8);
        action_row.set_valign(gtk4::Align::Center);
        action_row.set_hexpand(true);

        let search_entry = SearchEntry::builder()
            .placeholder_text(tr("proxy_search_placeholder"))
            .valign(gtk4::Align::Center)
            .hexpand(true)
            .build();
        action_row.append(&search_entry);

        let ping_all_btn = Button::builder()
            .icon_name("network-transmit-receive-symbolic")
            .tooltip_text(tr("tooltip_ping_all_groups"))
            .valign(gtk4::Align::Center)
            .css_classes(["flat"])
            .build();

        let refresh_btn = Button::builder()
            .icon_name("view-refresh-symbolic")
            .tooltip_text(tr("tooltip_refresh_proxies"))
            .valign(gtk4::Align::Center)
            .css_classes(["flat"])
            .build();

        action_row.append(&ping_all_btn);
        action_row.append(&refresh_btn);

        top_box.append(&mode_row);
        top_box.append(&action_row);

        // 2. Mode Notice Banners (Shown contextually)
        let global_banner_box = gtk4::Box::new(Orientation::Horizontal, 10);
        global_banner_box.add_css_class("card");
        global_banner_box.set_margin_bottom(4);
        global_banner_box.set_margin_top(2);
        let global_icon = gtk4::Image::from_icon_name("dialog-information-symbolic");
        let global_lbl = Label::builder()
            .label(tr("mode_global_banner"))
            .wrap(true)
            .halign(gtk4::Align::Start)
            .hexpand(true)
            .css_classes(["caption"])
            .build();
        global_banner_box.append(&global_icon);
        global_banner_box.append(&global_lbl);
        global_banner_box.set_visible(false);

        let direct_banner_box = gtk4::Box::new(Orientation::Horizontal, 10);
        direct_banner_box.add_css_class("card");
        direct_banner_box.set_margin_bottom(4);
        direct_banner_box.set_margin_top(2);
        let direct_icon = gtk4::Image::from_icon_name("network-offline-symbolic");
        let direct_lbl = Label::builder()
            .label(tr("mode_direct_banner"))
            .wrap(true)
            .halign(gtk4::Align::Start)
            .hexpand(true)
            .css_classes(["caption"])
            .build();
        direct_banner_box.append(&direct_icon);
        direct_banner_box.append(&direct_lbl);
        direct_banner_box.set_visible(false);

        // 3. Status page for empty state
        let status_page = adw::StatusPage::builder()
            .icon_name("network-server-symbolic")
            .title(tr("empty_proxies_title"))
            .description(tr("empty_proxies_desc"))
            .vexpand(true)
            .build();

        // 4. Main content container for stacked group cards
        let main_box = gtk4::Box::new(Orientation::Vertical, 10);
        main_box.set_visible(false);

        let cards_box = gtk4::Box::new(Orientation::Vertical, 10);
        main_box.append(&cards_box);

        content_box.append(&top_box);
        content_box.append(&global_banner_box);
        content_box.append(&direct_banner_box);
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
        let groups_cache: Rc<RefCell<Vec<ProxyGroup>>> = Rc::new(RefCell::new(Vec::new()));
        let current_mode = Rc::new(RefCell::new("rule".to_string()));
        let expanded_groups: Rc<RefCell<HashSet<String>>> = Rc::new(RefCell::new(HashSet::new()));
        let group_cards: Rc<RefCell<Vec<GroupCard>>> = Rc::new(RefCell::new(Vec::new()));
        let search_query = Rc::new(RefCell::new(String::new()));
        let search_debounce_source: Rc<RefCell<Option<glib::SourceId>>> =
            Rc::new(RefCell::new(None));
        let is_narrow = Rc::new(Cell::new(false));

        let breakpoint_bin = adw::BreakpointBin::new();
        breakpoint_bin.set_child(Some(&scrolled));

        // Threshold 560px: GNOME HIG / Libadwaita content-level standard breakpoint
        let breakpoint = adw::Breakpoint::new(
            adw::BreakpointCondition::parse("max-width: 560px")
                .expect("valid breakpoint condition"),
        );

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
            search_entry,
            ping_all_btn,
            refresh_btn,
            global_banner_box,
            direct_banner_box,
            cards_box,
            service,
            is_updating_mode,
            groups_cache,
            current_mode,
            expanded_groups,
            group_cards,
            search_query,
            search_debounce_source,
            is_narrow,
        };

        let view_apply = view.clone();
        breakpoint.connect_apply(move |_| {
            view_apply.set_narrow(true);
        });

        let view_unapply = view.clone();
        breakpoint.connect_unapply(move |_| {
            view_unapply.set_narrow(false);
        });
        breakpoint_bin.add_breakpoint(breakpoint);

        toolbar_view.set_content(Some(&breakpoint_bin));

        view.wire_handlers();
        view
    }

    fn wire_handlers(&self) {
        // Mode switch buttons
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

        // Global Batch Ping Button
        {
            let service = self.service.clone();
            let groups_cache = self.groups_cache.clone();
            let current_mode = self.current_mode.clone();
            let ping_btn_clone = self.ping_all_btn.clone();
            self.ping_all_btn.connect_clicked(move |_| {
                ping_btn_clone.set_sensitive(false);
                let service = service.clone();
                let groups = groups_cache.borrow().clone();
                let mode = current_mode.borrow().clone();
                let btn = ping_btn_clone.clone();

                let target_groups = filter_and_sort_groups(&groups, &mode);
                glib::MainContext::default().spawn_local(async move {
                    for g in target_groups {
                        let _ = service.test_group_delay(&g.name).await;
                    }
                    btn.set_sensitive(true);
                });
            });
        }

        // Refresh Button
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

        // Search Entry with 200ms debounce
        {
            let view_clone = self.clone();
            let debounce_source = self.search_debounce_source.clone();
            let search_query = self.search_query.clone();
            self.search_entry.connect_search_changed(move |entry| {
                if let Some(source) = debounce_source.borrow_mut().take() {
                    source.remove();
                }
                let text = entry.text().to_string();
                let view = view_clone.clone();
                let sq = search_query.clone();
                let debounce_holder = debounce_source.clone();
                let source_id =
                    glib::timeout_add_local_once(Duration::from_millis(200), move || {
                        *debounce_holder.borrow_mut() = None;
                        *sq.borrow_mut() = text;
                        view.reapply_search();
                    });
                *debounce_source.borrow_mut() = Some(source_id);
            });
        }
    }

    fn reapply_search(&self) {
        let query = self.search_query.borrow().clone();
        for card in self.group_cards.borrow().iter() {
            card.render_nodes(&query);
        }
    }

    pub fn set_narrow(&self, narrow: bool) {
        if self.is_narrow.get() == narrow {
            return;
        }
        self.is_narrow.set(narrow);

        // Adapt all cards and their internal node rows
        for card in self.group_cards.borrow().iter() {
            card.set_narrow(narrow);
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

        self.search_entry
            .set_placeholder_text(Some(tr("proxy_search_placeholder")));
        self.ping_all_btn
            .set_tooltip_text(Some(tr("tooltip_ping_all_groups")));
        self.refresh_btn
            .set_tooltip_text(Some(tr("tooltip_refresh_proxies")));

        let groups = self.groups_cache.borrow().clone();
        self.render_all_cards(&groups);
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
            "global" => {
                self.btn_global_mode.add_css_class("suggested-action");
                self.global_banner_box.set_visible(true);
                self.direct_banner_box.set_visible(false);
            }
            "direct" => {
                self.btn_direct_mode.add_css_class("suggested-action");
                self.global_banner_box.set_visible(false);
                self.direct_banner_box.set_visible(true);
            }
            _ => {
                self.btn_rule_mode.add_css_class("suggested-action");
                self.global_banner_box.set_visible(false);
                self.direct_banner_box.set_visible(false);
            }
        }

        self.is_updating_mode.set(false);

        let groups = self.groups_cache.borrow().clone();
        self.render_all_cards(&groups);
    }

    pub fn update_groups(&self, groups: &[ProxyGroup]) {
        *self.groups_cache.borrow_mut() = groups.to_vec();
        self.render_all_cards(groups);
    }

    fn render_all_cards(&self, groups: &[ProxyGroup]) {
        if groups.is_empty() {
            self.status_page.set_visible(true);
            self.main_box.set_visible(false);
            return;
        }

        self.status_page.set_visible(false);
        self.main_box.set_visible(true);

        let mode = self.current_mode.borrow().clone();
        let target_groups = filter_and_sort_groups(groups, &mode);

        if target_groups.is_empty() {
            if mode == "direct" {
                self.cards_box.set_visible(false);
            } else {
                self.cards_box.set_visible(false);
                self.status_page.set_visible(true);
            }
            return;
        }

        self.cards_box.set_visible(true);

        // Check if existing cards structurally match target_groups
        let mut cards = self.group_cards.borrow_mut();
        let can_incremental = cards.len() == target_groups.len()
            && cards
                .iter()
                .zip(target_groups.iter())
                .all(|(card, group)| card.group_name == group.name);

        let query = self.search_query.borrow().clone();

        if can_incremental {
            // In-place diff update for each card!
            for (card, group) in cards.iter().zip(target_groups.iter()) {
                card.update_data(group, &query);
            }
            return;
        }

        // Structural difference -> full rebuild of cards_box
        while let Some(child) = self.cards_box.first_child() {
            self.cards_box.remove(&child);
        }
        cards.clear();

        // If expanded_groups set is empty, default expand the 1st Selector group
        let mut expanded_set = self.expanded_groups.borrow_mut();
        if expanded_set.is_empty() {
            if let Some(first_sel) = target_groups.iter().find(|g| g.group_type == "Selector") {
                expanded_set.insert(first_sel.name.clone());
            } else if let Some(first) = target_groups.first() {
                expanded_set.insert(first.name.clone());
            }
        }

        let is_narrow = self.is_narrow.get();
        for group in &target_groups {
            let is_expanded = expanded_set.contains(&group.name);
            let expanded_holder = self.expanded_groups.clone();
            let card = GroupCard::new(
                group,
                is_expanded,
                self.service.clone(),
                is_narrow,
                move |name, expanded| {
                    if expanded {
                        expanded_holder.borrow_mut().insert(name.to_string());
                    } else {
                        expanded_holder.borrow_mut().remove(name);
                    }
                },
            );

            self.cards_box.append(&card.card_box);
            cards.push(card);
        }
    }
}

// --- Mode-based Group Filtering and Sorting ---

fn filter_and_sort_groups(groups: &[ProxyGroup], mode: &str) -> Vec<ProxyGroup> {
    let mode_lower = mode.to_lowercase();
    match mode_lower.as_str() {
        "global" => {
            // Global Mode: Only show the GLOBAL group
            let list: Vec<ProxyGroup> = groups
                .iter()
                .filter(|g| g.name == "GLOBAL")
                .cloned()
                .collect();
            if list.is_empty() {
                groups.to_vec()
            } else {
                list
            }
        }
        "direct" => {
            // Direct Mode: Do not display proxy groups
            Vec::new()
        }
        _ => {
            // Rule Mode: Filter out GLOBAL! Only show user's subscription routing groups
            let mut list: Vec<ProxyGroup> = groups
                .iter()
                .filter(|g| g.name != "GLOBAL")
                .cloned()
                .collect();

            if list.is_empty() {
                list = groups.to_vec();
            } else {
                // Priority: Selector groups first, then URLTest, Fallback, etc.
                list.sort_by(|a, b| {
                    let type_priority = |g: &ProxyGroup| match g.group_type.as_str() {
                        "Selector" => 0,
                        "URLTest" => 1,
                        "Fallback" => 2,
                        _ => 3,
                    };
                    type_priority(a)
                        .cmp(&type_priority(b))
                        .then_with(|| a.name.cmp(&b.name))
                });
            }
            list
        }
    }
}
