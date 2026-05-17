# CC-Proxy Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 从 cc-proxy 剥离 proxy 模块，创建独立可部署的 Rust 代理服务 cc-proxy，支持 TOML 配置、多 Provider 故障转移、格式转换和 Docker 部署。

**Architecture:** 基于 Axum 0.7 构建 HTTP 代理服务器，从 TOML 配置文件加载 Provider 配置，请求经过 Thinking 整流器 → Cache 注入器 → 格式转换 → Provider 路由（含熔断器）→ 上游转发 → 流式转换返回客户端。配置文件支持环境变量注入，Graceful Shutdown 通过 SIGTERM 信号处理。

**Tech Stack:** Rust 1.85, Axum 0.7, hyper 1.0, tokio, serde, toml, tower

---

## 文件结构

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
    ├── error.rs
    ├── router.rs
    ├── circuit_breaker.rs
    ├── forwarder.rs
    ├── http_client.rs
    ├── sse.rs
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

## Phase 1: 项目初始化

### Task 1: 创建项目骨架

**Files:**
- Create: `cc-proxy/Cargo.toml`
- Create: `cc-proxy/src/main.rs`
- Create: `cc-proxy/config.example.toml`

- [ ] **Step 1: 创建 Cargo.toml**

```toml
[package]
name = "cc-proxy"
version = "0.1.0"
edition = "2021"
rust-version = "1.85"

[dependencies]
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
env_logger = "0.11"
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
anyhow = "1.0"
thiserror = "2.0"

# TLS
rustls = "0.23"
webpki-roots = "0.26"

[profile.release]
codegen-units = 1
lto = "thin"
opt-level = "s"
panic = "unwind"
strip = "symbols"
```

- [ ] **Step 2: 创建 src/main.rs 入口**

```rust
//! cc-proxy: 独立 Claude Code 代理服务

mod config;
mod error;
mod handlers;
mod http_client;
mod router;
mod circuit_breaker;
mod forwarder;
mod sse;
mod cache_injector;
mod thinking_rectifier;
mod thinking_budget_rectifier;
mod server;
mod types;

mod providers;

use std::net::SocketAddr;
use tokio::signal;
use log::{info, error};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志
    env_logger::init();

    // 解析命令行参数
    let config_path = std::env::args()
        .skip(1)
        .find(|arg| arg == "--config")
        .and_then(|_| std::env::args().skip(2).next())
        .unwrap_or_else(|| "config.toml".to_string());

    // 加载配置
    let config = config::load(&config_path)?;
    info!("配置加载成功: {} providers", config.providers.len());

    // 启动服务器
    let addr: SocketAddr = format!("{}:{}", config.server.listen_address, config.server.listen_port)
        .parse()
        .expect("无效的监听地址");

    info!("启动 cc-proxy 于 {}", addr);

    let shutdown = async {
        signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");
        info!("收到 SIGTERM 信号，开始优雅关闭...");
    };

    let server = server::build(&config)?;
    axum::serve(server, shutdown).await?;

    info!("cc-proxy 已关闭");
    Ok(())
}
```

- [ ] **Step 3: 创建 config.example.toml**

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

[cache]
enabled = true
ttl = "1h"

[thinking]
signature_fix = true
budget_fix = true

[[providers]]
name = "Anthropic Official"
type = "claude"
base_url = "https://api.anthropic.com"
api_key = "${ANTHROPIC_API_KEY}"
models = ["claude-sonnet-4-6"]
priority = 1

[[providers]]
name = "OpenRouter"
type = "openrouter"
base_url = "https://openrouter.ai/api"
api_key = "${OPENROUTER_API_KEY}"
models = ["anthropic/claude-sonnet-4-6"]
priority = 2
```

- [ ] **Step 4: 运行 `cargo check` 验证项目结构**

Run: `cd cc-proxy && cargo check`

---

## Phase 2: 配置层

### Task 2: types.rs — 配置结构体

**Files:**
- Create: `cc-proxy/src/types.rs`

- [ ] **Step 1: 创建 src/types.rs**

```rust
//! 配置类型定义

use serde::{Deserialize, Serialize};

