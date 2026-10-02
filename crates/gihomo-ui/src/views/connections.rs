use adw::prelude::*;
use gihomo_app::AppService;
use gihomo_core::{ConnectionItem, ConnectionsSnapshot, SubscriptionUserInfo};
use gtk4::{Button, Label, Orientation, ScrolledWindow, SearchEntry};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::i18n::tr;

const CONN_PAGE_SIZE: usize = 80;
const CONN_LOAD_MORE_STEP: usize = 80;

#[derive(Clone)]
pub struct ConnectionsView {
    pub page: adw::NavigationPage,
    list_box: gtk4::Box,
    status_page: adw::StatusPage,
    search_entry: SearchEntry,
    close_all_btn: Button,
    refresh_btn: Button,
    load_more_btn: Button,
    spinner: gtk4::Spinner,
    service: AppService,
    parent_window: Rc<RefCell<Option<gtk4::Window>>>,
    cached_snapshot: Rc<RefCell<Option<ConnectionsSnapshot>>>,
    is_fetching: Rc<Cell<bool>>,
    search_query: Rc<RefCell<String>>,
    visible_count: Rc<RefCell<usize>>,
}

impl ConnectionsView {
    pub fn new(service: AppService) -> Self {
        let header_bar = adw::HeaderBar::builder()
            .show_title(true)
            .centering_policy(adw::CenteringPolicy::Strict)
            .build();

        let toolbar_view = adw::ToolbarView::new();
        toolbar_view.add_top_bar(&header_bar);

        let clamp = adw::Clamp::builder().maximum_size(860).build();
        let content_box = gtk4::Box::new(Orientation::Vertical, 16);
        content_box.set_margin_top(16);
        content_box.set_margin_bottom(16);
        content_box.set_margin_start(12);
        content_box.set_margin_end(12);

        // 1. Header Toolbar: Search Entry + Action Buttons
        let top_bar = gtk4::Box::new(Orientation::Horizontal, 8);
        top_bar.set_hexpand(true);

        let search_entry = SearchEntry::builder()
            .placeholder_text(tr("conn_search_placeholder"))
            .hexpand(true)
            .build();

        let spinner = gtk4::Spinner::builder().visible(false).build();

        let refresh_btn = Button::builder()
            .icon_name("view-refresh-symbolic")
            .tooltip_text(tr("tooltip_refresh_conn"))
            .valign(gtk4::Align::Center)
            .css_classes(["flat"])
            .build();

        let close_all_btn = Button::builder()
            .icon_name("network-offline-symbolic")
            .tooltip_text(tr("tooltip_close_all_conn"))
            .valign(gtk4::Align::Center)
            .css_classes(["flat", "destructive-action"])
            .sensitive(false)
            .build();

        let actions_box = gtk4::Box::new(Orientation::Horizontal, 4);
        actions_box.set_valign(gtk4::Align::Center);
        actions_box.append(&spinner);
        actions_box.append(&refresh_btn);
        actions_box.append(&close_all_btn);

        top_bar.append(&search_entry);
        top_bar.append(&actions_box);

        // 2. Empty Status Page
        let status_page = adw::StatusPage::builder()
            .icon_name("network-idle-symbolic")
            .title(tr("conn_empty_title"))
            .description(tr("conn_empty_desc"))
            .vexpand(true)
            .build();

        // 3. Connections List Container
        let list_box = gtk4::Box::new(Orientation::Vertical, 12);
        list_box.set_visible(false);

        // 4. Load more button
        let load_more_btn = Button::builder()
            .label(tr("btn_load_more_conn"))
            .css_classes(["suggested-action", "pill"])
            .halign(gtk4::Align::Center)
            .margin_top(12)
            .margin_bottom(12)
            .visible(false)
            .build();

        content_box.append(&top_bar);
        content_box.append(&status_page);
        content_box.append(&list_box);
        content_box.append(&load_more_btn);

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
            .title(tr("tab_connections"))
            .tag("connections")
            .child(&toolbar_view)
            .build();

        let parent_window: Rc<RefCell<Option<gtk4::Window>>> = Rc::new(RefCell::new(None));
        let cached_snapshot: Rc<RefCell<Option<ConnectionsSnapshot>>> = Rc::new(RefCell::new(None));
        let is_fetching = Rc::new(Cell::new(false));
        let search_query = Rc::new(RefCell::new(String::new()));
        let visible_count = Rc::new(RefCell::new(CONN_PAGE_SIZE));

        let view = Self {
            page,
            list_box,
            status_page,
            search_entry,
            close_all_btn,
            refresh_btn,
            load_more_btn,
            spinner,
            service,
            parent_window,
            cached_snapshot,
            is_fetching,
            search_query,
            visible_count,
        };

        view.bind_signals();
        view
    }

