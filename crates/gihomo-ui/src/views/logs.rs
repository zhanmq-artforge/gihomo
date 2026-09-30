use adw::prelude::*;
use gihomo_app::AppService;
use gihomo_core::LogMessage;
use gtk4::{
    Button, DropDown, Orientation, ScrolledWindow, SearchEntry, TextBuffer, TextMark,
    TextView, ToggleButton,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::i18n::tr;

pub struct LogsView {
    pub page: adw::NavigationPage,
    text_view: TextView,
    buffer: TextBuffer,
    search_entry: SearchEntry,
    level_dropdown: DropDown,
    auto_scroll_btn: ToggleButton,
    copy_btn: Button,
    popover: gtk4::PopoverMenu,
    service: AppService,
    all_logs: Rc<RefCell<Vec<LogMessage>>>,
    auto_scroll_enabled: Rc<Cell<bool>>,
    search_filter: Rc<RefCell<String>>,
    level_filter: Rc<Cell<u32>>, // 0: All, 1: Info, 2: Warning, 3: Error, 4: Debug
    is_updating_dropdown: Rc<Cell<bool>>,
    scroll_mark: TextMark,
}

impl LogsView {
    pub fn new(service: AppService) -> Self {
        let header_bar = adw::HeaderBar::builder()
            .show_title(true)
            .centering_policy(adw::CenteringPolicy::Strict)
            .build();

        // Auto-scroll toggle
        let auto_scroll_btn = ToggleButton::builder()
            .icon_name("go-down-symbolic")
            .tooltip_text(tr("tooltip_auto_scroll"))
            .active(true)
            .valign(gtk4::Align::Center)
            .css_classes(["flat"])
            .build();

        // Copy button
        let copy_btn = Button::builder()
            .icon_name("edit-copy-symbolic")
            .tooltip_text(tr("tooltip_copy_logs"))
            .valign(gtk4::Align::Center)
            .css_classes(["flat"])
            .build();

        let toolbar_view = adw::ToolbarView::new();
        toolbar_view.add_top_bar(&header_bar);

        // 1. Top Controls Bar
        let top_bar = gtk4::Box::new(Orientation::Horizontal, 8);
        top_bar.set_margin_top(8);
        top_bar.set_margin_bottom(8);
        top_bar.set_margin_start(12);
        top_bar.set_margin_end(12);

        // Log Level DropDown
        let levels = [
            tr("log_level_all"),
            tr("log_level_info"),
            tr("log_level_warn"),
            tr("log_level_error"),
            tr("log_level_debug"),
        ];
        let level_dropdown = DropDown::from_strings(&levels);
        level_dropdown.set_selected(0);
        level_dropdown.set_valign(gtk4::Align::Center);

        // Search filter
        let search_entry = SearchEntry::builder()
            .placeholder_text(tr("log_search_placeholder"))
            .valign(gtk4::Align::Center)
            .hexpand(true)
            .build();

        top_bar.append(&level_dropdown);
        top_bar.append(&search_entry);
        top_bar.append(&auto_scroll_btn);
        top_bar.append(&copy_btn);

        // 2. Console Text View & Buffer
        let buffer = TextBuffer::new(None);

        // Define Color Tags
        let tag_table = buffer.tag_table();

        let tag_info = gtk4::TextTag::builder()
            .name("info")
            .foreground("#3584e4")
            .weight(700)
            .build();
        let tag_warn = gtk4::TextTag::builder()
            .name("warning")
            .foreground("#e66100")
            .weight(700)
            .build();
        let tag_err = gtk4::TextTag::builder()
            .name("error")
            .foreground("#e01b24")
            .weight(700)
            .build();
        let tag_debug = gtk4::TextTag::builder()
            .name("debug")
            .foreground("#77767b")
            .build();
        let tag_dim = gtk4::TextTag::builder().name("dim").foreground("#9a9996").build();

        tag_table.add(&tag_info);
        tag_table.add(&tag_warn);
        tag_table.add(&tag_err);
        tag_table.add(&tag_debug);
        tag_table.add(&tag_dim);

        // Create end mark for auto-scroll
        let end_iter = buffer.end_iter();
        let scroll_mark = buffer.create_mark(Some("end_mark"), &end_iter, false);

        let text_view = TextView::builder()
            .buffer(&buffer)
            .editable(false)
            .cursor_visible(false)
            .monospace(true)
            .wrap_mode(gtk4::WrapMode::WordChar)
            .top_margin(12)
            .bottom_margin(12)
            .left_margin(16)
            .right_margin(16)
            .css_classes(["card", "terminal-window"])
            .vexpand(true)
            .hexpand(true)
            .build();

        let scrolled = ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Automatic)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .vexpand(true)
            .hexpand(true)
            .child(&text_view)
            .build();

        let content_box = gtk4::Box::new(Orientation::Vertical, 0);
        content_box.append(&top_bar);
        content_box.append(&scrolled);

        toolbar_view.set_content(Some(&content_box));

        let page = adw::NavigationPage::builder()
            .title(tr("tab_logs"))
            .tag("logs")
            .child(&toolbar_view)
            .build();

        let all_logs = Rc::new(RefCell::new(Vec::with_capacity(5000)));
        let auto_scroll_enabled = Rc::new(Cell::new(true));
        let search_filter = Rc::new(RefCell::new(String::new()));
        let level_filter = Rc::new(Cell::new(0));
        let is_updating_dropdown = Rc::new(Cell::new(false));

        let popover = Self::setup_context_menu(&text_view, &buffer, &all_logs);

        let view = Self {
            page,
            text_view,
            buffer,
            search_entry,
            level_dropdown,
            auto_scroll_btn,
            copy_btn,
            popover,
            service,
            all_logs,
            auto_scroll_enabled,
            search_filter,
            level_filter,
            is_updating_dropdown,
            scroll_mark,
        };

        view.bind_signals();
        view.init_log_stream();
        view
    }

    fn bind_signals(&self) {
        // Dropdown selection
        {
            let level_ref = self.level_filter.clone();
            let view_weak = self.downgrade_handle();
            let is_updating = self.is_updating_dropdown.clone();
            self.level_dropdown.connect_selected_notify(move |dd| {
                if is_updating.get() {
                    return;
                }
                level_ref.set(dd.selected());
                if let Some(view) = view_weak.upgrade() {
                    view.rebuild_buffer();
                }
            });
        }

        // Search text change
        {
            let search_ref = self.search_filter.clone();
            let view_weak = self.downgrade_handle();
            self.search_entry.connect_search_changed(move |entry| {
                *search_ref.borrow_mut() = entry.text().trim().to_lowercase();
                if let Some(view) = view_weak.upgrade() {
                    view.rebuild_buffer();
                }
            });
        }

        // Auto-scroll toggle
        {
            let auto_scroll_ref = self.auto_scroll_enabled.clone();
            let text_view = self.text_view.clone();
            let scroll_mark = self.scroll_mark.clone();
            self.auto_scroll_btn.connect_toggled(move |btn| {
                let active = btn.is_active();
                auto_scroll_ref.set(active);
                if active {
                    text_view.scroll_to_mark(&scroll_mark, 0.0, false, 0.0, 1.0);
                }
            });
        }

        // Copy all logs to clipboard
        {
            let buffer = self.buffer.clone();
            let copy_btn = self.copy_btn.clone();
            self.copy_btn.connect_clicked(move |_| {
                let (start, end) = buffer.bounds();
                let text = buffer.text(&start, &end, false);
                if let Some(display) = gdk4::Display::default() {
                    let clipboard = display.clipboard();
                    clipboard.set_text(&text);
                    copy_btn.set_icon_name("object-select-symbolic");
                    let btn_clone = copy_btn.clone();
                    glib::timeout_add_local_once(std::time::Duration::from_millis(1500), move || {
                        btn_clone.set_icon_name("edit-copy-symbolic");
                    });
                }
            });
        }
    }

    fn init_log_stream(&self) {
        let service = self.service.clone();
        let view_weak = self.downgrade_handle();

        // 1. Preload recent logs from local log file
        glib::MainContext::default().spawn_local(async move {
            let recent_logs = service.get_recent_kernel_logs(200).await;
            if let Some(view) = view_weak.upgrade() {
                for log in recent_logs {
                    view.append_log_message(&log, false);
                }
                view.rebuild_buffer();
            }
        });

        // 2. Establish live WebSocket log stream
        let service = self.service.clone();
        let (tx, rx) = async_channel::bounded::<LogMessage>(200);

        tokio::spawn(async move {
            loop {
                let _ = service.stream_logs("debug", tx.clone()).await;
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
            }
        });

        let view_weak = self.downgrade_handle();
        glib::MainContext::default().spawn_local(async move {
            while let Ok(log) = rx.recv().await {
                if let Some(view) = view_weak.upgrade() {
                    view.append_log_message(&log, true);
                } else {
                    break;
                }
            }
        });
    }

    fn append_log_message(&self, log: &LogMessage, update_ui_now: bool) {
        {
            let mut logs = self.all_logs.borrow_mut();
            if logs.len() >= 5000 {
                logs.remove(0);
            }
            logs.push(log.clone());
        }

        if update_ui_now && self.matches_filter(log) {
            self.append_line_to_buffer(log);
            if self.auto_scroll_enabled.get() {
                self.text_view
                    .scroll_to_mark(&self.scroll_mark, 0.0, false, 0.0, 1.0);
            }
        }
    }

    fn matches_filter(&self, log: &LogMessage) -> bool {
        let level_idx = self.level_filter.get();
        let level_lower = log.level.to_lowercase();

        let matches_level = match level_idx {
            1 => level_lower == "info",
            2 => level_lower == "warning" || level_lower == "warn",
            3 => level_lower == "error" || level_lower == "fatal",
            4 => level_lower == "debug",
            _ => true,
        };

        if !matches_level {
            return false;
        }

        let query = self.search_filter.borrow().clone();
        if query.is_empty() {
            true
        } else {
            log.payload.to_lowercase().contains(&query)
        }
    }

    fn rebuild_buffer(&self) {
        self.buffer.set_text("");
        let logs = self.all_logs.borrow().clone();
        for log in &logs {
            if self.matches_filter(log) {
                self.append_line_to_buffer(log);
            }
        }
        if self.auto_scroll_enabled.get() {
            self.text_view
                .scroll_to_mark(&self.scroll_mark, 0.0, false, 0.0, 1.0);
        }
    }

    fn append_line_to_buffer(&self, log: &LogMessage) {
        let mut end_iter = self.buffer.end_iter();

        let level_tag = match log.level.to_lowercase().as_str() {
            "error" | "fatal" => "error",
            "warning" | "warn" => "warning",
            "debug" => "debug",
            _ => "info",
        };

        let tag_badge = format!("[{:<5}] ", log.level.to_uppercase());
        self.buffer
            .insert_with_tags_by_name(&mut end_iter, &tag_badge, &[level_tag]);

        let payload_line = format!("{}\n", log.payload);
        self.buffer.insert(&mut end_iter, &payload_line);
    }

    pub fn update_ui_text(&self) {
        self.page.set_title(tr("tab_logs"));
        self.search_entry
            .set_placeholder_text(Some(tr("log_search_placeholder")));
        self.auto_scroll_btn
            .set_tooltip_text(Some(tr("tooltip_auto_scroll")));
        self.copy_btn
            .set_tooltip_text(Some(tr("tooltip_copy_logs")));

        let selected = self.level_dropdown.selected();
        let levels = [
            tr("log_level_all"),
            tr("log_level_info"),
            tr("log_level_warn"),
            tr("log_level_error"),
            tr("log_level_debug"),
        ];
        let model = gtk4::StringList::new(&levels);
        self.is_updating_dropdown.set(true);
        self.level_dropdown.set_model(Some(&model));
        self.level_dropdown.set_selected(selected);
        self.is_updating_dropdown.set(false);

        self.popover
            .set_menu_model(Some(&build_log_context_menu()));
    }

    fn setup_context_menu(
        text_view: &TextView,
        buffer: &TextBuffer,
        all_logs: &Rc<RefCell<Vec<LogMessage>>>,
    ) -> gtk4::PopoverMenu {
        let popover = gtk4::PopoverMenu::from_model(Some(&build_log_context_menu()));
        popover.set_parent(text_view);
        popover.set_has_arrow(false);

        let action_group = gio::SimpleActionGroup::new();

        let buffer_for_copy = buffer.clone();
        let copy_action = gio::SimpleAction::new("copy", None);
        copy_action.connect_activate(move |_, _| {
            let text = if let Some((start, end)) = buffer_for_copy.selection_bounds() {
                buffer_for_copy.text(&start, &end, false)
            } else {
                let (start, end) = buffer_for_copy.bounds();
                buffer_for_copy.text(&start, &end, false)
            };
            if let Some(display) = gdk4::Display::default() {
                let clipboard = display.clipboard();
                clipboard.set_text(&text);
            }
        });
        action_group.add_action(&copy_action);

        let buffer_for_select = buffer.clone();
        let select_all_action = gio::SimpleAction::new("select_all", None);
        select_all_action.connect_activate(move |_, _| {
            let (start, end) = buffer_for_select.bounds();
            buffer_for_select.select_range(&start, &end);
        });
        action_group.add_action(&select_all_action);

        let all_logs_for_clear = all_logs.clone();
        let buffer_for_clear = buffer.clone();
        let clear_action = gio::SimpleAction::new("clear", None);
        clear_action.connect_activate(move |_, _| {
            all_logs_for_clear.borrow_mut().clear();
            buffer_for_clear.set_text("");
        });
        action_group.add_action(&clear_action);

        text_view.insert_action_group("log", Some(&action_group));

        let gesture = gtk4::GestureClick::new();
        gesture.set_button(3);
        gesture.set_propagation_phase(gtk4::PropagationPhase::Capture);

        let popover_clone = popover.clone();
        gesture.connect_pressed(move |gesture, _n_press, x, y| {
            gesture.set_state(gtk4::EventSequenceState::Claimed);
            popover_clone.set_pointing_to(Some(&gdk4::Rectangle::new(x as i32, y as i32, 1, 1)));
            popover_clone.popup();
        });
        text_view.add_controller(gesture);

        popover
    }

    fn downgrade_handle(&self) -> LogsViewWeak {
        LogsViewWeak {
            page: self.page.clone(),
            text_view: self.text_view.clone(),
            buffer: self.buffer.clone(),
            search_entry: self.search_entry.clone(),
            level_dropdown: self.level_dropdown.clone(),
            auto_scroll_btn: self.auto_scroll_btn.clone(),
            copy_btn: self.copy_btn.clone(),
            popover: self.popover.clone(),
            service: self.service.clone(),
            all_logs: self.all_logs.clone(),
            auto_scroll_enabled: self.auto_scroll_enabled.clone(),
            search_filter: self.search_filter.clone(),
            level_filter: self.level_filter.clone(),
            is_updating_dropdown: self.is_updating_dropdown.clone(),
            scroll_mark: self.scroll_mark.clone(),
        }
    }
}

