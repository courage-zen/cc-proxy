# cc-proxy 无 UI 化实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 删除 React 前端，将 Tauri 应用改造为纯 Rust CLI 工具，所有配置通过 YAML 文件管理。

**Architecture:** 保持现有代理核心逻辑（Axum HTTP 服务器、Provider 路由、failover、circuit breaker），删除 Tauri 命令层和前端，添加 clap CLI 层，用 YAML 替代 SQLite 存储。

**Tech Stack:** Rust, clap, serde_yaml, tracing (stdout), tokio, axum

---

## 文件结构变更概览

```
删除:
  src/                              # 整个 React 前端
  src-tauri/src/commands/            # 所有 Tauri IPC 命令
  src-tauri/src/tray.rs             # 托盘图标
  src-tauri/src/lightweight.rs       # 轻量模式
  src-tauri/src/deeplink/            # Deep link 整个目录
  src-tauri/src/app_store.rs         # 依赖 AppHandle
  src-tauri/src/usage_script.rs      # 依赖前端
  src-tauri/capabilities/            # Tauri 权限配置

新建:
  src-tauri/src/cli.rs               # clap 子命令定义
  src-tauri/src/yaml_store.rs        # config.yaml 读写
  src-tauri/src/cli_config.rs         # CLI 配置类型（与 yaml_store 配合）

改造:
  src-tauri/src/main.rs              # 替换为 clap CLI 入口
  src-tauri/src/lib.rs               # 删除 Tauri 窗口/前端初始化
  src-tauri/src/proxy/server.rs      # AppHandle → 日志回调
  src-tauri/src/proxy/failover_switch.rs  # AppHandle → 日志
  src-tauri/src/proxy/forwarder.rs   # AppHandle → 日志
  src-tauri/src/services/proxy.rs     # AppHandle → 移除
  src-tauri/src/services/webdav_auto_sync.rs  # Emitter → 移除
  src-tauri/Cargo.toml               # 移除 tauri/tauri-plugin-*，添加 clap
```

---

## Task 1: 删除前端代码

**Files:**
- Delete: `src/` (整个目录)
- Delete: `src-tauri/src/commands/` (整个目录)
- Delete: `src-tauri/src/tray.rs`
- Delete: `src-tauri/src/lightweight.rs`
- Delete: `src-tauri/src/deeplink/` (整个目录)
- Delete: `src-tauri/src/app_store.rs`
- Delete: `src-tauri/src/usage_script.rs`
- Delete: `src-tauri/capabilities/` (整个目录)
- Delete: `src-tauri/icons/` (Tauri tray 图标，前端已删)
- Delete: `package.json` (或清空 src/ 相关内容)

- [ ] **Step 1: 删除 src/ 前端目录**

```bash
rm -rf /Users/zhengyong/code/cc-proxy-main/src
```

- [ ] **Step 2: 删除 commands/ 目录**

```bash
rm -rf /Users/zhengyong/code/cc-proxy-main/src-tauri/src/commands
```

- [ ] **Step 3: 删除前端相关 Rust 文件**

```bash
rm /Users/zhengyong/code/cc-proxy-main/src-tauri/src/tray.rs
rm /Users/zhengyong/code/cc-proxy-main/src-tauri/src/lightweight.rs
rm /Users/zhengyong/code/cc-proxy-main/src-tauri/src/app_store.rs
rm /Users/zhengyong/code/cc-proxy-main/src-tauri/src/usage_script.rs
```

- [ ] **Step 4: 删除 deeplink 目录**

```bash
rm -rf /Users/zhengyong/code/cc-proxy-main/src-tauri/src/deeplink
```

- [ ] **Step 5: 删除 capabilities 目录**

```bash
rm -rf /Users/zhengyong/code/cc-proxy-main/src-tauri/capabilities
```

- [ ] **Step 6: 删除 icons 目录**

```bash
rm -rf /Users/zhengyong/code/cc-proxy-main/src-tauri/icons
```

