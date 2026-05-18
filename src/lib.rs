//! cc-proxy 核心库
//!
//! 提供 HTTP 代理服务核心逻辑，供 CLI 工具调用。

mod app_config;
mod claude_desktop_config;
mod claude_mcp;
mod codex_config;
mod config;
mod error;
mod gemini_config;
mod gemini_mcp;
pub mod hermes_config;
mod init_status;
mod mcp;
mod openclaw_config;
mod opencode_config;
mod panic_hook;
mod prompt;
mod prompt_files;
mod provider;
mod provider_defaults;
pub mod cli;
pub mod cli_config;
pub mod cli_stub;
pub mod yaml_store;
pub mod proxy;
mod services;
mod settings;
pub mod store;

pub use app_config::{AppType, InstalledSkill, McpApps, McpServer, MultiAppConfig, SkillApps};
pub use codex_config::{get_codex_auth_path, get_codex_config_path, write_codex_live_atomic};
pub use config::{get_claude_mcp_path, get_claude_settings_path, read_json_file};
pub use error::AppError;
pub use mcp::{
    import_from_claude, import_from_codex, import_from_gemini, remove_server_from_claude,
    remove_server_from_codex, remove_server_from_gemini, sync_enabled_to_claude,
    sync_enabled_to_codex, sync_enabled_to_gemini, sync_single_server_to_claude,
    sync_single_server_to_codex, sync_single_server_to_gemini,
};
pub use provider::{Provider, ProviderMeta};
pub use services::{
    ConfigService, EndpointLatency, McpService, PromptService, ProviderService, ProxyService,
    SkillService, SpeedtestService,
};
pub use settings::{update_settings, AppSettings};
pub use store::AppState;

/// CLI/Kubernetes 入口点
///
/// 在后台守护进程模式下调用，初始化应用状态并返回。
pub fn init_app_from_config(config_dir: &str) -> Result<AppState, AppError> {
    // 设置 panic hook
    panic_hook::setup_panic_hook();

    // 初始化 app config dir
    panic_hook::init_app_config_dir(std::path::PathBuf::from(config_dir));

    // 加载 YAML 配置
    let store = crate::yaml_store::YamlStore::new(std::path::PathBuf::from(config_dir));
    let app_config = store.load_config().map_err(|e| AppError::Message(format!("配置加载失败: {e}")))?;

    // 构建 RuntimeConfig
    let runtime = crate::store::RuntimeConfig::from_app_config(app_config);
    let app_state = AppState::from_runtime(runtime);

    // 初始化全局 HTTP 客户端（直连，无上游代理）
    if let Err(e) = proxy::http_client::init(None) {
        log::warn!("[GlobalProxy] 初始化直连失败: {e}，将使用默认配置");
    }

    Ok(app_state)
}

/// Placeholder — CLI 入口将替换 main.rs
pub fn run() {
    unimplemented!("CLI 入口尚未实现，请使用新入口")
}