/// 根配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
    #[serde(default)]
    pub timeouts: TimeoutsConfig,
    #[serde(default)]
    pub circuit_breaker: CircuitBreakerConfig,
    #[serde(default)]
    pub cache: CacheConfig,
    #[serde(default)]
    pub thinking: ThinkingConfig,
    pub providers: Vec<ProviderConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub listen_address: String,
    pub listen_port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self { level: "info".to_string() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeoutsConfig {
    pub streaming_first_byte: u64,
    pub streaming_idle: u64,
    pub non_streaming: u64,
}

impl Default for TimeoutsConfig {
    fn default() -> Self {
        Self {
            streaming_first_byte: 60,
            streaming_idle: 120,
            non_streaming: 600,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerConfig {
    pub failure_threshold: u32,
    pub success_threshold: u32,
    pub timeout_seconds: u64,
    pub error_rate_threshold: f64,
    pub min_requests: u32,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            success_threshold: 3,
            timeout_seconds: 30,
            error_rate_threshold: 0.5,
            min_requests: 10,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    pub enabled: bool,
    pub ttl: String,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            ttl: "1h".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThinkingConfig {
    pub signature_fix: bool,
    pub budget_fix: bool,
}

impl Default for ThinkingConfig {
    fn default() -> Self {
        Self {
            signature_fix: true,
            budget_fix: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub name: String,
    pub #[serde(rename = "type")]
    pub provider_type: String,
    pub base_url: String,
    pub api_key: String,
    #[serde(default)]
    pub models: Vec<String>,
    pub priority: u32,
}

/// Provider 类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderType {
    Claude,
    ClaudeAuth,
    Codex,
    Gemini,
    OpenRouter,
}

impl ProviderType {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "claude" => Self::Claude,
            "claude_auth" | "claude-auth" => Self::ClaudeAuth,
            "codex" => Self::Codex,
            "gemini" => Self::Gemini,
            "openrouter" => Self::OpenRouter,
            _ => Self::Claude,
        }
    }
}
```

- [ ] **Step 2: 运行 `cargo check` 验证**

Run: `cargo check --manifest-path cc-proxy/Cargo.toml`

---

### Task 3: config.rs — TOML 配置加载 + 环境变量

**Files:**
- Create: `cc-proxy/src/config.rs`

- [ ] **Step 1: 创建 src/config.rs**

```rust
//! TOML 配置文件加载，支持 ${ENV_VAR} 环境变量注入

use std::collections::HashMap;
use std::path::Path;

use crate::types::Config;

const ENV_VAR_PATTERN: &str = "${";

/// 从 TOML 文件加载配置
pub fn load(path: &str) -> anyhow::Result<Config> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("读取配置文件失败 {}: {}", path, e))?;

    let content = substitute_env_vars(&content);

    let config: Config = toml::from_str(&content)
        .map_err(|e| anyhow::anyhow!("解析配置文件失败 {}: {}", path, e))?;

    // 验证配置
    validate_config(&config)?;

    Ok(config)
}

/// 替换 ${ENV_VAR} 为实际环境变量值
fn substitute_env_vars(content: &str) -> String {
    let mut result = content.to_string();

    while let Some(start) = result.find(ENV_VAR_PATTERN) {
        if let Some(end) = result[start..].find('}') {
            let var_end = start + end;
            let var_name = &result[start + 2..var_end];

            let replacement = std::env::var(var_name).unwrap_or_else(|_| {
                log::warn!("环境变量 {} 未设置，使用空字符串", var_name);
                String::new()
            });

            result = format!("{}{}{}", &result[..start], replacement, &result[var_end + 1..]);
        } else {
            break;
        }
    }

    result
}

/// 验证配置有效性
fn validate_config(config: &Config) -> anyhow::Result<()> {
    if config.providers.is_empty() {
        anyhow::bail!("至少需要配置一个 Provider");
    }

    for provider in &config.providers {
        if provider.api_key.is_empty() {
            anyhow::bail!("Provider '{}' 的 api_key 不能为空", provider.name);
        }
        if provider.base_url.is_empty() {
            anyhow::bail!("Provider '{}' 的 base_url 不能为空", provider.name);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_substitute_env_vars() {
        env::set_var("TEST_API_KEY", "secret-key-123");

        let input = r#"
[server]
api_key = "${TEST_API_KEY}"
base_url = "https://api.example.com"
"#;

        let result = substitute_env_vars(input);
        assert!(result.contains("secret-key-123"));
        assert!(!result.contains("${TEST_API_KEY}"));
    }

    #[test]
    fn test_substitute_env_vars_missing() {
        env::remove_var("MISSING_VAR");

        let input = r#"api_key = "${MISSING_VAR}""#;
        let result = substitute_env_vars(input);
        // 未设置的变量替换为空字符串
        assert!(result.contains(r#"api_key = """#));
    }
}
```

- [ ] **Step 2: 运行测试**

Run: `cd cc-proxy && cargo test config`

---

## Phase 3: 错误类型

### Task 4: error.rs — ProxyError

**Files:**
- Create: `cc-proxy/src/error.rs`

- [ ] **Step 1: 创建 src/error.rs**

```rust
//! 代理错误类型

use thiserror::Error;
use axum::{
    response::{IntoResponse, Response},
    http::StatusCode,
    Json,
};
use serde_json::{json, Value};

#[derive(Error, Debug)]
pub enum ProxyError {
    #[error("Bind failed: {0}")]
    BindFailed(String),

    #[error("Already running")]
    AlreadyRunning,

    #[error("Not running")]
    NotRunning,

    #[error("Stop timeout")]
    StopTimeout,

    #[error("Stop failed: {0}")]
    StopFailed(String),

    #[error("Transform error: {0}")]
    TransformError(String),

    #[error("Auth error: {0}")]
    AuthError(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("No available provider")]
    NoAvailableProvider,

    #[error("All providers circuit open")]
    AllProvidersCircuitOpen,

    #[error("Request timeout")]
    RequestTimeout,
}

impl IntoResponse for ProxyError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            ProxyError::BindFailed(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            ProxyError::AlreadyRunning => (StatusCode::CONFLICT, self.to_string()),
            ProxyError::NotRunning => (StatusCode::NOT_FOUND, self.to_string()),
            ProxyError::StopTimeout => (StatusCode::GATEWAY_TIMEOUT, self.to_string()),
            ProxyError::StopFailed(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            ProxyError::TransformError(_) => (StatusCode::BAD_GATEWAY, self.to_string()),
            ProxyError::AuthError(_) => (StatusCode::UNAUTHORIZED, self.to_string()),
            ProxyError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            ProxyError::NoAvailableProvider => (StatusCode::SERVICE_UNAVAILABLE, self.to_string()),
            ProxyError::AllProvidersCircuitOpen => (StatusCode::SERVICE_UNAVAILABLE, self.to_string()),
            ProxyError::RequestTimeout => (StatusCode::GATEWAY_TIMEOUT, self.to_string()),
        };

        let body = Json(json!({
            "error": {
                "type": "proxy_error",
                "message": message,
            }
        }));

        (status, body).into_response()
    }
}
```

- [ ] **Step 2: 运行 `cargo check`**

Run: `cd cc-proxy && cargo check 2>&1 | head -20`

---

## Phase 4: Provider 适配器

### Task 5: providers/auth.rs + adapter.rs + mod.rs

**Files:**
- Create: `cc-proxy/src/providers/auth.rs`
- Create: `cc-proxy/src/providers/adapter.rs`
- Create: `cc-proxy/src/providers/mod.rs`

- [ ] **Step 1: 创建 src/providers/auth.rs**

```rust
//! 认证信息类型

use http::HeaderValue;
use crate::error::ProxyError;

/// 认证信息
#[derive(Debug, Clone)]
pub struct AuthInfo {
    pub strategy: AuthStrategy,
    pub api_key: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthStrategy {
    ApiKey,        // x-api-key
    Bearer,        // Authorization: Bearer
    GoogApiKey,    // x-goog-api-key
}

impl AuthInfo {
    pub fn api_key(api_key: String) -> Self {
        Self {
            strategy: AuthStrategy::ApiKey,
            api_key,
        }
    }

    pub fn bearer(token: String) -> Self {
        Self {
            strategy: AuthStrategy::Bearer,
            api_key: token,
        }
    }

    pub fn goog_api_key(key: String) -> Self {
        Self {
            strategy: AuthStrategy::GoogApiKey,
            api_key: key,
        }
    }
}

/// 从 AuthInfo 获取 HTTP 认证头部
pub fn get_auth_headers(auth: &AuthInfo) -> Result<Vec<(http::HeaderName, HeaderValue)>, ProxyError> {
    match auth.strategy {
        AuthStrategy::ApiKey => {
            let name = http::header::HeaderName::from_static("x-api-key");
            let value = HeaderValue::from_str(&auth.api_key)
                .map_err(|e| ProxyError::AuthError(format!("invalid api key: {e}")))?;
            Ok(vec![(name, value)])
        }
        AuthStrategy::Bearer => {
            let name = http::header::AUTHORIZATION;
            let value = HeaderValue::from_str(&format!("Bearer {}", auth.api_key))
                .map_err(|e| ProxyError::AuthError(format!("invalid bearer token: {e}")))?;
            Ok(vec![(name, value)])
        }
        AuthStrategy::GoogApiKey => {
            let name = http::header::HeaderName::from_static("x-goog-api-key");
            let value = HeaderValue::from_str(&auth.api_key)
                .map_err(|e| ProxyError::AuthError(format!("invalid goog api key: {e}")))?;
            Ok(vec![(name, value)])
        }
    }
}
```

- [ ] **Step 2: 创建 src/providers/adapter.rs**

```rust
//! Provider Adapter Trait

use crate::error::ProxyError;
use crate::types::ProviderConfig;
use crate::providers::auth::AuthInfo;
use serde_json::Value;

/// Provider 适配器 Trait
pub trait ProviderAdapter: Send + Sync {
    fn name(&self) -> &'static str;
    fn extract_base_url(&self, provider: &ProviderConfig) -> String;
    fn extract_auth(&self, provider: &ProviderConfig) -> Option<AuthInfo>;
    fn build_url(&self, base_url: &str, endpoint: &str) -> String;
    fn needs_transform(&self, _provider: &ProviderConfig) -> bool {
        false
    }
    fn transform_request(&self, body: Value, _provider: &ProviderConfig) -> Result<Value, ProxyError> {
        Ok(body)
    }
}
```

- [ ] **Step 3: 创建 src/providers/mod.rs**

```rust
//! Provider 适配器模块

mod adapter;
mod auth;
mod claude;
mod gemini;
mod transform;
mod transform_responses;
mod transform_gemini;
mod streaming;
mod streaming_responses;
mod streaming_gemini;

pub use adapter::ProviderAdapter;
pub use auth::{AuthInfo, AuthStrategy, get_auth_headers};
pub use transform::{anthropic_to_openai, openai_to_anthropic};
pub use transform_responses::{anthropic_to_responses, responses_to_anthropic};
pub use transform_gemini::{anthropic_to_gemini, gemini_to_anthropic};
pub use streaming::{create_anthropic_sse_stream, create_openai_sse_stream};
pub use streaming_responses::create_anthropic_sse_stream_from_responses;
pub use streaming_gemini::create_anthropic_sse_stream_from_gemini;

use crate::types::{ProviderConfig, ProviderType};

/// 根据 ProviderType 获取对应适配器
pub fn get_adapter(provider_type: ProviderType) -> Box<dyn ProviderAdapter> {
    match provider_type {
        ProviderType::Claude | ProviderType::ClaudeAuth | ProviderType::OpenRouter => {
            Box::new(claude::ClaudeAdapter::new())
        }
        ProviderType::Codex => Box::new(claude::ClaudeAdapter::new()),
        ProviderType::Gemini => Box::new(gemini::GeminiAdapter::new()),
    }
}

/// 判断是否需要格式转换
pub fn needs_transform(provider_type: ProviderType) -> bool {
    false // 默认透传，由适配器覆盖
}
```

- [ ] **Step 4: 创建 src/providers/claude.rs (骨架)**

```rust
//! Claude Provider 适配器

use crate::providers::adapter::ProviderAdapter;
use crate::providers::auth::{AuthInfo, AuthStrategy};
use crate::types::ProviderConfig;

pub struct ClaudeAdapter {
    _phantom: std::marker::PhantomData<()>,
}

impl ClaudeAdapter {
    pub fn new() -> Self {
        Self {
            _phantom: std::marker::PhantomData,
        }
    }
}

impl ProviderAdapter for ClaudeAdapter {
    fn name(&self) -> &'static str {
        "Claude"
    }

    fn extract_base_url(&self, provider: &ProviderConfig) -> String {
        provider.base_url.clone()
    }

    fn extract_auth(&self, provider: &ProviderConfig) -> Option<AuthInfo> {
        Some(AuthInfo::api_key(provider.api_key.clone()))
    }

    fn build_url(&self, base_url: &str, endpoint: &str) -> String {
        format!("{}{}", base_url.trim_end_matches('/'), endpoint)
    }
}

impl Default for ClaudeAdapter {
    fn default() -> Self {
        Self::new()
    }
}
```

- [ ] **Step 5: 创建 src/providers/gemini.rs (骨架)**

```rust
//! Gemini Provider 适配器

use crate::providers::adapter::ProviderAdapter;
use crate::providers::auth::{AuthInfo, AuthStrategy};
use crate::types::ProviderConfig;

pub struct GeminiAdapter {
    _phantom: std::marker::PhantomData<()>,
}

impl GeminiAdapter {
    pub fn new() -> Self {
        Self {
            _phantom: std::marker::PhantomData,
        }
    }
}

impl ProviderAdapter for GeminiAdapter {
    fn name(&self) -> &'static str {
        "Gemini"
    }

    fn extract_base_url(&self, provider: &ProviderConfig) -> String {
        provider.base_url.clone()
    }

    fn extract_auth(&self, provider: &ProviderConfig) -> Option<AuthInfo> {
        Some(AuthInfo::goog_api_key(provider.api_key.clone()))
    }

    fn build_url(&self, base_url: &str, endpoint: &str) -> String {
        format!("{}{}", base_url.trim_end_matches('/'), endpoint)
    }
}

impl Default for GeminiAdapter {
    fn default() -> Self {
        Self::new()
    }
}
```

- [ ] **Step 6: 运行 `cargo check`**

Run: `cd cc-proxy && cargo check 2>&1 | grep -E "(error|warning)" | head -20`

---

### Task 6: transform.rs — Anthropic ↔ OpenAI Chat 转换

**Files:**
- Create: `cc-proxy/src/providers/transform.rs`
- Test: `cc-proxy/tests/transform_test.rs`

> 从 cc-proxy 的 `src-tauri/src/proxy/providers/transform.rs` 精简移植，移除数据库依赖，保留核心转换逻辑。

- [ ] **Step 1: 创建 tests/transform_test.rs**

```rust
use cc_proxy::providers::transform::{anthropic_to_openai, openai_to_anthropic};
use serde_json::{json, Value};

fn anthropic_to_openai(body: Value) -> Value {
    cc_proxy::providers::transform::anthropic_to_openai(body).unwrap()
}

fn openai_to_anthropic(body: Value) -> Value {
    cc_proxy::providers::transform::openai_to_anthropic(body).unwrap()
}

#[test]
fn test_simple_conversion() {
    let input = json!({
        "model": "claude-sonnet-4-6",
        "max_tokens": 1024,
        "messages": [{"role": "user", "content": "Hello"}]
    });

    let result = anthropic_to_openai(input);
    assert_eq!(result["model"], "claude-sonnet-4-6");
    assert_eq!(result["max_tokens"], 1024);
    assert_eq!(result["messages"][0]["role"], "user");
    assert_eq!(result["messages"][0]["content"], "Hello");
}

#[test]
fn test_system_prompt() {
    let input = json!({
        "model": "claude-sonnet-4-6",
        "system": "You are a helpful assistant.",
        "max_tokens": 1024,
        "messages": [{"role": "user", "content": "Hello"}]
    });

    let result = anthropic_to_openai(input);
    assert_eq!(result["messages"][0]["role"], "system");
    assert_eq!(result["messages"][0]["content"], "You are a helpful assistant.");
}

#[test]
fn test_tool_use_conversion() {
    let input = json!({
        "model": "claude-sonnet-4-6",
        "max_tokens": 1024,
        "messages": [{
            "role": "assistant",
            "content": [
                {"type": "text", "text": "Let me check"},
                {"type": "tool_use", "id": "call_123", "name": "get_weather", "input": {"location": "Tokyo"}}
            ]
        }]
    });

    let result = anthropic_to_openai(input);
    let msg = &result["messages"][0];
    assert_eq!(msg["role"], "assistant");
    assert!(msg.get("tool_calls").is_some());
    assert_eq!(msg["tool_calls"][0]["id"], "call_123");
}

#[test]
fn test_tool_result_conversion() {
    let input = json!({
        "model": "claude-sonnet-4-6",
        "max_tokens": 1024,
        "messages": [{
            "role": "user",
            "content": [
                {"type": "tool_result", "tool_use_id": "call_123", "content": "Sunny, 25°C"}
            ]
        }]
    });

    let result = anthropic_to_openai(input);
    let msg = &result["messages"][0];
    assert_eq!(msg["role"], "tool");
    assert_eq!(msg["tool_call_id"], "call_123");
}

#[test]
fn test_openai_to_anthropic_simple() {
    let input = json!({
        "id": "chatcmpl-123",
        "model": "gpt-4",
        "choices": [{
            "index": 0,
            "message": {"role": "assistant", "content": "Hello!"},
            "finish_reason": "stop"
        }],
        "usage": {"prompt_tokens": 10, "completion_tokens": 5}
    });

    let result = openai_to_anthropic(input);
    assert_eq!(result["id"], "chatcmpl-123");
    assert_eq!(result["type"], "message");
    assert_eq!(result["content"][0]["type"], "text");
    assert_eq!(result["content"][0]["text"], "Hello!");
    assert_eq!(result["stop_reason"], "end_turn");
}

#[test]
fn test_billing_header_stripping() {
    let input = json!({
        "model": "claude-sonnet-4-6",
        "max_tokens": 1024,
        "system": "x-anthropic-billing-header: cc_version=2.1;\n\nYou are a helpful assistant.",
        "messages": [{"role": "user", "content": "Hello"}]
    });

    let result = anthropic_to_openai(input);
    assert_eq!(
        result["messages"][0]["content"],
        "You are a helpful assistant."
    );
}

#[test]
fn test_o_series_max_completion_tokens() {
    let input = json!({
        "model": "o1",
        "max_tokens": 4096,
        "messages": [{"role": "user", "content": "Hello"}]
    });

    let result = anthropic_to_openai(input);
    assert!(result.get("max_tokens").is_none());
    assert_eq!(result["max_completion_tokens"], 4096);
}
```

- [ ] **Step 2: 运行测试验证**

Run: `cd cc-proxy && cargo test transform 2>&1 | tail -30`

- [ ] **Step 3: 实现 transform.rs 核心逻辑**

从 `cc-proxy/src-tauri/src/proxy/providers/transform.rs` 移植以下函数：
- `strip_leading_anthropic_billing_header()`
- `is_openai_o_series()`
- `supports_reasoning_effort()`
- `resolve_reasoning_effort()`
- `anthropic_to_openai()`
- `map_tool_choice_to_chat()`
- `normalize_openai_system_messages()`
- `convert_message_to_openai()`
- `clean_schema()`
- `openai_to_anthropic()`

> **关键**：移除所有 `crate::` 数据库引用，改为纯函数式实现。

- [ ] **Step 4: 再次运行测试**

Run: `cd cc-proxy && cargo test transform`

---

### Task 7: 格式转换（Responses API + Gemini）

**Files:**
- Create: `cc-proxy/src/providers/transform_responses.rs`
- Create: `cc-proxy/src/providers/transform_gemini.rs`

- [ ] **Step 1: 移植 transform_responses.rs**

从 `cc-proxy/src-tauri/src/proxy/providers/transform_responses.rs` 精简移植：
- `anthropic_to_responses()`
- `responses_to_anthropic()`

- [ ] **Step 2: 移植 transform_gemini.rs**

从 `cc-proxy/src-tauri/src/proxy/providers/transform_gemini.rs` 精简移植：
- `anthropic_to_gemini()`
- `gemini_to_anthropic()`

- [ ] **Step 3: 创建空的 streaming 骨架**

```rust
// cc-proxy/src/providers/streaming.rs
// 实现 SSE 流式转换
```

- [ ] **Step 4: 验证编译**

Run: `cd cc-proxy && cargo check 2>&1 | grep "^error" | head -10`

---

### Task 8: 流式转换

**Files:**
- Create: `cc-proxy/src/providers/streaming.rs`
- Create: `cc-proxy/src/providers/streaming_responses.rs`
- Create: `cc-proxy/src/providers/streaming_gemini.rs`
- Create: `cc-proxy/src/sse.rs`

- [ ] **Step 1: 移植 sse.rs**

从 `cc-proxy/src-tauri/src/proxy/sse.rs` 移植：
- `take_sse_block()`
- `strip_sse_field()`

- [ ] **Step 2: 移植 streaming.rs**

从 `cc-proxy/src-tauri/src/proxy/providers/streaming.rs` 精简移植：
- `create_anthropic_sse_stream()`
- `create_openai_sse_stream()`

- [ ] **Step 3: 移植其他 streaming**

- `streaming_responses.rs` → `create_anthropic_sse_stream_from_responses()`
- `streaming_gemini.rs` → `create_anthropic_sse_stream_from_gemini()`

- [ ] **Step 4: 验证编译**

Run: `cd cc-proxy && cargo check 2>&1 | grep "^error" | head -10`

---

## Phase 5: 核心代理逻辑

### Task 9: circuit_breaker.rs — 熔断器

**Files:**
- Create: `cc-proxy/src/circuit_breaker.rs`

- [ ] **Step 1: 从 cc-proxy 移植 circuit_breaker.rs**

从 `cc-proxy/src-tauri/src/proxy/circuit_breaker.rs` 精简移植，保留：
- `CircuitBreaker` 结构体
- `CircuitBreakerConfig` 配置
- `CircuitState` 状态机（Closed/Open/HalfOpen）
- `allow_request()` / `record_success()` / `record_failure()`
- 错误率计算逻辑

移除：
- 数据库写入
- UI 相关状态

- [ ] **Step 2: 验证编译**

Run: `cd cc-proxy && cargo check 2>&1 | grep "^error" | head -10`

---

### Task 10: router.rs — Provider 路由 + 故障转移

**Files:**
- Create: `cc-proxy/src/router.rs`

- [ ] **Step 1: 创建 src/router.rs**

基于 cc-proxy `provider_router.rs` 精简实现：

```rust
//! Provider 路由 + 熔断器管理

use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;

use crate::types::{Config, ProviderConfig, ProviderType};
use crate::circuit_breaker::{CircuitBreaker, CircuitBreakerConfig, AllowResult};

/// Provider 路由器
pub struct ProviderRouter {
    /// 已排序的 Provider 配置列表（按 priority 排序）
    providers: Vec<ProviderConfig>,
    /// 熔断器 Map: "provider_id" → CircuitBreaker
    circuit_breakers: Arc<RwLock<HashMap<String, Arc<CircuitBreaker>>>>,
    cb_config: CircuitBreakerConfig,
}

impl ProviderRouter {
    pub fn new(config: &Config) -> Self {
        let mut providers = config.providers.clone();
        // 按 priority 排序
        providers.sort_by_key(|p| p.priority);

        Self {
            providers,
            circuit_breakers: Arc::new(RwLock::new(HashMap::new())),
            cb_config: config.circuit_breaker.clone(),
        }
    }

    /// 选择可用的 Provider（按 priority 顺序，跳过熔断中的）
    pub async fn select_provider(&self) -> Option<ProviderConfig> {
        for provider in &self.providers {
            let breaker = self.get_or_create_breaker(&provider.name).await;
            if breaker.is_available().await {
                return Some(provider.clone());
            }
        }
        None
    }

    /// 记录请求结果，更新熔断器
    pub async fn record_result(&self, provider_name: &str, success: bool) {
        let breaker = self.get_or_create_breaker(provider_name).await;
        if success {
            breaker.record_success(false).await;
        } else {
            breaker.record_failure(false).await;
        }
    }

    async fn get_or_create_breaker(&self, name: &str) -> Arc<CircuitBreaker> {
        // 读锁优先
        {
            let breakers = self.circuit_breakers.read().await;
            if let Some(b) = breakers.get(name) {
                return b.clone();
            }
        }

        let mut breakers = self.circuit_breakers.write().await;
        if let Some(b) = breakers.get(name) {
            return b.clone();
        }

        let breaker = Arc::new(CircuitBreaker::new(self.cb_config.clone()));
        breakers.insert(name.to_string(), breaker.clone());
        breaker
    }

    /// 重置所有熔断器
    pub async fn reset_all(&self) {
        let mut breakers = self.circuit_breakers.write().await;
        for breaker in breakers.values() {
            breaker.reset().await;
        }
    }
}
```

- [ ] **Step 2: 验证编译**

Run: `cd cc-proxy && cargo check 2>&1 | grep "^error" | head -10`

---

### Task 11: http_client.rs — HTTP 客户端

**Files:**
- Create: `cc-proxy/src/http_client.rs`

- [ ] **Step 1: 移植 http_client.rs**

从 `cc-proxy/src-tauri/src/proxy/http_client.rs` 精简移植，保留：
- `ProxyHttpClient` 结构体
- HTTP/1.1 请求发送
- 超时控制
- 请求头透传

- [ ] **Step 2: 验证编译**

Run: `cd cc-proxy && cargo check 2>&1 | grep "^error" | head -10`

---

### Task 12: forwarder.rs — 请求转发

**Files:**
- Create: `cc-proxy/src/forwarder.rs`

- [ ] **Step 1: 创建 src/forwarder.rs**

核心职责：
1. 构建 URL（ProviderAdapter.build_url）
2. 注入认证头
3. 发送 HTTP 请求到上游
4. 处理响应状态码

```rust
//! 请求转发器

use bytes::Bytes;
use http_body_util::BodyExt;
use crate::error::ProxyError;
use crate::types::ProviderConfig;
use crate::providers::{ProviderAdapter, get_adapter, get_auth_headers};
use crate::http_client::ProxyHttpClient;

pub struct Forwarder {
    client: ProxyHttpClient,
}

impl Forwarder {
    pub fn new() -> Self {
        Self {
            client: ProxyHttpClient::new(),
        }
    }

    /// 转发请求到 Provider
    pub async fn forward(
        &self,
        provider: &ProviderConfig,
        endpoint: &str,
        method: http::Method,
        headers: http::HeaderMap,
        body: Vec<u8>,
    ) -> Result<ProxyResponse, ProxyError> {
        let adapter = get_adapter(ProviderType::from_str(&provider.provider_type));

        // 构建 URL
        let url = adapter.build_url(&provider.base_url, endpoint);

        // 获取认证头
        if let Some(auth) = adapter.extract_auth(provider) {
            let auth_headers = get_auth_headers(&auth)?;
            for (name, value) in auth_headers {
                headers.insert(name, value);
            }
        }

        // 发送请求
        let response = self.client
            .request(&url, method, headers, body)
            .await
            .map_err(|e| ProxyError::Internal(format!("HTTP request failed: {e}")))?;

        Ok(response)
    }
}

impl Default for Forwarder {
    fn default() -> Self {
        Self::new()
    }
}

/// Proxy 响应
pub struct ProxyResponse {
    pub status: u16,
    pub headers: http::HeaderMap,
    pub body: Bytes,
    pub is_sse: bool,
}
```

- [ ] **Step 2: 验证编译**

Run: `cd cc-proxy && cargo check 2>&1 | grep "^error" | head -10`

---

## Phase 6: 请求处理管道

### Task 13: Cache/Thinking 注入器

**Files:**
- Create: `cc-proxy/src/cache_injector.rs`
- Create: `cc-proxy/src/thinking_rectifier.rs`
- Create: `cc-proxy/src/thinking_budget_rectifier.rs`

- [ ] **Step 1: 移植 cache_injector.rs**

从 `cc-proxy/src-tauri/src/proxy/cache_injector.rs` 移植，保留：
- `inject()` 函数
- 移除对 `OptimizerConfig` 数据库的依赖，改为接受本地配置结构

- [ ] **Step 2: 移植 thinking_rectifier.rs**

从 `cc-proxy/src-tauri/src/proxy/thinking_rectifier.rs` 移植 `fix_thinking_signature()` 函数

- [ ] **Step 3: 移植 thinking_budget_rectifier.rs**

从 `cc-proxy/src-tauri/src/proxy/thinking_budget_rectifier.rs` 移植 `fix_thinking_budget()` 函数

- [ ] **Step 4: 验证编译**

Run: `cd cc-proxy && cargo check 2>&1 | grep "^error" | head -10`

---

### Task 14: handlers.rs — HTTP 处理器

**Files:**
- Create: `cc-proxy/src/handlers.rs`

- [ ] **Step 1: 创建 src/handlers.rs**

```rust
//! HTTP 请求处理器

use axum::{
    extract::{State, Request},
    response::IntoResponse,
    routing::{any, get, post},
    Json, Router,
};
use bytes::Bytes;
use http_body_util::BodyExt;
use serde_json::{json, Value};

use crate::error::ProxyError;
use crate::types::Config;
use crate::router::ProviderRouter;
use crate::forwarder::Forwarder;
use crate::cache_injector::inject as inject_cache;
use crate::thinking_rectifier::fix_thinking_signature;
use crate::thinking_budget_rectifier::fix_thinking_budget;

/// 应用状态
pub struct AppState {
    pub config: Config,
    pub router: ProviderRouter,
    pub forwarder: Forwarder,
}

impl Clone for AppState {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            router: ProviderRouter::new(&self.config),
            forwarder: Forwarder::new(),
        }
    }
}

/// 健康检查
pub async fn health_check() -> impl IntoResponse {
    (axum::http::StatusCode::OK, Json(json!({
        "status": "healthy",
        "timestamp": chrono::Utc::now().to_rfc3339(),
    })))
}

/// 服务状态
pub async fn get_status(State(state): State<AppState>) -> impl IntoResponse {
    Json(json!({
        "running": true,
        "address": state.config.server.listen_address,
        "port": state.config.server.listen_port,
        "providers_count": state.config.providers.len(),
    }))
}

/// 处理 /v1/messages（Claude API）
pub async fn handle_messages(
    State(state): State<AppState>,
    request: Request,
) -> Result<impl IntoResponse, ProxyError> {
    handle_anthropic(state, request, "/v1/messages").await
}

/// 处理 /v1/chat/completions（OpenAI Chat）
pub async fn handle_chat_completions(
    State(state): State<AppState>,
    request: Request,
) -> Result<impl IntoResponse, ProxyError> {
    // OpenAI Chat Completions 直接透传
    passthrough(state, request, "/v1/chat/completions").await
}

async fn handle_anthropic(
    state: AppState,
    request: Request,
    endpoint: &str,
) -> Result<impl IntoResponse, ProxyError> {
    let (parts, body) = request.into_parts();
    let headers = parts.headers;

    let body_bytes = body
        .collect()
        .await
        .map_err(|e| ProxyError::Internal(format!("读取请求体失败: {e}")))?
        .to_bytes();

    let mut request_body: Value = serde_json::from_slice(&body_bytes)
        .map_err(|e| ProxyError::Internal(format!("解析 JSON 失败: {e}")))?;

    // 1. Thinking 整流
    if state.config.thinking.signature_fix {
        fix_thinking_signature(&mut request_body);
    }
    if state.config.thinking.budget_fix {
        fix_thinking_budget(&mut request_body);
    }

    // 2. Cache 注入
    if state.config.cache.enabled {
        let cache_config = crate::types::CacheConfig {
            enabled: state.config.cache.enabled,
            ttl: state.config.cache.ttl.clone(),
        };
        inject_cache(&mut request_body, &cache_config);
    }

    // 3. 选择 Provider
    let provider = state.router.select_provider().await
        .ok_or(ProxyError::AllProvidersCircuitOpen)?;

    // 4. 格式转换（如需要）
    let (method, body_to_send) = if provider.provider_type == "claude_auth"
        || provider.provider_type == "openrouter"
    {
        let transformed = crate::providers::anthropic_to_openai(
            serde_json::to_vec(&request_body).unwrap()
        )?;
        (axum::http::Method::POST, transformed)
    } else {
        (axum::http::Method::POST, body_bytes.to_vec())
    };

    // 5. 转发请求
    let response = state.forwarder
        .forward(&provider, endpoint, method, headers, body_to_send)
        .await?;

    // 6. 记录结果
    state.router.record_result(&provider.name, response.status < 500).await;

    Ok((axum::http::StatusCode::from_u16(response.status).unwrap(), response.body))
}

async fn passthrough(
    state: AppState,
    request: Request,
    endpoint: &str,
) -> Result<impl IntoResponse, ProxyError> {
    let (parts, body) = request.into_parts();
    let headers = parts.headers;

    let body_bytes = body
        .collect()
        .await
        .map_err(|e| ProxyError::Internal(format!("读取请求体失败: {e}")))?
        .to_bytes();

    let provider = state.router.select_provider().await
        .ok_or(ProxyError::AllProvidersCircuitOpen)?;

    let response = state.forwarder
        .forward(&provider, endpoint, axum::http::Method::POST, headers, body_bytes.to_vec())
        .await?;

    state.router.record_result(&provider.name, response.status < 500).await;

    Ok((axum::http::StatusCode::from_u16(response.status).unwrap(), response.body))
}

/// 构建路由
pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/status", get(get_status))
        .route("/v1/messages", post(handle_messages))
        .route("/claude/v1/messages", post(handle_messages))
        .route("/v1/chat/completions", post(handle_chat_completions))
        .route("/chat/completions", post(handle_chat_completions))
        .route("/v1/responses", post(handle_messages))
        .route("/v1beta/*path", any(passthrough))
        .with_state(state)
}
```

- [ ] **Step 2: 验证编译**

Run: `cd cc-proxy && cargo check 2>&1 | grep "^error" | head -20`

---

## Phase 7: 服务器入口

### Task 15: server.rs — 服务器 + Graceful Shutdown

**Files:**
- Create: `cc-proxy/src/server.rs`

- [ ] **Step 1: 创建 src/server.rs**

```rust
//! Axum HTTP 服务器

use axum::serve;
use std::net::SocketAddr;
use tokio::signal;

use crate::config::Config;
use crate::handlers::{build_router, AppState};
use crate::router::ProviderRouter;
use crate::forwarder::Forwarder;

pub struct ProxyServer {
    config: Config,
}

impl ProxyServer {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    pub async fn run(self) -> anyhow::Result<()> {
        let addr: SocketAddr = format!(
            "{}:{}",
            self.config.server.listen_address,
            self.config.server.listen_port
        )
        .parse()
        .expect("无效的监听地址");

        let state = AppState {
            config: self.config.clone(),
            router: ProviderRouter::new(&self.config),
            forwarder: Forwarder::new(),
        };

        let router = build_router(state);

        let shutdown = async {
            signal::ctrl_c().await.expect("Failed to install CTRL+C handler");
            log::info!("收到 Ctrl+C 信号，开始优雅关闭...");
        };

        log::info!("cc-proxy 启动于 {}", addr);

        serve(tokio::net::TcpListener::bind(addr).await?, router)
            .with_graceful_shutdown(shutdown)
            .await?;

        log::info!("cc-proxy 已关闭");
        Ok(())
    }
}
```

- [ ] **Step 2: 更新 main.rs**

```rust
use cc_proxy::server::ProxyServer;
use cc_proxy::config::load;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    let config_path = std::env::args()
        .skip(1)
        .find(|arg| arg == "--config")
        .and_then(|_| std::env::args().skip(2).next())
        .unwrap_or_else(|| "config.toml".to_string());

    let config = load(&config_path)?;
    log::info!("配置加载成功: {} providers", config.providers.len());

    ProxyServer::new(config).run().await
}
```

- [ ] **Step 3: 验证编译**

Run: `cd cc-proxy && cargo build --release 2>&1 | tail -20`

---

## Phase 8: Docker 部署

### Task 16: Docker 构建

**Files:**
- Create: `cc-proxy/Dockerfile`
- Create: `cc-proxy/docker-compose.yml`
- Create: `cc-proxy/README.md`

- [ ] **Step 1: 创建 Dockerfile**

```dockerfile
# Stage 1: Build
FROM rust:1.85-slim as builder
WORKDIR /build

