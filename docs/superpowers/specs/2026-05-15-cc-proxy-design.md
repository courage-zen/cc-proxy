# CC-Proxy 设计文档

> 日期：2026-05-15
> 目标：从 cc-proxy 剥离 proxy 模块，创建独立可部署的 Rust 代理服务

---

## 1. 概述

### 1.1 项目目标

将 cc-proxy 的 HTTP 代理功能剥离为独立项目，支持：
- 通过配置文件管理多 Provider
- 自动故障转移
- Anthropic / OpenAI / Gemini / Responses API 格式转换
- Docker 容器化部署
- 为 Claude Code 提供本地代理服务

### 1.2 技术栈

- **语言**：Rust (2021 edition)
- **Web 框架**：Axum 0.7
- **HTTP 客户端**：hyper + hyper-util
- **配置格式**：TOML
- **容器化**：Docker + 多阶段构建

---

## 2. 配置设计

### 2.1 配置文件结构

```toml
[server]
listen_address = "0.0.0.0"
listen_port = 15721

[logging]
level = "info"              # error, warn, info, debug

[timeouts]
streaming_first_byte = 60   # 秒
streaming_idle = 120        # 秒
non_streaming = 600         # 秒

[circuit_breaker]
failure_threshold = 5
success_threshold = 3
timeout_seconds = 30
error_rate_threshold = 0.5
min_requests = 10

[[providers]]
name = "Anthropic Official"
type = "claude"             # claude, claude_auth, codex, gemini, openrouter
base_url = "https://api.anthropic.com"
api_key = "${ANTHROPIC_API_KEY}"
models = ["claude-sonnet-4-6", "claude-opus-4-6"]
priority = 1                # 故障转移优先级，1 最高

[[providers]]
name = "OpenRouter"
type = "openrouter"
base_url = "https://openrouter.ai/api"
api_key = "${OPENROUTER_API_KEY}"
models = ["anthropic/claude-sonnet-4-6"]
priority = 2
```

### 2.2 环境变量

- 配置中支持 `${ENV_VAR}` 语法注入环境变量
- 常用变量：`ANTHROPIC_API_KEY`、`OPENAI_API_KEY`、`OPENROUTER_API_KEY`

### 2.3 Provider 类型

| type | 说明 | 认证方式 |
|------|------|---------|
| `claude` | Anthropic 官方 API | x-api-key |
| `claude_auth` | Claude 中转服务 | Bearer Token |
| `codex` | OpenAI Codex | x-api-key |
| `gemini` | Google Gemini API | x-goog-api-key |
| `openrouter` | OpenRouter | Bearer Token |

---

## 3. API 端点

| 端点 | 方法 | 说明 |
|------|------|------|
| `/health` | GET | 健康检查 |
| `/status` | GET | 服务状态 |
| `/v1/messages` | POST | Claude API |
| `/claude/v1/messages` | POST | Claude API（别名） |
| `/v1/chat/completions` | POST | OpenAI Chat Completions |
| `/v1/responses` | POST | OpenAI Responses API |
| `/v1beta/*path` | ANY | Gemini API |

---

## 4. 核心模块

### 4.1 模块结构

```
src/
├── main.rs                  # 入口，加载配置，启动服务器
├── server.rs               # Axum 服务器构建
├── handlers.rs             # HTTP 请求处理器
├── config.rs               # TOML 配置加载
├── types.rs                # 配置结构体
├── router.rs               # Provider 路由 + 熔断器
├── circuit_breaker.rs      # 熔断器实现
├── forwarder.rs            # 请求转发
├── http_client.rs          # HTTP 客户端
├── sse.rs                  # SSE 解析
├── providers/
│   ├── mod.rs
│   ├── adapter.rs          # ProviderAdapter trait
│   ├── auth.rs             # 认证类型
│   ├── claude.rs           # Claude 适配器
│   ├── gemini.rs           # Gemini 适配器
│   ├── transform.rs        # Anthropic ↔ OpenAI Chat 转换
│   ├── transform_responses.rs # Anthropic ↔ OpenAI Responses 转换
│   ├── transform_gemini.rs # Anthropic ↔ Gemini Native 转换
│   ├── streaming.rs        # OpenAI 流式 SSE 转换
│   ├── streaming_responses.rs # Responses API 流式转换
│   └── streaming_gemini.rs # Gemini 流式转换
├── cache_injector.rs       # Cache 断点注入
├── thinking_rectifier.rs   # Thinking 签名整流
├── thinking_budget_rectifier.rs # Thinking budget 整流
└── error.rs                # 错误类型
```

### 4.2 各模块职责

