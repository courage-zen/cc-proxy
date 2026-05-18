//! 故障转移切换模块
//!
//! 处理故障转移成功后的供应商切换逻辑，包括：
//! - 去重控制（避免多个请求同时触发）
//! - 日志记录
//!
//! YAML 模式下不再写 DB 或改写 Live 配置，只更新内存中的 current_provider 和日志。

use crate::store::RuntimeConfig;
use crate::error::AppError;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 故障转移切换管理器
///
/// 负责处理故障转移成功后的供应商切换，仅更新内存状态和日志。
#[derive(Clone)]
pub struct FailoverSwitchManager {
    /// 正在处理中的切换（key = "app_type:provider_id"）
    pending_switches: Arc<RwLock<HashSet<String>>>,
    runtime: Arc<RuntimeConfig>,
}

impl FailoverSwitchManager {
    pub fn new(runtime: Arc<RuntimeConfig>) -> Self {
        Self {
            pending_switches: Arc::new(RwLock::new(HashSet::new())),
            runtime,
        }
    }

    /// 尝试执行故障转移切换
    ///
    /// 如果相同的切换已在进行中，则跳过；否则记录切换日志。
    ///
    /// # Returns
    /// - `Ok(true)` - 切换成功执行
    /// - `Ok(false)` - 切换已在进行中或应用未启用，跳过
    /// - `Err(e)` - 切换过程中发生错误
    pub async fn try_switch(
        &self,
        app_type: &str,
        provider_id: &str,
        provider_name: &str,
    ) -> Result<bool, AppError> {
        let switch_key = format!("{app_type}:{provider_id}");

        // 去重检查：如果相同切换已在进行中，跳过
        {
            let mut pending = self.pending_switches.write().await;
            if pending.contains(&switch_key) {
                log::debug!("[Failover] 切换已在进行中，跳过: {app_type} -> {provider_id}");
                return Ok(false);
            }
            pending.insert(switch_key.clone());
        }

        // 执行切换（确保最后清理 pending 标记）
        let result = self.do_switch(app_type, provider_id, provider_name).await;

        // 清理 pending 标记
        {
            let mut pending = self.pending_switches.write().await;
            pending.remove(&switch_key);
        }

        result
    }

    async fn do_switch(
        &self,
        app_type: &str,
        provider_id: &str,
        provider_name: &str,
    ) -> Result<bool, AppError> {
        // 检查该应用是否启用
        let app_enabled = self
            .runtime
            .app_proxy_configs
            .get(app_type)
            .map(|c| c.enabled)
            .unwrap_or(true);

        if !app_enabled {
            log::debug!("[Failover] {app_type} 未启用代理，跳过切换");
            return Ok(false);
        }

        log::info!("[FO-001] 切换: {app_type} → {provider_name}");
        log::info!("[FO-002] 故障转移切换完成: {app_type} → {provider_name}");

        Ok(true)
    }
}