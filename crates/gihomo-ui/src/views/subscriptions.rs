use adw::prelude::*;
use gihomo_app::AppService;
use gihomo_core::{Subscription, SubscriptionSource, SubscriptionUserInfo};
use gtk4::{Button, Label, Orientation, ProgressBar, ScrolledWindow};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::i18n::tr;

pub struct SubscriptionsView {
    pub page: adw::NavigationPage,
    list_box: gtk4::Box,
    status_page: adw::StatusPage,
    add_btn: Button,
    add_content: adw::ButtonContent,
    update_all_btn: Button,
    auto_update_group: adw::PreferencesGroup,
    auto_update_row: adw::ComboRow,
    is_updating_interval: Rc<Cell<bool>>,
    service: AppService,
    parent_window: Rc<RefCell<Option<gtk4::Window>>>,
    subs_cache: Rc<RefCell<Vec<Subscription>>>,
    updating_all: Rc<Cell<bool>>,
}

impl SubscriptionsView {
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

        // Top Toolbar: Add Button + Spacer + Bulk Spinner + Update All Button
        let top_bar = gtk4::Box::new(Orientation::Horizontal, 8);

        let add_content = adw::ButtonContent::builder()
            .icon_name("list-add-symbolic")
            .label(tr("btn_add_sub"))
            .build();
        let add_btn = Button::builder()
            .child(&add_content)
            .tooltip_text(tr("btn_add_sub"))
            .valign(gtk4::Align::Center)
            .css_classes(["suggested-action"])
            .build();

        let spacer = gtk4::Box::new(Orientation::Horizontal, 0);
        spacer.set_hexpand(true);

        let bulk_spinner = gtk4::Spinner::builder()
            .valign(gtk4::Align::Center)
            .visible(false)
            .build();

        let update_all_btn = Button::builder()
            .icon_name("view-refresh-symbolic")
            .tooltip_text(tr("tooltip_update_all_subs"))
            .sensitive(false)
            .valign(gtk4::Align::Center)
            .css_classes(["flat"])
            .build();

        top_bar.append(&add_btn);
        top_bar.append(&spacer);
        top_bar.append(&bulk_spinner);
        top_bar.append(&update_all_btn);

        let status_page = adw::StatusPage::builder()
            .icon_name("network-workgroup-symbolic")
            .title(tr("empty_sub_title"))
            .description(tr("empty_sub_desc"))
            .vexpand(true)
            .build();

        // List box for subscriptions
        let list_box = gtk4::Box::new(Orientation::Vertical, 12);
        list_box.set_visible(false);

        content_box.append(&top_bar);
        content_box.append(&status_page);
        content_box.append(&list_box);

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
            .title(tr("tab_subscriptions"))
            .tag("subscriptions")
            .child(&toolbar_view)
            .build();

        let parent_window: Rc<RefCell<Option<gtk4::Window>>> = Rc::new(RefCell::new(None));
        let subs_cache = Rc::new(RefCell::new(Vec::new()));
        let updating_all = Rc::new(Cell::new(false));

        let current_interval = crate::i18n::auto_update_interval_minutes_config();
        let interval_model = build_interval_string_list();
        let auto_update_row = adw::ComboRow::builder()
            .title(tr("sub_auto_update_interval"))
            .model(&interval_model)
            .selected(interval_to_index(current_interval))
            .build();

        let is_updating_interval = Rc::new(Cell::new(false));
        let is_updating_interval_clone = is_updating_interval.clone();
        auto_update_row.connect_selected_notify(move |row| {
            if is_updating_interval_clone.get() {
                return;
            }
            let idx = row.selected() as usize;
            if let Some(&(minutes, _)) = AUTO_UPDATE_INTERVALS.get(idx) {
                crate::i18n::set_auto_update_interval_minutes(minutes);
            }
        });

        let auto_update_group = adw::PreferencesGroup::builder()
            .title(tr("sub_settings_group"))
            .build();
        auto_update_group.add(&auto_update_row);

