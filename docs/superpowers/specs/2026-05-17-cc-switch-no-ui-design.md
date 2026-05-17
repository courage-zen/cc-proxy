# cc-proxy 无 UI 化设计方案

## 目标

去掉 React 前端（`src/`），保留 Rust 代理核心逻辑，改造成纯命令行工具，通过 YAML 配置文件管理所有数据。

## 决策摘要

| 维度 | 选择 |
|------|------|
| 交互方式 | 纯配置文件（YAML） |
| 系统托盘 | 不需要 |
| 启停控制 | 单一二进制 + 子命令（`cc-proxy start/stop/status/config/provider`） |
| 配置格式 | YAML |
| 数据存储 | 全部迁移到 YAML 文件 |
| 后端框架 | 纯 Rust 二进制（去掉 Tauri），用 clap 管理 CLI |

## 架构

```
cc-proxy (单一二进制，clap CLI)
│
├── CLI 层（新增）
│   ├── start     启动代理
│   ├── stop      停止代理
│   ├── status    查看状态
│   ├── config    读写配置
│   └── provider  管理 providers
│
├── 核心代理引擎（保留，改造 AppHandle → 日志）
│   ├── proxy/          HTTP 代理服务器（Axum）
│   ├── services/       ProxyService、ProviderService
│   └── session_manager/  Session 管理
│
├── 新增 YAML 存储层（替代 SQLite DAO）
│   └── yaml_store.rs   config.yaml 读写
│
└── 删除
    ├── src/                          React 前端全部删除
    ├── src-tauri/src/commands/       Tauri IPC 命令全部删除
    ├── src-tauri/src/tray.rs         托盘代码删除
    ├── src-tauri/src/lightweight.rs   轻量模式删除
    ├── src-tauri/src/deeplink/       Deep link 删除
    └── lib.rs 中前端初始化逻辑       删除
```

## 配置目录结构

通过 `--config-dir` 指定（默认 `~/.config/cc-proxy/`）：

```
~/.config/cc-proxy/
└── config.yaml   唯一配置文件（proxy 配置、failover 配置、providers）
```

failover 队列状态不持久化，每次启动从 priority=1 的 provider 重新开始。

## 配置文件格式（config.yaml）

```yaml
proxy:
  listen: "127.0.0.1"
  port: 8080
  mode: "global"  # global | per-app

failover:
  enabled: true
  auto_switch: true

logging:
  level: "info"  # debug | info | warn | error

providers:
  - name: "anthropic-main"
    type: "anthropic"
    api_key: "sk-..."
    base_url: "https://api.anthropic.com"
    models:
      - "claude-sonnet-4-7-20250514"
    priority: 1
    enabled: true
    model_map:
      default: "claude-sonnet-4-7-20250514"
      sonnet: "claude-sonnet-4-7-20250514"
      opus: "claude-opus-4-5-20251114"
      haiku: "claude-haiku-4-5"

  - name: "google-gemini"
    type: "gemini"
    api_key: "AIza..."
    base_url: "https://generativelanguage.googleapis.com"
    models:
      - "gemini-2.5-flash"
    priority: 2
    enabled: true
```

## CLI 子命令

### start

```bash
cc-proxy start [OPTIONS]

OPTIONS:
  -d, --daemon       后台守护进程模式（写 PID 文件）
  -c, --config-dir   指定配置目录（默认 ~/.config/cc-proxy/）
  -l, --log-level    日志级别（默认 info）
```

### stop

```bash
cc-proxy stop
```

停止后台守护进程（读取 PID 文件）。

### status

```bash
cc-proxy status
```

输出示例：

```
Proxy:     Running (PID: 12345)
Listen:    127.0.0.1:8080
Mode:      global
Provider:  anthropic-main (active)
Health:    3/3 providers healthy
Uptime:    2h 34m
```

### config

```bash
cc-proxy config get [key]       读取配置（支持嵌套 key，如 proxy.port）
cc-proxy config set <key> <value>  修改配置
cc-proxy config list            列出所有配置
```

### provider

```bash
cc-proxy provider list              列出所有 providers（含名称、类型、优先级、enabled 状态）
cc-proxy provider models <name>     获取指定 provider 的可用模型列表
cc-proxy provider health <name>     健康检查指定 provider
cc-proxy provider health all        健康检查所有 provider
cc-proxy provider test-endpoint <url>  测试端点响应延迟
```

> `config.yaml` 由用户在体外自行编辑管理，不提供 add/edit/delete 命令。

### failover

```bash
cc-proxy failover switch <name>    手动切换到指定 provider
```

> failover 队列状态不持久化，重启后自动从 priority=1 的 provider 开始。

## 核心改造点

### 1. 删除文件

- `src/` — 整个前端目录
- `src-tauri/src/commands/` — 所有 Tauri 命令
- `src-tauri/src/tray.rs`
- `src-tauri/src/lightweight.rs`
- `src-tauri/src/deeplink/` — 整个目录
- `src-tauri/src/app_store.rs` — 依赖 AppHandle 的 store

### 2. 改造 proxy/server.rs

将 `app_handle: Option<tauri::AppHandle>` 替换为日志回调机制：

