# 更新日志 (Changelog)

本项目的每一次重要变更与版本发布均会记录在此文件中。

格式参考自 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.0.0/)，
并且本项目严格遵循 [语义化版本 (Semantic Versioning)](https://semver.org/lang/zh-CN/) 规范。

[English](CHANGELOG.md) | [简体中文](CHANGELOG.zh-CN.md)

---

## [1.0.1] - 2026-09-30

### 🛠️ 维护更新与交互优化

#### 📋 内核日志控制台右键菜单定制
* **精简原生右键菜单**：通过 GTK4 捕获阶段点击手势屏蔽富文本编辑器默认冗杂且不可用的右键菜单项（剪切、粘贴、撤销、重做、删除等），重构为终端控制台专属的极简右键菜单：
  * **复制**：若已划词选中则精确复制选中文本，未选中则默认复制控制台全部日志。
  * **全选**：一键快速选中控制台当前所有日志内容。
  * **清空控制台**：清空日志内存缓存与控制台文本视图。
* **顶栏工具精简**：从顶部工具栏移除独立的清空按钮，彻底杜绝在点击复制按钮时可能产生的误触清空风险。

#### 🌐 设置页面与多语言 (i18n) 增强
* **语言与主题下拉选项净化**：去除语言和主题色彩下拉框中冗余的母语对照括号（如 `(System Default)`、`(Simplified Chinese)`、`(Light)`、`(Dark)`），采用纯净规范的原生术语（跟随系统 / 浅色模式 / 深色模式）。
* **语言重载防递归保护**：为语言下拉框增加 `is_updating_lang` 重入保护，杜绝动态刷新选项模型时触发的循环递归信号。
* **Pango 标记解析修复**：修复由于未转义的 `&` 字符导致 Libadwaita `PreferencesGroup` 和 `ActionRow` 标题解析失败并静默回退中文的问题（`外观与界面语言`、`网络与核心端口`、`Mihomo 内核与特权` 等），确保界面语言切换为英文时完全生效。

## [1.0.0] - 2026-09-30

### 🚀 Gihomo 1.0.0 正式版发布 (General Availability)

Gihomo 是专为 Linux 与 GNOME 桌面环境打造的极致轻量（常驻仅约 200MB 内存）、优雅、现代且高性能的原生 Mihomo (Clash.Meta) 客户端，基于纯 Rust 与 GTK4 / Libadwaita 构建，严格遵循 Clean Architecture（整洁架构）分层设计。

#### 🎨 官方 GNOME HIG / Libadwaita 自适应分栏导航架构
* **GNOME 官方标准导航架构**：主界面基于 `AdwNavigationSplitView` + 独立 `AdwNavigationPage` 架构，根除全局尺寸锁死问题，最小适配宽度下探至 360px。
* **严格居中 HeaderBar 设计**：全部 7 个核心页面（仪表盘、节点、分流、连接、日志、订阅、设置）均内聚专属 `AdwHeaderBar` 并开启 `CenteringPolicy::Strict` 绝对居中策略。
* **顶栏操作收敛与界面降噪**：顶栏操作按钮（刷新、添加订阅、清空/复制日志、断开全部连接等）统一收敛至 `HeaderBar.pack_end`，连接断开图标更新为语义更清晰的断开图标（`network-offline-symbolic`）。
* **全视口自适应体验升级**：内置 `< 720px` 响应式断点（Breakpoint），自动折叠侧边栏并启用 `<` 返回上一级导航；节点列表、分流规则、连接监控及订阅信息在窄屏视口下自动换行或居中重排。

#### 🌐 完整国际化 (i18n) 与免重启即时热重载
* 完整支持 **简体中文 (`zh-CN`)** 与 **English (`en-US`)**，在设置中即时热切换生效，界面即刻刷新无需重启。
* 采用 `tokio::sync::broadcast` 广播事件总线机制，彻底解决多组件争抢事件队列导致的切换失效 Bug。
* 覆盖全项目文本国际化：包括全部页面元素、下拉菜单、Tooltip 浮窗、弹窗对话框以及后台异步通知 Toast 消息的智能映射。

#### ⚡ 内嵌式原生内核监管与退出安全保障
* 基于 Tokio 多线程异步运行时直接监管原生 Mihomo 子进程生命周期，彻底摆脱外部容器与虚拟机依赖。
* 支持内核多级自动发现：打包内置路径 (`/usr/lib/gihomo/bin/mihomo`) $\to$ 用户本地目录 (`~/.local/share/art.artforge.Gihomo/bin/mihomo`) $\to$ 系统 `$PATH`。
* 引入 Linux `PR_SET_PDEATHSIG` 特性与直接 `libc::kill` POSIX 信号绑定，父进程异常终止时子进程自动退出，彻底避免僵尸孤儿进程与端口冲突。
* 统筹 Teardown 清理钩子：应用退出时自动复位 GNOME 系统代理为直连模式，保障系统网络不中断。

#### 🛡️ 免密码极速 TUN 模式与系统代理
* 利用 Linux File Capabilities (`CAP_NET_ADMIN` + `CAP_NET_BIND_SERVICE`)，无需以 root 身份运行客户端即可创建并管理 TUN 虚拟网卡。
* FreeDesktop Polkit 鉴权集成，授权配置 `systemd-resolved` D-Bus 接口接管全局 DNS，彻底免除重复输入 sudo 密码的弹窗打扰。
* 深度集成 GNOME GSettings，一键接管/还原系统全局 HTTP/SOCKS 代理。

#### 📑 三合一多维度配置与订阅管理
* **远程订阅 (Remote URL)**：支持机场订阅异步拉取、ETag 智能缓存、`Subscription-UserInfo` 流量配额解析（上传、下载、总量与到期时间）。
* **单节点与分享链接批量导入 (Share Links)**：纯本地离线解析 Shadowsocks (`ss://`)、VMess (`vmess://`)、VLESS (`vless://` 含 Reality)、Trojan (`trojan://`) 与 Hysteria 2 (`hysteria2://`, `hy2://`)，支持剪贴板自动嗅探与实时渲染预览，自动生成高可用本地 Profile。
* **本地文件导入 (Local YAML)**：支持直接导入既有完整配置文件。
* **定时自动更新调度器**：原生 `adw::ComboRow` 下拉选择周期（不自动更新、30分钟、1/2/6/12/24小时），后台静默定时调度全量订阅增量刷新并热重载。
* **保真深度合成**：自动保留配置中的域名嗅探（sniffer）、自定义 hosts、geox 加速镜像及自定义 DNS 分流策略。

#### 📊 实时仪表盘与流量监控
* WebSocket 高频直连内核控制器流，毫秒级仪表盘呈现上行/下行实时速率仪表盘与会话累计消耗流量。
* 系统托盘图标动态 Tooltip 展示实时网络流速。

#### 🎯 策略组与单节点独立测速
* 实时渲染全部代理策略组（Proxy Groups），支持选择组切换、Fallback 组与 URL-Test 自动测速组状态追踪。
* 细粒度节点测速：不仅支持策略组一键全量并发测速，还支持对列表中任意单节点进行独立延迟测速，提供色彩分级 Badge 反馈（绿色 < 400ms、橙色 400-1000ms、红色超时）。

#### 🔍 分流规则检索与外部规则集管理
* 全局分流规则即时检索：支持按域名后缀、IP-CIDR、GeoIP、目标策略或进程名即时模糊过滤。
* 虚拟滚动性能优化，轻松承载上万条分流规则极速滚动不卡顿。
* 外部规则集（Rule Providers）在线状态检测与一键手动拉取更新。

#### 🌐 实时连接监控与控制 (Connections View)
* 实时追踪系统当前所有活跃的 TCP/UDP 连接会话，统计传输流速与累计流量。
* 深入透视连接元数据：关联源应用进程名、目标域名/IP 与端口、入站类型、命中规则与完整代理链路径。
* 支持连接关键词即时过滤、单连接精确主动关闭，以及带二次警示确认（`adw::AlertDialog`）的全局连接一键断开。

#### 📜 内核实时日志与诊断控制台 (Logs View)
* 双源无缝平滑衔接：启动自动预加载本地历史文件（`mihomo.log` 最近 200 行），无缝衔接 WebSocket 实时推送。
* 支持日志级别分类筛选（全部 / 信息 / 警告 / 错误 / 调试）与关键词即时检索过滤。
* 等宽字体控制台与专业色彩语法高亮，配备锁定滚动到底部（Auto Scroll）、一键复制到剪贴板及清空显示。

#### 🔔 原生 D-Bus 系统托盘与后台常驻守护
* 基于纯 Rust `ksni` 接入 Linux 桌面标准的 StatusNotifierItem (SNI) D-Bus 协议，原生兼容 GNOME (AppIndicator)、KDE Plasma、XFCE 及 Sway。
* 快捷托盘右键菜单：打开主面板、系统代理开关、TUN 模式开关、代理模式切换（规则/全局/直连）及安全退出。
* 单击托盘图标唤醒置顶窗口，关闭窗口自动隐藏至托盘后台常驻（`gio::ApplicationHoldGuard`），支持开机自启（`--minimized`）。

#### 🌍 Geo 数据库管理
* GeoIP 与 GeoSite 数据库存在性、体积与更新时间看板，支持官方及镜像加速源一键在线更新并热重载。