/// CLI 入口点（供 main.rs 调用）
pub fn run_cli(cli: crate::cli::Cli) -> Result<(), String> {
    use crate::cli::{Commands, ConfigCommands, ProviderCommands, FailoverCommands};
    use crate::yaml_store::YamlStore;
    

    let config_dir = cli
        .config_dir
        .unwrap_or_else(YamlStore::default_dir);

    let store = YamlStore::new(config_dir.clone());

    match cli.command {
        Commands::Start { daemon } => {
            if daemon {
                eprintln!("Daemon mode not yet implemented - use foreground mode");
                return Err("Daemon mode not yet implemented".to_string());
            }
            // 前台启动代理服务器
            start_proxy(&store).map_err(|e| e.to_string())
        }
        Commands::Stop => {
            // TODO: 实现停止
            Err("Stop command not yet implemented".to_string())
        }
        Commands::Status => {
            // TODO: 实现状态查询
            Err("Status command not yet implemented".to_string())
        }
        Commands::Config { action } => {
            match action {
                ConfigCommands::List => {
                    let config = store.load_config().map_err(|e| e.to_string())?;
                    let yaml = serde_yaml::to_string(&config).map_err(|e| e.to_string())?;
                    println!("{}", yaml);
                    Ok(())
                }
                ConfigCommands::Get { key } => {
                    let config = store.load_config().map_err(|e| e.to_string())?;
                    match key {
                        None => {
                            let yaml = serde_yaml::to_string(&config).map_err(|e| e.to_string())?;
                            println!("{}", yaml);
                        }
                        Some(k) => {
                            let parts: Vec<&str> = k.split('.').collect();
                            if parts.is_empty() {
                                return Err("Invalid key".to_string());
                            }
                            // 获取嵌套值
                            let value = match parts[0] {
                                "proxy" => {
                                    if parts.len() == 2 {
                                        match parts[1] {
                                            "listen" => Some(serde_json::json!(config.proxy.listen)),
                                            "port" => Some(serde_json::json!(config.proxy.port)),
                                            "mode" => Some(serde_json::json!(config.proxy.mode)),
                                            _ => None
                                        }
                                    } else {
                                        None
                                    }
                                }
                                "failover" => {
                                    if parts.len() == 2 {
                                        match parts[1] {
                                            "enabled" => Some(serde_json::json!(config.failover.enabled)),
                                            "auto_switch" => Some(serde_json::json!(config.failover.auto_switch)),
                                            _ => None
                                        }
                                    } else {
                                        None
                                    }
                                }
                                "logging" => {
                                    if parts.len() == 2 && parts[1] == "level" {
                                        Some(serde_json::json!(config.logging.level))
                                    } else {
                                        None
                                    }
                                }
                                "providers" => {
                                    // providers 是数组，不支持 dotted key 访问单个项
                                    println!("Use 'config list' to see all providers");
                                    None
                                }
                                _ => None
                            };
                            match value {
                                Some(v) => println!("{}", v),
                                None => {
                                    return Err(format!("Unknown key: {}", k))
                                }
                            }
                        }
                    }
                    Ok(())
                }
                ConfigCommands::Set { key, value } => {
                    let mut config = store.load_config().map_err(|e| e.to_string())?;
                    let parts: Vec<&str> = key.split('.').collect();
                    if parts.is_empty() {
                        return Err("Invalid key".to_string());
                    }

                    match parts[0] {
                        "proxy" => {
                            if parts.len() == 2 {
                                match parts[1] {
                                    "listen" => config.proxy.listen = value,
                                    "port" => {
                                        config.proxy.port = value.parse().map_err(|e| format!("Invalid port: {}", e))?;
                                    }
                                    "mode" => config.proxy.mode = value,
                                    _ => return Err(format!("Unknown proxy key: {}", parts[1]))
                                }
                            } else {
                                return Err("Invalid proxy key".to_string());
                            }
                        }
                        "failover" => {
                            if parts.len() == 2 {
                                match parts[1] {
                                    "enabled" => {
                                        config.failover.enabled = value.parse().map_err(|e| format!("Invalid boolean: {}", e))?;
                                    }
                                    "auto_switch" => {
                                        config.failover.auto_switch = value.parse().map_err(|e| format!("Invalid boolean: {}", e))?;
                                    }
                                    _ => return Err(format!("Unknown failover key: {}", parts[1]))
                                }
                            } else {
                                return Err("Invalid failover key".to_string());
                            }
                        }
                        "logging" => {
                            if parts.len() == 2 && parts[1] == "level" {
                                config.logging.level = value;
                            } else {
                                return Err("Invalid logging key".to_string());
                            }
                        }
                        "providers" => {
                            return Err("Cannot set providers via key, use config list to see structure".to_string());
                        }
                        _ => return Err(format!("Unknown top-level key: {}", parts[0]))
                    }

                    store.save_config(&config).map_err(|e| e.to_string())?;
                    println!("Updated {}", key);
                    Ok(())
                }
            }
        }
        Commands::Provider { action } => {
            match action {
                ProviderCommands::List => {
                    let config = store.load_config().map_err(|e| e.to_string())?;
                    if config.providers.is_empty() {
                        println!("No providers configured. Edit config.yaml to add providers.");
                    } else {
                        println!("{:20} {:10} {:8} {:8}", "NAME", "TYPE", "PRIORITY", "ENABLED");
                        println!("{:-<20} {:-10} {:-8} {:-8}", "", "", "", "");
                        for p in &config.providers {
                            println!(
                                "{:20} {:10} {:8} {:8}",
                                p.name, p.provider_type, p.priority, p.enabled
                            );
                        }
                    }
                    Ok(())
                }
                ProviderCommands::Models { name } => {
                    let config = store.load_config().map_err(|e| e.to_string())?;
                    let provider = config.providers.iter()
                        .find(|p| p.name == name)
                        .ok_or_else(|| format!("Provider '{}' not found", name))?;

                    if provider.models.is_empty() {
                        println!("No models configured for '{}'", name);
                    } else {
                        println!("Models for '{}':", name);
                        for model in &provider.models {
                            println!("  - {}", model);
                        }
                    }
                    Ok(())
                }
                ProviderCommands::Health { name } => {
                    let config = store.load_config().map_err(|e| e.to_string())?;
                    let providers: Vec<_> = match name {
                        Some(ref n) => config.providers.iter().filter(|p| p.name == *n).collect(),
                        None => config.providers.iter().collect(),
                    };

                    if providers.is_empty() {
                        return Err(format!("Provider '{}' not found", name.unwrap_or_default()));
                    }

                    // 使用 tokio runtime 执行健康检查
                    let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
                    rt.block_on(async {
                        for provider in providers {
                            if !provider.enabled {
                                println!("{}: DISABLED", provider.name);
                                continue;
                            }

                            let start = std::time::Instant::now();
                            let client = reqwest::Client::new();
                            let result = client.get(&provider.base_url)
                                .timeout(std::time::Duration::from_secs(5))
                                .send()
                                .await;

                            match result {
                                Ok(response) => {
                                    let elapsed = start.elapsed().as_millis() as u64;
                                    println!("{}: OK ({}ms, status={})", provider.name, elapsed, response.status());
                                }
                                Err(e) => {
                                    println!("{}: FAILED ({})", provider.name, e);
                                }
                            }
                        }
                    });
                    Ok(())
                }
                ProviderCommands::TestEndpoint { url } => {
                    // 使用 tokio runtime 执行端点测试
                    let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
                    rt.block_on(async {
                        let start = std::time::Instant::now();
                        let client = reqwest::Client::new();
                        match client.get(&url)
                            .timeout(std::time::Duration::from_secs(10))
                            .send()
                            .await
                        {
                            Ok(response) => {
                                let elapsed = start.elapsed().as_millis() as u64;
                                println!("Endpoint: {}", url);
                                println!("Status: {}", response.status());
                                println!("Time: {}ms", elapsed);
                            }
                            Err(e) => {
                                eprintln!("Failed to connect: {}", e);
                            }
                        }
                    });
                    Ok(())
                }
                ProviderCommands::TestModel { name, model } => {
                    let config = store.load_config().map_err(|e| e.to_string())?;
                    let provider = config.providers.iter()
                        .find(|p| p.name == name)
                        .ok_or_else(|| format!("Provider '{}' not found", name))?;

                    let model_to_test = model.unwrap_or_else(|| {
                        provider.model_map.as_ref()
                            .and_then(|m| m.default.clone())
                            .unwrap_or_else(|| {
                                provider.models.first().cloned().unwrap_or_default()
                            })
                    });

                    // 获取 API 格式
                    let api_format = provider.model_map.as_ref()
                        .and_then(|m| m.api_format.clone())
                        .unwrap_or_else(|| "openai_chat".to_string());

                    // 根据 API 格式构建 URL
                    let base_url = provider.base_url.trim_end_matches('/');
                    let url = match api_format.as_str() {
                        "anthropic" => format!("{}/v1/messages", base_url),
                        "openai_responses" => {
                            if base_url.ends_with("/v1") {
                                format!("{}/responses", base_url)
                            } else {
                                format!("{}/v1/responses", base_url)
                            }
                        }
                        "gemini_native" => format!("{}/v1beta/models/{}:streamGenerateContent?alt=sse", base_url, model_to_test),
                        _ => {  // openai_chat
                            if base_url.contains("/v1") || base_url.contains("/v1beta") {
                                format!("{}/chat/completions", base_url)
                            } else {
                                format!("{}/v1/chat/completions", base_url)
                            }
                        }
                    };

                    // 使用 tokio runtime 执行模型测试
                    let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
                    rt.block_on(async {
                        let client = reqwest::Client::builder()
                            .timeout(std::time::Duration::from_secs(30))
                            .build()
                            .expect("Failed to build HTTP client");

                        let start = std::time::Instant::now();

                        // 根据 API 格式构建请求体
                        let body = if api_format == "anthropic" {
                            serde_json::json!({
                                "model": model_to_test,
                                "max_tokens": 1,
                                "messages": [{ "role": "user", "content": "Hi, reply with just 'OK'." }],
                                "stream": true
                            })
                        } else {
                            serde_json::json!({
                                "model": model_to_test,
                                "messages": [{ "role": "user", "content": "Hi, reply with just 'OK'." }],
                                "max_tokens": 10,
                                "stream": false
                            })
                        };

                        match client.post(&url)
                            .header("Authorization", format!("Bearer {}", provider.api_key))
                            .header("Content-Type", "application/json")
                            .header("Accept", "application/json")
                            .json(&body)
                            .send()
                            .await
                        {
                            Ok(response) => {
                                let elapsed = start.elapsed().as_millis() as u64;
                                let status = response.status().as_u16();
                                println!("Provider: {}", name);
                                println!("Model: {}", model_to_test);
                                println!("API Format: {}", api_format);
                                println!("URL: {}", url);
                                println!("Status: {}", status);
                                println!("Time: {}ms", elapsed);

                                if response.status().is_success() {
                                    if let Ok(json) = response.json::<serde_json::Value>().await {
                                        if let Some(content) = json.pointer("/choices/0/message/content")
                                            .or(json.pointer("/content/0/text"))
                                            .or(json.pointer("/candidates/0/content/parts/0/text"))
                                        {
                                            println!("Response: {}", content);
                                        }
                                    }
                                } else {
                                    if let Ok(text) = response.text().await {
                                        println!("Error: {}", text.chars().take(500).collect::<String>());
                                    }
                                }
                            }
                            Err(e) => {
                                eprintln!("Connection error: {}", e);
                            }
                        }
                    });
                    Ok(())
                }
            }
        }
        Commands::Failover { action } => {
            match action {
                FailoverCommands::Switch { name } => {
                    // 初始化应用状态
                    let config_dir_str = config_dir.to_string_lossy().to_string();
                    let app_state = init_app_from_config(&config_dir_str).map_err(|e| e.to_string())?;
                    let failover_manager = crate::proxy::failover_switch::FailoverSwitchManager::new(app_state.runtime.clone());

                    // 使用 tokio runtime 执行切换
                    let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
                    rt.block_on(async {
                        // 尝试找到 provider
                        let store = YamlStore::new(std::env::var("CC_SWITCH_CONFIG_DIR")
                            .map(std::path::PathBuf::from)
                            .unwrap_or_else(|_| YamlStore::default_dir()));
                        let config = match store.load_config() {
                            Ok(c) => c,
                            Err(e) => {
                                eprintln!("Failed to load config: {}", e);
                                return;
                            }
                        };

                        let provider = config.providers.iter()
                            .find(|p| p.name == name && p.enabled)
                            .ok_or_else(|| format!("Provider '{}' not found or disabled", name));

                        match provider {
                            Ok(p) => {
                                match failover_manager.try_switch("claude", &format!("provider-{}", p.name), &p.name).await {
                                    Ok(true) => println!("Switched to provider: {}", name),
                                    Ok(false) => println!("Provider '{}' is already current", name),
                                    Err(e) => eprintln!("Failed to switch: {}", e),
                                }
                            }
                            Err(e) => {
                                eprintln!("{}", e);
                            }
                        }
                    });
                    Ok(())
                }
            }
        }
    }
}

