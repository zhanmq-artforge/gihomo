use adw::prelude::*;
use gihomo_app::AppService;
use gihomo_core::{Subscription, SubscriptionSource};
use gtk4::{Button, FileDialog, FileFilter};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use crate::i18n::tr;

pub fn show_edit_subscription_dialog(
    parent: &gtk4::Window,
    service: AppService,
    subscription: Subscription,
) {
    let dialog = adw::Dialog::builder()
        .title(tr("dialog_edit_sub_title"))
        .content_width(360)
        .build();

    let header_bar = adw::HeaderBar::new();
    header_bar.set_show_end_title_buttons(false);
    header_bar.set_show_start_title_buttons(false);

    // Cancel Button (Start)
    let cancel_btn = Button::builder()
        .label(tr("btn_cancel"))
        .css_classes(["flat"])
        .build();
    {
        let dialog_for_cancel = dialog.clone();
        cancel_btn.connect_clicked(move |_| {
            dialog_for_cancel.close();
        });
    }
    header_bar.pack_start(&cancel_btn);

    // Save Button (End, Suggested Action)
    let save_btn = Button::builder()
        .label(tr("btn_save_sub"))
        .css_classes(["suggested-action"])
        .build();
    header_bar.pack_end(&save_btn);

    let toolbar_view = adw::ToolbarView::new();
    toolbar_view.add_top_bar(&header_bar);

    let scrolled = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .propagate_natural_height(true)
        .build();

    let content_box = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
    content_box.set_margin_top(12);
    content_box.set_margin_bottom(16);
    content_box.set_margin_start(12);
    content_box.set_margin_end(12);

    let group = adw::PreferencesGroup::new();
    let name_row = adw::EntryRow::builder()
        .title(tr("sub_name_row"))
        .text(&subscription.name)
        .show_apply_button(false)
        .build();
    group.add(&name_row);

    let selected_path = Rc::new(RefCell::new(None::<PathBuf>));
    let url_row = match &subscription.source {
        SubscriptionSource::Url(url) => {
            let row = adw::EntryRow::builder()
                .title(tr("sub_url_row"))
                .text(url)
                .show_apply_button(false)
                .build();
            group.add(&row);
            Some(row)
        }
        SubscriptionSource::LocalFile(path) => {
            let path = PathBuf::from(path);
            let file_row = adw::ActionRow::builder()
                .title(tr("file_chooser_row"))
                .subtitle(path.to_string_lossy())
                .title_lines(1)
                .subtitle_lines(1)
                .activatable(true)
                .build();
            let browse_button = Button::builder()
                .icon_name("document-open-symbolic")
                .valign(gtk4::Align::Center)
                .css_classes(["flat"])
                .build();
            file_row.add_suffix(&browse_button);
            group.add(&file_row);
            *selected_path.borrow_mut() = Some(path);

            let open_picker = {
                let selected_path = selected_path.clone();
                let file_row = file_row.clone();
                let parent_window = parent.clone();
                Rc::new(move || {
                    let filter = FileFilter::new();
                    filter.set_name(Some("YAML (*.yaml, *.yml)"));
                    filter.add_pattern("*.yaml");
                    filter.add_pattern("*.yml");
                    let file_dialog = FileDialog::builder()
                        .title(tr("file_chooser_row"))
                        .modal(true)
                        .default_filter(&filter)
                        .build();
                    let selected_path = selected_path.clone();
                    let file_row = file_row.clone();
                    file_dialog.open(
                        Some(&parent_window),
                        gio::Cancellable::NONE,
                        move |result| {
                            if let Ok(file) = result {
                                if let Some(path) = file.path() {
                                    file_row.set_subtitle(&path.to_string_lossy());
                                    *selected_path.borrow_mut() = Some(path);
                                }
                            }
                        },
                    );
                })
            };

            let picker_clone = open_picker.clone();
            browse_button.connect_clicked(move |_| picker_clone());
            file_row.connect_activated(move |_| open_picker());
            None
        }
        SubscriptionSource::ShareLinks(raw) => {
            let count = raw
                .lines()
                .filter(|l| !l.trim().is_empty() && !l.trim().starts_with('#'))
                .count();
            let row = adw::ActionRow::builder()
                .title(tr("tab_share_links"))
                .subtitle(format!("{} {}", count, tr("sub_nodes_count")))
                .title_lines(1)
                .subtitle_lines(1)
                .build();
            group.add(&row);
            None
        }
    };

    let error_label = gtk4::Label::builder()
        .xalign(0.0)
        .wrap(true)
        .visible(false)
        .css_classes(["error", "caption"])
        .build();

    content_box.append(&group);
    content_box.append(&error_label);
    scrolled.set_child(Some(&content_box));
    toolbar_view.set_content(Some(&scrolled));
    dialog.set_child(Some(&toolbar_view));

    // Save action
    {
        let service = service;
        let dialog = dialog.clone();
        let name_row = name_row.clone();
        let error_label = error_label.clone();
        let subscription_id = subscription.id.clone();
        let subscription_source = subscription.source.clone();
        let selected_path = selected_path.clone();
        let url_row = url_row.clone();

        save_btn.connect_clicked(move |btn| {
            let name = name_row.text().trim().to_string();
            if name.is_empty() {
                error_label.set_label(tr("sub_name_required"));
                error_label.set_visible(true);
                return;
            }

            let source = if let Some(ref url_row) = url_row {
                let url = url_row.text().trim().to_string();
                if url.is_empty() {
                    error_label.set_label(tr("sub_url_required"));
                    error_label.set_visible(true);
                    return;
                }
                SubscriptionSource::Url(url)
            } else if let Some(path) = selected_path.borrow().clone() {
                SubscriptionSource::LocalFile(path.to_string_lossy().to_string())
            } else if let SubscriptionSource::ShareLinks(ref raw) = subscription_source {
                SubscriptionSource::ShareLinks(raw.clone())
            } else {
                error_label.set_label(tr("file_chooser_none"));
                error_label.set_visible(true);
                return;
            };

            error_label.set_visible(false);
            btn.set_sensitive(false);
            btn.set_label(tr("sub_saving"));
            let service = service.clone();
            let dialog = dialog.clone();
            let btn = btn.clone();
            let error_label = error_label.clone();
            let id = subscription_id.clone();

            glib::MainContext::default().spawn_local(async move {
                match service.edit_subscription(&id, name, source).await {
                    Ok(()) => {
                        dialog.close();
                    }
                    Err(error) => {
                        error_label.set_label(&error.to_string());
                        error_label.set_visible(true);
                        btn.set_label(tr("btn_save_sub"));
                        btn.set_sensitive(true);
                    }
                }
            });
        });
    }

    // Enter key activation
    {
        let save_btn = save_btn.clone();
        name_row.connect_entry_activated(move |_| {
            save_btn.emit_clicked();
        });
    }
    if let Some(ref url_row) = url_row {
        let save_btn = save_btn.clone();
        url_row.connect_entry_activated(move |_| {
            save_btn.emit_clicked();
        });
    }

    dialog.present(Some(parent));
}
