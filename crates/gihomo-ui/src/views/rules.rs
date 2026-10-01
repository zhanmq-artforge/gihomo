use crate::i18n::tr;
use adw::prelude::*;
use gihomo_app::AppService;
use gihomo_core::{RuleItem, RuleProvider};
use gtk4::{
    Button, Label, Orientation, ScrolledWindow, SearchEntry,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

struct RuleRowHandle {
    title_vbox: gtk4::Box,
    suffix_box: gtk4::Box,
    meta_box: gtk4::Box,
    is_narrow: Cell<bool>,
}

impl RuleRowHandle {
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

pub struct RulesView {
    pub page: adw::NavigationPage,
    service: AppService,
    all_rules: Rc<RefCell<Vec<RuleItem>>>,
    all_providers: Rc<RefCell<Vec<RuleProvider>>>,
    rules_group: adw::PreferencesGroup,
    rules_list: gtk4::ListBox,
    providers_group: adw::PreferencesGroup,
    providers_list: gtk4::ListBox,
    search_entry: SearchEntry,
    refresh_btn: Button,
    status_page: adw::StatusPage,
    load_more_btn: Button,
    visible_count: Rc<RefCell<usize>>,
    current_filter: Rc<RefCell<String>>,
    row_handles: Rc<RefCell<Vec<RuleRowHandle>>>,
    is_narrow: Rc<Cell<bool>>,
}

const PAGE_SIZE: usize = 200;

impl RulesView {
    pub fn new(service: AppService) -> Self {
        let header_bar = adw::HeaderBar::builder()
            .show_title(true)
            .centering_policy(adw::CenteringPolicy::Strict)
            .build();

        let toolbar_view = adw::ToolbarView::new();
        toolbar_view.add_top_bar(&header_bar);

        let clamp = adw::Clamp::builder()
            .maximum_size(960)
            .tightening_threshold(700)
            .build();

        let content_box = gtk4::Box::new(Orientation::Vertical, 16);
        content_box.set_margin_top(16);
        content_box.set_margin_bottom(24);
        content_box.set_margin_start(12);
        content_box.set_margin_end(12);

        // Top Toolbar: Search Entry + Refresh Button
        let top_bar = gtk4::Box::new(Orientation::Horizontal, 8);

        let search_entry = SearchEntry::builder()
            .placeholder_text(tr("rules_search_placeholder"))
            .hexpand(true)
            .build();
        top_bar.append(&search_entry);

        let refresh_btn = Button::builder()
            .icon_name("view-refresh-symbolic")
            .tooltip_text(tr("rules_refresh_tooltip"))
            .valign(gtk4::Align::Center)
            .css_classes(["flat"])
            .build();
        top_bar.append(&refresh_btn);

        // Providers Group
        let providers_group = adw::PreferencesGroup::builder()
            .title(tr("rules_providers_title"))
            .description(tr("rules_providers_desc"))
            .build();
        providers_group.set_visible(false);

        let providers_list = gtk4::ListBox::new();
        providers_list.set_selection_mode(gtk4::SelectionMode::None);
        providers_list.add_css_class("boxed-list");
        providers_group.add(&providers_list);

        // Rules List Group
        let rules_group = adw::PreferencesGroup::builder()
            .title(tr("rules_list_title"))
            .description(tr("rules_list_desc"))
            .build();

        let rules_list = gtk4::ListBox::new();
        rules_list.set_selection_mode(gtk4::SelectionMode::None);
        rules_list.add_css_class("boxed-list");
        rules_group.add(&rules_list);

        // Load more button
        let load_more_btn = Button::builder()
            .label(tr("btn_load_more"))
            .css_classes(["suggested-action", "pill"])
            .halign(gtk4::Align::Center)
            .margin_top(12)
            .margin_bottom(12)
            .build();
        load_more_btn.set_visible(false);

        // Status page for empty state
        let status_page = adw::StatusPage::builder()
            .icon_name("view-list-bullet-symbolic")
            .title(tr("empty_rules_title"))
            .description(tr("empty_rules_desc"))
            .vexpand(true)
            .build();

        content_box.append(&top_bar);
        content_box.append(&status_page);
        content_box.append(&providers_group);
        content_box.append(&rules_group);
        content_box.append(&load_more_btn);

        clamp.set_child(Some(&content_box));

        let scrolled = ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .vexpand(true)
            .hexpand(true)
            .child(&clamp)
            .build();

        let row_handles: Rc<RefCell<Vec<RuleRowHandle>>> = Rc::new(RefCell::new(Vec::new()));
        let is_narrow = Rc::new(Cell::new(false));

        let breakpoint_bin = adw::BreakpointBin::new();
        breakpoint_bin.set_child(Some(&scrolled));

        let breakpoint = adw::Breakpoint::new(
            adw::BreakpointCondition::parse("max-width: 560px")
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
            .title(tr("tab_rules"))
            .tag("rules")
            .child(&toolbar_view)
            .build();

        let all_rules = Rc::new(RefCell::new(Vec::new()));
        let all_providers = Rc::new(RefCell::new(Vec::new()));
        let visible_count = Rc::new(RefCell::new(PAGE_SIZE));
        let current_filter = Rc::new(RefCell::new(String::new()));

        let view = Self {
            page,
            service: service.clone(),
            all_rules: all_rules.clone(),
            all_providers: all_providers.clone(),
            rules_group: rules_group.clone(),
            rules_list: rules_list.clone(),
            providers_group: providers_group.clone(),
            providers_list: providers_list.clone(),
            search_entry: search_entry.clone(),
            refresh_btn: refresh_btn.clone(),
            status_page: status_page.clone(),
            load_more_btn: load_more_btn.clone(),
            visible_count: visible_count.clone(),
            current_filter: current_filter.clone(),
            row_handles: row_handles.clone(),
            is_narrow: is_narrow.clone(),
        };

        // Wire Refresh button
        {
            let service = service.clone();
            let btn = refresh_btn.clone();
            let rules_ref = all_rules.clone();
            let providers_ref = all_providers.clone();
            let rules_group_clone = rules_group.clone();
            let rules_list_clone = rules_list.clone();
            let providers_group_clone = providers_group.clone();
            let providers_list_clone = providers_list.clone();
            let status_pg = status_page.clone();
            let load_btn = load_more_btn.clone();
            let vis_count = visible_count.clone();
            let curr_filter = current_filter.clone();
            let row_handles_clone = row_handles.clone();
            let is_narrow_clone = is_narrow.clone();

            refresh_btn.connect_clicked(move |_| {
                btn.set_sensitive(false);
                let service = service.clone();
                let btn = btn.clone();
                let rules_ref = rules_ref.clone();
                let providers_ref = providers_ref.clone();
                let rules_group_clone = rules_group_clone.clone();
                let rules_list_clone = rules_list_clone.clone();
                let providers_group_clone = providers_group_clone.clone();
                let providers_list_clone = providers_list_clone.clone();
                let status_pg = status_pg.clone();
                let load_btn = load_btn.clone();
                let vis_count = vis_count.clone();
                let curr_filter = curr_filter.clone();
                let row_handles_clone = row_handles_clone.clone();
                let is_narrow_clone = is_narrow_clone.clone();

                glib::MainContext::default().spawn_local(async move {
                    if let Ok(rules) = service.fetch_rules().await {
                        *rules_ref.borrow_mut() = rules;
                    }
                    if let Ok(providers) = service.fetch_rule_providers().await {
                        *providers_ref.borrow_mut() = providers;
                    }

                    *vis_count.borrow_mut() = PAGE_SIZE;
                    Self::render_rules(
                        &rules_ref.borrow(),
                        &curr_filter.borrow(),
                        *vis_count.borrow(),
                        &rules_list_clone,
                        &rules_group_clone,
                        &status_pg,
                        &load_btn,
                        &row_handles_clone,
                        is_narrow_clone.get(),
                    );
                    Self::render_providers(
                        &providers_ref.borrow(),
                        &providers_group_clone,
                        &providers_list_clone,
                        &service,
                    );

                    btn.set_sensitive(true);
                });
            });
        }

        // Wire Search Entry with 200ms debounce
        {
            let rules_ref = all_rules.clone();
            let rules_group_clone = rules_group.clone();
            let rules_list_clone = rules_list.clone();
            let status_pg = status_page.clone();
            let load_btn = load_more_btn.clone();
            let vis_count = visible_count.clone();
            let curr_filter = current_filter.clone();
            let row_handles_clone = row_handles.clone();
            let is_narrow_clone = is_narrow.clone();
            let debounce_timer = Rc::new(RefCell::new(None::<glib::SourceId>));

            search_entry.connect_search_changed(move |entry| {
                if let Some(source_id) = debounce_timer.borrow_mut().take() {
                    source_id.remove();
                }

                let filter = entry.text().trim().to_lowercase();
                let rules_ref = rules_ref.clone();
                let rules_group_clone = rules_group_clone.clone();
                let rules_list_clone = rules_list_clone.clone();
                let status_pg = status_pg.clone();
                let load_btn = load_btn.clone();
                let vis_count = vis_count.clone();
                let curr_filter = curr_filter.clone();
                let row_handles_clone = row_handles_clone.clone();
                let is_narrow = is_narrow_clone.get();
                let debounce_timer_inner = debounce_timer.clone();

                let source_id = glib::timeout_add_local_once(
                    std::time::Duration::from_millis(200),
                    move || {
                        *debounce_timer_inner.borrow_mut() = None;
                        *curr_filter.borrow_mut() = filter.clone();
                        *vis_count.borrow_mut() = PAGE_SIZE;

                        Self::render_rules(
                            &rules_ref.borrow(),
                            &filter,
                            *vis_count.borrow(),
                            &rules_list_clone,
                            &rules_group_clone,
                            &status_pg,
                            &load_btn,
                            &row_handles_clone,
                            is_narrow,
                        );
                    },
                );

                *debounce_timer.borrow_mut() = Some(source_id);
            });
        }

        // Wire Load More Button
        {
            let rules_ref = all_rules.clone();
            let rules_group_clone = rules_group.clone();
            let rules_list_clone = rules_list.clone();
            let status_pg = status_page.clone();
            let load_btn = load_more_btn.clone();
            let vis_count = visible_count.clone();
            let curr_filter = current_filter.clone();
            let row_handles_clone = row_handles.clone();
            let is_narrow_clone = is_narrow.clone();

            load_more_btn.connect_clicked(move |_| {
                let mut c = vis_count.borrow_mut();
                *c += PAGE_SIZE;
                let current_limit = *c;

                Self::render_rules(
                    &rules_ref.borrow(),
                    &curr_filter.borrow(),
                    current_limit,
                    &rules_list_clone,
                    &rules_group_clone,
                    &status_pg,
                    &load_btn,
                    &row_handles_clone,
                    is_narrow_clone.get(),
                );
            });
        }

        view
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

    pub fn update_rules(&self, rules: &[RuleItem]) {
        *self.all_rules.borrow_mut() = rules.to_vec();
        *self.visible_count.borrow_mut() = PAGE_SIZE;
        Self::render_rules(
            &self.all_rules.borrow(),
            &self.current_filter.borrow(),
            *self.visible_count.borrow(),
            &self.rules_list,
            &self.rules_group,
            &self.status_page,
            &self.load_more_btn,
            &self.row_handles,
            self.is_narrow.get(),
        );
    }

    pub fn update_providers(&self, providers: &[RuleProvider]) {
        *self.all_providers.borrow_mut() = providers.to_vec();
        Self::render_providers(
            &self.all_providers.borrow(),
            &self.providers_group,
            &self.providers_list,
            &self.service,
        );
    }

    fn render_rules(
        rules: &[RuleItem],
        filter: &str,
        limit: usize,
        list: &gtk4::ListBox,
        rules_group: &adw::PreferencesGroup,
        status_pg: &adw::StatusPage,
        load_btn: &Button,
        row_handles: &Rc<RefCell<Vec<RuleRowHandle>>>,
        is_narrow: bool,
    ) {
        // Clear list
        while let Some(child) = list.first_child() {
            list.remove(&child);
        }

        row_handles.borrow_mut().clear();

        if rules.is_empty() {
            status_pg.set_visible(true);
            rules_group.set_visible(false);
            load_btn.set_visible(false);
            return;
        }

        status_pg.set_visible(false);
        rules_group.set_visible(true);

        let filtered: Vec<(usize, &RuleItem)> = rules
            .iter()
            .enumerate()
            .filter(|(_, r)| {
                if filter.is_empty() {
                    true
                } else {
                    r.payload.to_lowercase().contains(filter)
                        || r.rule_type.to_lowercase().contains(filter)
                        || r.proxy.to_lowercase().contains(filter)
                }
            })
            .collect();

        let total_matches = filtered.len();
        if filter.is_empty() {
            let desc = tr("rules_list_desc_format").replace("{count}", &rules.len().to_string());
            rules_group.set_description(Some(&desc));
        } else {
            let desc = tr("rules_list_desc_filtered")
                .replace("{filtered}", &total_matches.to_string())
                .replace("{total}", &rules.len().to_string());
            rules_group.set_description(Some(&desc));
        }

        let display_items = filtered.iter().take(limit);

        for (idx, rule) in display_items {
            let prow = adw::PreferencesRow::builder()
                .activatable(false)
                .selectable(false)
                .build();

            let hbox = gtk4::Box::new(Orientation::Horizontal, 12);
            hbox.set_margin_top(8);
            hbox.set_margin_bottom(8);
            hbox.set_margin_start(12);
            hbox.set_margin_end(12);

            // Prefix: Rule Index
            let index_label = Label::builder()
                .label(format!("#{}", idx + 1))
                .css_classes(["dim-label", "caption", "numeric"])
                .valign(gtk4::Align::Center)
                .width_chars(4)
                .build();
            hbox.append(&index_label);

            let title_vbox = gtk4::Box::new(Orientation::Vertical, 4);
            title_vbox.set_hexpand(true);
            title_vbox.set_valign(gtk4::Align::Center);

            let title_text = if rule.payload.is_empty() {
                tr("rule_match_all").to_string()
            } else {
                rule.payload.clone()
            };

            let title_label = Label::builder()
                .label(&title_text)
                .tooltip_text(&title_text)
                .halign(gtk4::Align::Start)
                .ellipsize(gtk4::pango::EllipsizeMode::End)
                .lines(1)
                .hexpand(true)
                .build();
            title_vbox.append(&title_label);

            // Meta Box: Type badge + Target Proxy badge
            let meta_box = gtk4::Box::new(Orientation::Horizontal, 6);
            meta_box.set_valign(gtk4::Align::Center);
            meta_box.set_halign(gtk4::Align::Start);

            let type_css = if rule.rule_type.starts_with("DOMAIN") {
                "accent"
            } else if rule.rule_type.starts_with("IP") {
                "purple"
            } else if rule.rule_type.starts_with("GEO") {
                "teal"
            } else if rule.rule_type == "MATCH" {
                "warning"
            } else {
                "dim-label"
            };

            let type_badge = Label::builder()
                .label(&rule.rule_type)
                .css_classes(["pill", "caption", type_css])
                .valign(gtk4::Align::Center)
                .build();
            meta_box.append(&type_badge);

            let proxy_css = if rule.proxy == "DIRECT" {
                "success"
            } else if rule.proxy == "REJECT" {
                "error"
            } else {
                "accent"
            };

            let proxy_badge = Label::builder()
                .label(&rule.proxy)
                .tooltip_text(&rule.proxy)
                .css_classes(["pill", "caption", proxy_css])
                .valign(gtk4::Align::Center)
                .ellipsize(gtk4::pango::EllipsizeMode::End)
                .max_width_chars(25)
                .build();
            meta_box.append(&proxy_badge);

            let suffix_box = gtk4::Box::new(Orientation::Horizontal, 8);
            suffix_box.set_valign(gtk4::Align::Center);
            suffix_box.set_halign(gtk4::Align::End);

            if is_narrow {
                title_vbox.append(&meta_box);
            } else {
                suffix_box.prepend(&meta_box);
            }

            hbox.append(&title_vbox);
            hbox.append(&suffix_box);
            prow.set_child(Some(&hbox));

            let handle = RuleRowHandle {
                title_vbox,
                suffix_box,
                meta_box,
                is_narrow: Cell::new(is_narrow),
            };
            row_handles.borrow_mut().push(handle);

            list.append(&prow);
        }

        load_btn.set_visible(total_matches > limit);
    }

    fn render_providers(
        providers: &[RuleProvider],
        group: &adw::PreferencesGroup,
        list: &gtk4::ListBox,
        service: &AppService,
    ) {
        // Clear children
        while let Some(child) = list.first_child() {
            list.remove(&child);
        }

        if providers.is_empty() {
            group.set_visible(false);
            return;
        }

        group.set_visible(true);

        for p in providers {
            let updated_at = p
                .updated_at
                .as_deref()
                .map(|value| {
                    chrono::DateTime::parse_from_rfc3339(value)
                        .map(|time| {
                            time.with_timezone(&chrono::Local)
                                .format("%Y-%m-%d %H:%M")
                                .to_string()
                        })
                        .unwrap_or_else(|_| value.to_string())
                })
                .unwrap_or_else(|| tr("sub_never_updated").to_string());
            let row = adw::ActionRow::builder()
                .title(&p.name)
                .subtitle(
                    tr("rules_provider_subtitle")
                        .replace("{type}", &p.vehicle_type)
                        .replace("{count}", &p.rule_count.to_string())
                        .replace("{time}", &updated_at),
                )
                .subtitle_lines(0)
                .build();

            let update_btn = Button::builder()
                .label(tr("btn_update"))
                .css_classes(["flat"])
                .valign(gtk4::Align::Center)
                .build();

            let service = service.clone();
            let provider_name = p.name.clone();
            let btn_clone = update_btn.clone();
            update_btn.connect_clicked(move |_| {
                btn_clone.set_sensitive(false);
                btn_clone.set_label(tr("sub_refreshing"));
                let service = service.clone();
                let name = provider_name.clone();
                let btn = btn_clone.clone();
                glib::MainContext::default().spawn_local(async move {
                    let _ = service.update_rule_provider(&name).await;
                    btn.set_label(tr("btn_update"));
                    btn.set_sensitive(true);
                });
            });

            row.add_suffix(&update_btn);
            list.append(&row);
        }
    }

    pub fn update_ui_text(&self) {
        self.page.set_title(tr("tab_rules"));
        self.status_page.set_title(tr("empty_rules_title"));
        self.status_page.set_description(Some(tr("empty_rules_desc")));
        self.search_entry.set_placeholder_text(Some(tr("rules_search_placeholder")));
        self.refresh_btn.set_tooltip_text(Some(tr("rules_refresh_tooltip")));
        self.providers_group.set_title(tr("rules_providers_title"));
        self.providers_group.set_description(Some(tr("rules_providers_desc")));
        self.rules_group.set_title(tr("rules_list_title"));
        self.load_more_btn.set_label(tr("btn_load_more"));
        Self::render_rules(
            &self.all_rules.borrow(),
            &self.current_filter.borrow(),
            *self.visible_count.borrow(),
            &self.rules_list,
            &self.rules_group,
            &self.status_page,
            &self.load_more_btn,
            &self.row_handles,
            self.is_narrow.get(),
        );
        Self::render_providers(
            &self.all_providers.borrow(),
            &self.providers_group,
            &self.providers_list,
            &self.service,
        );
    }
}
