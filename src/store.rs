use crate::cli_config::AppConfig;
use crate::provider::Provider;
use crate::proxy::types::{
    AppProxyConfig, CopilotOptimizerConfig, OptimizerConfig, RectifierConfig,
};
use crate::services::ProxyService;
use std::collections::HashMap;
use std::sync::Arc;

/// 运行时配置（YAML 驱动）
///
/// 从 AppConfig 构建而来，在代理服务器启动时创建，
/// 通过 Arc 共享给 ProxyService、ProviderRouter 等组件。
#[derive(Debug)]
pub struct RuntimeConfig {
    /// 原始 AppConfig（用于构建代理配置等）
    pub app_config: Arc<AppConfig>,
    /// 每个应用的代理配置 (app_type -> AppProxyConfig)
    pub app_proxy_configs: HashMap<String, AppProxyConfig>,
    /// 每个应用的供应商列表 (app_type -> Vec<Provider>)，按 priority 排序
    pub providers_by_app: HashMap<String, Vec<Provider>>,
    /// 整流器配置
    pub rectifier_config: RectifierConfig,
    /// 优化器配置
    pub optimizer_config: OptimizerConfig,
    /// Copilot 优化器配置
    pub copilot_optimizer_config: CopilotOptimizerConfig,
}

impl RuntimeConfig {
    /// 从 AppConfig 构建 RuntimeConfig
    pub fn from_app_config(config: AppConfig) -> Self {
        // 构建每个应用的供应商列表
        // YAML 配置中的 providers 字段是全局的，按 provider_type 分组到各 app
        let providers_by_app = build_providers_by_app(&config.providers);

        // 构建 app 代理配置
        let app_proxy_configs = build_app_proxy_configs(&config);

        // 整流器/优化器配置暂时使用默认值
        let rectifier_config = RectifierConfig::default();
        let optimizer_config = OptimizerConfig::default();
        let copilot_optimizer_config = CopilotOptimizerConfig::default();

        Self {
            app_config: Arc::new(config),
            app_proxy_configs,
            providers_by_app,
            rectifier_config,
            optimizer_config,
            copilot_optimizer_config,
        }
    }
}

/// 将 YAML ProviderConfig 列表按 provider_type 分组到各应用
///
/// provider_type 映射到 app_type：
/// - "openai_chat" / "openai_responses" -> "claude" (Claude Code 用 OpenAI 兼容格式)
/// - "anthropic" -> "claude"
/// - "gemini_native" -> "gemini"
///
/// 所有供应商默认归入 "claude" 分组，除非明确指定为 gemini。
fn build_providers_by_app(providers: &[crate::cli_config::ProviderConfig]) -> HashMap<String, Vec<Provider>> {
    let mut map: HashMap<String, Vec<Provider>> = HashMap::new();

    for pc in providers {
        if !pc.enabled {
            continue;
        }
        let provider = Provider::from_yaml_config(pc);
        let app_type = match pc.provider_type.as_str() {
            "gemini_native" => "gemini",
            _ => "claude", // openai_chat, openai_responses, anthropic 等
        };
        map.entry(app_type.to_string())
            .or_default()
            .push(provider);
    }

    // 按 priority 排序（priority 数字越小优先级越高）
    for providers in map.values_mut() {
        providers.sort_by_key(|p| {
            p.sort_index.unwrap_or(999)
        });
    }

    map
}

/// 从 AppConfig 构建 per-app 代理配置
fn build_app_proxy_configs(config: &AppConfig) -> HashMap<String, AppProxyConfig> {
    let mut map = HashMap::new();

    for app_type in ["claude", "codex", "gemini"] {
        map.insert(
            app_type.to_string(),
            AppProxyConfig {
                app_type: app_type.to_string(),
                enabled: true,
                auto_failover_enabled: config.failover.enabled && config.failover.auto_switch,
                max_retries: 3,
                streaming_first_byte_timeout: 60,
                streaming_idle_timeout: 120,
                non_streaming_timeout: 600,
                circuit_failure_threshold: 5,
                circuit_success_threshold: 3,
                circuit_timeout_seconds: 30,
                circuit_error_rate_threshold: 0.5,
                circuit_min_requests: 10,
            },
        );
    }

    map
}

/// 全局应用状态
pub struct AppState {
    pub runtime: Arc<RuntimeConfig>,
    pub proxy_service: ProxyService,
}

impl AppState {
    /// 从 RuntimeConfig 创建应用状态（YAML 模式）
    pub fn from_runtime(runtime: RuntimeConfig) -> Self {
        let runtime = Arc::new(runtime);
        let proxy_service = ProxyService::new(runtime.clone());
        Self {
            runtime,
            proxy_service,
        }
    }
}