        // Wire Add Button
        {
            let service = service.clone();
            let parent_window = parent_window.clone();
            add_btn.connect_clicked(move |_| {
                if let Some(parent) = parent_window.borrow().as_ref() {
                    crate::dialogs::show_add_subscription_dialog(parent, service.clone());
                }
            });
        }

        {
            let service = service.clone();
            let update_all_btn = update_all_btn.clone();
            let add_btn = add_btn.clone();
            let bulk_spinner = bulk_spinner.clone();
            let list_box = list_box.clone();
            let updating_all = updating_all.clone();
            update_all_btn.clone().connect_clicked(move |_| {
                if updating_all.replace(true) {
                    return;
                }

                update_all_btn.set_sensitive(false);
                update_all_btn.set_tooltip_text(Some(tr("sub_refreshing")));
                add_btn.set_sensitive(false);
                list_box.set_sensitive(false);
                bulk_spinner.set_visible(true);
                bulk_spinner.start();

                let service = service.clone();
                let update_all_btn = update_all_btn.clone();
                let add_btn = add_btn.clone();
                let bulk_spinner = bulk_spinner.clone();
                let list_box = list_box.clone();
                let updating_all = updating_all.clone();
                glib::MainContext::default().spawn_local(async move {
                    match service.update_all_subscriptions().await {
                        Ok(summary) if summary.failures.is_empty() => {
                            service.emit_event(gihomo_app::AppEvent::Notification(format!(
                                "{} ({}/{})",
                                tr("sub_update_all_done"),
                                summary.updated,
                                summary.total
                            )));
                        }
                        Ok(summary) => {
                            let details = summary.failures.join("; ");
                            service.emit_event(gihomo_app::AppEvent::ErrorOccurred(format!(
                                "{} ({}/{}): {}",
                                tr("sub_update_all_partial"),
                                summary.updated,
                                summary.total,
                                details
                            )));
                        }
                        Err(error) => service.emit_event(gihomo_app::AppEvent::ErrorOccurred(
                            format!("{}: {}", tr("toast_sub_failed"), error),
                        )),
                    }

                    updating_all.set(false);
                    bulk_spinner.stop();
                    bulk_spinner.set_visible(false);
                    update_all_btn.set_tooltip_text(Some(tr("tooltip_update_all_subs")));
                    update_all_btn.set_sensitive(list_box.first_child().is_some());
                    add_btn.set_sensitive(true);
                    list_box.set_sensitive(true);
                });
            });
        }