- [ ] **Step 7: 清空/删除 package.json**

由于不再需要 pnpm workspace，删除或归档 `package.json`（项目整体不需要 Node.js 构建了）。

- [ ] **Step 8: 提交**

```bash
git add -A
git commit -m "chore: remove frontend code (src/, commands/, tray, deeplink, etc.)"
```

---

## Task 2: 清理 lib.rs 中的前端初始化逻辑

**Files:**
- Modify: `src-tauri/src/lib.rs`

删除所有前端相关内容：窗口创建、前端事件监听、托盘初始化、`FrontendState`、plugin 初始化（dialog、deep-link、window-state、store、updater 等前端专用插件）。

- [ ] **Step 1: 读取 lib.rs 全文，确认要删除的代码块**

重点删除区域：
- `use tauri_plugin_deep_link::DeepLinkExt` 到 `use tauri_plugin_window_state::StateFlags` 的 use 语句
- `handle_deeplink_url` 函数（删除）
- `update_tray_menu` 函数（删除）
- `macos_tray_icon` 函数（删除）
- `run()` 函数中的 `tauri::Builder` 窗口配置、托盘初始化、plugin 注册（dialog、deep-link、window-state、store、updater）
- `on_window_event` 回调（删除）
- `single_instance` 插件中的窗口操作逻辑

保留：
- `panic_hook::setup_panic_hook()`
- 数据库初始化
- ProxyService 初始化
- ProviderService 初始化
- Settings 加载
- 模块重新导出（删除 commands 相关的）

- [ ] **Step 2: 重写 lib.rs**

新的 `lib.rs` 结构：

```rust
// 移除的前端模块
mod app_store;   // 删除（下一步）
mod deeplink;    // 删除（Task 1）
mod tray;        // 删除（Task 1）
mod lightweight; // 删除（Task 1）
mod commands;    // 删除（Task 1）
mod usage_script;

mod app_config;
mod auto_launch;
// ... 其他保留的模块

// 保留核心导出，删除 commands 导出
pub use app_config::{AppType, InstalledSkill, McpApps, McpServer, MultiAppConfig, SkillApps};
pub use config::{get_claude_mcp_path, get_claude_settings_path, read_json_file};
pub use database::Database;
pub use error::AppError;
pub use provider::{Provider, ProviderMeta};
pub use services::{
    ConfigService, EndpointLatency, McpService, PromptService, ProxyService,
    SkillService, SpeedtestService,
};
pub use settings::{update_settings, AppSettings};
pub use store::AppState;

// lib.rs 不再有 run()，改为提供初始化函数供 cli.rs 调用
```

- [ ] **Step 3: 编译验证**

```bash
cd /Users/zhengyong/code/cc-proxy-main/src-tauri
cargo check 2>&1 | head -100
```

预期：大量编译错误（AppHandle 类型找不到等），继续 Task 3-5 逐步修复。

- [ ] **Step 4: 提交**

```bash
git add src-tauri/src/lib.rs
git commit -m "refactor: remove frontend initialization from lib.rs"
```

---

## Task 3: 移除 proxy 模块中的 AppHandle 依赖

**Files:**
- Modify: `src-tauri/src/proxy/server.rs:41-42`
- Modify: `src-tauri/src/proxy/failover_switch.rs`
- Modify: `src-tauri/src/proxy/forwarder.rs`

- [ ] **Step 1: 改造 proxy/server.rs — 移除 app_handle 字段**

在 `ProxyState` 中，将：

```rust
pub app_handle: Option<tauri::AppHandle>,
```

替换为：

```rust
pub event_logger: Arc<Option<Box<dyn Fn(&str, &str) + Send + Sync>>>,
```

`event_logger` 是日志回调函数签名：`fn(event_type: &str, message: &str)`。

`ProxyServer::new` 的 `app_handle` 参数也同步改为 `event_logger`。

