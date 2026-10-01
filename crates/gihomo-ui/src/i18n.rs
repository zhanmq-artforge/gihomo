use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
    sync::{mpsc, LazyLock, RwLock},
};
use tracing::{debug, info};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Language {
    #[default]
    System,
    ZhCn,
    EnUs,
}

impl Language {
    pub fn display_name(&self) -> &'static str {
        match self {
            Language::System => "跟随系统",
            Language::ZhCn => "简体中文",
            Language::EnUs => "English",
        }
    }

    pub fn to_index(&self) -> u32 {
        match self {
            Language::System => 0,
            Language::ZhCn => 1,
            Language::EnUs => 2,
        }
    }

    pub fn from_index(index: u32) -> Self {
        match index {
            1 => Language::ZhCn,
            2 => Language::EnUs,
            _ => Language::System,
        }
    }

    pub fn resolved(&self) -> ResolvedLanguage {
        match self {
            Language::ZhCn => ResolvedLanguage::ZhCn,
            Language::EnUs => ResolvedLanguage::EnUs,
            Language::System => detect_system_language(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolvedLanguage {
    ZhCn,
    EnUs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ThemeMode {
    #[default]
    System,
    Light,
    Dark,
}

impl ThemeMode {
    pub fn to_index(&self) -> u32 {
        match self {
            ThemeMode::System => 0,
            ThemeMode::Light => 1,
            ThemeMode::Dark => 2,
        }
    }

    pub fn from_index(index: u32) -> Self {
        match index {
            1 => ThemeMode::Light,
            2 => ThemeMode::Dark,
            _ => ThemeMode::System,
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_false() -> bool {
    false
}

fn default_zero_u32() -> u32 {
    0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub language: Language,
    #[serde(default)]
    pub theme: ThemeMode,
    #[serde(default = "default_true")]
    pub close_to_tray: bool,
    #[serde(default = "default_true")]
    pub auto_start_kernel: bool,
    #[serde(default = "default_false")]
    pub auto_restore_proxy: bool,
    #[serde(default = "default_false")]
    pub last_proxy_enabled: bool,
    #[serde(default = "default_false")]
    pub last_tun_enabled: bool,
    #[serde(default = "default_zero_u32")]
    pub auto_update_interval_minutes: u32,
}

enum ConfigWrite {
    Save(AppConfig),
    Flush(mpsc::SyncSender<()>),
}

static CONFIG_WRITER: LazyLock<mpsc::Sender<ConfigWrite>> = LazyLock::new(|| {
    let (sender, receiver) = mpsc::channel();
    if let Err(error) = std::thread::Builder::new()
        .name("gihomo-config-writer".to_string())
        .spawn(move || {
            while let Ok(command) = receiver.recv() {
                match command {
                    ConfigWrite::Save(config) => write_config_file(&config),
                    ConfigWrite::Flush(reply) => {
                        let _ = reply.send(());
                    }
                }
            }
        })
    {
        tracing::error!("Failed to start config writer thread: {}", error);
    }
    sender
});

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            language: Language::default(),
            theme: ThemeMode::default(),
            close_to_tray: true,
            auto_start_kernel: true,
            auto_restore_proxy: false,
            last_proxy_enabled: false,
            last_tun_enabled: false,
            auto_update_interval_minutes: 0,
        }
    }
}

fn config_path() -> PathBuf {
    let mut path = glib::user_config_dir();
    path.push("art.artforge.Gihomo");
    path.push("config.json");
    path
}

pub fn load_config() -> AppConfig {
    let path = config_path();
    if path.exists() {
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(config) = serde_json::from_str::<AppConfig>(&content) {
                return config;
            }
        }
    }
    AppConfig::default()
}

pub fn preload_config() {
    let _ = I18N.read().unwrap().config.language;
}

pub fn save_config(config: &AppConfig) {
    if CONFIG_WRITER
        .send(ConfigWrite::Save(config.clone()))
        .is_err()
    {
        tracing::error!("Config writer is unavailable; preference was not saved");
    }
}

fn write_config_file(config: &AppConfig) {
    let path = config_path();
    if let Some(parent) = path.parent() {
        if let Err(error) = fs::create_dir_all(parent) {
            tracing::error!("Failed to create config directory: {}", error);
            return;
        }
    }
    match serde_json::to_string_pretty(config) {
        Ok(content) => {
            if let Err(error) = fs::write(path, content) {
                tracing::error!("Failed to save preferences: {}", error);
            }
        }
        Err(error) => tracing::error!("Failed to serialize preferences: {}", error),
    }
}

pub fn flush_config_writes() {
    let (reply, response) = mpsc::sync_channel(1);
    if CONFIG_WRITER.send(ConfigWrite::Flush(reply)).is_ok() {
        let _ = response.recv();
    }
}

fn detect_system_language() -> ResolvedLanguage {
    let names = glib::language_names();
    for name in names {
        let n = name.to_lowercase();
        if n.starts_with("zh") {
            debug!(detected = %name, "Detected Chinese system locale");
            return ResolvedLanguage::ZhCn;
        }
    }

    for var in &["LANG", "LC_ALL", "LC_MESSAGES"] {
        if let Ok(val) = std::env::var(var) {
            let v = val.to_lowercase();
            if v.starts_with("zh") {
                debug!(var, detected = %val, "Detected Chinese locale from environment variable");
                return ResolvedLanguage::ZhCn;
            }
        }
    }

    debug!("Defaulting to English locale");
    ResolvedLanguage::EnUs
}

struct I18nManager {
    config: AppConfig,
    active_language: ResolvedLanguage,
    en_dict: HashMap<&'static str, &'static str>,
    zh_dict: HashMap<&'static str, &'static str>,
}

static I18N: LazyLock<RwLock<I18nManager>> = LazyLock::new(|| {
    let config = load_config();
    let active = config.language.resolved();

    info!(
        config_lang = ?config.language,
        resolved_lang = ?active,
        config_theme = ?config.theme,
        "Initializing Gihomo i18n & theme subsystem"
    );

    let mut zh = HashMap::new();
    let mut en = HashMap::new();

    // App & Navigation
    zh.insert("app_name", "Gihomo");
    en.insert("app_name", "Gihomo");
    zh.insert("tab_dashboard", "仪表盘");
    en.insert("tab_dashboard", "Dashboard");
    zh.insert("tab_proxies", "节点选择");
    en.insert("tab_proxies", "Proxies");
    zh.insert("tab_rules", "分流规则");
    en.insert("tab_rules", "Rules");
    zh.insert("tab_subscriptions", "订阅管理");
    en.insert("tab_subscriptions", "Subscriptions");
    zh.insert("tab_connections", "连接监控");
    en.insert("tab_connections", "Connections");
    zh.insert("tab_logs", "内核日志");
    en.insert("tab_logs", "Logs");
    zh.insert("tab_settings", "设置");
    en.insert("tab_settings", "Settings");
    zh.insert("sidebar_title", "导航");
    en.insert("sidebar_title", "Navigation");
    zh.insert("nav_back", "返回");
    en.insert("nav_back", "Back");

    // Dashboard Controls
    zh.insert("ctrl_group_title", "快捷控制");
    en.insert("ctrl_group_title", "Quick Controls");
    zh.insert("ctrl_group_desc", "控制本地系统代理设置与 Mihomo 核心行为");
    en.insert(
        "ctrl_group_desc",
        "Manage system proxy and Mihomo core settings",
    );
    zh.insert("sys_proxy_title", "系统代理 (GNOME)");
    en.insert("sys_proxy_title", "System Proxy (GNOME)");
    zh.insert(
        "sys_proxy_sub",
        "修改 org.gnome.system.proxy，将宿主机流量导向混合端口 (默认关闭)",
    );
    en.insert(
        "sys_proxy_sub",
        "Route host traffic through mixed port via org.gnome.system.proxy",
    );
    zh.insert("tun_title", "TUN 模式 (全局虚拟网卡)");
    en.insert("tun_title", "TUN Mode (Global Virtual NIC)");
    zh.insert("tun_sub", "需内核具备 CAP_NET_ADMIN 网络特权");
    en.insert("tun_sub", "Requires kernel with CAP_NET_ADMIN capability");
    zh.insert("kernel_status_title", "内核运行状态");
    en.insert("kernel_status_title", "Kernel Status");
    zh.insert("kernel_running", "运行中");
    en.insert("kernel_running", "Running");
    zh.insert("kernel_stopped", "已停止");
    en.insert("kernel_stopped", "Stopped");
    zh.insert("kernel_not_found", "未找到");
    en.insert("kernel_not_found", "Not Found");
    zh.insert("kernel_error", "异常");
    en.insert("kernel_error", "Error");
    zh.insert("error_prefix", "错误: ");
    en.insert("error_prefix", "Error: ");
    zh.insert("kernel_starting", "启动中...");
    en.insert("kernel_starting", "Starting...");
    zh.insert("kernel_stopping", "停止中...");
    en.insert("kernel_stopping", "Stopping...");
    zh.insert("kernel_restarting", "重启中...");
    en.insert("kernel_restarting", "Restarting...");
    zh.insert("kernel_sub_running", "Mihomo 内核进程正常运行");
    en.insert(
        "kernel_sub_running",
        "Mihomo kernel process is running normally",
    );
    zh.insert("kernel_sub_stopped", "内核进程已停止");
    en.insert("kernel_sub_stopped", "Kernel process is stopped");
    zh.insert(
        "kernel_sub_not_found",
        "未检测到内核文件，请放置在 bin 目录",
    );
    en.insert(
        "kernel_sub_not_found",
        "Kernel binary not found, please place in bin directory",
    );
    zh.insert("btn_start_kernel_tooltip", "启动内核 (mihomo -d)");
    en.insert("btn_start_kernel_tooltip", "Start kernel (mihomo -d)");
    zh.insert("btn_restart_kernel_tooltip", "重启内核");
    en.insert("btn_restart_kernel_tooltip", "Restart kernel");
    zh.insert("btn_stop_kernel_tooltip", "停止内核");
    en.insert("btn_stop_kernel_tooltip", "Stop kernel");

    // Traffic Stats
    zh.insert("traffic_title", "流量监控");
    en.insert("traffic_title", "Traffic Monitor");
    zh.insert("traffic_desc", "Mihomo 实时网络速率与总计传输量");
    en.insert(
        "traffic_desc",
        "Real-time network transfer speed and total stats",
    );
    zh.insert("realtime_up", "实时上行");
    en.insert("realtime_up", "Real-time Up");
    zh.insert("realtime_down", "实时下行");
    en.insert("realtime_down", "Real-time Down");
    zh.insert("total_prefix", "总计: ");
    en.insert("total_prefix", "Total: ");

    // Active Subscription
    zh.insert("active_sub_title", "当前激活配置");
    en.insert("active_sub_title", "Active Configuration");
    zh.insert(
        "active_sub_desc",
        "直接从下拉菜单切换可用订阅，配置会自动热重载",
    );
    en.insert(
        "active_sub_desc",
        "Switch subscriptions via dropdown; config reloads automatically",
    );
    zh.insert("active_sub_row", "活动订阅");
    en.insert("active_sub_row", "Active Profile");
    zh.insert("not_active", "未激活");
    en.insert("not_active", "Inactive");
    zh.insert("package_traffic_title", "套餐流量使用");
    en.insert("package_traffic_title", "Package Traffic Usage");
    zh.insert("select_sub_hint", "请使用右侧下拉菜单选择订阅激活");
    en.insert(
        "select_sub_hint",
        "Select a subscription from the dropdown to activate",
    );
    zh.insert("current_using", "当前使用: ");
    en.insert("current_using", "Active: ");
    zh.insert("no_available_sub", "(暂无可用订阅)");
    en.insert("no_available_sub", "(No Subscriptions Available)");

    // Proxies View
    zh.insert("tooltip_refresh_proxies", "刷新节点列表与状态");
    en.insert("tooltip_refresh_proxies", "Refresh proxy nodes and status");
    zh.insert("tooltip_ping_all_groups", "并发测试全部节点延迟");
    en.insert("tooltip_ping_all_groups", "Test latency for all proxy nodes");
    zh.insert("proxies_mode_label", "分流模式:");
    en.insert("proxies_mode_label", "Mode:");
    zh.insert("mode_rule", "规则");
    en.insert("mode_rule", "Rule");
    zh.insert("mode_global", "全局");
    en.insert("mode_global", "Global");
    zh.insert("mode_direct", "直连");
    en.insert("mode_direct", "Direct");
    zh.insert("mode_rule_tooltip", "规则分流 (Rule)");
    en.insert("mode_rule_tooltip", "Rule-based routing");
    zh.insert("mode_global_tooltip", "全局代理 (Global)");
    en.insert("mode_global_tooltip", "Global proxy");
    zh.insert("mode_direct_tooltip", "直接连接 (Direct)");
    en.insert("mode_direct_tooltip", "Direct connection");
    zh.insert("empty_proxies_title", "未检测到代理节点");
    en.insert("empty_proxies_title", "No Proxy Nodes Detected");
    zh.insert(
        "empty_proxies_desc",
        "请确保内核处于运行状态且已激活包含节点的订阅配置",
    );
    en.insert(
        "empty_proxies_desc",
        "Ensure Mihomo kernel is running and a valid subscription is loaded",
    );
    zh.insert("btn_ping", "测速");
    en.insert("btn_ping", "Ping");
    zh.insert("btn_pinging", "测速中...");
    en.insert("btn_pinging", "Testing...");
    zh.insert("toast_ping_finished", "节点延迟测速完成");
    en.insert("toast_ping_finished", "Node latency testing completed");
    zh.insert("toast_node_switched", "已切换节点");
    en.insert("toast_node_switched", "Node switched");
    zh.insert("btn_select", "选择");
    en.insert("btn_select", "Select");
    zh.insert("selected_badge", "✓ 当前使用");
    en.insert("selected_badge", "✓ In Use");
    zh.insert("timeout_badge", "超时");
    en.insert("timeout_badge", "Timeout");
    zh.insert("group_type_prefix", "策略类型: ");
    en.insert("group_type_prefix", "Type: ");
    zh.insert("current_node_prefix", "当前节点: ");
    en.insert("current_node_prefix", "Current: ");
    zh.insert("tooltip_ping_node", "测试单节点延迟");
    en.insert("tooltip_ping_node", "Test node latency");
    zh.insert("proxy_search_placeholder", "搜索节点名称...");
    en.insert("proxy_search_placeholder", "Search node name...");
    zh.insert("empty_group_nodes", "当前策略组下无匹配节点");
    en.insert("empty_group_nodes", "No matching nodes in this group");
    zh.insert("proxy_group_label", "策略组:");
    en.insert("proxy_group_label", "Group:");

    // Rules View
    zh.insert("rules_search_placeholder", "搜索分流规则 (域名 / IP / 策略)...");
    en.insert("rules_search_placeholder", "Search rules (domain, IP, proxy)...");
    zh.insert("rules_count_empty", "暂无分流规则");
    en.insert("rules_count_empty", "No rules available");
    zh.insert("rules_count_prefix", "共匹配规则: ");
    en.insert("rules_count_prefix", "Matching rules: ");
    zh.insert("rules_refresh_tooltip", "刷新分流规则与规则集");
    en.insert("rules_refresh_tooltip", "Refresh routing rules");
    zh.insert("rules_providers_title", "规则集 (Rule Providers)");
    en.insert("rules_providers_title", "Rule Providers");
    zh.insert("rules_providers_desc", "订阅配置中引用的外部规则集");
    en.insert("rules_providers_desc", "External rule providers defined in subscription");
    zh.insert("rules_list_title", "生效规则列表");
    en.insert("rules_list_title", "Active Rules");
    zh.insert("rules_list_desc", "流量命中规则将按从上到下顺序依次匹配");
    en.insert("rules_list_desc", "Traffic matches rules sequentially from top to bottom");
    zh.insert(
        "rules_list_desc_format",
        "流量命中规则将按从上到下顺序依次匹配 · 共 {count} 条生效规则",
    );
    en.insert(
        "rules_list_desc_format",
        "Traffic matches rules sequentially from top to bottom · {count} active rules",
    );
    zh.insert(
        "rules_list_desc_filtered",
        "流量命中规则将按从上到下顺序依次匹配 · 已筛选匹配 {filtered} / {total} 条规则",
    );
    en.insert(
        "rules_list_desc_filtered",
        "Traffic matches rules sequentially from top to bottom · Matching {filtered} / {total} rules",
    );
    zh.insert("rules_provider_subtitle", "{type} ({count}条规则) | 更新时间: {time}");
    en.insert("rules_provider_subtitle", "{type} ({count} rules) | Updated: {time}");
    zh.insert("empty_rules_title", "未获取到分流规则");
    en.insert("empty_rules_title", "No Rules Detected");
    zh.insert("empty_rules_desc", "请确认内核处于运行状态且已激活有效订阅");
    en.insert("empty_rules_desc", "Ensure Mihomo kernel is running with an active subscription");
    zh.insert("rule_match_all", "(全流量匹配 MATCH)");
    en.insert("rule_match_all", "(Match all traffic)");
    zh.insert("btn_load_more", "加载更多规则...");
    en.insert("btn_load_more", "Load More Rules...");
    zh.insert("btn_load_more_nodes", "加载更多节点...");
    en.insert("btn_load_more_nodes", "Load more nodes...");
    zh.insert("btn_load_more_conn", "加载更多连接...");
    en.insert("btn_load_more_conn", "Load more connections...");
    zh.insert("btn_update", "更新");
    en.insert("btn_update", "Update");

    // Connections View
    zh.insert("conn_count_zero", "0 个活跃连接");
    en.insert("conn_count_zero", "0 active connections");
    zh.insert("conn_count_badge", "{count} 个活跃连接");
    en.insert("conn_count_badge", "{count} active connections");
    zh.insert("conn_count_filtered", "已筛选 {filtered} / {total} 个连接");
    en.insert("conn_count_filtered", "Filtered {filtered} / {total} connections");
    zh.insert("conn_showing_top", "已显示前 {count} 条");
    en.insert("conn_showing_top", "Showing top {count}");
    zh.insert("conn_search_placeholder", "搜索域名、IP、进程或规则...");
    en.insert("conn_search_placeholder", "Search domain, IP, process, or rule...");
    zh.insert("tooltip_refresh_conn", "刷新当前连接列表");
    en.insert("tooltip_refresh_conn", "Refresh connection list");
    zh.insert("btn_close_all_conn", "断开全部");
    en.insert("btn_close_all_conn", "Close All");
    zh.insert("tooltip_close_all_conn", "断开当前所有活跃网络连接");
    en.insert("tooltip_close_all_conn", "Close all active network connections");
    zh.insert("conn_empty_title", "暂无活跃网络连接");
    en.insert("conn_empty_title", "No Active Connections");
    zh.insert(
        "conn_empty_desc",
        "当系统或应用程序通过代理核心发起网络请求时，此处将实时展示连接信息",
    );
    en.insert(
        "conn_empty_desc",
        "Connections will appear here in real-time when traffic routes through the core",
    );
    zh.insert("dialog_close_all_conn_title", "断开全部连接？");
    en.insert("dialog_close_all_conn_title", "Close all connections?");
    zh.insert(
        "dialog_close_all_conn_body",
        "这将立即终止当前所有活跃的网络连接。此操作可能会中断正在进行的下载或网络传输。",
    );
    en.insert(
        "dialog_close_all_conn_body",
        "This will immediately terminate all active network connections. Ongoing downloads or transfers may be interrupted.",
    );
    zh.insert("btn_close_all_confirm", "立即断开全部");
    en.insert("btn_close_all_confirm", "Close All Now");
    zh.insert("toast_close_conn_failed", "断开连接失败");
    en.insert("toast_close_conn_failed", "Failed to close connections");
    zh.insert("toast_close_all_conn_success", "已成功断开全部网络连接");
    en.insert("toast_close_all_conn_success", "All network connections closed successfully");
    zh.insert("conn_list_title", "活跃连接明细");
    en.insert("conn_list_title", "Active Connections Details");
    zh.insert(
        "conn_list_desc",
        "实时显示连接目标、分流规则、出口代理节点与传输流量",
    );
    en.insert(
        "conn_list_desc",
        "Live tracking of destinations, routing rules, outbound chains, and bandwidth",
    );
    zh.insert("conn_rule_label", "规则");
    en.insert("conn_rule_label", "Rule");
    zh.insert("conn_chain_label", "链");
    en.insert("conn_chain_label", "Chain");
    zh.insert("tooltip_close_single_conn", "断开该连接");
    en.insert("tooltip_close_single_conn", "Close this connection");
    zh.insert("conn_src_addr", "来源地址");
    en.insert("conn_src_addr", "Source Address");
    zh.insert("conn_dst_addr", "目标地址与端口");
    en.insert("conn_dst_addr", "Destination Address &amp; Port");
    zh.insert("conn_proc_path", "所属进程路径");
    en.insert("conn_proc_path", "Process Path");
    zh.insert("conn_rule_detail", "命中规则明细");
    en.insert("conn_rule_detail", "Matched Rule");
    zh.insert("conn_chain_detail", "出口代理链路");
    en.insert("conn_chain_detail", "Outbound Proxy Chain");
    zh.insert("conn_start_time", "连接建立时间");
    en.insert("conn_start_time", "Connection Established Time");

    // Logs View
    zh.insert("log_level_all", "全部");
    en.insert("log_level_all", "All");
    zh.insert("log_level_info", "信息");
    en.insert("log_level_info", "Info");
    zh.insert("log_level_warn", "警告");
    en.insert("log_level_warn", "Warning");
    zh.insert("log_level_error", "错误");
    en.insert("log_level_error", "Error");
    zh.insert("log_level_debug", "调试");
    en.insert("log_level_debug", "Debug");
    zh.insert("log_search_placeholder", "过滤日志关键字...");
    en.insert("log_search_placeholder", "Filter log keywords...");
    zh.insert("tooltip_auto_scroll", "锁定滚动到底部");
    en.insert("tooltip_auto_scroll", "Lock scroll to bottom");
    zh.insert("btn_auto_scroll", "锁定滚动");
    en.insert("btn_auto_scroll", "Auto Scroll");
    zh.insert("tooltip_copy_logs", "复制日志内容到剪贴板");
    en.insert("tooltip_copy_logs", "Copy logs to clipboard");
    zh.insert("btn_copy_logs", "复制");
    en.insert("btn_copy_logs", "Copy");
    zh.insert("btn_copied", "已复制");
    en.insert("btn_copied", "Copied");
    zh.insert("btn_export_logs", "导出");
    en.insert("btn_export_logs", "Export");
    zh.insert("tooltip_export_logs", "导出当前日志至下载目录");
    en.insert("tooltip_export_logs", "Export current logs to Downloads directory");
    zh.insert("toast_export_logs_success", "日志已成功导出至下载目录: {filename}");
    en.insert("toast_export_logs_success", "Logs successfully exported to Downloads: {filename}");
    zh.insert("toast_export_logs_empty", "当前无日志可导出");
    en.insert("toast_export_logs_empty", "No logs available to export");
    zh.insert("toast_export_logs_failed", "导出日志文件失败");
    en.insert("toast_export_logs_failed", "Failed to export log file");
    zh.insert("toast_copy_logs_success", "已复制当前日志至剪贴板");
    en.insert("toast_copy_logs_success", "Current logs copied to clipboard");
    zh.insert("btn_clear_logs", "清空");
    en.insert("btn_clear_logs", "Clear");
    zh.insert("tooltip_clear_logs", "清空当前控制台日志");
    en.insert("tooltip_clear_logs", "Clear console logs");
    zh.insert("log_menu_copy", "复制");
    en.insert("log_menu_copy", "Copy");
    zh.insert("log_menu_select_all", "全选");
    en.insert("log_menu_select_all", "Select All");
    zh.insert("log_menu_clear", "清空控制台");
    en.insert("log_menu_clear", "Clear Console");

    // Subscriptions View
    zh.insert("btn_add_sub", "添加订阅");
    en.insert("btn_add_sub", "Add Subscription");
    zh.insert("btn_add", "添加");
    en.insert("btn_add", "Add");
    zh.insert("btn_update_all_subs", "更新全部");
    en.insert("btn_update_all_subs", "Update All");
    zh.insert("tooltip_update_all_subs", "更新所有订阅和本地配置");
    en.insert(
        "tooltip_update_all_subs",
        "Refresh all subscriptions and local configs",
    );
    zh.insert("sub_update_all_done", "全部更新完成");
    en.insert("sub_update_all_done", "All items updated");
    zh.insert("sub_update_all_partial", "部分更新失败");
    en.insert("sub_update_all_partial", "Some updates failed");
    zh.insert("empty_sub_title", "暂无订阅");
    en.insert("empty_sub_title", "No Subscriptions");
    zh.insert(
        "empty_sub_desc",
        "点击上方「添加订阅」按钮导入远程链接或本地 YAML 配置文件",
    );
    en.insert(
        "empty_sub_desc",
        "Click the 'Add Subscription' button above to import a remote URL or local YAML file",
    );
    zh.insert("sub_list_title", "订阅列表");
    en.insert("sub_list_title", "Subscription List");
    zh.insert(
        "sub_list_desc",
        "使用行末勾选按钮切换当前配置，活动配置会自动热加载",
    );
    en.insert(
        "sub_list_desc",
        "Use the check button to switch active profiles; Mihomo reloads automatically",
    );
    zh.insert("btn_set_active", "设为活动");
    en.insert("btn_set_active", "Set Active");
    zh.insert("badge_in_use", "✓ 当前使用中");
    en.insert("badge_in_use", "✓ In Use");
    zh.insert("sub_updated_time", "最后更新时间");
    en.insert("sub_updated_time", "Last Updated");
    zh.insert("sub_never_updated", "未更新");
    en.insert("sub_never_updated", "Never updated");
    zh.insert("sub_address_title", "订阅地址");
    en.insert("sub_address_title", "Subscription Address");
    zh.insert("sub_local_file_title", "本地文件");
    en.insert("sub_local_file_title", "Local File");
    zh.insert("tooltip_copy_address", "复制订阅地址");
    en.insert("tooltip_copy_address", "Copy subscription address");
    zh.insert("tooltip_refresh_sub", "拉取/更新订阅内容");
    en.insert("tooltip_refresh_sub", "Update subscription content");
    zh.insert("sub_refreshing", "刷新中...");
    en.insert("sub_refreshing", "Updating...");
    zh.insert("toast_sub_refreshed", "订阅更新成功");
    en.insert("toast_sub_refreshed", "Subscription updated successfully");
    zh.insert("toast_sub_failed", "订阅更新失败");
    en.insert("toast_sub_failed", "Failed to update subscription");
    zh.insert("toast_sub_added", "订阅添加成功");
    en.insert("toast_sub_added", "Subscription added successfully");
    zh.insert("tooltip_delete_sub", "删除该订阅");
    en.insert("tooltip_delete_sub", "Delete subscription");
    zh.insert("dialog_delete_sub_title", "删除订阅？");
    en.insert("dialog_delete_sub_title", "Delete subscription?");
    zh.insert(
        "dialog_delete_sub_body",
        "确定删除“{name}”及其缓存配置？此操作无法撤销。",
    );
    en.insert(
        "dialog_delete_sub_body",
        "Delete “{name}” and its cached configuration? This cannot be undone.",
    );
    zh.insert("btn_delete_sub", "删除");
    en.insert("btn_delete_sub", "Delete");
    zh.insert("toast_sub_delete_failed", "删除订阅失败");
    en.insert("toast_sub_delete_failed", "Failed to delete subscription");
    zh.insert("sub_activate_failed", "切换活动订阅失败");
    en.insert("sub_activate_failed", "Failed to activate subscription");
    zh.insert("tooltip_edit_sub", "编辑订阅名称或来源");
    en.insert("tooltip_edit_sub", "Edit subscription name or source");
    zh.insert("sub_settings_group", "自动更新设置");
    en.insert("sub_settings_group", "Auto-Update Settings");
    zh.insert("sub_auto_update_interval", "自动更新频率");
    en.insert("sub_auto_update_interval", "Auto-Update Interval");
    zh.insert("sub_interval_never", "不自动更新");
    en.insert("sub_interval_never", "Never");
    zh.insert("sub_interval_30m", "30 分钟");
    en.insert("sub_interval_30m", "30 Minutes");
    zh.insert("sub_interval_1h", "1 小时");
    en.insert("sub_interval_1h", "1 Hour");
    zh.insert("sub_interval_2h", "2 小时");
    en.insert("sub_interval_2h", "2 Hours");
    zh.insert("sub_interval_6h", "6 小时");
    en.insert("sub_interval_6h", "6 Hours");
    zh.insert("sub_interval_12h", "12 小时");
    en.insert("sub_interval_12h", "12 Hours");
    zh.insert("sub_interval_24h", "24 小时 (1 天)");
    en.insert("sub_interval_24h", "24 Hours (1 Day)");
    zh.insert(
        "sub_auto_update_disabled",
        "已关闭自动更新",
    );
    en.insert(
        "sub_auto_update_disabled",
        "Auto-update disabled",
    );
    zh.insert("sub_auto_update_enabled_m", "每 {m} 分钟自动更新全部订阅");
    en.insert(
        "sub_auto_update_enabled_m",
        "Automatically update all subscriptions every {m} minutes",
    );
    zh.insert(
        "sub_auto_update_enabled_h",
        "每 {m} 分钟 (约 {h} 小时) 自动更新全部订阅",
    );
    en.insert(
        "sub_auto_update_enabled_h",
        "Automatically update all subscriptions every {m} minutes (~{h}h)",
    );

    // Add Subscription Dialog
    zh.insert("dialog_add_sub_title", "添加订阅");
    en.insert("dialog_add_sub_title", "Add Subscription");
    zh.insert("dialog_edit_sub_title", "编辑订阅");
    en.insert("dialog_edit_sub_title", "Edit Subscription");
    zh.insert("btn_cancel", "取消");
    en.insert("btn_cancel", "Cancel");
    zh.insert("btn_save_sub", "保存");
    en.insert("btn_save_sub", "Save");
    zh.insert("sub_saving", "保存中...");
    en.insert("sub_saving", "Saving...");
    zh.insert("sub_name_required", "请输入订阅名称");
    en.insert("sub_name_required", "Enter a subscription name");
    zh.insert("sub_url_required", "请输入订阅链接");
    en.insert("sub_url_required", "Enter a subscription URL");
    zh.insert("sub_adding", "正在添加...");
    en.insert("sub_adding", "Adding...");
    zh.insert("tab_remote_sub", "远程订阅");
    en.insert("tab_remote_sub", "Remote URL");
    zh.insert("tab_local_sub", "本地文件");
    en.insert("tab_local_sub", "Local File");
    zh.insert("group_remote_title", "从链接拉取订阅");
    en.insert("group_remote_title", "Pull from URL");
    zh.insert("group_remote_desc", "支持 Clash / Mihomo 格式的订阅链接");
    en.insert(
        "group_remote_desc",
        "Supports Clash / Mihomo format subscription URLs",
    );
    zh.insert("sub_name_row", "订阅名称");
    en.insert("sub_name_row", "Subscription Name");
    zh.insert("sub_url_row", "订阅链接 (URL)");
    en.insert("sub_url_row", "Subscription URL");
    zh.insert("btn_confirm_pull", "确认拉取");
    en.insert("btn_confirm_pull", "Download & Add");
    zh.insert("group_local_title", "导入本地配置文件");
    en.insert("group_local_title", "Import Local Config");
    zh.insert("group_local_desc", "导入已有的 config.yaml 或订阅文件");
    en.insert(
        "group_local_desc",
        "Import existing config.yaml or subscription file",
    );
    zh.insert("config_name_row", "配置名称");
    en.insert("config_name_row", "Config Name");
    zh.insert("file_chooser_row", "选择 YAML 文件");
    en.insert("file_chooser_row", "Choose YAML File");
    zh.insert("file_chooser_none", "未选择任何文件");
    en.insert("file_chooser_none", "No file selected");
    zh.insert("btn_browse", "浏览...");
    en.insert("btn_browse", "Browse...");
    zh.insert("btn_confirm_import", "确认导入");
    en.insert("btn_confirm_import", "Import");

    // Share Links Import
    zh.insert("tab_share_links", "节点链接");
    en.insert("tab_share_links", "Share Links");
    zh.insert("group_share_links_title", "导入节点分享链接");
    en.insert("group_share_links_title", "Import Share Links");
    zh.insert(
        "group_share_links_desc",
        "支持多行批量输入 ss://, vmess://, vless://, trojan://, hysteria2://",
    );
    en.insert(
        "group_share_links_desc",
        "Supports multi-line paste of ss://, vmess://, vless://, trojan://, hysteria2://",
    );
    zh.insert("share_links_name_row", "配置名称");
    en.insert("share_links_name_row", "Profile Name");
    zh.insert("share_links_default_name", "自建/分享节点");
    en.insert("share_links_default_name", "Custom Nodes");
    zh.insert(
        "share_links_paste_hint",
        "粘贴节点链接（支持换行批量输入，附带 #备注）：",
    );
    en.insert(
        "share_links_paste_hint",
        "Paste share links (one per line with optional #remark):",
    );
    zh.insert("btn_paste_clipboard", "从剪贴板粘贴");
    en.insert("btn_paste_clipboard", "Paste from Clipboard");
    zh.insert("share_links_preview_title", "已识别节点");
    en.insert("share_links_preview_title", "Recognized Nodes");
    zh.insert(
        "share_links_preview_empty",
        "尚未识别到有效节点，请在上方粘贴链接",
    );
    en.insert(
        "share_links_preview_empty",
        "No valid nodes recognized yet. Paste links above.",
    );
    zh.insert("btn_confirm_import_links", "导入并生成配置");
    en.insert("btn_confirm_import_links", "Import & Create Profile");
    zh.insert("sub_nodes_count", "个节点");
    en.insert("sub_nodes_count", "nodes");

    // Settings View
    zh.insert("settings_appearance", "外观与界面语言");
    en.insert("settings_appearance", "Appearance &amp; Language");
    zh.insert("settings_appearance_desc", "设置主题深浅风格与界面展示语言");
    en.insert(
        "settings_appearance_desc",
        "Configure color scheme and display language",
    );
    zh.insert("settings_theme", "主题色彩");
    en.insert("settings_theme", "Color Scheme");
    zh.insert("theme_system", "跟随系统");
    en.insert("theme_system", "Follow System");
    zh.insert("theme_light", "浅色模式");
    en.insert("theme_light", "Light Mode");
    zh.insert("theme_dark", "深色模式");
    en.insert("theme_dark", "Dark Mode");
    zh.insert("settings_language", "界面语言");
    en.insert("settings_language", "Language");
    zh.insert("lang_system", "跟随系统");
    en.insert("lang_system", "System Default");
    zh.insert("settings_net_ports", "网络与核心端口");
    en.insert("settings_net_ports", "Network &amp; Core Ports");
    zh.insert(
        "settings_net_ports_desc",
        "Mihomo 运行时的端口映射与外部控制接口",
    );
    en.insert(
        "settings_net_ports_desc",
        "Mihomo runtime port mapping and external controller API",
    );
    zh.insert("settings_mixed_port", "混合代理端口 (Mixed Port)");
    en.insert("settings_mixed_port", "Mixed Proxy Port");
    zh.insert("settings_mixed_port_sub", "7890 (HTTP &amp; SOCKS5 共用端口)");
    en.insert(
        "settings_mixed_port_sub",
        "7890 (Shared HTTP &amp; SOCKS5 port)",
    );
    zh.insert(
        "settings_controller",
        "外部控制器 API (External Controller)",
    );
    en.insert("settings_controller", "External Controller API");
    zh.insert(
        "settings_controller_sub",
        "127.0.0.1:9090 (REST API / WebSocket)",
    );
    en.insert(
        "settings_controller_sub",
        "127.0.0.1:9090 (REST API / WebSocket)",
    );
    zh.insert("settings_kernel", "Mihomo 内核与特权");
    en.insert("settings_kernel", "Mihomo Kernel &amp; Privileges");
    zh.insert(
        "settings_kernel_desc",
        "原生内核运行路径与 TUN 模式网络权限",
    );
    en.insert(
        "settings_kernel_desc",
        "Native kernel binary path and TUN network capabilities",
    );
    zh.insert("settings_kernel_path", "内核文件路径");
    en.insert("settings_kernel_path", "Kernel Binary Path");
    zh.insert("settings_tun_cap", "TUN 特权 (CAP_NET_ADMIN)");
    en.insert("settings_tun_cap", "TUN Capability (CAP_NET_ADMIN)");
    zh.insert("settings_tun_cap_granted", "已授权 (正常)");
    en.insert("settings_tun_cap_granted", "Granted (Normal)");
    zh.insert("settings_tun_cap_missing", "未授权 (需提权)");
    en.insert("settings_tun_cap_missing", "Missing (Auth Needed)");
    zh.insert("btn_authorize_tun", "一键授权 TUN");
    en.insert("btn_authorize_tun", "Authorize TUN");
    zh.insert("btn_open_config_dir", "打开配置目录");
    en.insert("btn_open_config_dir", "Open Config Folder");
    zh.insert("btn_open_log_dir", "打开日志目录");
    en.insert("btn_open_log_dir", "Open Logs Folder");

    // Geo Database Management
    zh.insert("settings_geo", "Geo 数据库管理");
    en.insert("settings_geo", "Geo Databases");
    zh.insert("settings_geo_desc", "用于地理位置与域名分流的 GeoIP 与 GeoSite 数据库文件");
    en.insert("settings_geo_desc", "GeoIP and GeoSite databases for IP and domain routing");
    zh.insert("btn_update_geo", "一键更新 Geo 数据库");
    en.insert("btn_update_geo", "Update Geo Databases");
    zh.insert("geo_checking", "检测中...");
    en.insert("geo_checking", "Checking...");
    zh.insert("geo_not_installed", "未安装 (缺少数据库文件)");
    en.insert("geo_not_installed", "Not installed (Missing DB files)");
    zh.insert("geo_db_unknown", "未知");
    en.insert("geo_db_unknown", "Unknown");
    zh.insert("geo_db_info", "{size} MB | 更新于: {time}");
    en.insert("geo_db_info", "{size} MB | Updated: {time}");

    zh.insert("settings_about", "关于 Gihomo");
    en.insert("settings_about", "About Gihomo");
    zh.insert("settings_app_id", "应用标识符 (App ID)");
    en.insert("settings_app_id", "Application ID (App ID)");
    zh.insert("settings_version", "版本");
    en.insert("settings_version", "Version");

    // General Settings (Autostart & Tray)
    zh.insert("settings_general", "通用设置");
    en.insert("settings_general", "General");
    zh.insert("settings_general_desc", "开机启动与托盘运行行为");
    en.insert("settings_general_desc", "Startup and system tray behavior");
    zh.insert("settings_autostart", "开机自启动");
    en.insert("settings_autostart", "Start at Login");
    zh.insert(
        "settings_autostart_sub",
        "在登录系统时自动在后台启动 Gihomo",
    );
    en.insert(
        "settings_autostart_sub",
        "Automatically launch Gihomo in background on system login",
    );
    zh.insert("settings_close_to_tray", "关闭时最小化到托盘");
    en.insert("settings_close_to_tray", "Close to System Tray");
    zh.insert(
        "settings_close_to_tray_sub",
        "点击窗口关闭按钮时隐藏至后台托盘，保持代理运行",
    );
    en.insert(
        "settings_close_to_tray_sub",
        "Keep proxy running in system tray when window is closed",
    );
    zh.insert("settings_auto_start_kernel", "启动应用时自动运行内核");
    en.insert("settings_auto_start_kernel", "Auto-start Kernel on Launch");
    zh.insert(
        "settings_auto_start_kernel_sub",
        "检测到有效订阅时，在应用启动后自动运行 Mihomo 内核服务",
    );
    en.insert(
        "settings_auto_start_kernel_sub",
        "Automatically start Mihomo kernel in background when active profile exists",
    );
    zh.insert("settings_auto_restore_proxy", "启动应用时自动恢复代理状态");
    en.insert("settings_auto_restore_proxy", "Auto-restore Proxy on Launch");
    zh.insert(
        "settings_auto_restore_proxy_sub",
        "内核成功运行后，自动恢复上次开启的系统代理或 TUN 模式",
    );
    en.insert(
        "settings_auto_restore_proxy_sub",
        "Automatically restore previous System Proxy or TUN mode once kernel starts",
    );

    // Tray Menu Localizations
    zh.insert("tray_show_window", "打开主界面");
    en.insert("tray_show_window", "Open Dashboard");
    zh.insert("tray_system_proxy", "系统代理");
    en.insert("tray_system_proxy", "System Proxy");
    zh.insert("tray_tun_mode", "TUN 模式");
    en.insert("tray_tun_mode", "TUN Mode");
    zh.insert("tray_proxy_mode", "分流模式");
    en.insert("tray_proxy_mode", "Proxy Mode");
    zh.insert("tray_mode_rule", "规则模式 (Rule)");
    en.insert("tray_mode_rule", "Rule Mode");
    zh.insert("tray_mode_global", "全局模式 (Global)");
    en.insert("tray_mode_global", "Global Mode");
    zh.insert("tray_mode_direct", "直连模式 (Direct)");
    en.insert("tray_mode_direct", "Direct Mode");
    zh.insert("tray_quit", "退出");
    en.insert("tray_quit", "Quit");

    RwLock::new(I18nManager {
        config,
        active_language: active,
        en_dict: en,
        zh_dict: zh,
    })
});

static LANGUAGE_CHANNEL: LazyLock<tokio::sync::broadcast::Sender<()>> =
    LazyLock::new(|| tokio::sync::broadcast::channel(16).0);

/// Retrieve localized string for the given key in the currently active language.
pub fn tr(key: &'static str) -> &'static str {
    let mgr = I18N.read().unwrap();
    let dict = match mgr.active_language {
        ResolvedLanguage::ZhCn => &mgr.zh_dict,
        ResolvedLanguage::EnUs => &mgr.en_dict,
    };

    if let Some(&text) = dict.get(key) {
        text
    } else if let Some(&en_text) = mgr.en_dict.get(key) {
        en_text
    } else {
        key
    }
}

pub fn current_language_config() -> Language {
    I18N.read().unwrap().config.language
}

pub fn current_theme_config() -> ThemeMode {
    I18N.read().unwrap().config.theme
}

pub fn active_resolved_language() -> ResolvedLanguage {
    I18N.read().unwrap().active_language
}

pub fn language_change_receiver() -> tokio::sync::broadcast::Receiver<()> {
    LANGUAGE_CHANNEL.subscribe()
}

pub fn set_language(lang: Language) {
    let mut mgr = I18N.write().unwrap();
    mgr.config.language = lang;
    mgr.active_language = lang.resolved();
    save_config(&mgr.config);
    info!(new_language = ?lang, resolved = ?mgr.active_language, "Language preference updated");

    let _ = LANGUAGE_CHANNEL.send(());
}

/// Helper to translate backend AppEvent notifications if English is active.
pub fn localize_notification(msg: &str) -> String {
    if active_resolved_language() == ResolvedLanguage::ZhCn {
        return msg.to_string();
    }

    match msg {
        "Mihomo 内核已启动" => "Mihomo kernel started".to_string(),
        "Mihomo 内核已重启" => "Mihomo kernel restarted".to_string(),
        "Mihomo 内核已停止" => "Mihomo kernel stopped".to_string(),
        "TUN 模式已开启" => "TUN mode enabled".to_string(),
        "TUN 模式已关闭" => "TUN mode disabled".to_string(),
        "订阅已删除" => "Subscription deleted".to_string(),
        "正在更新 Geo 数据库..." => "Updating Geo databases...".to_string(),
        "Geo 数据库更新完成" => "Geo databases updated successfully".to_string(),
        "已成功断开全部网络连接" => "All network connections closed successfully".to_string(),
        other => {
            if let Some(rest) = other.strip_prefix("已切换至 [") {
                if let Some(mode) = rest.strip_suffix("] 模式") {
                    return format!("Switched to [{}] mode", mode);
                }
            }
            if let Some(rest) = other.strip_prefix("已切换 [") {
                if let Some((group, node)) = rest.split_once("] -> ") {
                    return format!("Switched [{}] -> {}", group, node);
                }
            }
            if let Some(rest) = other.strip_prefix("正在测试 [") {
                if let Some(group) = rest.strip_suffix("] 延迟...") {
                    return format!("Testing [{}] latency...", group);
                }
            }
            if let Some(rest) = other.strip_prefix('[') {
                if let Some(group) = rest.strip_suffix("] 测速完成") {
                    return format!("[{}] Latency test completed", group);
                }
            }
            if let Some(rest) = other.strip_prefix("订阅 [") {
                if let Some(name) = rest.strip_suffix("] 添加成功") {
                    return format!("Subscription [{}] added successfully", name);
                }
                if let Some(name) = rest.strip_suffix("] 更新完成") {
                    return format!("Subscription [{}] update completed", name);
                }
                if let Some(name) = rest.strip_suffix("] 已保存") {
                    return format!("Subscription [{}] saved", name);
                }
            }
            if let Some(rest) = other.strip_prefix("已激活订阅：") {
                return format!("Activated subscription: {}", rest);
            }
            if let Some(rest) = other.strip_prefix("本地配置 [") {
                if let Some(name) = rest.strip_suffix("] 导入成功") {
                    return format!("Local config [{}] imported successfully", name);
                }
                if let Some(name) = rest.strip_suffix("] 重新载入成功") {
                    return format!("Local config [{}] reloaded successfully", name);
                }
            }
            if let Some(rest) = other.strip_prefix("节点配置 [") {
                if let Some(name) = rest.strip_suffix("] 重新载入成功") {
                    return format!("Node config [{}] reloaded successfully", name);
                }
            }
            if let Some(rest) = other.strip_prefix("规则集 [") {
                if let Some(name) = rest.strip_suffix("] 已触发更新") {
                    return format!("Rule provider [{}] update triggered", name);
                }
            }
            if let Some(rest) = other.strip_prefix("已成功导入 ") {
                if let Some((count_str, after)) = rest.split_once(" 个节点到 [") {
                    if let Some(name) = after.strip_suffix(']') {
                        return format!("Successfully imported {} nodes into [{}]", count_str, name);
                    }
                }
            }
            other.to_string()
        }
    }
}

pub fn set_theme(theme: ThemeMode) {
    let mut mgr = I18N.write().unwrap();
    mgr.config.theme = theme;
    save_config(&mgr.config);
    apply_theme(theme);
    info!(new_theme = ?theme, "Theme preference updated");
}

pub fn close_to_tray_config() -> bool {
    I18N.read().unwrap().config.close_to_tray
}

pub fn last_proxy_enabled_config() -> bool {
    I18N.read().unwrap().config.last_proxy_enabled
}

pub fn set_close_to_tray(enabled: bool) {
    let mut mgr = I18N.write().unwrap();
    mgr.config.close_to_tray = enabled;
    save_config(&mgr.config);
    info!(close_to_tray = enabled, "Close to tray preference updated");
}

pub fn auto_start_kernel_config() -> bool {
    I18N.read().unwrap().config.auto_start_kernel
}

pub fn set_auto_start_kernel(enabled: bool) {
    let mut mgr = I18N.write().unwrap();
    mgr.config.auto_start_kernel = enabled;
    save_config(&mgr.config);
    info!(auto_start_kernel = enabled, "Auto start kernel preference updated");
}

pub fn auto_restore_proxy_config() -> bool {
    I18N.read().unwrap().config.auto_restore_proxy
}

pub fn set_auto_restore_proxy(enabled: bool) {
    let mut mgr = I18N.write().unwrap();
    mgr.config.auto_restore_proxy = enabled;
    save_config(&mgr.config);
    info!(auto_restore_proxy = enabled, "Auto restore proxy preference updated");
}

pub fn auto_update_interval_minutes_config() -> u32 {
    I18N.read().unwrap().config.auto_update_interval_minutes
}

pub fn set_auto_update_interval_minutes(minutes: u32) {
    let mut mgr = I18N.write().unwrap();
    if mgr.config.auto_update_interval_minutes != minutes {
        mgr.config.auto_update_interval_minutes = minutes;
        save_config(&mgr.config);
        info!(
            auto_update_interval_minutes = minutes,
            "Auto update interval preference updated"
        );
    }
}

pub fn update_last_proxy_state(proxy_enabled: bool, tun_enabled: Option<bool>) {
    let mut mgr = I18N.write().unwrap();
    mgr.config.last_proxy_enabled = proxy_enabled;
    if let Some(tun) = tun_enabled {
        mgr.config.last_tun_enabled = tun;
    }
    save_config(&mgr.config);
}

pub fn apply_theme(theme: ThemeMode) {
    let style_mgr = adw::StyleManager::default();
    match theme {
        ThemeMode::System => style_mgr.set_color_scheme(adw::ColorScheme::Default),
        ThemeMode::Light => style_mgr.set_color_scheme(adw::ColorScheme::ForceLight),
        ThemeMode::Dark => style_mgr.set_color_scheme(adw::ColorScheme::ForceDark),
    }
}

pub fn init_i18n_and_theme() {
    let theme = current_theme_config();
    apply_theme(theme);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language_indices() {
        assert_eq!(Language::System.to_index(), 0);
        assert_eq!(Language::ZhCn.to_index(), 1);
        assert_eq!(Language::EnUs.to_index(), 2);

        assert_eq!(Language::from_index(0), Language::System);
        assert_eq!(Language::from_index(1), Language::ZhCn);
        assert_eq!(Language::from_index(2), Language::EnUs);
    }

    #[test]
    fn test_theme_mode_indices() {
        assert_eq!(ThemeMode::System.to_index(), 0);
        assert_eq!(ThemeMode::Light.to_index(), 1);
        assert_eq!(ThemeMode::Dark.to_index(), 2);

        assert_eq!(ThemeMode::from_index(0), ThemeMode::System);
        assert_eq!(ThemeMode::from_index(1), ThemeMode::Light);
        assert_eq!(ThemeMode::from_index(2), ThemeMode::Dark);
    }

    #[test]
    fn test_dictionary_completeness() {
        let mgr = I18N.read().unwrap();
        // Ensure all zh keys exist in en dict
        for key in mgr.zh_dict.keys() {
            assert!(
                mgr.en_dict.contains_key(key),
                "Missing English translation for key: {}",
                key
            );
        }
        // Ensure all en keys exist in zh dict
        for key in mgr.en_dict.keys() {
            assert!(
                mgr.zh_dict.contains_key(key),
                "Missing Chinese translation for key: {}",
                key
            );
        }
    }

    #[test]
    fn test_app_config_serde() {
        let config = AppConfig {
            language: Language::ZhCn,
            theme: ThemeMode::Dark,
            close_to_tray: true,
            auto_start_kernel: true,
            auto_restore_proxy: false,
            last_proxy_enabled: true,
            last_tun_enabled: false,
            auto_update_interval_minutes: 60,
        };
        let serialized = serde_json::to_string(&config).unwrap();
        let deserialized: AppConfig = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.language, Language::ZhCn);
        assert_eq!(deserialized.theme, ThemeMode::Dark);
        assert!(deserialized.close_to_tray);
        assert!(deserialized.auto_start_kernel);
        assert!(!deserialized.auto_restore_proxy);
        assert!(deserialized.last_proxy_enabled);
        assert!(!deserialized.last_tun_enabled);
        assert_eq!(deserialized.auto_update_interval_minutes, 60);
    }

    #[test]
    fn test_localize_notification() {
        assert_eq!(localize_notification("Mihomo 内核已启动"), "Mihomo 内核已启动");
        // Test pattern matching logic directly
        assert_eq!(
            localize_notification("已切换至 [rule] 模式"),
            "已切换至 [rule] 模式"
        );
    }

    #[tokio::test]
    async fn test_language_channel_broadcast() {
        let mut rx1 = language_change_receiver();
        let mut rx2 = language_change_receiver();

        let _ = LANGUAGE_CHANNEL.send(());

        assert!(rx1.recv().await.is_ok());
        assert!(rx2.recv().await.is_ok());
    }
}
