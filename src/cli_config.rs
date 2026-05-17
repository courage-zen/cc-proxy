//! CLI 配置类型定义
//!
//! 定义 YAML 配置文件中的所有配置结构

use serde::{Deserialize, Serialize};

/// 根配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub proxy: ProxyConfig,
    pub failover: FailoverConfig,
    pub logging: LoggingConfig,
    #[serde(default)]
    pub providers: Vec<ProviderConfig>,
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

/// 代理配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyConfig {
    /// 监听地址
    pub listen: String,
    /// 监听端口
    pub port: u16,
    /// 模式: "global" 或 "per-app"
    pub mode: String,
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

/// 故障转移配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailoverConfig {
    pub enabled: bool,
    pub auto_switch: bool,
}

impl Default for FailoverConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            auto_switch: true,
        }
    }
}

/// 日志配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
        }
    }
}

/// Provider 配置
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
    #[serde(default)]
    pub model_map: Option<ModelMap>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelMap {
    pub default: Option<String>,
    pub sonnet: Option<String>,
    pub opus: Option<String>,
    pub haiku: Option<String>,
    /// API 格式：openai_chat（默认）, anthropic, openai_responses, gemini_native
    #[serde(rename = "api_format")]
    #[serde(default)]
    pub api_format: Option<String>,
}