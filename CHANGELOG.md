# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

[English](CHANGELOG.md) | [简体中文](CHANGELOG.zh-CN.md)

---

## [1.0.2] - 2026-10-01

### ⚡ Architecture, Security, High-Concurrency & UI/UX Evolution

#### 🛡️ Security & Protocol Compliance
* **Local Loopback Controller Binding**: Enforced strict `127.0.0.1` binding for `external-controller`, completely eliminating LAN exposure risks from binding to `0.0.0.0`.
* **HTTP 304 (Not Modified) Subscription Handling**: Properly recognized HTTP 304 responses during subscription updates. Retains existing configuration while updating metadata (ETag, bandwidth quota, timestamp), eliminating redundant kernel reloads and false error reports.
* **Pure Rust Kernel Discovery**: Replaced synchronous child process calls to `which` with `std::env::split_paths`, improving kernel discovery speed and portability across minimal environments without external tools.

#### 🚀 Memory Protection & High-Concurrency Performance
* **Log Console Memory Protection**: Introduced `MAX_BUFFER_LINES = 3000` auto-pruning to eliminate unbounded memory growth during long uptimes; migrated from `Vec` to `VecDeque` for $O(1)$ head eviction; batched incoming WebSocket log lines into `TextBuffer`.
* **Connections Lazy Pagination (Anti-Crash)**: Implemented chunked lazy loading (`CONN_PAGE_SIZE = 80`) with a `Load more connections... (remaining X)` capsule button, preventing GTK4 rendering freezes under high-concurrency workloads (e.g. BT/PT or thousands of open sockets).
* **In-Place Proxy Diff Updates**: Implemented in-place proxy group and node updates when structure matches, preventing scrollbar jumps and UI flickering during automatic or manual latency refreshes.
* **Universal Search Debounce**: Added 200ms debounce across Connections, Rules, and Logs search entries, avoiding continuous regex evaluation and frame drops during rapid typing.
* **Accurate Traffic Rates**: Consumed real-time speed metrics directly from the kernel WebSocket stream, eliminating speed calculation jitter and zero-drop anomalies caused by polling diffs.

#### 🎨 Native GNOME HIG & UI / UX Overhaul
* **Proxies View Architecture Refinement**:
  * Extracted proxy group header into a standalone native header (title, wrapped multi-line subtitle, test latency and refresh buttons) instead of embedding fake controls inside card lists.
  * Standardized proxy group switching using Libadwaita's native `gtk::DropDown`, eliminating hacky custom CSS and restoring system theme consistency.
  * Added single-connection in-place termination in the active connection list.
* **Kernel Logs View Toolbar & One-Click Export**:
  * Rebuilt top controls into a clean dual-row layout:
    * Row 1: Level filter dropdown (`All` / `Info` / `Warning` / `Error` / `Debug`) + keyword search entry.
    * Row 2: Compact icon toolbar with tooltips (`[⬇️ Auto-Scroll Toggle]`, `[📋 Copy]`, `[💾 Export]`, `[🗑️ Clear]`).
  * **One-Click Log Export**: Added direct export to user's download directory (`~/Downloads/gihomo-kernel-YYYYMMDD-HHMMSS.log`) with visual checkmark feedback and Toast confirmation.
  * Replaced copy text with green checkmark feedback animation; decluttered the view by removing redundant context menus.
* **Subscription Dialogs GNOME HIG Modernization**:
  * **Native Adaptive Dialog (`adw::Dialog`)**: Refactored both "Add Subscription" and "Edit Subscription" from fixed-size floating windows (`adw::Window` 520x580) into modern Libadwaita adaptive dialog sheets with `content_width: 360`.
  * **Standard HeaderBar Controls**: Replaced bottom action buttons with standard HeaderBar navigation: `[Cancel]` on the top-left and `[Add]` / `[Save]` on the top-right (with input validation and Enter key submission).
  * **Narrow Screen (360px) Fitting & Truncation Fixes**:
    * Added `ellipsize(End)` to node import guidance labels to prevent single long labels from exceeding the minimum container width.
    * Replaced the text "Paste from Clipboard" button with a compact icon button (`edit-paste-symbolic`) with tooltip.
    * Set `subtitle_lines(1)` on local file path rows to prevent horizontal overflow in narrow mobile/tiled views.