        Self {
            page,
            list_box,
            status_page,
            add_btn,
            add_content,
            update_all_btn,
            auto_update_group,
            auto_update_row,
            is_updating_interval,
            service,
            parent_window,
            subs_cache,
            updating_all,
        }
    }

    pub fn set_parent_window(&self, window: gtk4::Window) {
        *self.parent_window.borrow_mut() = Some(window);
    }

    pub fn update_ui_text(&self) {
        self.page.set_title(tr("tab_subscriptions"));
        self.add_content.set_label(tr("btn_add_sub"));
        self.add_btn.set_tooltip_text(Some(tr("btn_add_sub")));
        self.status_page.set_title(tr("empty_sub_title"));
        self.status_page.set_description(Some(tr("empty_sub_desc")));
        self.auto_update_group.set_title(tr("sub_settings_group"));
        self.auto_update_row
            .set_title(tr("sub_auto_update_interval"));
        let selected = self.auto_update_row.selected();
        let new_model = build_interval_string_list();
        self.is_updating_interval.set(true);
        self.auto_update_row.set_model(Some(&new_model));
        self.auto_update_row.set_selected(selected);
        self.is_updating_interval.set(false);

        let subs = self.subs_cache.borrow().clone();
        self.render_subscriptions(&subs);
        let tooltip = if self.updating_all.get() {
            tr("sub_refreshing")
        } else {
            tr("tooltip_update_all_subs")
        };
        self.update_all_btn.set_tooltip_text(Some(tooltip));
    }

    pub fn update_subscriptions(&self, subs: &[Subscription]) {
        *self.subs_cache.borrow_mut() = subs.to_vec();
        self.render_subscriptions(subs);
    }

    fn render_subscriptions(&self, subs: &[Subscription]) {
        // Clear existing items in list_box
        while let Some(child) = self.list_box.first_child() {
            self.list_box.remove(&child);
        }
        self.update_all_btn
            .set_sensitive(!subs.is_empty() && !self.updating_all.get());

        if subs.is_empty() {
            self.status_page.set_visible(true);
            self.list_box.set_visible(false);
            return;
        }

        self.status_page.set_visible(false);
        self.list_box.set_visible(true);

        self.list_box.append(&self.auto_update_group);

        let group = adw::PreferencesGroup::builder()
            .title(tr("sub_list_title"))
            .description(tr("sub_list_desc"))
            .build();

        for sub in subs {
            let expander = adw::ExpanderRow::builder()
                .title(&sub.name)
                .title_lines(1)
                .subtitle_lines(1)
                .expanded(sub.is_active)
                .build();

            let updated_time_str = sub
                .updated_at
                .map(|t| {
                    t.with_timezone(&chrono::Local)
                        .format("%Y-%m-%d %H:%M:%S")
                        .to_string()
                })
                .unwrap_or_else(|| tr("sub_never_updated").to_string());
            expander.set_subtitle(&updated_time_str);
            if sub.updated_at.is_some() {
                expander.set_tooltip_text(Some(&format!(
                    "{}: {}",
                    tr("sub_updated_time"),
                    updated_time_str
                )));
            } else {
                expander.set_tooltip_text(Some(tr("sub_never_updated")));
            }

            let sub_spinner = gtk4::Spinner::builder()
                .valign(gtk4::Align::Center)
                .visible(false)
                .build();

            let actions_box = gtk4::Box::new(Orientation::Horizontal, 4);
            actions_box.set_valign(gtk4::Align::Center);

            // 1. Status Indicator / Activate Button
            if sub.is_active {
                let active_badge = Label::builder()
                    .label(tr("badge_in_use"))
                    .css_classes(["pill", "caption", "success"])
                    .valign(gtk4::Align::Center)
                    .build();
                actions_box.append(&active_badge);
            } else {
                let select_btn = Button::builder()
                    .icon_name("emblem-ok-symbolic")
                    .tooltip_text(tr("btn_set_active"))
                    .valign(gtk4::Align::Center)
                    .css_classes(["flat"])
                    .build();

                let service = self.service.clone();
                let sub_id = sub.id.clone();
                let select_btn_clone = select_btn.clone();
                let spinner = sub_spinner.clone();
                select_btn.connect_clicked(move |_| {
                    select_btn_clone.set_sensitive(false);
                    spinner.set_visible(true);
                    spinner.start();

                    let service = service.clone();
                    let sub_id = sub_id.clone();
                    let select_btn = select_btn_clone.clone();
                    let spinner = spinner.clone();
                    glib::MainContext::default().spawn_local(async move {
                        if let Err(error) = service.activate_subscription(&sub_id).await {
                            service.emit_event(gihomo_app::AppEvent::ErrorOccurred(format!(
                                "{}: {}",
                                tr("sub_activate_failed"),
                                error
                            )));
                        }
                        spinner.stop();
                        spinner.set_visible(false);
                        select_btn.set_sensitive(true);
                    });
                });
                actions_box.append(&select_btn);
            }

            // 2. Action: Refresh Button
            let refresh_btn = Button::builder()
                .icon_name("view-refresh-symbolic")
                .tooltip_text(tr("tooltip_refresh_sub"))
                .valign(gtk4::Align::Center)
                .css_classes(["flat"])
                .build();

            {
                let service = self.service.clone();
                let sub_id = sub.id.clone();
                let refresh_btn_clone = refresh_btn.clone();
                let spinner = sub_spinner.clone();
                refresh_btn.connect_clicked(move |_| {
                    refresh_btn_clone.set_sensitive(false);
                    spinner.set_visible(true);
                    spinner.start();

                    let service = service.clone();
                    let sub_id = sub_id.clone();
                    let refresh_btn = refresh_btn_clone.clone();
                    let spinner = spinner.clone();
                    glib::MainContext::default().spawn_local(async move {
                        if let Err(error) = service.update_subscription(&sub_id).await {
                            service.emit_event(gihomo_app::AppEvent::ErrorOccurred(format!(
                                "{}: {}",
                                tr("toast_sub_failed"),
                                error
                            )));
                        }
                        spinner.stop();
                        spinner.set_visible(false);
                        refresh_btn.set_sensitive(true);
                    });
                });
            }
            actions_box.append(&refresh_btn);

            // 3. Action: Edit Button
            let edit_btn = Button::builder()
                .icon_name("document-edit-symbolic")
                .tooltip_text(tr("tooltip_edit_sub"))
                .valign(gtk4::Align::Center)
                .css_classes(["flat"])
                .build();
            {
                let service = self.service.clone();
                let parent_window = self.parent_window.clone();
                let subscription = sub.clone();
                edit_btn.connect_clicked(move |_| {
                    if let Some(parent) = parent_window.borrow().as_ref() {
                        crate::dialogs::show_edit_subscription_dialog(
                            parent,
                            service.clone(),
                            subscription.clone(),
                        );
                    }
                });
            }
            actions_box.append(&edit_btn);

            // 4. Action: Delete Button (Destructive action at the end)
            let delete_btn =
                build_delete_button(self.service.clone(), self.parent_window.clone(), sub);
            actions_box.append(&delete_btn);

            // 5. Spinner for async progress
            actions_box.append(&sub_spinner);

            expander.add_suffix(&actions_box);

            // Sub-row 1: Subscription Address / Source (above traffic info)
            let (source_title, source_val, is_copyable) = match &sub.source {
                SubscriptionSource::Url(url) => (tr("sub_address_title"), url.clone(), true),
                SubscriptionSource::LocalFile(path) => {
                    (tr("sub_local_file_title"), path.clone(), true)
                }
                SubscriptionSource::ShareLinks(raw) => {
                    let count = raw
                        .lines()
                        .filter(|l| !l.trim().is_empty() && !l.trim().starts_with('#'))
                        .count();
                    (
                        tr("tab_share_links"),
                        format!("{} {}", count, tr("sub_nodes_count")),
                        false,
                    )
                }
            };

            let address_row = adw::ActionRow::builder()
                .title(source_title)
                .subtitle(&source_val)
                .title_lines(1)
                .subtitle_lines(1)
                .tooltip_text(&source_val)
                .build();

            if is_copyable {
                let copy_btn = Button::builder()
                    .icon_name("edit-copy-symbolic")
                    .tooltip_text(tr("tooltip_copy_address"))
                    .valign(gtk4::Align::Center)
                    .css_classes(["flat"])
                    .build();

                let val_to_copy = source_val.clone();
                let btn_clone = copy_btn.clone();
                copy_btn.connect_clicked(move |_| {
                    if let Some(display) = gdk4::Display::default() {
                        let clipboard = display.clipboard();
                        clipboard.set_text(&val_to_copy);
                        btn_clone.set_icon_name("object-select-symbolic");
                        let b = btn_clone.clone();
                        glib::timeout_add_local_once(
                            std::time::Duration::from_millis(1500),
                            move || {
                                b.set_icon_name("edit-copy-symbolic");
                            },
                        );
                    }
                });
                address_row.add_suffix(&copy_btn);
            }
            expander.add_row(&address_row);

            // Sub-row 2: Traffic Info if available
            if let Some(info) = &sub.user_info {
                let used_str = SubscriptionUserInfo::format_bytes(info.upload + info.download);
                let total_str = SubscriptionUserInfo::format_bytes(info.total);
                let percentage = (info.usage_percentage() * 100.0) as u32;

                let traffic_row = adw::ActionRow::builder()
                    .title(tr("package_traffic_title"))
                    .subtitle(format!("{} / {} ({}%)", used_str, total_str, percentage))
                    .title_lines(1)
                    .subtitle_lines(1)
                    .build();

                let pbar = ProgressBar::builder()
                    .fraction(info.usage_percentage())
                    .valign(gtk4::Align::Center)
                    .hexpand(true)
                    .build();
                traffic_row.add_suffix(&pbar);
                expander.add_row(&traffic_row);
            }

            group.add(&expander);
        }

        self.list_box.append(&group);
    }
}