    pub fn set_parent_window(&self, window: gtk4::Window) {
        *self.parent_window.borrow_mut() = Some(window);
    }

    fn bind_signals(&self) {
        // Search filter input with 200ms debounce
        {
            let query_ref = self.search_query.clone();
            let visible_count_ref = self.visible_count.clone();
            let view_weak = self.downgrade_handle();
            let debounce_timer = Rc::new(RefCell::new(None::<glib::SourceId>));
            self.search_entry.connect_search_changed(move |entry| {
                if let Some(source_id) = debounce_timer.borrow_mut().take() {
                    source_id.remove();
                }

                let text = entry.text().trim().to_lowercase();
                let query_ref = query_ref.clone();
                let visible_count_ref = visible_count_ref.clone();
                let view_weak = view_weak.clone();
                let debounce_timer_inner = debounce_timer.clone();

                let source_id = glib::timeout_add_local_once(
                    std::time::Duration::from_millis(200),
                    move || {
                        *debounce_timer_inner.borrow_mut() = None;
                        *query_ref.borrow_mut() = text;
                        *visible_count_ref.borrow_mut() = CONN_PAGE_SIZE;
                        if let Some(view) = view_weak.upgrade() {
                            view.re_render_cached();
                        }
                    },
                );
                *debounce_timer.borrow_mut() = Some(source_id);
            });
        }

        // Load more connections button
        {
            let view_weak = self.downgrade_handle();
            let visible_count_ref = self.visible_count.clone();
            self.load_more_btn.connect_clicked(move |_| {
                *visible_count_ref.borrow_mut() += CONN_LOAD_MORE_STEP;
                if let Some(view) = view_weak.upgrade() {
                    view.re_render_cached();
                }
            });
        }

        // Manual refresh button
        {
            let view_weak = self.downgrade_handle();
            self.refresh_btn.connect_clicked(move |_| {
                if let Some(view) = view_weak.upgrade() {
                    view.fetch_connections();
                }
            });
        }

        // Close all connections button
        {
            let view_weak = self.downgrade_handle();
            self.close_all_btn.connect_clicked(move |_| {
                let Some(view) = view_weak.upgrade() else {
                    return;
                };
                let Some(parent) = view.parent_window.borrow().clone() else {
                    return;
                };

                let dialog = adw::AlertDialog::new(
                    Some(tr("dialog_close_all_conn_title")),
                    Some(tr("dialog_close_all_conn_body")),
                );
                dialog.add_response("cancel", tr("btn_cancel"));
                dialog.add_response("close", tr("btn_close_all_confirm"));
                dialog.set_default_response(Some("cancel"));
                dialog.set_response_appearance("close", adw::ResponseAppearance::Destructive);

                let view_for_dialog = view_weak.clone();
                dialog.choose(&parent, None::<&gio::Cancellable>, move |response| {
                    if response.as_str() != "close" {
                        return;
                    }
                    if let Some(v) = view_for_dialog.upgrade() {
                        let service = v.service.clone();
                        let v_clone = view_for_dialog.clone();
                        glib::MainContext::default().spawn_local(async move {
                            if let Err(e) = service.close_all_connections().await {
                                service.emit_event(gihomo_app::AppEvent::ErrorOccurred(format!(
                                    "{}: {}",
                                    tr("toast_close_conn_failed"),
                                    e
                                )));
                            } else {
                                service.emit_event(gihomo_app::AppEvent::Notification(
                                    tr("toast_close_all_conn_success").to_string(),
                                ));
                                if let Some(view) = v_clone.upgrade() {
                                    view.fetch_connections();
                                }
                            }
                        });
                    }
                });
            });
        }
    }

    pub fn fetch_connections(&self) {
        if self.is_fetching.replace(true) {
            return;
        }

        self.spinner.set_visible(true);
        self.spinner.start();
        self.refresh_btn.set_sensitive(false);

        let service = self.service.clone();
        let view_weak = self.downgrade_handle();

        glib::MainContext::default().spawn_local(async move {
            let res = service.get_connections().await;
            if let Some(view) = view_weak.upgrade() {
                view.is_fetching.set(false);
                view.spinner.stop();
                view.spinner.set_visible(false);
                view.refresh_btn.set_sensitive(true);

                match res {
                    Ok(snapshot) => {
                        *view.cached_snapshot.borrow_mut() = Some(snapshot.clone());
                        view.render_snapshot(&snapshot);
                    }
                    Err(err) => {
                        tracing::debug!("Failed to fetch connections: {}", err);
                        // If error occurred (e.g. kernel stopped), clear snapshot
                        *view.cached_snapshot.borrow_mut() = None;
                        view.render_empty();
                    }
                }
            }
        });
    }

