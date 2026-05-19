use crate::app_config::{AppType, McpServer};
use crate::error::AppError;
use crate::mcp;
use crate::store::AppState;

/// MCP 相关业务逻辑
pub struct McpService;

impl McpService {
    /// 同步所有启用的 MCP 服务器到对应的应用
    /// YAML 模式下没有数据库持久化的 MCP 服务器，直接返回 Ok
    pub fn sync_all_enabled(_state: &AppState) -> Result<(), AppError> {
        Ok(())
    }
}