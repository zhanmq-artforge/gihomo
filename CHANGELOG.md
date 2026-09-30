# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

[English](CHANGELOG.md) | [简体中文](CHANGELOG.zh-CN.md)

---

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