#### 📦 Packaging & Build
* Bumped project version to `1.0.2` across `Cargo.toml`, `Cargo.lock`, and Debian packaging scripts (`scripts/package-deb.sh`).
* Verified clean `.deb` package generation targeting `dist/gihomo_1.0.2_amd64.deb`.

## [1.0.1] - 2026-09-30

### 🛠️ Maintenance & Usability Improvements

#### 📋 Kernel Log Console Context Menu
* **Focused Right-Click Context Menu**: Replaced GTK4 `TextView`'s default bloated editor context menu with a clean, terminal-focused context menu containing:
  * **Copy**: Copies selected text if a selection exists; otherwise copies the entire console log buffer.
  * **Select All**: Selects all text currently displayed in the console buffer.
  * **Clear Console**: Clears in-memory log entries and the active text buffer.
* **Toolbar Streamlining**: Removed the standalone clear button from the top toolbar to prevent accidental clears when aiming for the copy button.

#### 🌐 Settings & Internationalization (i18n)
* **Language & Theme Dropdown Polish**: Removed parenthesized bilingual translation contrasts (`(System Default)`, `(Simplified Chinese)`, `(Light)`, `(Dark)`, `(英语)`), standardizing on clean and modern terminology in both language and color scheme selectors.
* **Safe Language Reloading**: Added `is_updating_lang` re-entrancy protection to eliminate recursive notification signals when updating language models dynamically.
* **Pango Markup Resolution**: Fixed Pango markup parse errors caused by unescaped ampersands (`&`) in `AdwPreferencesGroup` and `AdwActionRow` titles and subtitles (`settings_appearance`, `settings_net_ports`, `settings_kernel`, `settings_mixed_port_sub`, `conn_dst_addr`), allowing these sections to reliably switch to English without GTK markup warnings.

## [1.0.0] - 2026-09-30

### 🚀 Gihomo 1.0.0 Official Release (General Availability)

Gihomo is an ultra-lightweight (~200MB memory footprint), elegant, and high-performance native Mihomo (Clash.Meta) management client built from the ground up in pure Rust and GTK4 / Libadwaita following Clean Architecture principles, tailored specifically for Linux and GNOME desktop environments.

#### 🎨 Official GNOME HIG / Libadwaita Adaptive Split-View Architecture
* **Native Navigation Architecture**: Standard `AdwNavigationSplitView` + individual `AdwNavigationPage` structure following GNOME Settings standards, eliminating width lock and layout constraints down to 360px.
* **Strict Centered HeaderBars**: All 7 functional views (`Dashboard`, `Proxies`, `Rules`, `Connections`, `Logs`, `Subscriptions`, `Settings`) feature an independent `AdwHeaderBar` with `CenteringPolicy::Strict`.
* **Action Relocation & Decluttered Layout**: Consolidated primary action controls into header bars (`pack_end`) and contextual page areas; refreshed disconnect icons (`network-offline-symbolic`) and clean button groups.
* **Adaptive Narrow Layout Down to 360px**: Integrated `AdwBreakpoint` (`< 720px`) with automatic sidebar collapsing, `<` back-button navigation, wrapped metadata rows, and interior `AdwClamp` padding.

#### 🌐 Complete Internationalization (i18n) & Zero-Restart Hot-Switching
* Full bilingual support for **Simplified Chinese (`zh-CN`)** and **English (`en-US`)** with instant zero-restart dynamic switching in Preferences (`Ctrl+,`).
* Broadcast event channel architecture ensuring both main window and system tray synchronize simultaneously without consumer starvation.
* Automatic localization for backend system notifications, toast alerts, node counters, and dynamically formatted subtitles.

#### ⚡ Embedded Native Kernel Supervision & Process Safety
* Direct asynchronous child process supervision powered by Tokio runtime — zero dependencies on containers or virtual machines.
* Multi-tier kernel auto-discovery: Bundled binary (`/usr/lib/gihomo/bin/mihomo`) $\to$ User directory (`~/.local/share/art.artforge.Gihomo/bin/mihomo`) $\to$ System `$PATH`.
* Linux `PR_SET_PDEATHSIG` child protection combined with direct `libc::kill` POSIX signal handling eliminates orphan zombie processes and port conflicts.
* Coordinated exit teardown: automatically resets GNOME system proxy to direct mode upon exit, guaranteeing uninterrupted network connectivity.