- [ ] **Step 2: 改造 proxy/failover_switch.rs — AppHandle → 日志**

找到所有 `app.emit("provider-switched", ...)` 调用，改为：

```rust
if let Some(ref logger) = self.event_logger {
    logger("provider_switched", &format!("Switched to provider: {}", provider_name));
} else {
    log::info!("[Failover] Switched to provider: {}", provider_name);
}
```

同样处理 `proxy-flags-changed` 事件。

- [ ] **Step 3: 改造 proxy/forwarder.rs — AppHandle → 日志**

找到所有 `app.emit(...)` 调用（Copilot/Codex 认证不可用时的错误提示），改为 `log::warn!()` 输出。

- [ ] **Step 4: 编译验证**

```bash
cd /Users/zhengyong/code/cc-proxy-main/src-tauri
cargo check 2>&1 | grep "app_handle\|AppHandle" | head -30
```

预期：剩余 AppHandle 错误应在 `services/proxy.rs` 和 `services/webdav_auto_sync.rs`。

- [ ] **Step 5: 提交**

```bash
git add src-tauri/src/proxy/
git commit -m "refactor(proxy): remove AppHandle, use logging callbacks instead"
```

---

## Task 4: 移除 services 中的 AppHandle 依赖

**Files:**
- Modify: `src-tauri/src/services/proxy.rs`
- Modify: `src-tauri/src/services/webdav_auto_sync.rs` (删除整个文件，或移除 Emitter)

- [ ] **Step 1: 改造 services/proxy.rs**

找到 `app_handle: Arc<RwLock<Option<tauri::AppHandle>>>` 和 `set_app_handle` 方法，删除这两个字段/方法。

找到所有 `handle.emit(...)` 调用（proxy 状态变化通知），改为 `log::info!()` 输出。

- [ ] **Step 2: 删除 webdav_auto_sync.rs**

WebDAV 自动同步功能依赖前端，不保留。删除文件并从 `services/mod.rs` 中移除 `webdav_auto_sync` 模块声明。

- [ ] **Step 3: 编译验证**

```bash
cargo check 2>&1 | head -50
```

预期：剩余错误应在 lib.rs（删除了模块但引用未清理）和 store.rs。

- [ ] **Step 4: 提交**

```bash
git add src-tauri/src/services/
git commit -m "refactor(services): remove AppHandle and Emitter, drop WebDAV auto sync"
```

---

## Task 5: 新建 YAML 配置存储层

**Files:**
- Create: `src-tauri/src/yaml_store.rs`
- Create: `src-tauri/src/cli_config.rs`

- [ ] **Step 1: 设计配置类型（cli_config.rs）**

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    pub proxy: ProxyConfig,
    pub failover: FailoverConfig,
    pub logging: LoggingConfig,
    pub providers: Vec<ProviderConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyConfig {
    pub listen: String,        // default: "127.0.0.1"
    pub port: u16,            // default: 8080
    pub mode: String,         // "global" | "per-app"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailoverConfig {
    pub enabled: bool,
    pub auto_switch: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,  // "debug" | "info" | "warn" | "error"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub name: String,
    #[serde(rename = "type")]
    pub provider_type: String,
    pub api_key: String,
    pub base_url: String,
    pub models: Vec<String>,
    pub priority: i32,
    pub enabled: bool,
    pub model_map: Option<ModelMap>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelMap {
    pub default: Option<String>,
    pub sonnet: Option<String>,
    pub opus: Option<String>,
    pub haiku: Option<String>,
}

impl Default for ProxyConfig {
    fn default() -> Self {
        Self {
            listen: "127.0.0.1".to_string(),
            port: 8080,
            mode: "global".to_string(),
        }
    }
}

impl Default for FailoverConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            auto_switch: true,
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
        }
    }
}
```

- [ ] **Step 2: 实现 yaml_store.rs**

```rust
use crate::cli_config::{AppConfig, LoggingConfig, ProxyConfig, FailoverConfig, ProviderConfig};
use std::path::{Path, PathBuf};
use anyhow::Result;