| 模块 | 职责 |
|------|------|
| `config.rs` | 解析 TOML 配置文件，支持环境变量注入 |
| `types.rs` | 定义配置结构体（ProxyConfig、AppProxyConfig 等） |
| `router.rs` | Provider 选择、熔断器管理、故障转移队列 |
| `circuit_breaker.rs` | 熔断器实现（Closed/Open/HalfOpen 状态机） |
| `forwarder.rs` | 发送 HTTP 请求到上游，处理重试 |
| `providers/transform.rs` | Anthropic 请求 → OpenAI 格式 |
| `providers/transform_responses.rs` | Anthropic 请求 → OpenAI Responses API 格式 |
| `providers/streaming.rs` | OpenAI SSE → Anthropic SSE 流式转换 |
| `cache_injector.rs` | 自动注入 cache_control 断点 |
| `thinking_rectifier.rs` | 修复 thinking 签名错误 |
| `thinking_budget_rectifier.rs` | 修复 thinking budget 约束错误 |

### 4.3 请求流程

```
Client Request
    ↓
handlers.rs           (路由匹配)
    ↓
RequestContext        (解析请求，读取配置)
    ↓
Thinking Rectifier   (可选，修复 thinking 签名/budget)
    ↓
Cache Injector       (可选，注入 cache_control 断点)
    ↓
Format Transform     (按目标 Provider 类型转换格式)
    ↓
router.rs            (选择 Provider，检查熔断器)
    ↓
forwarder.rs          (转发请求到上游)
    ↓
Stream Transform     (SSE 流式转换)
    ↓
Client Response
```

---

## 5. 格式转换

### 5.1 Anthropic → OpenAI Chat

```
system: "..."                    →  messages: [{role: "system", content: "..."}]
content[].type: "text"           →  content: "text"
content[].type: "image"           →  image_url: {url: "data:...base64"}
content[].type: "tool_use"       →  tool_calls: [{function: {...}}]
content[].type: "tool_result"    →  {role: "tool", tool_call_id: "..."}
thinking + budget                →  reasoning_effort (o-series/gpt-5+)
```

### 5.2 OpenAI → Anthropic

```
choices[].message        →  content: [{type: "text", text: "..."}]
finish_reason           →  stop_reason
reasoning_content       →  content: [{type: "thinking", thinking: "..."}]
tool_calls              →  content: [{type: "tool_use", ...}]
```

---

## 6. 故障转移机制

### 6.1 触发条件

- 上游返回 5xx 错误
- 连接超时
- 熔断器打开（失败次数达到阈值）

### 6.2 行为

1. 按 `priority` 顺序尝试 Provider
2. 失败后自动切换到下一个
3. 熔断器状态跨请求保持
4. 所有 Provider 熔断后返回错误

### 6.3 熔断器配置

```toml
[circuit_breaker]
failure_threshold = 5    # 5 次失败后熔断
success_threshold = 3     # 3 次成功后半-open
timeout_seconds = 30      # 30 秒后尝试恢复
```

---

## 7. Docker 部署

### 7.1 多阶段构建

```dockerfile
# Stage 1: Build
FROM rust:1.85-slim as builder
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release
RUN strip target/release/cc-proxy

# Stage 2: Runtime
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /build/target/release/cc-proxy /usr/local/bin/cc-proxy
COPY config.example.toml /etc/cc-proxy/config.toml
ENTRYPOINT ["cc-proxy", "--config", "/etc/cc-proxy/config.toml"]
```

### 7.2 基础镜像大小目标

- 构建阶段：~1GB
- 运行阶段：~30-50MB

### 7.3 Docker Compose 示例

```yaml
version: "3.8"
services:
  cc-proxy:
    image: cc-proxy:latest
    ports:
      - "15721:15721"
    volumes:
      - ./config.toml:/etc/cc-proxy/config.toml:ro
    environment:
      - ANTHROPIC_API_KEY=${ANTHROPIC_API_KEY}
      - OPENAI_API_KEY=${OPENAI_API_KEY}
    restart: unless-stopped
```

---

## 8. 配置文件示例

