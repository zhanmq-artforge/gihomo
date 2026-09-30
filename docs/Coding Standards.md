# Gihomo 编码规范与开发指南 (Coding Standards)

> **产品**：Gihomo (`art.artforge.Gihomo`)  
> **品牌**：ArtForge  
> **适用范围**：Gihomo 工作空间下的所有 Crate 与代码贡献。

---

## 1. 核心设计原则

1. **安全第一 (Safety First)**：默认使用安全 Rust。`unsafe` 仅允许出现在基础设施层的窄范围平台 FFI 中，必须附带 `SAFETY` 注释说明不变量，并检查系统调用错误。
2. **主事件循环零阻塞 (Main Thread Responsiveness)**：GTK4 主循环只负责渲染与轻量级用户事件分发。磁盘 I/O、网络、进程管理及可能阻塞的系统调用必须使用 Tokio 异步接口或专属工作线程；偏好配置应在进入 GTK 主循环前预加载。
3. **架构边界清晰 (Strict Architectural Boundaries)**：纯领域层（`gihomo-core`）严禁反向依赖 UI 框架（`gtk4` / `libadwaita`）或网络驱动（`reqwest` / `tokio`）。
4. **健壮的错误处理 (Robust Error Handling)**：生产代码严禁随意使用 `unwrap()` 或 `expect()`。Core、Infrastructure 和 App 层通过 `thiserror` 错误类型传递错误；UI 层再转换为人类可读提示。
5. **最小权限原则 (Least Privilege)**：客户端主体以普通用户身份运行，仅通过 Linux Capabilities 和 Polkit 机制受控委托系统网络权限。

---

## 2. 模块依赖与边界约束

工作空间采用严格的单向依赖设计：

```text
┌──────────────────────────────────────────────────────────┐
│                      gihomo (bin)                        │
└────────────────────────────┬─────────────────────────────┘
                             ▼
┌──────────────────────────────────────────────────────────┐
│                      gihomo-ui                           │
│              (GTK4, Libadwaita, GIO, i18n)               │
└────────────────────────────┬─────────────────────────────┘
                             ▼
┌──────────────────────────────────────────────────────────┐
│                      gihomo-app                          │
│           (AppService, AppEvent, State Machine)          │
└──────────────┬────────────────────────────┬──────────────┘
               ▼                            ▼
┌──────────────────────────────┐   ┌───────────────────────┐
│         gihomo-core          │   │      gihomo-infra     │
│   (Domain Models, Synthesis) │   │ (Kernel, API, GSettings)│
└──────────────────────────────┘   └───────────────────────┘
```

### 依赖规则矩阵

| Crate | 允许依赖的库 | 严禁依赖的库 | 允许 `unsafe`？ |
|---|---|---|---|
| `gihomo-core` | `serde`, `serde_json`, `serde_yaml`, `thiserror`, `uuid`, `chrono`, `tracing` | `gtk4`, `libadwaita`, `reqwest`, `tokio` | **禁止** |
| `gihomo-infra`| `gihomo-core`, `tokio`, `futures-util`, `reqwest`, `tokio-tungstenite`, `serde`, `serde_json`, `serde_yaml`, `gio`, `glib`, `dirs`, `thiserror`, `tracing`, `async-channel`, `chrono`, `uuid`, `libc` | `gtk4`, `libadwaita`, `gihomo-ui` | Linux FFI only, audited |
| `gihomo-app`  | `gihomo-core`, `gihomo-infra`, `tokio`, `async-channel`, `tracing`, `thiserror`, `serde`, `serde_json`, `chrono`, `uuid`, `glib` | `gtk4`, `libadwaita` | **禁止** |
| `gihomo-ui`   | `gihomo-core`, `gihomo-app`, `gtk4`, `libadwaita`, `glib`, `gio`, `gdk4`, `async-channel`, `tracing`, `chrono`, `serde`, `serde_json`, `ksni`, `tokio`, `dirs` | `gihomo-infra` | **禁止** |

`gihomo-infra` 中的 GSettings 对象必须由其专属 GLib 主上下文线程拥有，不得通过手写 `Send` / `Sync` 实现跨线程共享。`AppEvent` 使用 Tokio broadcast；接收端必须考虑 lagged 消息并重新同步状态。

---

## 3. 代码风格与 Rust 规范

### 3.1 命名规范
* **类型与 Trait**：使用 `UpperCamelCase`（如 `StorageManager`, `ProxyGroup`）。
* **函数与方法**：使用 `snake_case`（如 `check_tun_capabilities`, `merge_subscription_config`）。
* **常量与静态变量**：使用 `SCREAMING_SNAKE_CASE`（如 `APPLICATION_ID`）。
* **异步方法命名**：无需加 `_async` 后缀，直接依靠 `async fn` 关键字区分。

### 3.2 错误处理准则
* 基础设施与领域模块使用 `thiserror` 定义精准的错误枚举（如 `InfraError`, `CoreError`）。
* 错误消息应当具备明确的上下文描述，避免出现模糊的 `"Error occurred"`。
* 只有当逻辑上 100% 确信不可能出错（如解析固定常数字符串）时，才允许使用带有清晰注释的 `expect("why this cannot fail")`。

---

## 4. UI 响应式设计与线程模型

### 4.1 异步任务派发
从 GTK 事件回调触发异步操作时，统一通过 `glib::MainContext::default().spawn_local` 发起：

```rust
let service = service.clone();
btn.connect_clicked(move |_| {
    let service = service.clone();
    glib::MainContext::default().spawn_local(async move {
        if let Err(e) = service.start_kernel().await {
            tracing::error!("Failed to start kernel: {}", e);
        }
    });
});
```

### 4.2 UI 防抖与防反馈锁 (Debounce Guards)
当后端状态通过 `AppEvent` 通知 UI 更新 Switch 或 DropDown 时，必须使用原子标记（如 `Rc<Cell<bool>>`）进行屏蔽，防止触发控件的 `connect_active_notify` 导致反向向后端重复发请求：

```rust
pub fn update_tun_status(&self, enabled: bool) {
    if self.tun_switch.is_active() != enabled {
        self.is_updating_tun.set(true);
        self.tun_switch.set_active(enabled);
        self.is_updating_tun.set(false);
    }
}
```