# 依赖缓存层
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo fetch 2>/dev/null || true

# 复制源码
COPY src ./src
COPY Cargo.toml Cargo.lock ./

# 构建
RUN cargo build --release
RUN strip target/release/cc-proxy || true

# Stage 2: Runtime
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    openssl \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/cc-proxy /usr/local/bin/cc-proxy

# 默认配置
COPY config.example.toml /etc/cc-proxy/config.toml

EXPOSE 15721

ENTRYPOINT ["cc-proxy", "--config", "/etc/cc-proxy/config.toml"]
```

- [ ] **Step 2: 创建 docker-compose.yml**

```yaml
version: "3.8"
services:
  cc-proxy:
    image: cc-proxy:latest
    build: .
    ports:
      - "15721:15721"
    volumes:
      - ./config.toml:/etc/cc-proxy/config.toml:ro
    environment:
      - ANTHROPIC_API_KEY=${ANTHROPIC_API_KEY}
      - OPENAI_API_KEY=${OPENAI_API_KEY}
      - OPENROUTER_API_KEY=${OPENROUTER_API_KEY}
    restart: unless-stopped
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:15721/health"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 10s
```

- [ ] **Step 3: 创建 README.md**

```markdown
# cc-proxy

独立的 Claude Code 代理服务，支持多 Provider 故障转移和格式转换。

