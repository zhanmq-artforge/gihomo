# Contributing to Gihomo (贡献指南)

[English](#english) | [简体中文](#简体中文)

---

<a name="english"></a>
## English

Thank you for your interest in contributing to **Gihomo**! We welcome contributions of all kinds: bug reports, documentation updates, feature suggestions, and code contributions.

### 1. Prerequisites & Environment Setup

Gihomo is built with **Rust (2021 edition)** and native **GTK4 / Libadwaita**.

#### Debian / Ubuntu (22.04+)
```bash
sudo apt-get update
sudo apt-get install -y \
  build-essential \
  pkg-config \
  libgtk-4-dev \
  libadwaita-1-dev \
  libglib2.0-dev \
  libssl-dev \
  libpolkit-gobject-1-dev \
  libcap2-bin \
  desktop-file-utils \
  appstream
```

#### Fedora / RHEL
```bash
sudo dnf install -y \
  gcc \
  pkg-config \
  gtk4-devel \
  libadwaita-devel \
  glib2-devel \
  openssl-devel \
  polkit-devel \
  libcap-devel \
  desktop-file-utils \
  appstream
```

#### Arch Linux / Manjaro
```bash
sudo pacman -Syu --needed \
  base-devel \
  gtk4 \
  libadwaita \
  glib2 \
  openssl \
  polkit \
  libcap \
  desktop-file-utils \
  appstream
```

---

### 2. Development Workflow

1. **Fork and Clone**:
   ```bash
   git clone https://github.com/your-username/gihomo.git
   cd gihomo
   git checkout -b feature/your-feature-name
   ```

2. **Run and Debug**:
   ```bash
   # Run in development mode
   cargo run

   # Run tests
   cargo test --workspace
   ```

3. **Code Quality Gates**:
   Before submitting your changes, ensure your code satisfies all project baselines:
   ```bash
   # Format code
   cargo fmt

   # Ensure no clippy warnings
   cargo clippy --workspace --all-targets -- -D warnings

   # Run test suite
   cargo test --workspace
   ```

4. **Commit Message Guidelines**:
   We follow [Conventional Commits](https://www.conventionalcommits.org/):
   - `feat:` A new user-facing feature
   - `fix:` A bug fix
   - `docs:` Documentation-only changes
   - `perf:` Performance improvements
   - `refactor:` Code restructuring without behavior changes
   - `style:` Formatting, missing semicolons, etc.
   - `test:` Adding or updating tests
   - `ci:` CI/CD pipeline changes

---

### 3. Architecture & Design Rules

Please read the design specifications in `docs/` before making architectural changes:
- [`docs/Gihomo Architecture.md`](docs/Gihomo%20Architecture.md) — Clean Architecture layers and dependency directions.
- [`docs/Coding Standards.md`](docs/Coding%20Standards.md) — Thread safety, memory protection, and UI responsive rules.
- [`docs/Internationalization Guidelines.md`](docs/Internationalization%20Guidelines.md) — Bilingual dictionary standards.

---

<a name="简体中文"></a>
## 简体中文

感谢你对 **Gihomo** 项目的关注与支持！我们欢迎各种形式的贡献，包括问题反馈、文档完善、功能提议以及代码改进。

### 1. 开发环境准备

Gihomo 基于 **Rust (2021 edition)** 和原生的 **GTK4 / Libadwaita** 构建。

在不同 Linux 发行版上安装所需编译依赖：
- **Ubuntu / Debian**：参考上文安装 `libgtk-4-dev`、`libadwaita-1-dev` 等。
- **Fedora**：参考上文安装 `gtk4-devel`、`libadwaita-devel` 等。
- **Arch Linux**：参考上文安装 `gtk4`、`libadwaita` 等。

### 2. 本地开发与提交流程

1. **创建分支**：从最新的 `main` 分支拉出开发分支，如 `git checkout -b feat/my-new-feature`。
2. **本地调试与测试**：
   ```bash
   # 本地调试运行
   cargo run

   # 运行全量单元测试
   cargo test --workspace
   ```
3. **提交前门禁自检**（必测）：
   ```bash
   # 代码自动格式化
   cargo fmt

   # 严格静态检查（保证零 Warning）
   cargo clippy --workspace --all-targets -- -D warnings

   # 单元测试全绿
   cargo test --workspace
   ```
4. **提交规范**：遵循标准语义化提交（如 `feat:`, `fix:`, `docs:`, `perf:` 等）。

### 3. 架构设计原则
进行核心逻辑改动前，请务必参阅 [`docs/Gihomo Architecture.md`](docs/Gihomo%20Architecture.md) 与 [`docs/Coding Standards.md`](docs/Coding%20Standards.md)，遵守领域模型不依赖 UI、单向异步事件总线及最小权限原则。
