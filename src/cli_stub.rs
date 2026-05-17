//! CLI 模式下 stub 类型
//!
//! CopilotAuthState 和 CodexOAuthState 在 CLI 模式下不需要（项目不使用 Copilot/Codex）。
//! 提供最小化 stub 避免 forwarder.rs 编译错误。

use std::sync::Arc;
use tokio::sync::RwLock;

/// Stub: Copilot 认证状态管理器（CLI 模式下为空）
pub struct CopilotAuthManager;

impl CopilotAuthManager {
    pub fn new(_config_dir: std::path::PathBuf) -> Self {
        Self
    }

    pub async fn get_valid_token(&self) -> Result<String, String> {
        Err("Copilot auth not available in CLI mode".to_string())
    }

    pub async fn get_valid_token_for_account(&self, _id: &str) -> Result<String, String> {
        Err("Copilot auth not available in CLI mode".to_string())
    }

    pub async fn get_default_api_endpoint(&self) -> String {
        String::new()
    }

    pub async fn get_api_endpoint(&self, _id: &str) -> String {
        String::new()
    }

    pub async fn fetch_models(&self) -> Result<Vec<String>, String> {
        Err("Copilot not available in CLI mode".to_string())
    }

    pub async fn fetch_models_for_account(&self, _id: &str) -> Result<Vec<String>, String> {
        Err("Copilot not available in CLI mode".to_string())
    }

    pub async fn get_model_vendor(&self, _model_id: &str) -> Result<String, String> {
        Err("Copilot not available in CLI mode".to_string())
    }

    pub async fn get_model_vendor_for_account(&self, _id: &str, _model_id: &str) -> Result<String, String> {
        Err("Copilot not available in CLI mode".to_string())
    }
}

/// Stub: Copilot 认证状态
pub struct CopilotAuthState(pub Arc<RwLock<CopilotAuthManager>>);

/// Stub: Codex OAuth 状态管理器（CLI 模式下为空）
pub struct CodexOAuthManager;

impl CodexOAuthManager {
    pub fn new(_config_dir: std::path::PathBuf) -> Self {
        Self
    }

    pub async fn get_valid_token(&self) -> Result<String, String> {
        Err("Codex OAuth not available in CLI mode".to_string())
    }

    pub async fn get_valid_token_for_account(&self, _id: &str) -> Result<String, String> {
        Err("Codex OAuth not available in CLI mode".to_string())
    }

    pub async fn default_account_id(&self) -> Option<String> {
        None
    }
}

/// Stub: Codex OAuth 状态
pub struct CodexOAuthState(pub Arc<RwLock<CodexOAuthManager>>);

/// Stub: WebDAV 自动同步通知（CLI 模式下为空实现）
pub fn notify_db_changed(_table: &str) {
    // CLI 模式不支持 WebDAV 自动同步
}