use adw::prelude::*;
use gihomo_app::AppService;
use gtk4::{Button, FileDialog, FileFilter};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use tracing::error;

use crate::i18n::tr;

pub fn show_add_subscription_dialog(parent: &gtk4::Window, service: AppService) {
    let window = adw::Window::builder()
        .title(tr("dialog_add_sub_title"))
        .transient_for(parent)
        .modal(true)
        .default_width(520)
        .default_height(580)
        .build();

    let toolbar_view = adw::ToolbarView::new();
    let header_bar = adw::HeaderBar::new();
    toolbar_view.add_top_bar(&header_bar);

    let clamp = adw::Clamp::builder().maximum_size(480).build();
    let scrolled = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .build();

    let content_box = gtk4::Box::new(gtk4::Orientation::Vertical, 16);
    content_box.set_margin_top(16);
    content_box.set_margin_bottom(16);
    content_box.set_margin_start(16);
    content_box.set_margin_end(16);

    let view_stack = adw::ViewStack::new();
    let view_switcher = adw::ViewSwitcher::builder()
        .stack(&view_stack)
        .policy(adw::ViewSwitcherPolicy::Wide)
        .build();
    header_bar.set_title_widget(Some(&view_switcher));

    // =========================================================================
    // Page 1: Remote URL (远程订阅)
    // =========================================================================
    let url_group = adw::PreferencesGroup::builder()
        .title(tr("group_remote_title"))
        .description(tr("group_remote_desc"))
        .build();

    let url_name_row = adw::EntryRow::builder().title(tr("sub_name_row")).build();
    let url_link_row = adw::EntryRow::builder().title(tr("sub_url_row")).build();
    url_group.add(&url_name_row);
    url_group.add(&url_link_row);

    let submit_url_btn = Button::builder()
        .label(tr("btn_confirm_pull"))
        .css_classes(["suggested-action", "pill"])
        .margin_top(12)
        .halign(gtk4::Align::Center)
        .build();

    let url_box = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
    url_box.append(&url_group);
    url_box.append(&submit_url_btn);

    view_stack.add_titled_with_icon(
        &url_box,
        Some("url_page"),
        tr("tab_remote_sub"),
        "network-workgroup-symbolic",
    );

    // =========================================================================
    // Page 2: Share Links (节点链接导入)
    // =========================================================================
    let links_group = adw::PreferencesGroup::builder()
        .title(tr("group_share_links_title"))
        .description(tr("group_share_links_desc"))
        .build();

    let links_name_row = adw::EntryRow::builder()
        .title(tr("share_links_name_row"))
        .text(tr("share_links_default_name"))
        .build();
    links_group.add(&links_name_row);

    let paste_header_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    paste_header_box.set_margin_top(8);
    let hint_label = gtk4::Label::builder()
        .label(tr("share_links_paste_hint"))
        .halign(gtk4::Align::Start)
        .hexpand(true)
        .css_classes(["dim-label", "caption"])
        .build();
    let paste_btn = Button::builder()
        .icon_name("edit-paste-symbolic")
        .label(tr("btn_paste_clipboard"))
        .valign(gtk4::Align::Center)
        .css_classes(["flat", "caption"])
        .build();
    paste_header_box.append(&hint_label);
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
        .max_content_height(150)
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .css_classes(["card"])
        .build();

    let preview_group = adw::PreferencesGroup::builder()
        .title(tr("share_links_preview_title"))
        .margin_top(12)
        .build();
    let preview_list = gtk4::ListBox::builder()
        .css_classes(["boxed-list"])
        .selection_mode(gtk4::SelectionMode::None)
        .build();
    let placeholder_row = adw::ActionRow::builder()
        .title(tr("share_links_preview_empty"))
        .build();
    preview_list.append(&placeholder_row);
    preview_group.add(&preview_list);

    let submit_links_btn = Button::builder()
        .label(tr("btn_confirm_import_links"))
        .css_classes(["suggested-action", "pill"])
        .margin_top(12)
        .halign(gtk4::Align::Center)
        .sensitive(false)
        .build();

    let links_box = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
    links_box.append(&links_group);
    links_box.append(&paste_header_box);
    links_box.append(&text_scroll);
    links_box.append(&preview_group);
    links_box.append(&submit_links_btn);

    view_stack.add_titled_with_icon(
        &links_box,
        Some("links_page"),
        tr("tab_share_links"),
        "insert-link-symbolic",
    );

    // =========================================================================
    // Page 3: Local File (本地文件)
    // =========================================================================
    let local_group = adw::PreferencesGroup::builder()
        .title(tr("group_local_title"))
        .description(tr("group_local_desc"))
        .build();

    let local_name_row = adw::EntryRow::builder()
        .title(tr("config_name_row"))
        .build();
    let file_chooser_row = adw::ActionRow::builder()
        .title(tr("file_chooser_row"))
        .subtitle(tr("file_chooser_none"))
        .build();

    let pick_file_btn = Button::builder()
        .label(tr("btn_browse"))
        .valign(gtk4::Align::Center)
        .build();
    file_chooser_row.add_suffix(&pick_file_btn);

    local_group.add(&local_name_row);
    local_group.add(&file_chooser_row);

    let submit_local_btn = Button::builder()
        .label(tr("btn_confirm_import"))
        .css_classes(["suggested-action", "pill"])
        .margin_top(12)
        .halign(gtk4::Align::Center)
        .build();

    let local_box = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
    local_box.append(&local_group);
    local_box.append(&submit_local_btn);

    view_stack.add_titled_with_icon(
        &local_box,
        Some("local_page"),
        tr("tab_local_sub"),
        "document-open-symbolic",
    );

    clamp.set_child(Some(&view_stack));
    content_box.append(&clamp);
    scrolled.set_child(Some(&content_box));
    toolbar_view.set_content(Some(&scrolled));
    window.set_content(Some(&toolbar_view));

    // =========================================================================
    // Logic: Share Links live preview & clipboard handling
    // =========================================================================
    {
        let buffer = text_view.buffer();
        let preview_list = preview_list.clone();
        let preview_group = preview_group.clone();
        let submit_links_btn = submit_links_btn.clone();

        buffer.connect_changed(move |buf| {
            let text = buf.text(&buf.start_iter(), &buf.end_iter(), false).to_string();
            let (nodes, _) = gihomo_core::parse_share_links(&text);

            while let Some(child) = preview_list.first_child() {
                preview_list.remove(&child);
            }

            if nodes.is_empty() {
                let placeholder = adw::ActionRow::builder()
                    .title(tr("share_links_preview_empty"))
                    .build();
                preview_list.append(&placeholder);
                preview_group.set_title(tr("share_links_preview_title"));
                submit_links_btn.set_sensitive(false);
            } else {
                preview_group.set_title(&format!(
                    "{} ({} {})",
                    tr("share_links_preview_title"),
                    nodes.len(),
                    tr("sub_nodes_count")
                ));

                for node in &nodes {
                    let row = adw::ActionRow::builder()
                        .title(&node.name)
                        .subtitle(format!("{}:{}", node.server, node.port))
                        .build();

                    let badge = gtk4::Label::builder()
                        .label(&node.protocol)
                        .css_classes(["pill", "caption", "accent"])
                        .valign(gtk4::Align::Center)
                        .build();
                    row.add_prefix(&badge);
                    preview_list.append(&row);
                }

                submit_links_btn.set_sensitive(true);
            }
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

    // Submit Share Links
    {
        let service = service.clone();
        let window = window.clone();
        let links_name_row = links_name_row.clone();
        let text_view = text_view.clone();

        submit_links_btn.connect_clicked(move |btn| {
            let name = links_name_row.text().trim().to_string();
            let buf = text_view.buffer();
            let text = buf.text(&buf.start_iter(), &buf.end_iter(), false).to_string();

            if name.is_empty() || text.trim().is_empty() {
                return;
            }

            btn.set_sensitive(false);
            btn.set_label(tr("sub_adding"));
            let service = service.clone();
            let window = window.clone();
            let btn = btn.clone();

            glib::MainContext::default().spawn_local(async move {
                match service.add_share_links_subscription(name, text).await {
                    Ok(_) => {
                        window.close();
                    }
                    Err(e) => {
                        error!("Add share links subscription failed: {}", e);
                        btn.set_label(tr("btn_confirm_import_links"));
                        btn.set_sensitive(true);
                    }
                }
            });
        });
    }

    // =========================================================================
    // Logic: File Picker
    // =========================================================================
    let selected_file_path: Rc<RefCell<Option<PathBuf>>> = Rc::new(RefCell::new(None));
    {
        let selected_file_path = selected_file_path.clone();
        let file_chooser_row = file_chooser_row.clone();
        let local_name_row = local_name_row.clone();
        let win_clone = window.clone();

        pick_file_btn.connect_clicked(move |_| {
            let filter = FileFilter::new();
            filter.set_name(Some("YAML (*.yaml, *.yml)"));
            filter.add_pattern("*.yaml");
            filter.add_pattern("*.yml");

            let dialog = FileDialog::builder()
                .title(tr("file_chooser_row"))
                .modal(true)
                .default_filter(&filter)
                .build();

            let selected_file_path = selected_file_path.clone();
            let file_chooser_row = file_chooser_row.clone();
            let local_name_row = local_name_row.clone();

            dialog.open(Some(&win_clone), gio::Cancellable::NONE, move |res| {
                if let Ok(file) = res {
                    if let Some(path) = file.path() {
                        file_chooser_row.set_subtitle(&path.to_string_lossy());
                        if local_name_row.text().is_empty() {
                            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                                local_name_row.set_text(stem);
                            }
                        }
                        *selected_file_path.borrow_mut() = Some(path);
                    }
                }
            });
        });
    }

    // =========================================================================
    // Logic: Submit URL
    // =========================================================================
    {
        let service = service.clone();
        let window = window.clone();
        let url_name_row = url_name_row.clone();
        let url_link_row = url_link_row.clone();

        submit_url_btn.connect_clicked(move |btn| {
            let name = url_name_row.text().trim().to_string();
            let url = url_link_row.text().trim().to_string();

            if name.is_empty() || url.is_empty() {
                return;
            }

            btn.set_sensitive(false);
            btn.set_label(tr("sub_adding"));
            let service = service.clone();
            let window = window.clone();
            let btn = btn.clone();

            glib::MainContext::default().spawn_local(async move {
                match service.add_url_subscription(name, url).await {
                    Ok(_) => {
                        window.close();
                    }
                    Err(e) => {
                        error!("Add URL subscription failed: {}", e);
                        btn.set_label(tr("btn_confirm_pull"));
                        btn.set_sensitive(true);
                    }
                }
            });
        });
    }

    // =========================================================================
    // Logic: Submit Local File
    // =========================================================================
    {
        let service = service;
        let window = window.clone();
        let local_name_row = local_name_row;

        submit_local_btn.connect_clicked(move |btn| {
            let name = local_name_row.text().trim().to_string();
            let path_opt = selected_file_path.borrow().clone();

            let path = match path_opt {
                Some(p) => p,
                None => return,
            };

            if name.is_empty() {
                return;
            }

            btn.set_sensitive(false);
            btn.set_label(tr("sub_adding"));
            let service = service.clone();
            let window = window.clone();
            let btn = btn.clone();

            glib::MainContext::default().spawn_local(async move {
                match service.add_local_subscription(name, &path).await {
                    Ok(_) => {
                        window.close();
                    }
                    Err(e) => {
                        error!("Add local subscription failed: {}", e);
                        btn.set_label(tr("btn_confirm_import"));
                        btn.set_sensitive(true);
                    }
                }
            });
        });
    }

    window.present();
}