fn build_delete_button(
    service: AppService,
    parent_window: Rc<RefCell<Option<gtk4::Window>>>,
    subscription: &Subscription,
) -> Button {
    let button = Button::builder()
        .icon_name("user-trash-symbolic")
        .tooltip_text(tr("tooltip_delete_sub"))
        .valign(gtk4::Align::Center)
        .css_classes(["flat", "destructive-action"])
        .build();
    let subscription_id = subscription.id.clone();
    let subscription_name = subscription.name.clone();

    button.connect_clicked(move |_| {
        let Some(parent) = parent_window.borrow().clone() else {
            return;
        };
        let body = tr("dialog_delete_sub_body").replace("{name}", &subscription_name);
        let dialog = adw::AlertDialog::new(Some(tr("dialog_delete_sub_title")), Some(&body));
        dialog.add_response("cancel", tr("btn_cancel"));
        dialog.add_response("delete", tr("btn_delete_sub"));
        dialog.set_default_response(Some("cancel"));
        dialog.set_response_appearance("delete", adw::ResponseAppearance::Destructive);

        let service = service.clone();
        let subscription_id = subscription_id.clone();
        let subscription_name = subscription_name.clone();
        dialog.choose(&parent, None::<&gio::Cancellable>, move |response| {
            if response.as_str() != "delete" {
                return;
            }
            let service = service.clone();
            let subscription_id = subscription_id.clone();
            let subscription_name = subscription_name.clone();
            glib::MainContext::default().spawn_local(async move {
                if let Err(error) = service.delete_subscription(&subscription_id).await {
                    service.emit_event(gihomo_app::AppEvent::ErrorOccurred(format!(
                        "{} [{}]: {}",
                        tr("toast_sub_delete_failed"),
                        subscription_name,
                        error
                    )));
                }
            });
        });
    });

    button
}

const AUTO_UPDATE_INTERVALS: &[(u32, &str)] = &[
    (0, "sub_interval_never"),
    (30, "sub_interval_30m"),
    (60, "sub_interval_1h"),
    (120, "sub_interval_2h"),
    (360, "sub_interval_6h"),
    (720, "sub_interval_12h"),
    (1440, "sub_interval_24h"),
];

fn build_interval_string_list() -> gtk4::StringList {
    let list: Vec<&str> = AUTO_UPDATE_INTERVALS
        .iter()
        .map(|(_, key)| tr(key))
        .collect();
    gtk4::StringList::new(&list)
}

fn interval_to_index(minutes: u32) -> u32 {
    AUTO_UPDATE_INTERVALS
        .iter()
        .position(|(m, _)| *m == minutes)
        .map(|idx| idx as u32)
        .unwrap_or_else(|| {
            AUTO_UPDATE_INTERVALS
                .iter()
                .enumerate()
                .min_by_key(|(_, (m, _))| (*m as i64 - minutes as i64).abs())
                .map(|(idx, _)| idx as u32)
                .unwrap_or(0)
        })
}