```toml
[server]
listen_address = "0.0.0.0"
listen_port = 15721

[logging]
level = "info"

[timeouts]
streaming_first_byte = 60
streaming_idle = 120
non_streaming = 600

[circuit_breaker]
failure_threshold = 5
success_threshold = 3
timeout_seconds = 30
error_rate_threshold = 0.5
min_requests = 10

# Cache 注入配置（Claude Prompt Caching）
[cache]
enabled = true
ttl = "1h"                  # 缓存 TTL: "5m", "1h"

# Thinking 整流配置
[thinking]
signature_fix = true       # 修复签名错误
budget_fix = true          # 修复 budget 约束

# Provider 列表
[[providers]]
name = "Anthropic Official"
type = "claude"
base_url = "https://api.anthropic.com"
api_key = "${ANTHROPIC_API_KEY}"
models = ["claude-sonnet-4-6", "claude-opus-4-6", "claude-3-5-sonnet-20250608"]
priority = 1

[[providers]]
name = "OpenRouter Claude"
type = "openrouter"
base_url = "https://openrouter.ai/api"
api_key = "${OPENROUTER_API_KEY}"
models = ["anthropic/claude-sonnet-4-6", "anthropic/claude-3-5-sonnet-20250608"]
priority = 2

[[providers]]
name = "OpenAI"
type = "codex"
base_url = "https://api.openai.com"
api_key = "${OPENAI_API_KEY}"
models = ["gpt-4o", "gpt-5"]
priority = 3
```

---

## 9. CLI 参数

```bash
cc-proxy --config <path>     # 配置文件路径（默认 ./config.toml）
cc-proxy --help              # 显示帮助
```

---

## 10. 健康检查响应

```bash
$ curl http://localhost:15721/health
{
  "status": "healthy",
  "timestamp": "2026-05-15T10:30:00Z"
}

$ curl http://localhost:15721/status
{
  "running": true,
  "address": "0.0.0.0",
  "port": 15721,
  "active_targets": [
    {"app_type": "claude", "provider_id": "anthropic-official", "provider_name": "Anthropic Official"}
  ],
  "uptime_seconds": 3600
}
```

---

## 11. 兼容性

### 11.1 Claude Code 配置

Claude Code 通过环境变量使用本地代理：

```bash
export ANTHROPIC_BASE_URL=http://localhost:15721
export ANTHROPIC_AUTH_TOKEN=<your-token>
claude
```

### 11.2 Codex CLI 配置

```bash
export OPENAI_BASE_URL=http://localhost:15721/v1
export OPENAI_API_KEY=<your-key>
codex
```

---

## 12. 项目结构

```
cc-proxy/
├── Cargo.toml
├── config.example.toml
├── Dockerfile
├── docker-compose.yml
├── README.md
└── src/
    ├── main.rs
    ├── server.rs
    ├── handlers.rs
    ├── config.rs
    ├── types.rs
    ├── router.rs
    ├── circuit_breaker.rs
    ├── forwarder.rs
    ├── http_client.rs
    ├── sse.rs
    ├── error.rs
    ├── cache_injector.rs
    ├── thinking_rectifier.rs
    ├── thinking_budget_rectifier.rs
    └── providers/
        ├── mod.rs
        ├── adapter.rs
        ├── auth.rs
        ├── claude.rs
        ├── gemini.rs
        ├── transform.rs
        ├── transform_responses.rs
        ├── transform_gemini.rs
        ├── streaming.rs
        ├── streaming_responses.rs
        └── streaming_gemini.rs
```

---

## 13. 依赖清理

### 13.1 从 cc-proxy 保留的依赖

```toml
# Web 框架
axum = "0.7"
tower = "0.4"
tower-http = { version = "0.5", features = ["cors"] }
hyper = { version = "1.0", features = ["full"] }
hyper-util = { version = "0.1", features = ["tokio", "http1", "client-legacy"] }
hyper-rustls = "0.27"

# 异步运行时
tokio = { version = "1", features = ["macros", "rt-multi-thread", "time", "sync"] }
futures = "0.3"
async-stream = "0.3"

# 序列化
serde = { version = "1.0", features = ["derive"] }
serde_json = { version = "1.0", features = ["preserve_order"] }
toml = "0.8"

# 日志
log = "0.4"
chrono = { version = "0.4", features = ["serde"] }

# HTTP
http = "1"
http-body = "1"
http-body-util = "0.1"

# 工具
bytes = "1.5"
base64 = "0.22"
regex = "1.10"
url = "2.5"
uuid = { version = "1.11", features = ["v4"] }
sha2 = "0.10"
```

### 13.2 移除的依赖

- `tauri` 系列（GUI 框架）
- `rusqlite`（SQLite 数据库）
- `rquickjs`（JavaScript 运行时）
- `arboard`、`zip` 等 UI 相关库
- `tauri-plugin-*` 系列插件

### 13.3 最终二进制大小目标

- 压缩后：~4-6MB
- Docker 镜像：~30-50MB

---

## 14. 待明确事项

- [ ] 是否需要 Prometheus 指标端点（`/metrics`）
- [ ] 是否需要 graceful shutdown 信号处理
- [ ] 是否需要配置热加载（文件变更自动重载）