fn build_log_context_menu() -> gio::Menu {
    let menu = gio::Menu::new();

    let section_edit = gio::Menu::new();
    section_edit.append(Some(&tr("log_menu_copy")), Some("log.copy"));
    section_edit.append(Some(&tr("log_menu_select_all")), Some("log.select_all"));
    menu.append_section(None, &section_edit);

    let section_clear = gio::Menu::new();
    section_clear.append(Some(&tr("log_menu_clear")), Some("log.clear"));
    menu.append_section(None, &section_clear);

    menu
}

#[derive(Clone)]
struct LogsViewWeak {
    page: adw::NavigationPage,
    text_view: TextView,
    buffer: TextBuffer,
    search_entry: SearchEntry,
    level_dropdown: DropDown,
    auto_scroll_btn: ToggleButton,
    copy_btn: Button,
    popover: gtk4::PopoverMenu,
    service: AppService,
    all_logs: Rc<RefCell<Vec<LogMessage>>>,
    auto_scroll_enabled: Rc<Cell<bool>>,
    search_filter: Rc<RefCell<String>>,
    level_filter: Rc<Cell<u32>>,
    is_updating_dropdown: Rc<Cell<bool>>,
    scroll_mark: TextMark,
}

impl LogsViewWeak {
    fn upgrade(&self) -> Option<LogsView> {
        Some(LogsView {
            page: self.page.clone(),
            text_view: self.text_view.clone(),
            buffer: self.buffer.clone(),
            search_entry: self.search_entry.clone(),
            level_dropdown: self.level_dropdown.clone(),
            auto_scroll_btn: self.auto_scroll_btn.clone(),
            copy_btn: self.copy_btn.clone(),
            popover: self.popover.clone(),
            service: self.service.clone(),
            all_logs: self.all_logs.clone(),
            auto_scroll_enabled: self.auto_scroll_enabled.clone(),
            search_filter: self.search_filter.clone(),
            level_filter: self.level_filter.clone(),
            is_updating_dropdown: self.is_updating_dropdown.clone(),
            scroll_mark: self.scroll_mark.clone(),
        })
    }
}