## 快速开始

### Docker

```bash
# 构建
docker build -t cc-proxy .

# 运行
docker run -d \
  -p 15721:15721 \
  -v $(pwd)/config.toml:/etc/cc-proxy/config.toml:ro \
  -e ANTHROPIC_API_KEY=your-key \
  cc-proxy
```

### 源码编译

```bash
cargo build --release
./target/release/cc-proxy --config config.example.toml
```

## 配置

参考 `config.example.toml`。

## Claude Code 配置

```bash
export ANTHROPIC_BASE_URL=http://localhost:15721
export ANTHROPIC_AUTH_TOKEN=<your-api-key>
claude
```

## API 端点

| 端点 | 说明 |
|------|------|
| GET /health | 健康检查 |
| GET /status | 服务状态 |
| POST /v1/messages | Claude API |
| POST /v1/chat/completions | OpenAI Chat |
```

- [ ] **Step 4: 验证 Dockerfile 语法**

Run: `cd cc-proxy && docker build --no-cache -t cc-proxy:test . 2>&1 | tail -20`

---

## Phase 9: 端到端测试

### Task 17: 集成测试

- [ ] **Step 1: 运行完整测试套件**

Run: `cd cc-proxy && cargo test 2>&1 | tail -30`

- [ ] **Step 2: 手动测试健康检查**

```bash
# 启动服务
cargo run -- --config config.example.toml &
sleep 2