#### 🛡️ Zero-Password Seamless TUN Mode & System Proxy
* Leverages Linux file capabilities (`CAP_NET_ADMIN` + `CAP_NET_BIND_SERVICE`) to manage TUN virtual network adapters without root privileges.
* FreeDesktop Polkit integration authorizes `systemd-resolved` D-Bus calls for DNS hijacking without repetitive sudo prompts.
* Live GNOME GSettings synchronization for system-wide HTTP/SOCKS proxy switching.

#### 📑 Three-in-One Multi-Dimensional Profile & Subscription Management
* **Remote Subscriptions**: Asynchronous remote downloading, ETag conditional caching, and `Subscription-UserInfo` traffic quota parsing (upload, download, total, and expiry date).
* **Proxy Share Links Batch Import**: 100% offline local parsing of Shadowsocks (`ss://`), VMess (`vmess://`), VLESS (`vless://` including Reality), Trojan (`trojan://`), and Hysteria 2 (`hysteria2://`, `hy2://`) with automatic clipboard sniffing, live rendering preview, and instant profile generation.
* **Local Profiles**: Direct import and management of local configuration YAML files.
* **Periodic Auto-Update Scheduler**: Native `adw::ComboRow` interval selector (Never, 30m, 1h, 2h, 6h, 12h, 24h) with background silent batch updating and hot-reloading.
* **High-Fidelity Merging**: Preserves inbound configuration, domain sniffer, custom hosts, geox mirrors, and specialized DNS routing directives.

#### 📊 Real-Time Telemetry & Throughput Gauges
* High-frequency WebSocket streaming from Mihomo's controller API with millisecond-grade upstream/downstream gauges and session bandwidth accumulators.
* System tray dynamic tooltip showing live network speed.

#### 🎯 Proxy Groups & Granular Node Latency Probing
* Real-time presentation of all proxy groups (Selector, Fallback, and URL-Test).
* Granular latency testing: group-wide parallel probing and individual node latency tests with color-coded badges (Green < 400ms, Orange 400-1000ms, Red timeout).

#### 🔍 Active Routing Rules Inspection & Rule Providers
* Instant fuzzy search across routing rules by domain suffix, IP-CIDR, GeoIP, policy target, or process name.
* Virtualized list rendering supporting tens of thousands of rules without UI stutter.
* External Rule Providers status tracking and one-click manual update triggers.

#### 🌐 Active Connection Monitoring & Management (Connections View)
* Real-time inspection of active TCP/UDP connection sessions with live speeds and cumulative bandwidth.
* Deep metadata inspection: source process, destination host/IP and port, inbound interface, matched rule, and proxy chains.
* Instant keyword search filtering, single-connection termination, and batch close-all with safe `adw::AlertDialog` confirmation.

#### 📜 Live Kernel Log Stream & Diagnostic Console (Logs View)
* Seamless dual-source logs: preloads recent disk history (`mihomo.log` last 200 lines) and streams live kernel events via WebSocket.
* Multi-level log filtering (All / Info / Warning / Error / Debug) and instant keyword search.
* Monospace console with color-coded syntax highlights, auto-scroll toggle, clear buffer, and one-click clipboard copy.

#### 🔔 Native D-Bus System Tray & Daemon Persistence
* Pure Rust `ksni` StatusNotifierItem (SNI) integration, natively compatible with GNOME Shell (AppIndicator), KDE Plasma, XFCE, and Sway.
* Comprehensive context menu: Open Window, System Proxy toggle, TUN toggle, Proxy Mode switcher (Rule / Global / Direct), and Safe Quit.
* Minimize-to-tray on window close, background daemon persistence (`gio::ApplicationHoldGuard`), and system login autostart (`--minimized`).

#### 🌍 Geo Database Manager
* GeoIP and GeoSite database presence, file size, and timestamp tracker with one-click online mirror updates.