    fn re_render_cached(&self) {
        let snapshot_opt = self.cached_snapshot.borrow().clone();
        if let Some(snapshot) = snapshot_opt {
            self.render_snapshot(&snapshot);
        } else {
            self.render_empty();
        }
    }

    fn render_empty(&self) {
        while let Some(child) = self.list_box.first_child() {
            self.list_box.remove(&child);
        }
        self.status_page.set_visible(true);
        self.list_box.set_visible(false);
        self.close_all_btn.set_sensitive(false);
        self.load_more_btn.set_visible(false);
    }

    fn render_snapshot(&self, snapshot: &ConnectionsSnapshot) {
        while let Some(child) = self.list_box.first_child() {
            self.list_box.remove(&child);
        }

        let total_count = snapshot.connections.len();
        let query = self.search_query.borrow().clone();

        let filtered: Vec<&ConnectionItem> = snapshot
            .connections
            .iter()
            .filter(|conn| {
                if query.is_empty() {
                    return true;
                }
                let dest = conn.metadata.destination().to_lowercase();
                let proc = conn.metadata.process_name().to_lowercase();
                let rule = conn.rule.to_lowercase();
                let ip = conn.metadata.destination_ip.to_lowercase();
                let net = conn.metadata.network.to_lowercase();
                dest.contains(&query)
                    || proc.contains(&query)
                    || rule.contains(&query)
                    || ip.contains(&query)
                    || net.contains(&query)
            })
            .collect();

        let up_total_str = SubscriptionUserInfo::format_bytes(snapshot.upload_total);
        let down_total_str = SubscriptionUserInfo::format_bytes(snapshot.download_total);

        if total_count == 0 {
            self.status_page.set_visible(true);
            self.list_box.set_visible(false);
            self.close_all_btn.set_sensitive(false);
            self.load_more_btn.set_visible(false);
            return;
        }

        self.close_all_btn.set_sensitive(true);
        self.status_page.set_visible(false);
        self.list_box.set_visible(true);

        let visible_limit = *self.visible_count.borrow();
        let total_filtered = filtered.len();
        let display_items = if total_filtered > visible_limit {
            &filtered[..visible_limit]
        } else {
            &filtered[..]
        };

        let base_count_str = if query.is_empty() {
            tr("conn_count_badge").replace("{count}", &total_count.to_string())
        } else {
            tr("conn_count_filtered")
                .replace("{filtered}", &total_filtered.to_string())
                .replace("{total}", &total_count.to_string())
        };

        let count_str = if total_filtered > visible_limit {
            format!(
                "{} ({})",
                base_count_str,
                tr("conn_showing_top").replace("{count}", &display_items.len().to_string())
            )
        } else {
            base_count_str
        };

        let desc_text = format!("{} · ↑ {}  ↓ {}", count_str, up_total_str, down_total_str);

        let group = adw::PreferencesGroup::builder()
            .title(tr("conn_list_title"))
            .description(&desc_text)
            .build();

        for conn in display_items {
            let expander = adw::ExpanderRow::builder()
                .title(conn.metadata.destination())
                .title_lines(1)
                .subtitle_lines(1)
                .expanded(false)
                .build();

            let proc_name = conn.metadata.process_name();
            let proc_tag = if !proc_name.is_empty() {
                format!("[{}] ", proc_name)
            } else {
                String::new()
            };
            let chain_str = conn.chains.join(" → ");
            let subtitle = format!(
                "{}[{}] {}: {} | {}: {}",
                proc_tag,
                conn.metadata.network.to_uppercase(),
                tr("conn_rule_label"),
                conn.rule,
                tr("conn_chain_label"),
                if chain_str.is_empty() {
                    "-"
                } else {
                    &chain_str
                }
            );
            expander.set_subtitle(&subtitle);

            // Suffix 1: Traffic rate/total for this connection
            let up_str = SubscriptionUserInfo::format_bytes(conn.upload);
            let down_str = SubscriptionUserInfo::format_bytes(conn.download);
            let traffic_label = Label::builder()
                .label(format!("↑{} ↓{}", up_str, down_str))
                .css_classes(["caption", "dim-label"])
                .valign(gtk4::Align::Center)
                .build();
            expander.add_suffix(&traffic_label);

            // Suffix 2: Close single connection button
            let close_btn = Button::builder()
                .icon_name("window-close-symbolic")
                .tooltip_text(tr("tooltip_close_single_conn"))
                .css_classes(["flat", "destructive-action"])
                .valign(gtk4::Align::Center)
                .build();

            {
                let service = self.service.clone();
                let conn_id = conn.id.clone();
                let view_weak = self.downgrade_handle();
                let close_btn_clone = close_btn.clone();
                close_btn.connect_clicked(move |_| {
                    close_btn_clone.set_sensitive(false);
                    let service = service.clone();
                    let conn_id = conn_id.clone();
                    let view_weak = view_weak.clone();
                    glib::MainContext::default().spawn_local(async move {
                        let _ = service.close_connection(&conn_id).await;
                        if let Some(view) = view_weak.upgrade() {
                            view.fetch_connections();
                        }
                    });
                });
            }
            expander.add_suffix(&close_btn);

            // Sub-row 1: Source Address
            let src_row = adw::ActionRow::builder()
                .title(tr("conn_src_addr"))
                .subtitle(format!(
                    "{}:{}",
                    conn.metadata.source_ip, conn.metadata.source_port
                ))
                .title_lines(1)
                .subtitle_lines(1)
                .build();
            expander.add_row(&src_row);

            // Sub-row 2: Destination IP & Port
            let dst_row = adw::ActionRow::builder()
                .title(tr("conn_dst_addr"))
                .subtitle(format!(
                    "{}:{}",
                    conn.metadata.destination_ip, conn.metadata.destination_port
                ))
                .title_lines(1)
                .subtitle_lines(1)
                .build();
            expander.add_row(&dst_row);

            // Sub-row 3: Full Process Path if available
            if !conn.metadata.process_path.is_empty() {
                let proc_row = adw::ActionRow::builder()
                    .title(tr("conn_proc_path"))
                    .subtitle(&conn.metadata.process_path)
                    .title_lines(1)
                    .subtitle_lines(1)
                    .build();
                expander.add_row(&proc_row);
            }

            // Sub-row 4: Rule details
            let rule_detail = if conn.rule_payload.is_empty() {
                conn.rule.clone()
            } else {
                format!("{} ({})", conn.rule, conn.rule_payload)
            };
            let rule_row = adw::ActionRow::builder()
                .title(tr("conn_rule_detail"))
                .subtitle(rule_detail)
                .title_lines(1)
                .subtitle_lines(1)
                .build();
            expander.add_row(&rule_row);

            // Sub-row 5: Outbound chains
            if !conn.chains.is_empty() {
                let chain_row = adw::ActionRow::builder()
                    .title(tr("conn_chain_detail"))
                    .subtitle(conn.chains.join(" → "))
                    .title_lines(1)
                    .subtitle_lines(1)
                    .build();
                expander.add_row(&chain_row);
            }

            // Sub-row 6: Start time
            if !conn.start.is_empty() {
                let start_row = adw::ActionRow::builder()
                    .title(tr("conn_start_time"))
                    .subtitle(&conn.start)
                    .title_lines(1)
                    .subtitle_lines(1)
                    .build();
                expander.add_row(&start_row);
            }

            group.add(&expander);
        }

        self.list_box.append(&group);

        if total_filtered > visible_limit {
            let remaining = total_filtered - visible_limit;
            self.load_more_btn
                .set_label(&format!("{} ({})", tr("btn_load_more_conn"), remaining));
            self.load_more_btn.set_visible(true);
        } else {
            self.load_more_btn.set_visible(false);
        }
    }

    pub fn update_ui_text(&self) {
        self.page.set_title(tr("tab_connections"));
        self.search_entry
            .set_placeholder_text(Some(tr("conn_search_placeholder")));
        self.refresh_btn
            .set_tooltip_text(Some(tr("tooltip_refresh_conn")));
        self.close_all_btn
            .set_tooltip_text(Some(tr("tooltip_close_all_conn")));
        self.load_more_btn.set_label(tr("btn_load_more_conn"));
        self.status_page.set_title(tr("conn_empty_title"));
        self.status_page
            .set_description(Some(tr("conn_empty_desc")));

        self.re_render_cached();
    }

    fn downgrade_handle(&self) -> Self {
        self.clone()
    }

    fn upgrade(&self) -> Option<Self> {
        Some(self.clone())
    }
}
