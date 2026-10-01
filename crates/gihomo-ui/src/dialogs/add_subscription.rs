use adw::prelude::*;
use gihomo_app::AppService;
use gtk4::{Button, FileDialog, FileFilter};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use tracing::error;

use crate::i18n::tr;

pub fn show_add_subscription_dialog(parent: &gtk4::Window, service: AppService) {
    let dialog = adw::Dialog::builder()
        .title(tr("dialog_add_sub_title"))
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

    // Add Button (End, Primary Action)
    let add_btn = Button::builder()
        .label(tr("btn_add"))
        .css_classes(["suggested-action"])
        .sensitive(false)
        .build();
    header_bar.pack_end(&add_btn);

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

    let view_stack = adw::ViewStack::new();
    let view_switcher = adw::ViewSwitcher::builder()
        .stack(&view_stack)
        .policy(adw::ViewSwitcherPolicy::Narrow)
        .halign(gtk4::Align::Center)
        .build();

    // -------------------------------------------------------------------------
    // Page 1: Remote URL (远程订阅)
    // -------------------------------------------------------------------------
    let url_group = adw::PreferencesGroup::new();

    let url_name_row = adw::EntryRow::builder()
        .title(tr("sub_name_row"))
        .show_apply_button(false)
        .build();

    let url_link_row = adw::EntryRow::builder()
        .title(tr("sub_url_row"))
        .show_apply_button(false)
        .build();

    url_group.add(&url_name_row);
    url_group.add(&url_link_row);

    let url_box = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    url_box.append(&url_group);

    view_stack.add_titled_with_icon(
        &url_box,
        Some("url_page"),
        tr("tab_remote_sub"),
        "network-workgroup-symbolic",
    );

    // -------------------------------------------------------------------------
    // Page 2: Share Links (节点链接导入)
    // -------------------------------------------------------------------------
    let links_group = adw::PreferencesGroup::new();

    let links_name_row = adw::EntryRow::builder()
        .title(tr("share_links_name_row"))
        .text(tr("share_links_default_name"))
        .show_apply_button(false)
        .build();
    links_group.add(&links_name_row);

    let paste_header_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    paste_header_box.set_margin_top(4);

    let status_badge = gtk4::Label::builder()
        .label(tr("share_links_preview_empty"))
        .halign(gtk4::Align::Start)
        .hexpand(true)
        .ellipsize(gtk4::pango::EllipsizeMode::End)
        .lines(1)
        .css_classes(["dim-label", "caption"])
        .build();

    let paste_btn = Button::builder()
        .icon_name("edit-paste-symbolic")
        .tooltip_text(tr("btn_paste_clipboard"))
        .valign(gtk4::Align::Center)
        .css_classes(["flat"])
        .build();

    paste_header_box.append(&status_badge);
    paste_header_box.append(&paste_btn);

    let text_view = gtk4::TextView::builder()
        .wrap_mode(gtk4::WrapMode::Char)
        .monospace(true)
        .top_margin(8)
        .bottom_margin(8)
        .left_margin(8)
        .right_margin(8)
        .build();

    let text_scroll = gtk4::ScrolledWindow::builder()
        .child(&text_view)
        .min_content_height(100)
        .max_content_height(140)
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .css_classes(["card"])
        .build();

    let links_box = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
    links_box.append(&links_group);
    links_box.append(&paste_header_box);
    links_box.append(&text_scroll);

    view_stack.add_titled_with_icon(
        &links_box,
        Some("links_page"),
        tr("tab_share_links"),
        "insert-link-symbolic",
    );

    // -------------------------------------------------------------------------
    // Page 3: Local File (本地文件)
    // -------------------------------------------------------------------------
    let local_group = adw::PreferencesGroup::new();

    let local_name_row = adw::EntryRow::builder()
        .title(tr("config_name_row"))
        .show_apply_button(false)
        .build();

    let file_chooser_row = adw::ActionRow::builder()
        .title(tr("file_chooser_row"))
        .subtitle(tr("file_chooser_none"))
        .title_lines(1)
        .subtitle_lines(1)
        .activatable(true)
        .build();

    let pick_file_btn = Button::builder()
        .icon_name("document-open-symbolic")
        .valign(gtk4::Align::Center)
        .css_classes(["flat"])
        .build();
    file_chooser_row.add_suffix(&pick_file_btn);

    local_group.add(&local_name_row);
    local_group.add(&file_chooser_row);

    let local_box = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    local_box.append(&local_group);

    view_stack.add_titled_with_icon(
        &local_box,
        Some("local_page"),
        tr("tab_local_sub"),
        "document-open-symbolic",
    );

    content_box.append(&view_switcher);
    content_box.append(&view_stack);
    scrolled.set_child(Some(&content_box));
    toolbar_view.set_content(Some(&scrolled));
    dialog.set_child(Some(&toolbar_view));

    // -------------------------------------------------------------------------
    // State management & Sensitivity synchronization
    // -------------------------------------------------------------------------
    let selected_file_path: Rc<RefCell<Option<PathBuf>>> = Rc::new(RefCell::new(None));
    let parsed_nodes_count = Rc::new(RefCell::new(0usize));

    let update_add_btn_state = {
        let view_stack = view_stack.clone();
        let url_link_row = url_link_row.clone();
        let links_name_row = links_name_row.clone();
        let local_name_row = local_name_row.clone();
        let selected_file_path = selected_file_path.clone();
        let parsed_nodes_count = parsed_nodes_count.clone();
        let add_btn = add_btn.clone();

        Rc::new(move || {
            let page = view_stack.visible_child_name().unwrap_or_default();
            let is_valid = match page.as_str() {
                "url_page" => !url_link_row.text().trim().is_empty(),
                "links_page" => {
                    *parsed_nodes_count.borrow() > 0 && !links_name_row.text().trim().is_empty()
                }
                "local_page" => {
                    selected_file_path.borrow().is_some()
                        && !local_name_row.text().trim().is_empty()
                }
                _ => false,
            };
            add_btn.set_sensitive(is_valid);
        })
    };

    // React to view_stack changes
    {
        let update_state = update_add_btn_state.clone();
        view_stack.connect_visible_child_notify(move |_| {
            update_state();
        });
    }

    // React to entry text changes
    {
        let update_state = update_add_btn_state.clone();
        url_link_row.connect_changed(move |_| {
            update_state();
        });
    }
    {
        let update_state = update_add_btn_state.clone();
        links_name_row.connect_changed(move |_| {
            update_state();
        });
    }
    {
        let update_state = update_add_btn_state.clone();
        local_name_row.connect_changed(move |_| {
            update_state();
        });
    }

    // Live parse share links
    {
        let buffer = text_view.buffer();
        let status_badge = status_badge.clone();
        let parsed_nodes_count = parsed_nodes_count.clone();
        let update_state = update_add_btn_state.clone();

        buffer.connect_changed(move |buf| {
            let text = buf
                .text(&buf.start_iter(), &buf.end_iter(), false)
                .to_string();
            let (nodes, _) = gihomo_core::parse_share_links(&text);
            let count = nodes.len();
            *parsed_nodes_count.borrow_mut() = count;

            if count == 0 {
                status_badge.set_label(tr("share_links_preview_empty"));
                status_badge.remove_css_class("accent");
                status_badge.add_css_class("dim-label");
            } else {
                status_badge.set_label(&format!("✓ {} {}", count, tr("sub_nodes_count")));
                status_badge.remove_css_class("dim-label");
                status_badge.add_css_class("accent");
            }
            update_state();
        });
    }

    // Paste button click
    {
        let tv = text_view.clone();
        paste_btn.connect_clicked(move |_| {
            if let Some(display) = gdk4::Display::default() {
                let clipboard = display.clipboard();
                let tv_inner = tv.clone();
                clipboard.read_text_async(gio::Cancellable::NONE, move |res| {
                    if let Ok(Some(text)) = res {
                        let buf = tv_inner.buffer();
                        let current = buf.text(&buf.start_iter(), &buf.end_iter(), false);
                        let s = text.as_str().trim();
                        let new_content = if current.trim().is_empty() {
                            s.to_string()
                        } else {
                            format!("{}\n{}", current.trim_end(), s)
                        };
                        buf.set_text(&new_content);
                    }
                });
            }
        });
    }

    // Auto-detect clipboard on dialog launch
    {
        let tv_init = text_view.clone();
        let view_stack_init = view_stack.clone();
        if let Some(display) = gdk4::Display::default() {
            let clipboard = display.clipboard();
            clipboard.read_text_async(gio::Cancellable::NONE, move |res| {
                if let Ok(Some(text)) = res {
                    let s = text.as_str().trim();
                    if s.contains("ss://")
                        || s.contains("vmess://")
                        || s.contains("vless://")
                        || s.contains("trojan://")
                        || s.contains("hysteria2://")
                        || s.contains("hy2://")
                    {
                        tv_init.buffer().set_text(s);
                        view_stack_init.set_visible_child_name("links_page");
                    }
                }
            });
        }
    }

    // File picker for local file
    {
        let open_picker = {
            let selected_file_path = selected_file_path.clone();
            let file_chooser_row = file_chooser_row.clone();
            let local_name_row = local_name_row.clone();
            let parent_for_picker = parent.clone();
            let update_state = update_add_btn_state.clone();

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

                let selected_file_path = selected_file_path.clone();
                let file_chooser_row = file_chooser_row.clone();
                let local_name_row = local_name_row.clone();
                let update_state = update_state.clone();

                file_dialog.open(
                    Some(&parent_for_picker),
                    gio::Cancellable::NONE,
                    move |res| {
                        if let Ok(file) = res {
                            if let Some(path) = file.path() {
                                file_chooser_row.set_subtitle(&path.to_string_lossy());
                                if local_name_row.text().is_empty() {
                                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                                        local_name_row.set_text(stem);
                                    }
                                }
                                *selected_file_path.borrow_mut() = Some(path);
                                update_state();
                            }
                        }
                    },
                );
            })
        };

        let picker_clone = open_picker.clone();
        pick_file_btn.connect_clicked(move |_| {
            picker_clone();
        });

        file_chooser_row.connect_activated(move |_| {
            open_picker();
        });
    }

    // Unified Submit Action
    {
        let service = service;
        let dialog = dialog.clone();
        let view_stack = view_stack.clone();
        let url_name_row = url_name_row.clone();
        let url_link_row = url_link_row.clone();
        let links_name_row = links_name_row.clone();
        let text_view = text_view.clone();
        let local_name_row = local_name_row.clone();
        let selected_file_path = selected_file_path.clone();

        add_btn.connect_clicked(move |btn| {
            let page = view_stack.visible_child_name().unwrap_or_default();
            btn.set_sensitive(false);
            btn.set_label(tr("sub_adding"));

            let btn_clone = btn.clone();
            let dialog_clone = dialog.clone();
            let service = service.clone();

            match page.as_str() {
                "url_page" => {
                    let mut name = url_name_row.text().trim().to_string();
                    let url = url_link_row.text().trim().to_string();
                    if name.is_empty() {
                        name = url
                            .split("://")
                            .nth(1)
                            .and_then(|s| s.split('/').next())
                            .filter(|s| !s.is_empty())
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| tr("dialog_add_sub_title").to_string());
                    }
                    glib::MainContext::default().spawn_local(async move {
                        match service.add_url_subscription(name, url).await {
                            Ok(_) => {
                                dialog_clone.close();
                            }
                            Err(e) => {
                                error!("Add URL subscription failed: {}", e);
                                btn_clone.set_label(tr("btn_add"));
                                btn_clone.set_sensitive(true);
                            }
                        }
                    });
                }
                "links_page" => {
                    let name = links_name_row.text().trim().to_string();
                    let buf = text_view.buffer();
                    let text = buf
                        .text(&buf.start_iter(), &buf.end_iter(), false)
                        .to_string();
                    glib::MainContext::default().spawn_local(async move {
                        match service.add_share_links_subscription(name, text).await {
                            Ok(_) => {
                                dialog_clone.close();
                            }
                            Err(e) => {
                                error!("Add share links subscription failed: {}", e);
                                btn_clone.set_label(tr("btn_add"));
                                btn_clone.set_sensitive(true);
                            }
                        }
                    });
                }
                "local_page" => {
                    let name = local_name_row.text().trim().to_string();
                    let path_opt = selected_file_path.borrow().clone();
                    let Some(path) = path_opt else {
                        btn_clone.set_label(tr("btn_add"));
                        btn_clone.set_sensitive(true);
                        return;
                    };
                    glib::MainContext::default().spawn_local(async move {
                        match service.add_local_subscription(name, &path).await {
                            Ok(_) => {
                                dialog_clone.close();
                            }
                            Err(e) => {
                                error!("Add local subscription failed: {}", e);
                                btn_clone.set_label(tr("btn_add"));
                                btn_clone.set_sensitive(true);
                            }
                        }
                    });
                }
                _ => {
                    btn_clone.set_label(tr("btn_add"));
                    btn_clone.set_sensitive(true);
                }
            }
        });
    }

    // Connect EntryRow Enter key activation
    {
        let add_btn = add_btn.clone();
        url_link_row.connect_entry_activated(move |_| {
            if add_btn.is_sensitive() {
                add_btn.emit_clicked();
            }
        });
    }
    {
        let add_btn = add_btn.clone();
        url_name_row.connect_entry_activated(move |_| {
            if add_btn.is_sensitive() {
                add_btn.emit_clicked();
            }
        });
    }
    {
        let add_btn = add_btn.clone();
        links_name_row.connect_entry_activated(move |_| {
            if add_btn.is_sensitive() {
                add_btn.emit_clicked();
            }
        });
    }
    {
        let add_btn = add_btn.clone();
        local_name_row.connect_entry_activated(move |_| {
            if add_btn.is_sensitive() {
                add_btn.emit_clicked();
            }
        });
    }

    dialog.present(Some(parent));
}