```rust
// Before
pub app_handle: Option<tauri::AppHandle>,
if let Err(e) = app.emit("provider-switched", &event_data) { ... }

// After
pub event_logger: Option<Box<dyn Fn(Event) + Send + Sync>>,
event_logger.map(|f| f(Event::ProviderSwitched { ... }));
```

### 3. SQLite → YAML 迁移

将 `database/dao/` 中的 ProviderDAO、SettingsDAO 等改造为 YAML 文件读写：

```rust
// yaml_store.rs
pub struct YamlStore {
    dir: PathBuf,
}

impl YamlStore {
    pub fn load_config(&self) -> Result<AppConfig>;     // 读取 config.yaml（含 providers）
    pub fn save_config(&self, config: &AppConfig) -> Result<()>;
}
```

providers 不再单独存储，统一写入 config.yaml。

### 4. 新建 CLI 入口

用 clap 定义子命令，替换 Tauri 的 `main.rs` 入口：

```rust
// cli.rs / main.rs
#[derive(Subcommand)]
enum Commands {
    Start(StartArgs),
    Stop,
    Status,
    Config {
        #[command(subcommand)]
        action: ConfigCommands,
    },
    Provider {
        #[command(subcommand)]
        action: ProviderCommands,
    },
    Failover {
        #[command(subcommand)]
        action: FailoverCommands,
    },
}
```

### 5. 后台进程模式

使用 PID 文件管理守护进程：

- 启动时写 PID 到 `~/.config/cc-proxy/cc-proxy.pid`
- `cc-proxy stop` 读取 PID 并发送 SIGTERM
- `cc-proxy start -d` 检查已有 PID，避免重复启动

### 6. lib.rs 改造

删除所有前端相关内容：
- 删除窗口创建逻辑
- 删除前端事件监听（`app.listen()` 等）
- 删除 `FrontendState` 初始化
- 保留核心服务初始化、数据库初始化

## 依赖变更

### 移除的依赖

**Cargo.toml:**
- `tauri` 及所有 `tauri-plugin-*`
- 删除 `src-tauri/capabilities/` 目录

**package.json (删除):**
- 整个 `src/` 的前端依赖全部删除

### 新增的依赖

**Cargo.toml:**
- `clap = { version = "4", features = ["derive"] }` — CLI 框架
- `serde_yaml = "0.9"` — YAML 序列化

### 保留的依赖

所有核心代理相关依赖：
- `axum`, `tower`, `hyper` — HTTP 服务器
- `reqwest` — HTTP 客户端
- `tokio` — 异步运行时
- `tracing` — 日志（输出到 stdout/stderr）
- `serde`, `serde_json` — 序列化

### 删除的依赖

- `tauri` 及所有 `tauri-plugin-*`
- `rusqlite` — 不用 SQLite
- `tracing-subscriber` — 容器场景日志由 Docker 管理，应用只管输出到 stdout
- `rusqlite` — 不用 SQLite，全部 YAML
- 前端相关依赖（React, Tailwind, Vite 等）在 package.json 中的部分

## 工作量拆分

### 阶段 1：删除前端代码

1. 删除 `src/` 目录
2. 删除 `src-tauri/src/commands/` 目录
3. 删除前端相关文件（tray、lightweight、deeplink、app_store）
4. 清理 `lib.rs` 中的前端初始化逻辑
5. 清理 Cargo.toml 中的 tauri 依赖
6. 删除 `package.json`（不再需要 pnpm workspace）

7. 改造 `proxy/server.rs` — 移除 `AppHandle` 依赖
8. 改造 `proxy/failover_switch.rs` — AppHandle → 日志
9. 改造 `proxy/forwarder.rs` — AppHandle → 日志
10. 改造 `services/proxy.rs` — 移除 `AppHandle`
11. 改造 `services/webdav_auto_sync.rs` — 移除 Emitter

### 阶段 3：存储层迁移

12. 新建 `yaml_store.rs`（config.yaml 读写）
13. usage 数据输出为 JSON Lines 到 stdout（由 Docker 日志驱动管理）

### 阶段 4：CLI 层

14. 新建 `cli.rs` 定义所有子命令
15. 改造 `main.rs` 入口
16. 实现后台守护进程模式（PID 文件）
17. 实现 `start -d`、`stop`、`status` 命令

### 阶段 5：收尾

18. 清理所有未使用的 imports
19. 更新 README
20. 编译验证

## 风险与注意事项

1. **OAuth 认证** — 项目不使用 Copilot/Codex，相关 OAuth 逻辑在删除前端时一并移除。
2. **Tauri build** — 不再需要 `pnpm tauri build`，改为 `cargo build --release`。

## 附录：Docker 部署建议

### 日志管理

所有日志（包括 usage 统计）输出到 stdout/stderr，由 Docker 容器运行时统一管理日志轮转，不写任何日志文件。

```yaml
services:
  cc-proxy:
    image: cc-proxy:latest
    logging:
      max-size: "50m"
      max-file: "5"
      compress: true
```

### 配置文件挂载

将宿主机上的 `config.yaml` 挂载到容器内：

```yaml
services:
  cc-proxy:
    image: cc-proxy:latest
    volumes:
      - ./config.yaml:/root/.config/cc-proxy/config.yaml:ro
    command: ["start", "-d"]
```