pub struct YamlStore {
    dir: PathBuf,
}

impl YamlStore {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    pub fn config_path(&self) -> PathBuf {
        self.dir.join("config.yaml")
    }

    /// 加载配置，不存在时返回默认配置
    pub fn load_config(&self) -> Result<AppConfig> {
        let path = self.config_path();
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            let config: AppConfig = serde_yaml::from_str(&content)?;
            Ok(config)
        } else {
            // 返回默认配置
            Ok(AppConfig::default())
        }
    }

    /// 保存配置到文件
    pub fn save_config(&self, config: &AppConfig) -> Result<()> {
        // 确保目录存在
        std::fs::create_dir_all(&self.dir)?;
        let content = serde_yaml::to_string(config)?;
        std::fs::write(self.config_path(), content)?;
        Ok(())
    }

    /// 获取默认配置目录 (~/.config/cc-proxy/)
    pub fn default_dir() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("cc-proxy")
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            proxy: ProxyConfig::default(),
            failover: FailoverConfig::default(),
            logging: LoggingConfig::default(),
            providers: Vec::new(),
        }
    }
}
```

- [ ] **Step 3: 添加到 lib.rs 模块声明**

在 `src-tauri/src/lib.rs` 中添加：

```rust
pub mod yaml_store;
pub mod cli_config;
```

- [ ] **Step 4: 编译验证**

```bash
cargo check 2>&1 | head -30
```

- [ ] **Step 5: 提交**

```bash
git add src-tauri/src/yaml_store.rs src-tauri/src/cli_config.rs
git commit -m "feat: add YAML config store layer"
```

---

## Task 6: 新建 CLI 入口 (clap)

**Files:**
- Create: `src-tauri/src/cli.rs`
- Modify: `src-tauri/src/main.rs`

- [ ] **Step 1: 定义 CLI 结构（cli.rs）**

```rust
use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "cc-proxy")]
#[command(version = "3.14.1")]
#[command(about = "All-in-One HTTP Proxy for Claude Code, Gemini CLI and more")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// 指定配置目录（默认 ~/.config/cc-proxy/）
    #[arg(short, long, global = true)]
    pub config_dir: Option<PathBuf>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// 前台启动代理服务器
    Start {
        /// 后台守护进程模式
        #[arg(short, long)]
        daemon: bool,
    },
    /// 停止后台守护进程
    Stop,
    /// 查看代理运行状态
    Status,
    /// 配置管理
    Config {
        #[command(subcommand)]
        action: ConfigCommands,
    },
    /// Provider 管理
    Provider {
        #[command(subcommand)]
        action: ProviderCommands,
    },
    /// 手动触发故障转移
    Failover {
        #[command(subcommand)]
        action: FailoverCommands,
    },
}

#[derive(Subcommand)]
pub enum ConfigCommands {
    /// 列出所有配置项
    List,
    /// 读取配置项
    Get { key: Option<String> },
    /// 设置配置项
    Set { key: String, value: String },
}

#[derive(Subcommand)]
pub enum ProviderCommands {
    /// 列出所有 providers
    List,
    /// 获取指定 provider 的可用模型列表
    Models { name: String },
    /// 健康检查
    Health {
        name: Option<String>,  // None means "all"
    },
    /// 测试端点延迟
    TestEndpoint {
        url: String,
    },
}

#[derive(Subcommand)]
pub enum FailoverCommands {
    /// 手动切换到指定 provider
    Switch { name: String },
}
```

- [ ] **Step 2: 改造 main.rs**

```rust
use clap::Parser;
use cc_switch_lib::cli::Cli;

