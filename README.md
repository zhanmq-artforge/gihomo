# Gihomo (原生 Mihomo 桌面客户端)

<div align="center">

[![Rust](https://img.shields.io/badge/rust-stable-brightgreen.svg)](https://www.rust-lang.org/)
[![GTK4](https://img.shields.io/badge/GTK4-4.12+-blue.svg)](https://gtk.org/)
[![libadwaita](https://img.shields.io/badge/libadwaita-1.5+-purple.svg)](https://gnome.pages.gitlab.gnome.org/libadwaita/)
[![Mihomo](https://img.shields.io/badge/Mihomo-v1.19+-orange.svg)](https://github.com/MetaCubeX/mihomo)
[![License: GPL-3.0](https://img.shields.io/badge/License-GPL_3.0-blue.svg)](LICENSE)

**A modern native Libadwaita & GTK4 client for Mihomo on Linux and GNOME.**  
**适用于 Linux 和 GNOME 桌面环境的现代化原生 GTK4 + Libadwaita Mihomo (Clash.Meta) 管理客户端。**

[English](#english) | [简体中文](#简体中文)

</div>

---

<a name="english"></a>
## 🇬🇧 English

### ✨ Key Features

* **⚡ Native Embedded Mihomo Kernel Supervision**:
  * Direct process supervision powered by Tokio asynchronous runtime — zero containers or virtualization dependencies.
  * Multi-tier pure Rust kernel discovery (`std::env::split_paths`): Bundled system path (`/usr/lib/gihomo/bin/mihomo`) $\to$ User local path (`~/.local/share/art.artforge.Gihomo/bin/mihomo`) $\to$ System `$PATH`.
  * Strict `127.0.0.1` local loopback controller binding to eliminate unauthorized LAN access risks.
  * Real-time child process lifecycle management with automatic PID file tracking and clean termination on exit.
* **🛡️ Zero-Password Seamless TUN Mode**:
  * Linux file capabilities (`CAP_NET_ADMIN` + `CAP_NET_BIND_SERVICE`) allow running TUN interfaces without root privileges.
  * FreeDesktop Polkit integration (`/usr/share/polkit-1/rules.d/art.artforge.Gihomo.rules`) authorizes `systemd-resolved` D-Bus configuration for DNS hijacking, completely eliminating repeated sudo password popups.
* **🎨 Native GNOME & Ubuntu Settings User Experience**:
  * Built strictly on GTK4 and Libadwaita 1.5+ adhering to GNOME Human Interface Guidelines (HIG).
  * Ubuntu Settings / GNOME Control Center adaptive navigation split view (`AdwNavigationSplitView`) with responsive sidebar collapse on compact screens.
  * **Frequency-tiered sidebar navigation** with native 1px dividers: Tier 1 (Dashboard, Proxies, Subscriptions), Tier 2 (Connections, Rules, Logs), Tier 3 (Settings).
  * Rich visual feedback with animated loading spinners, debounced operations, and status toasts.
  * System proxy synchronization directly via GNOME GSettings (`org.gnome.system.proxy`).
  * Dark / Light / Follow-System color scheme support.
* **🌐 Live In-Place i18n Hot-Switching**:
  * Comprehensive bilingual support for **Simplified Chinese (`zh-CN`)** and **English (`en-US`)**.
  * Auto-detects system locale on startup with graceful English fallback.
  * Settings view with **instant zero-restart live language and theme switching**.
* **📊 Real-time Telemetry & Traffic Monitoring**:
  * High-frequency WebSocket streaming from Mihomo's controller API consuming real-time instant speeds without jitter.
  * Real-time upstream and downstream throughput gauges and session bandwidth accumulators.
* **🔔 Native System Tray & Background Daemon Integration**:
  * Pure Rust D-Bus StatusNotifierItem (SNI) integration via `ksni`, fully compatible with GNOME Shell (AppIndicator), KDE Plasma, XFCE, and Sway.
  * Comprehensive context menu: Open Dashboard, **Quick Subscription Switcher** (dynamic submenu with live active checkmark `✓`), System Proxy toggle, TUN toggle, Proxy Mode switcher (Rule / Global / Direct), and Quit.
  * Left-click tray icon to instantly present the window, real-time status tooltip displaying current active subscription, system proxy, TUN, and routing mode.
  * Close-to-tray window minimization, background daemon lifecycle persistence (`gio::ApplicationHoldGuard`), and system login autostart (`--minimized`).
* **🚀 Ultra-Lightweight Native Resource Footprint**:
  * Native Mihomo kernel ~60MB RAM + Native GTK4/Libadwaita UI ~170MB RAM $\approx$ **~230MB total memory consumption** (only ~1/3 to 1/4 of Chromium/Electron-based proxy clients consuming 700–800MB+).
* **🎯 Active Routing Rules Inspection & Real-Time Search**:
  * Dedicated **Rules View** with 200ms debounced instant keyword search across active rules (by domain suffix, IP-CIDR, GeoIP, process name, or target proxy).
  * Color-coded badges for quick identification of rule types and target policies (`DIRECT`, `REJECT`, `PROXY`).
  * External Rule Providers management with one-click upstream update triggers.
  * Optimized list virtualization supporting thousands of rules without UI stutter.
* **⚡ Native Proxy Selection & Granular Latency Testing**:
  * Full-width dedicated action toolbar (`hexpand(true)`) with instant keyword search, batch ping (`⚡`), and reload (`🔄`), completely eliminating layout shift.
  * Breakpoint-aware adaptive layout (`max-width: 560px`): wraps current node badges under group name, node metadata under node name, eliminating overflow on compact screens.
  * Standardized Libadwaita `gtk::DropDown` selector for proxy groups, free of custom CSS hacks.
  * In-place diff updates: refreshes latency badges and selections without scroll jumps or list re-rendering.
  * Individual on-demand node latency probing with dedicated ping buttons and color-graded badges (Green < 400ms, Orange 400-1000ms, Red > 1000ms / Timeout).
* **🌍 GeoIP & GeoSite Database Management**:
  * Built-in Geo database manager inspecting file presence, file size, and last updated timestamps.
  * One-click upstream database updater pulling directly from official/mirror sources with automatic kernel reload.
* **🛡️ Hardened Exit Safety & Process Supervision**:
  * Linux `PR_SET_PDEATHSIG` child process protection ensures the Mihomo kernel terminates immediately if the parent process exits, preventing orphan processes and port conflicts.
  * Coordinated teardown hook on exit resets GNOME system proxy to direct mode, eliminating the risk of network loss after closing the client.
* **📑 Adaptive Subscription Dialogs & Configuration Merging**:
  * Multi-subscription management with remote URL download, ETag & HTTP 304 caching, and local YAML storage.
  * **GNOME HIG Adaptive Dialogs**: Add and Edit subscriptions powered by modern `adw::Dialog` with compact 360px layout, standard HeaderBar navigation (`[Cancel]` / `[Add]` / `[Save]`), and Enter-key submission.
  * Native offline parsing of proxy share links (`ss://`, `vmess://`, `vless://`, `trojan://`, `hysteria2://`) with batch import, live preview, and automatic clipboard detection.
  * Background periodic auto-refresh scheduler with native preset intervals (Never, 30m, 1h, 2h, 6h, 12h, 24h).
  * Subscription traffic quota parsing via `Subscription-UserInfo` headers (upload, download, total, and expiry date).
  * Structural deep-merging that preserves high-end directives like `sniffer`, `hosts`, `geox-url`, and custom DNS policies.
* **🔍 Real-Time High-Concurrency Connection Monitoring**:
  * Anti-crash chunked pagination (`CONN_PAGE_SIZE = 80`) with `[Load more connections... (remaining X)]` capsule button, effortlessly handling thousands of concurrent connections.
  * Detailed TCP/UDP active connection tracking with cumulative traffic and live speeds.
  * Comprehensive metadata inspection: host, destination IP/port, process name, inbound interface, rule chain, and proxy path.
  * Real-time search filtering with 200ms debounce, in-place single connection termination, and close-all with `adw::AlertDialog` confirmation.
* **📜 High-Performance Kernel Log Stream & Diagnostic Console**:
  * Seamless dual-source logs: preloads recent disk history (`mihomo.log`) and streams live events via WebSocket with $O(1)$ circular queue buffer (3,000-line memory protection).
  * Clean dual-row top toolbar:
    * Row 1: Level filter (`All` / `Info` / `Warning` / `Error` / `Debug`) and 200ms debounced keyword search.
    * Row 2: Compact icon buttons with tooltips for Auto-Scroll toggle, Copy (with checkmark feedback), One-Click Export to Downloads, and Clear Console.
  * **One-Click Download/Export**: Exports full logs directly to `~/Downloads/gihomo-kernel-YYYYMMDD-HHMMSS.log` with instant Toast confirmation.

---

### 🏛️ Architecture Overview

The repository is organized following Clean Architecture principles as a decoupled Cargo workspace:

* **`src/main.rs`** — Application binary entrypoint, multi-threaded Tokio runtime bootstrap, Unix signal handling, and clean shutdown coordination.
* **[`crates/gihomo-core`](crates/gihomo-core)** — Pure domain layer: domain models (`Subscription`, `ProxyGroup`, `RuleItem`, `GeoDatabaseInfo`, `KernelStatus`), configuration templates, and YAML merger (zero GUI dependencies).
* **[`crates/gihomo-infra`](crates/gihomo-infra)** — Infrastructure adapters: native Mihomo process supervisor (`KernelManager`), REST/WebSocket API client (`MihomoApiClient`), GNOME GSettings proxy controller (`SystemProxyManager`), and local filesystem storage (`StorageManager`).
* **[`crates/gihomo-app`](crates/gihomo-app)** — Application service layer: coordinator service (`AppService`), reactive state machine, and asynchronous broadcast event bus (`AppEvent`).
* **[`crates/gihomo-ui`](crates/gihomo-ui)** — Presentation layer: Libadwaita native views (`DashboardView`, `ProxiesView`, `RulesView`, `ConnectionsView`, `LogsView`, `SubscriptionsView`, `SettingsView`), responsive navigation split view, i18n manager, and theme engine.

---

### 📦 Prerequisites & System Dependencies

On Ubuntu / Debian:

```bash
sudo apt update && sudo apt install -y \
    build-essential \
    pkg-config \
    libgtk-4-dev \
    libadwaita-1-dev \
    libglib2.0-dev \
    libcap2-bin
```

On Fedora:

```bash
sudo dnf install -y \
    gcc \
    pkgconf-pkg-config \
    gtk4-devel \
    libadwaita-devel \
    glib2-devel \
    libcap
```

On Arch Linux:

```bash
sudo pacman -S --needed \
    base-devel \
    pkgconf \
    gtk4 \
    libadwaita \
    glib2 \
    libcap
```

---

### 🚀 Build, Test & Run

```bash
# 1. Check code quality
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check

# 2. Run unit tests
cargo test --workspace

# 3. Launch Gihomo in development mode
cargo run
```

---

### 📦 Distribution Packaging

#### Debian / Ubuntu (`.deb`)
```bash
./scripts/package-deb.sh
# Generated artifact: dist/gihomo_1.1.0_amd64.deb
sudo dpkg -i dist/gihomo_1.1.0_amd64.deb
```

#### Fedora / RHEL (`.rpm`)
```bash
./scripts/package-rpm.sh
# Or using cargo-generate-rpm:
cargo install cargo-generate-rpm
cargo generate-rpm
```

---

### 📋 Documentation & Changelog

* 📋 [Release Changelog](CHANGELOG.md) | [更新日志](CHANGELOG.zh-CN.md)
* 📐 [Architecture Documentation](docs/Gihomo%20Architecture.md)
* 📝 [Coding Standards](docs/Coding%20Standards.md)
* 🌐 [Internationalization Guidelines](docs/Internationalization%20Guidelines.md)

---

<a name="简体中文"></a>
## 🇨🇳 简体中文

### ✨ 核心功能亮点

* **⚡ 原生嵌入式 Mihomo 内核进程直接驱动**：
  * 基于 Tokio 异步多线程驱动，纯原生子进程直接托管与生命周期监管，彻底告别容器依赖与虚拟化开销；
  * 纯 Rust 内核发现引擎（`std::env::split_paths`，免除外部 `which` 子进程）：安装包内置路径（`/usr/lib/gihomo/bin/mihomo`） $\to$ 用户私有路径（`~/.local/share/art.artforge.Gihomo/bin/mihomo`） $\to$ 系统全局 `$PATH`；
  * 控制器强制绑定 `127.0.0.1` 本地安全回环，彻底消除局域网未授权控制风险。
* **🛡️ 免密码极速全局 TUN 模式**：
  * 内核二进制预置 `CAP_NET_ADMIN` 与 `CAP_NET_BIND_SERVICE` 文件权能，无需 root 提权即可操作内核虚拟网卡；
  * 集成标准 FreeDesktop Polkit 策略文件（`/usr/share/polkit-1/rules.d/art.artforge.Gihomo.rules`），授权 `systemd-resolved` D-Bus 接口配置 DNS 劫持，**彻底终结开启 TUN 时的多次密码弹窗困扰**。
* **🎨 GNOME 与 Ubuntu 系统设置原生风格**：
  * 深度遵循 GNOME 人机交互指南（HIG），采用 Ubuntu 系统设置风格自适应侧边栏分栏（`AdwNavigationSplitView`）；
  * **高频优先的侧边栏功能梯队**：以原生 1px 细线优雅隔离三大使用梯队（核心日常：仪表盘/节点代理/订阅管理；诊断排查：连接监控/分流规则/实时日志；通用：应用设置）；
  * 完美适配小尺寸视口：窄屏下自动收缩为单列页面导航并在顶栏提供原生 `< 返回` 按钮，宽屏下展开双栏；
  * 丰富生动的交互状态反馈：内核启停动态 Spinner 指示器、订阅刷新微动画、节点测速状态切换与操作防抖锁定；
  * 深度集成 GNOME GSettings（`org.gnome.system.proxy`），一键同步系统 HTTP/Socks 代理设置；
  * 原生支持深色、浅色与跟随系统主题切换。
* **🎯 实时分流规则检索与规则集（Rule Providers）管理**：
  * 独立“分流规则”视图，支持 200ms 防抖的毫秒级域名、IP-CIDR、GeoIP、目标策略关键字过滤检索；
  * 规则类型与策略指向颜色徽章分级（`DIRECT` 绿色、`REJECT` 红色、`PROXY` 品牌蓝）；
  * 外部规则集（Rule Providers）状态追踪与一键手动拉取更新；
  * 采用分段虚拟渲染优化，轻松承载上万条分流规则极速滚动不卡顿。
* **⚡ 原生节点选择与细粒度延迟测速**：
  * 通栏自适应操作工具栏：搜索框、批量测速 `[⚡]` 与刷新 `[🔄]` 独占一行且等宽自适应（`hexpand: true`），消除拉伸窗口时的跳行动画；
  * 窄屏断点自适应下沉（`≤ 560px`）：策略组已选节点及单节点元数据自动换行下沉至标题正下方并精准缩进，360px 视口下长文本末尾智能省略，彻底杜绝内容截断挤压；
  * 策略组切换采用原生 Libadwaita `gtk::DropDown` 下拉控件，彻底移除多余自定义 CSS，体验高度贴合 GNOME 桌面；
  * 原地增量刷新（In-Place Diff Update）：节点列表刷新与测速结果更新时原地更新数据，不重建控件，彻底消除滚动条跳动与界面闪烁；
  * 节点列表中每个节点均配备专属独立即时测速图标按钮，提供色彩分级 Badge 反馈（绿色 < 400ms、橙色 400-1000ms、红色 > 1000ms 或超时）。
* **🌍 GeoIP 与 GeoSite 数据库管理**：
  * “设置”界面内置 Geo 数据库看板，清晰显示数据库存在状态、文件体积与最后更新时间；
  * 支持官方源及镜像加速一键在线更新，更新后自动触发内核配置热重载。
* **🛡️ 退出安全清理与子进程孤儿防护**：
  * 引入 Linux `PR_SET_PDEATHSIG` 特性，父进程异常终止时子进程自动收到 SIGTERM 退出，绝除僵尸进程与端口占用冲突；
  * 挂接应用退出、窗口关闭与系统 SIGINT/SIGTERM 信号的统筹清理（Teardown hook），自动将系统代理复位为直连模式，防止用户断网。
* **🔔 原生系统托盘与常驻后台守护**：
  * 基于纯 Rust `ksni` 库接入 Linux 现代桌面标准的 StatusNotifierItem (SNI) D-Bus 协议，原生兼容 GNOME (AppIndicator)、KDE Plasma、XFCE 及 Sway 等桌面；
  * **完备托盘菜单**：打开主界面、**一键快速切换订阅**（动态子菜单实时标识当前激活订阅 `✓`，后台即时静默重载）、系统代理开关、TUN 模式开关、分流模式切换（规则/全局/直连）及安全退出；
  * 鼠标左键点击托盘图标即时唤醒置顶窗口，悬浮提示 Tooltip 实时呈现当前生效订阅与网络状态；
  * 支持关闭窗口时最小化至系统托盘、后台常驻守护（`gio::ApplicationHoldGuard`）与开机自启动（`--minimized`）。
* **🚀 极致原生轻量低资源开销**：
  * 原生 Mihomo 内核进程内存仅约 60MB + 原生 GTK4/Libadwaita 界面仅约 170MB $\approx$ **常驻总内存约 230MB**（仅为 Electron/Chromium 类客户端动辄 700–800MB 内存的 1/3 ~ 1/4）。
* **🌐 毫秒级即时热重载国际化 (i18n)**：
  * 完整支持 **简体中文 (`zh-CN`)** 与 **English (`en-US`)**；
  * 启动自动侦测系统 Locale，“设置”页面支持**即时热重载**，切换后界面毫秒级无感刷新，无需重启程序。
* **📊 实时流量与网络遥测**：
  * WebSocket 实时直连内核事件流，毫秒级直接呈现上下行瞬时速率，消除速率抖动与当次累计消耗流量。
* **📑 现代自适应订阅管理与配置保真深度合成**：
  * 支持多订阅托管、远程链接异步抓取、ETag 与 HTTP 304 智能缓存（保留本地配置免去无谓内核重载）与本地 YAML 存储；
  * **GNOME HIG 原生自适应弹窗**：添加/编辑订阅升级为现代化 `adw::Dialog` 浮层设计，完美适配 360px 极窄视口，HeaderBar 左取消右确认，回车快捷提交；
  * 纯本地离线解析单节点分享链接（`ss://`, `vmess://`, `vless://`, `trojan://`, `hysteria2://`），支持多行批量导入、实时解析预览与智能剪贴板感知；
  * 后台定时自动增量更新调度器，提供原生下拉周期选项（不自动更新、30分钟、1/2/6/12/24小时）；
  * 深度逆向合并策略完整保留订阅中的 `sniffer` 域名嗅探、`hosts` 自定义解析、`geox-url` 加速源与自定义分流 DNS；
  * 自动解析机场 `Subscription-UserInfo` 响应头（已用上传、已用下载、总流量配额与过期时间）。
* **🔍 高并发连接监控与防爆分页**：
  * 引入 `CONN_PAGE_SIZE = 80` 分批懒加载机制与 `[加载更多连接... (剩余 X 条)]` 胶囊按钮，轻松应对 BT/PT 与海量并发连接，杜绝界面渲染卡死；
  * 详尽的 TCP/UDP 活跃连接实时追踪，统计上下行累计流量与瞬时传输速率；
  * 深度元数据透视：域名主机、目标 IP/端口、关联源进程、入站类型、匹配分流规则及代理节点链；
  * 支持 200ms 防抖关键词即时模糊过滤、单连接就地针对性断开与二次警示确认的全量连接一键清空。
* **📜 实时内核日志流与诊断控制台**：
  * 双源日志无缝衔接：启动自动预加载本地历史日志文件（`mihomo.log` 最近200行）并平滑衔接 WebSocket 实时日志流，引入 3000 行环形队列（$O(1)$ 出队）内存防爆机制；
  * 规范的双行顶栏布局：
    * 第一行：日志级别下拉筛选（全部 / 信息 / 警告 / 错误 / 调试）与 200ms 防抖关键字检索；
    * 第二行：纯图标按钮栏（带悬停 Tooltip）：滚动锁定切换、复制（带打勾动效）、一键导出到下载目录、清空控制台；
  * **一键导出至下载目录**：一键将控制台全部日志导出保存至用户下载目录（`~/Downloads/gihomo-kernel-YYYYMMDD-HHMMSS.log`），伴随 Toast 即时反馈。

---

### 🏛️ 架构分层

* **`src/main.rs`** — 应用程序二进制入口、Tokio 多线程执行环境初始化、Unix 系统信号监听与优雅退出统筹。
* **[`crates/gihomo-core`](crates/gihomo-core)** — 纯领域层：领域实体（`Subscription`、`ProxyGroup`、`RuleItem`、`GeoDatabaseInfo`、`KernelStatus`）、配置模板与 YAML 智能合成引擎（与界面完全解耦）。
* **[`crates/gihomo-infra`](crates/gihomo-infra)** — 基础设施适配层：原生内核进程监管（`KernelManager`）、REST 与 WebSocket API 客户端（`MihomoApiClient`）、GNOME GSettings 代理驱动（`SystemProxyManager`）与本地文件持久化（`StorageManager`）。
* **[`crates/gihomo-app`](crates/gihomo-app)** — 应用服务协调层：中央控制器（`AppService`）、响应式状态机与跨线程异步广播事件总线（`AppEvent`）。
* **[`crates/gihomo-ui`](crates/gihomo-ui)** — 原生展示层：Libadwaita 原生视图（`DashboardView`、`ProxiesView`、`RulesView`、`ConnectionsView`、`LogsView`、`SubscriptionsView`、`SettingsView`）、i18n 国际化引擎与主题调度器。

---

### 📚 项目核心文档

* 📋 [版本更新日志 (Changelog)](CHANGELOG.zh-CN.md) | [English](CHANGELOG.md)
* 📐 [系统架构设计规范 (Gihomo Architecture)](docs/Gihomo%20Architecture.md)
* 📝 [工程编码规范 (Coding Standards)](docs/Coding%20Standards.md)
* 🌐 [国际化 (i18n) 开发指南 (Internationalization Guidelines)](docs/Internationalization%20Guidelines.md)

---

## 📄 License (开源协议)

This project is licensed under the [GNU General Public License v3.0](LICENSE).  
本项目采用 GPL-3.0 开源协议。
