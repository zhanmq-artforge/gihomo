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
    let window = adw::Window::builder()
        .title(tr("dialog_edit_sub_title"))
        .transient_for(parent)
        .modal(true)
        .default_width(480)
        .default_height(300)
        .build();

    let toolbar_view = adw::ToolbarView::new();
    let header_bar = adw::HeaderBar::new();
    toolbar_view.add_top_bar(&header_bar);

    let content_box = gtk4::Box::new(gtk4::Orientation::Vertical, 16);
    content_box.set_margin_top(20);
    content_box.set_margin_bottom(20);
    content_box.set_margin_start(20);
    content_box.set_margin_end(20);

    let group = adw::PreferencesGroup::new();
    let name_row = adw::EntryRow::builder()
        .title(tr("sub_name_row"))
        .text(&subscription.name)
        .build();
    group.add(&name_row);

    let selected_path = Rc::new(RefCell::new(None::<PathBuf>));
    let url_row = match &subscription.source {
        SubscriptionSource::Url(url) => {
            let row = adw::EntryRow::builder()
                .title(tr("sub_url_row"))
                .text(url)
                .build();
            group.add(&row);
            Some(row)
        }
        SubscriptionSource::LocalFile(path) => {
            let path = PathBuf::from(path);
            let file_row = adw::ActionRow::builder()
                .title(tr("file_chooser_row"))
                .subtitle(path.to_string_lossy())
                .build();
            let browse_button = Button::builder()
                .label(tr("btn_browse"))
                .valign(gtk4::Align::Center)
                .build();
            file_row.add_suffix(&browse_button);
            group.add(&file_row);
            *selected_path.borrow_mut() = Some(path);

            let selected_path = selected_path.clone();
            let file_row = file_row.clone();
            let parent_window = window.clone();
            browse_button.connect_clicked(move |_| {
                let filter = FileFilter::new();
                filter.set_name(Some("YAML (*.yaml, *.yml)"));
                filter.add_pattern("*.yaml");
                filter.add_pattern("*.yml");
                let dialog = FileDialog::builder()
                    .title(tr("file_chooser_row"))
                    .modal(true)
                    .default_filter(&filter)
                    .build();
                let selected_path = selected_path.clone();
                let file_row = file_row.clone();
                dialog.open(
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
            });
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
                .build();
            group.add(&row);
            None
        }
    };

    let error_label = gtk4::Label::builder()
        .xalign(0.0)
        .wrap(true)
        .visible(false)
        .css_classes(["error"])
        .build();
    let footer = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    footer.set_halign(gtk4::Align::End);
    footer.set_margin_top(8);
    footer.set_margin_bottom(12);
    footer.set_margin_start(16);
    footer.set_margin_end(16);

    let cancel_button = Button::builder()
        .label(tr("btn_cancel"))
        .css_classes(["flat"])
        .build();
    let save_button = Button::builder()
        .label(tr("btn_save_sub"))
        .css_classes(["suggested-action"])
        .build();
    footer.append(&cancel_button);
    footer.append(&save_button);

    content_box.append(&group);
    content_box.append(&error_label);
    toolbar_view.set_content(Some(&content_box));
    toolbar_view.add_bottom_bar(&footer);
    window.set_content(Some(&toolbar_view));

    let window_to_close = window.clone();
    cancel_button.connect_clicked(move |_| window_to_close.close());

    let service = service.clone();
    let window_to_close = window.clone();
    let name_row = name_row.clone();
    let error_label = error_label.clone();
    let save_button_for_handler = save_button.clone();
    save_button.connect_clicked(move |_| {
        let name = name_row.text().trim().to_string();
        if name.is_empty() {
            error_label.set_label(tr("sub_name_required"));
            error_label.set_visible(true);
            return;
        }

        let source = if let Some(url_row) = &url_row {
            let url = url_row.text().trim().to_string();
            if url.is_empty() {
                error_label.set_label(tr("sub_url_required"));
                error_label.set_visible(true);
                return;
            }
            SubscriptionSource::Url(url)
        } else if let Some(path) = selected_path.borrow().clone() {
            SubscriptionSource::LocalFile(path.to_string_lossy().to_string())
        } else if let SubscriptionSource::ShareLinks(raw) = &subscription.source {
            SubscriptionSource::ShareLinks(raw.clone())
        } else {
            error_label.set_label(tr("file_chooser_none"));
            error_label.set_visible(true);
            return;
        };

        error_label.set_visible(false);
        save_button_for_handler.set_sensitive(false);
        save_button_for_handler.set_label(tr("sub_saving"));
        let service = service.clone();
        let window = window_to_close.clone();
        let name = name.clone();
        let save_button = save_button_for_handler.clone();
        let error_label = error_label.clone();
        let id = subscription.id.clone();
        glib::MainContext::default().spawn_local(async move {
            match service.edit_subscription(&id, name, source).await {
                Ok(()) => window.close(),
                Err(error) => {
                    error_label.set_label(&error.to_string());
                    error_label.set_visible(true);
                    save_button.set_label(tr("btn_save_sub"));
                    save_button.set_sensitive(true);
                }
            }
        });
    });

    window.present();
}