# 健康检查
curl http://localhost:15721/health

# 停止服务
pkill cc-proxy
```

- [ ] **Step 3: Docker 集成测试**

```bash
docker compose up -d
sleep 3
curl http://localhost:15721/health
docker compose down
```

---

## 实现顺序

```
Phase 1 (Task 1)  → 项目骨架 + cargo check
Phase 2 (Task 2-3) → types + config（配置层独立测试）
Phase 3 (Task 4)  → error type
Phase 4 (Task 5-8) → providers 适配器 + 格式转换
Phase 5 (Task 9-12) → circuit_breaker + router + http_client + forwarder
Phase 6 (Task 13) → cache + thinking 注入器
Phase 7 (Task 14-15) → handlers + server + main
Phase 8 (Task 16) → Docker
Phase 9 (Task 17) → 集成测试
```

---

## 自我检查清单

- [ ] 每个 Phase 完成后 `cargo check` 通过
- [ ] 所有 `cargo test` 通过
- [ ] Docker 镜像成功构建
- [ ] `docker compose up` 能正常启动
- [ ] `/health` 端点返回正常
- [ ] README 文档完整

---

**Plan complete.** 保存至 `docs/superpowers/plans/2026-05-15-cc-proxy-implementation.md`.

Two execution options:

**1. Subagent-Driven (recommended)** — I dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** — Execute tasks in this session using executing-plans

Which approach?