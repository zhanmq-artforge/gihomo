# Gihomo 国际化 (i18n) 与多语言开发指南

> **适用范围**：Gihomo 原生客户端的所有 UI 视图、对话框、提示信息与系统通知。  
> **基准支持语言**：🇨🇳 简体中文 (`zh-CN`), 🇺🇸 English (`en-US`)。  
> **默认决策策略**：启动自动探测系统 Locale，优先匹配中文，否则回退至英文。

---

## 1. 核心设计原则

1. **零硬编码文案 (Zero Hardcoded User-Facing Strings)**：
   * 所有在界面中向用户呈现的文本（包括窗口标题、栏目名称、按钮文字、Switch 说明、设置选项、Toast 弹窗和系统通知）**一律严禁硬编码**。
   * 所有文本必须统一通过 `crate::i18n::tr("key")` 获取。
   * 仅内部操作标识符（如 action 名称 `"app.preferences"`）、GSettings Schema 键名、日志参数与 CSS 类名允许保留为 ASCII 常量。

2. **即时热重载与无重启刷新 (Live In-Place Hot-Switching)**：
   * 用户在“设置”页中切换语言或主题时，必须**立即在当前进程中生效，毫秒级就地刷新全界面文字，严禁要求用户重启应用程序**。
   * 所有自定义 View 必须提供公开的 `pub fn update_ui_text(&self)` 方法，在语言变更通知广播时由窗口统一调度重绘。

3. **双语对称性 (Bilingual Parity)**：
   * 任何在 `crates/gihomo-ui/src/i18n.rs` 中新增的键，**必须同时在 `zh` 与 `en` 两个 HashMap 中注册**。

---

## 2. 键名命名规范 (Key Naming Conventions)

所有国际化键名统一使用小写 `snake_case`，并携带清晰的模块前缀：

| 键名前缀 | 覆盖作用域 | 示例 |
|---|---|---|
| `app_*` | 应用名称、全局副标题、退出提示 | `app_name`, `app_subtitle` |
| `tab_*` | 主侧边栏/导航视图 Tab 项 | `tab_dashboard`, `tab_proxies`, `tab_subscriptions`, `tab_settings` |
| `ctrl_*` | 控制面板卡片分组与标题 | `ctrl_group_title`, `ctrl_group_desc` |
| `sys_proxy_*`| 系统代理 Switch 及说明 | `sys_proxy_title`, `sys_proxy_sub` |
| `tun_*` | TUN 模式 Switch 及说明 | `tun_title`, `tun_sub` |
| `kernel_*` | 内核运行状态与控制按钮 | `kernel_status_title`, `kernel_running`, `kernel_stopped` |
| `traffic_*`| 实时流量监控分组与文字 | `traffic_title`, `realtime_up`, `realtime_down` |
| `sub_*` | 订阅列表、详情、配额与按钮 | `sub_add_title`, `sub_refresh`, `sub_expire` |
| `settings_*`| 设置页各项设置标题与状态 | `settings_lang`, `settings_theme`, `settings_tun_cap` |
| `btn_*` | 动作按钮通用标签 | `btn_start`, `btn_stop`, `btn_restart`, `btn_authorize_tun` |
| `err_*` | 业务错误提示文案 | `err_kernel_start_failed`, `err_invalid_yaml` |

---

## 3. 新增国际化文本的开发流程

### 第一步：在 `crates/gihomo-ui/src/i18n.rs` 中注册词条
```rust
// 简体中文
zh.insert("my_feature_title", "我的新特性");

// 英文
en.insert("my_feature_title", "My New Feature");
```

### 第二步：在 Widget 构建时应用文本
```rust
let label = Label::builder()
    .label(tr("my_feature_title"))
    .build();
```

### 第三步：在 `update_ui_text` 中支持热重载
```rust
pub fn update_ui_text(&self) {
    self.my_label.set_label(tr("my_feature_title"));
}
```