/// 启动代理服务器（前台模式）
fn start_proxy(store: &crate::yaml_store::YamlStore) -> Result<(), Box<dyn std::error::Error>> {
    // 加载 YAML 配置
    let config = store.load_config()?;

    // 设置日志级别
    let log_level = match config.logging.level.as_str() {
        "debug" => log::LevelFilter::Debug,
        "info" => log::LevelFilter::Info,
        "warn" => log::LevelFilter::Warn,
        "error" => log::LevelFilter::Error,
        _ => log::LevelFilter::Info,
    };
    env_logger::Builder::from_default_env()
        .filter_level(log_level)
        .init();

    println!("Starting cc-proxy proxy server...");
    println!("Listen: {}:{}", config.proxy.listen, config.proxy.port);
    println!("Mode: {}", config.proxy.mode);

    // 从 YAML 配置构建 RuntimeConfig
    let runtime = crate::store::RuntimeConfig::from_app_config(config.clone());
    let runtime = std::sync::Arc::new(runtime);

    // 使用 tokio runtime 运行服务器
    let rt = tokio::runtime::Runtime::new().expect("Failed to create runtime");
    rt.block_on(async {
        use crate::proxy::server::ProxyServer;
        use crate::proxy::types::ProxyConfig as ProxyConfigType;

        let proxy_config = ProxyConfigType {
            listen_address: config.proxy.listen.clone(),
            listen_port: config.proxy.port,
            ..Default::default()
        };

        let server = ProxyServer::new(proxy_config, runtime);
        let info = server.start().await.expect("Failed to start proxy server");

        println!("Proxy server started at {}:{}", info.address, info.port);
        println!("Press Ctrl+C to stop");

        // 等待 Ctrl+C
        let _ = tokio::signal::ctrl_c().await;
        println!("Shutting down...");
        let _ = server.stop().await;
        println!("Proxy server stopped");
    });

    Ok(())
}