fn main() {
    let cli = Cli::parse();

    // 设置日志输出到 stdout（容器友好）
    // 简单的 println/eprintln 即可，Docker 日志驱动管理轮转
    if let Err(e) = cc_switch_lib::run_cli(cli) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
```

- [ ] **Step 3: 在 lib.rs 中添加 run_cli 函数**

```rust
/// CLI 入口点（供 main.rs 调用）
pub fn run_cli(cli: crate::cli::Cli) -> Result<(), String> {
    use crate::cli::{Cli, Commands, ConfigCommands, ProviderCommands, FailoverCommands, Provider, yaml_store::YamlStore};

    let config_dir = cli.config_dir
        .unwrap_or_else(YamlStore::default_dir);

    let store = YamlStore::new(config_dir.clone());

    match cli.command {
        Commands::Start { daemon } => {
            // ... 实现启动逻辑
        }
        Commands::Stop => {
            // ... 读取 PID 文件，杀进程
        }
        Commands::Status => {
            // ... 输出状态
        }
        Commands::Config { action } => {
            // ... config get/set/list
        }
        Commands::Provider { action } => {
            match action {
                ProviderCommands::List => { /* ... */ }
                ProviderCommands::Models { name } => { /* ... */ }
                ProviderCommands::Health { name } => { /* ... */ }
                ProviderCommands::TestEndpoint { url } => { /* ... */ }
            }
        }
        Commands::Failover { action } => {
            match action {
                FailoverCommands::Switch { name } => { /* ... */ }
            }
        }
    }

    Ok(())
}
```

- [ ] **Step 4: 编译验证**

```bash
cargo check 2>&1 | head -50
```

- [ ] **Step 5: 提交**

```bash
git add src-tauri/src/cli.rs src-tauri/src/main.rs
git commit -m "feat: add clap CLI entry point"
```

---

## Task 7: 实现 start/stop/status 命令

**Files:**
- Modify: `src-tauri/src/cli.rs` (添加 daemon/PID 逻辑)
- Modify: `src-tauri/src/lib.rs` (实现 run_cli 中的 start/stop/status)

- [ ] **Step 1: 实现 start 命令**

后台模式：
1. 检查 PID 文件（`~/.config/cc-proxy/cc-proxy.pid`），如果进程还在运行则报错退出
2. fork 守护进程（Linux 上用 `daemon(0, 0)`，macOS 上类似）
3. 写 PID 到文件
4. 加载 YAML 配置
5. 初始化数据库连接（如果还有数据库相关代码）
6. 启动 ProxyServer

前台模式：
1. 加载 YAML 配置
2. 启动 ProxyServer
3. 阻塞直到收到 SIGTERM/SIGINT

- [ ] **Step 2: 实现 stop 命令**

读取 PID 文件，发送 SIGTERM，杀掉进程，删除 PID 文件。

- [ ] **Step 3: 实现 status 命令**

读取 PID 文件，检查进程是否存活，输出：
```
Proxy:     Running (PID: 12345)
Listen:    127.0.0.1:8080
Uptime:    2h 34m
```

如果未运行：
```
Proxy:     Stopped
```

- [ ] **Step 4: 编译验证**

```bash
cargo build --release 2>&1 | tail -20
```

- [ ] **Step 5: 手动测试**

```bash
# 前台启动
./target/release/cc-proxy start

# 另一个终端查看状态
./target/release/cc-proxy status

# 停止
./target/release/cc-proxy stop
```

- [ ] **Step 6: 提交**

```bash
git add src-tauri/src/
git commit -m "feat: implement start/stop/status daemon commands"
```

---

## Task 8: 实现 config 子命令

**Files:**
- Modify: `src-tauri/src/lib.rs` (run_cli 中的 Config 匹配分支)

- [ ] **Step 1: config list**

读取 config.yaml，格式化输出所有配置项（YAML 格式或 key: value 列表）。

- [ ] **Step 2: config get [key]**

支持嵌套 key 如 `proxy.port`，使用 JSON path 或 YAML dotted key 访问。

```rust
// 示例输出
// cc-proxy config get proxy.port
// 8080
```

- [ ] **Step 3: config set <key> <value>**

设置指定 key，保存回 config.yaml。注意处理类型转换（字符串 → bool/int 等）。

- [ ] **Step 4: 测试**

```bash
./cc-proxy config list
./cc-proxy config get proxy.port
./cc-proxy config set proxy.port 9090
./cc-proxy config get proxy.port  # 应输出 9090
```

- [ ] **Step 5: 提交**

```bash
git commit -m "feat: implement config get/set/list commands"
```

---

## Task 9: 实现 provider 子命令

**Files:**
- Modify: `src-tauri/src/lib.rs` (run_cli 中的 Provider 匹配分支)

- [ ] **Step 1: provider list**

从 config.yaml 读取 providers，输出表格：

```
NAME              TYPE      PRIORITY  ENABLED
anthropic-main    anthropic  1        true
google-gemini     gemini     2        true
```

- [ ] **Step 2: provider models <name>**

调用 `services::model_fetch::fetch_models()`，输出模型列表。

```bash
./cc-proxy --config-dir /path/to/config provider models anthropic-main
```

- [ ] **Step 3: provider health [name]**

单个：调用 `StreamCheckService::check_with_retry()`
全部：遍历所有 provider 并发检查

输出格式：
```
anthropic-main:  ✓ OK (latency: 120ms)
google-gemini:   ✓ OK (latency: 85ms)
```

- [ ] **Step 4: provider test-endpoint <url>**

调用 `SpeedtestService::test_endpoints()`：

```bash
./cc-proxy provider test-endpoint https://api.anthropic.com/v1/messages
```

- [ ] **Step 5: 测试**

```bash
./cc-proxy provider list
./cc-proxy provider test-endpoint https://api.anthropic.com
```

- [ ] **Step 6: 提交**

```bash
git commit -m "feat: implement provider list/models/health/test-endpoint commands"
```

---

## Task 10: 实现 failover switch 命令

**Files:**
- Modify: `src-tauri/src/lib.rs` (run_cli 中的 Failover 匹配分支)

- [ ] **Step 1: failover switch <name>**

调用 `ProxyService::switch_proxy_target()` 或直接操作 `FailoverSwitchManager`。

- [ ] **Step 2: 测试**

```bash
./cc-proxy failover switch google-gemini
```

- [ ] **Step 3: 提交**

```bash
git commit -m "feat: implement failover switch command"
```

---

## Task 11: 清理和最终编译验证

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/lib.rs` (清理未使用的 imports)

- [ ] **Step 1: 清理 Cargo.toml**

移除：
```toml
tauri = { version = "2.8.2", features = ["tray-icon", "protocol-asset", "image-png"] }
tauri-plugin-log = "2"
tauri-plugin-opener = "2"
tauri-plugin-process = "2"
tauri-plugin-updater = "2"
tauri-plugin-dialog = "2"
tauri-plugin-store = "2"
tauri-plugin-deep-link = "2"
tauri-plugin-window-state = "2"
tauri-plugin-single-instance = "2"
webkit2gtk = { version = "2.0.1", features = ["v2_16"] }  # Linux webkit

# 移除 tauri-build
[build-dependencies]
tauri-build = { version = "2.4.0", features = [] }
```

新增：
```toml
clap = { version = "4", features = ["derive"] }
```

- [ ] **Step 2: 删除 Cargo.lock 和 target 目录后重新编译**

```bash
cd /Users/zhengyong/code/cc-proxy-main/src-tauri
rm -rf target Cargo.lock
cargo build --release 2>&1 | tail -30
```

- [ ] **Step 3: 修复所有编译错误**

逐个修复错误，通常是：
- 未使用的 imports（删除）
- 删除的 module 仍有引用（清理）
- 类型不匹配（调整）

- [ ] **Step 4: 最终编译成功验证**

```bash
cargo build --release 2>&1
./target/release/cc-proxy --help
```

预期输出：
```
cc-proxy 3.14.1
All-in-One HTTP Proxy for Claude Code, Gemini CLI and more

Usage: cc-proxy [OPTIONS] <COMMAND>

Commands:
  start      前台启动代理服务器
  stop       停止后台守护进程
  status     查看代理运行状态
  config     配置管理
  provider   Provider 管理
  failover   手动触发故障转移
  help       Print help
```

- [ ] **Step 5: 功能验证**

```bash
# 1. 创建一个测试 config.yaml
mkdir -p /tmp/cc-proxy-test
cat > /tmp/cc-proxy-test/config.yaml << 'EOF'
proxy:
  listen: "127.0.0.1"
  port: 18080
  mode: "global"
failover:
  enabled: true
  auto_switch: true
logging:
  level: "debug"
providers:
  - name: "test-anthropic"
    type: "anthropic"
    api_key: "sk-test"
    base_url: "https://api.anthropic.com"
    models: ["claude-sonnet-4-7-20250514"]
    priority: 1
    enabled: true
    model_map:
      default: "claude-sonnet-4-7-20250514"
      sonnet: "claude-sonnet-4-7-20250514"
EOF

# 2. 测试 config list
./target/release/cc-proxy --config-dir /tmp/cc-proxy-test config list

# 3. 测试 provider list
./target/release/cc-proxy --config-dir /tmp/cc-proxy-test provider list

# 4. 测试 provider test-endpoint
./target/release/cc-proxy --config-dir /tmp/cc-proxy-test provider test-endpoint https://api.anthropic.com

# 5. 测试 start (前台，5秒后 Ctrl+C)
timeout 5 ./target/release/cc-proxy --config-dir /tmp/cc-proxy-test start || true
```

- [ ] **Step 6: 提交**

```bash
git add -A
git commit -m "chore: remove Tauri dependencies, add clap, complete CLI tool"
```

---

## Task 12: 更新文档

**Files:**
- Modify: `README.md` (更新构建和使用说明)

- [ ] **Step 1: 更新 README**

删除 pnpm/Tauri 相关构建指令，改为：

```markdown
## 构建

```bash
cd src-tauri
cargo build --release
```

## 使用

```bash
# 启动代理（前台）
cc-proxy start

# 后台运行
cc-proxy start --daemon

# 查看状态
cc-proxy status

# 停止
cc-proxy stop

# 配置管理
cc-proxy config list
cc-proxy config get proxy.port
cc-proxy config set proxy.port 9090
```

## 配置文件

默认配置目录：`~/.config/cc-proxy/config.yaml`
```

- [ ] **Step 2: 提交**

```bash
git commit -m "docs: update README for CLI-only build"
```

---

## 实施顺序

```
Task 1  删除前端代码
Task 2  清理 lib.rs（依赖 Task 1）
Task 3  移除 proxy AppHandle（依赖 Task 2）
Task 4  移除 services AppHandle（依赖 Task 3）
Task 5  新建 YAML 存储层（独立）
Task 6  新建 CLI 入口（依赖 Task 5）
Task 7  实现 start/stop/status（依赖 Task 6）
Task 8  实现 config 命令（依赖 Task 6）
Task 9  实现 provider 命令（依赖 Task 6）
Task 10 实现 failover 命令（依赖 Task 6）
Task 11 清理 Cargo.toml + 最终编译（依赖 Task 3,4,7,8,9,10）
Task 12 更新文档（依赖 Task 11）
```

---

## 自查清单

- [ ] spec 覆盖率：每个 spec 要求都能在 plan 里找到对应 task
- [ ] 无 placeholder：没有 TBD/TODO/实现细节待定
- [ ] 类型一致性：AppHandle 替换方案在所有文件中一致
- [ ] 编译通过：Task 11 编译成功即为完成
- [ ] 功能验证：Task 11 的测试步骤全部通过