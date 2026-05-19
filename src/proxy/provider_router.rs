//! 供应商路由器模块
//!
//! 负责选择和管理代理目标供应商，实现智能故障转移

use crate::error::AppError;
use crate::provider::Provider;
use crate::proxy::circuit_breaker::{AllowResult, CircuitBreaker, CircuitBreakerConfig};
use crate::store::RuntimeConfig;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 供应商路由器
pub struct ProviderRouter {
    /// 运行时配置（YAML 驱动）
    runtime: Arc<RuntimeConfig>,
    /// 熔断器管理器 - key 格式: "app_type:provider_id"
    circuit_breakers: Arc<RwLock<HashMap<String, Arc<CircuitBreaker>>>>,
}

impl ProviderRouter {
    /// 创建新的供应商路由器
    pub fn new(runtime: Arc<RuntimeConfig>) -> Self {
        Self {
            runtime,
            circuit_breakers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 选择可用的供应商（支持故障转移）
    ///
    /// 返回按优先级排序的可用供应商列表：
    /// - 故障转移关闭时：仅返回最高优先级的供应商
    /// - 故障转移开启时：按 priority 顺序依次尝试（P1 → P2 → ...）
    pub async fn select_providers(&self, app_type: &str) -> Result<Vec<Provider>, AppError> {
        let mut result = Vec::new();
        let mut total_providers = 0usize;
        let mut circuit_open_count = 0usize;

        // 从 RuntimeConfig 读取该应用的自动故障转移开关
        let auto_failover_enabled = self
            .runtime
            .app_proxy_configs
            .get(app_type)
            .map(|c| c.auto_failover_enabled)
            .unwrap_or(true);

        // 从 RuntimeConfig 读取 providers
        let providers = self
            .runtime
            .providers_by_app
            .get(app_type)
            .cloned()
            .unwrap_or_default();

        if auto_failover_enabled {
            // 故障转移开启：按 priority 顺序依次尝试
            total_providers = providers.len();

            for provider in &providers {
                let circuit_key = format!("{app_type}:{}", provider.id);
                let breaker = self.get_or_create_circuit_breaker(&circuit_key).await;

                if breaker.is_available().await {
                    result.push(provider.clone());
                } else {
                    circuit_open_count += 1;
                }
            }
        } else {
            // 故障转移关闭：仅使用最高优先级供应商
            if let Some(first) = providers.first() {
                total_providers = 1;
                result.push(first.clone());
            }
        }

        if result.is_empty() {
            if total_providers > 0 && circuit_open_count == total_providers {
                log::warn!("[{app_type}] [FO-004] 所有供应商均已熔断");
                return Err(AppError::AllProvidersCircuitOpen);
            } else {
                log::warn!("[{app_type}] [FO-005] 未配置供应商");
                return Err(AppError::NoProvidersConfigured);
            }
        }

        Ok(result)
    }

    /// 请求执行前获取熔断器"放行许可"
    pub async fn allow_provider_request(&self, provider_id: &str, app_type: &str) -> AllowResult {
        let circuit_key = format!("{app_type}:{provider_id}");
        let breaker = self.get_or_create_circuit_breaker(&circuit_key).await;
        breaker.allow_request().await
    }

    /// 记录供应商请求结果
    pub async fn record_result(
        &self,
        provider_id: &str,
        app_type: &str,
        used_half_open_permit: bool,
        success: bool,
        error_msg: Option<String>,
    ) -> Result<(), AppError> {
        // 更新熔断器状态
        let circuit_key = format!("{app_type}:{provider_id}");
        let breaker = self.get_or_create_circuit_breaker(&circuit_key).await;

        if success {
            breaker.record_success(used_half_open_permit).await;
        } else {
            breaker.record_failure(used_half_open_permit).await;
        }

        if let Some(msg) = error_msg {
            log::debug!("[{app_type}:{provider_id}] request failed: {msg}");
        }

        Ok(())
    }

    /// 重置熔断器（手动恢复）
    pub async fn reset_circuit_breaker(&self, circuit_key: &str) {
        let breakers = self.circuit_breakers.read().await;
        if let Some(breaker) = breakers.get(circuit_key) {
            breaker.reset().await;
        }
    }

    /// 重置指定供应商的熔断器
    pub async fn reset_provider_breaker(&self, provider_id: &str, app_type: &str) {
        let circuit_key = format!("{app_type}:{provider_id}");
        self.reset_circuit_breaker(&circuit_key).await;
    }

    /// 仅释放 HalfOpen permit，不影响健康统计（neutral 接口）
    pub async fn release_permit_neutral(
        &self,
        provider_id: &str,
        app_type: &str,
        used_half_open_permit: bool,
    ) {
        if !used_half_open_permit {
            return;
        }
        let circuit_key = format!("{app_type}:{provider_id}");
        let breaker = self.get_or_create_circuit_breaker(&circuit_key).await;
        breaker.release_half_open_permit();
    }

    /// 更新所有熔断器的配置（热更新）
    pub async fn update_all_configs(&self, config: CircuitBreakerConfig) {
        let breakers = self.circuit_breakers.read().await;
        for breaker in breakers.values() {
            breaker.update_config(config.clone()).await;
        }
    }

    /// 获取熔断器状态
    #[allow(dead_code)]
    pub async fn get_circuit_breaker_stats(
        &self,
        provider_id: &str,
        app_type: &str,
    ) -> Option<crate::proxy::circuit_breaker::CircuitBreakerStats> {
        let circuit_key = format!("{app_type}:{provider_id}");
        let breakers = self.circuit_breakers.read().await;

        if let Some(breaker) = breakers.get(&circuit_key) {
            Some(breaker.get_stats().await)
        } else {
            None
        }
    }

    /// 获取或创建熔断器
    async fn get_or_create_circuit_breaker(&self, key: &str) -> Arc<CircuitBreaker> {
        // 先尝试读锁获取
        {
            let breakers = self.circuit_breakers.read().await;
            if let Some(breaker) = breakers.get(key) {
                return breaker.clone();
            }
        }

        // 如果不存在，获取写锁创建
        let mut breakers = self.circuit_breakers.write().await;

        // 双重检查，防止竞争条件
        if let Some(breaker) = breakers.get(key) {
            return breaker.clone();
        }

        // 从 RuntimeConfig 读取熔断器配置
        let app_type = key.split(':').next().unwrap_or("claude");
        let config = self
            .runtime
            .app_proxy_configs
            .get(app_type)
            .map(|c| CircuitBreakerConfig {
                failure_threshold: c.circuit_failure_threshold,
                success_threshold: c.circuit_success_threshold,
                timeout_seconds: c.circuit_timeout_seconds as u64,
                error_rate_threshold: c.circuit_error_rate_threshold,
                min_requests: c.circuit_min_requests,
            })
            .unwrap_or_default();

        let breaker = Arc::new(CircuitBreaker::new(config));
        breakers.insert(key.to_string(), breaker.clone());

        breaker
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli_config::AppConfig;
    use crate::cli_config::ProviderConfig;

    fn make_runtime_with_providers(providers: Vec<ProviderConfig>) -> Arc<RuntimeConfig> {
        let mut config = AppConfig::default();
        config.providers = providers;
        Arc::new(RuntimeConfig::from_app_config(config))
    }

    #[tokio::test]
    async fn test_failover_disabled_uses_first_provider() {
        let runtime = make_runtime_with_providers(vec![
            ProviderConfig {
                name: "A".to_string(),
                provider_type: "openai_chat".to_string(),
                api_key: "key-a".to_string(),
                base_url: "http://a".to_string(),
                models: vec!["model".to_string()],
                priority: 1,
                enabled: true,
                model_map: None,
            },
            ProviderConfig {
                name: "B".to_string(),
                provider_type: "openai_chat".to_string(),
                api_key: "key-b".to_string(),
                base_url: "http://b".to_string(),
                models: vec!["model".to_string()],
                priority: 2,
                enabled: true,
                model_map: None,
            },
        ]);

        // 禁用故障转移
        let mut app_config = (*runtime.app_config).clone();
        app_config.failover.enabled = false;
        app_config.failover.auto_switch = false;
        let runtime = Arc::new(RuntimeConfig::from_app_config(app_config));

        let router = ProviderRouter::new(runtime);
        let providers = router.select_providers("claude").await.unwrap();

        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0].name, "A");
    }

    #[tokio::test]
    async fn test_failover_enabled_returns_all_providers() {
        let runtime = make_runtime_with_providers(vec![
            ProviderConfig {
                name: "A".to_string(),
                provider_type: "openai_chat".to_string(),
                api_key: "key-a".to_string(),
                base_url: "http://a".to_string(),
                models: vec!["model".to_string()],
                priority: 2,
                enabled: true,
                model_map: None,
            },
            ProviderConfig {
                name: "B".to_string(),
                provider_type: "openai_chat".to_string(),
                api_key: "key-b".to_string(),
                base_url: "http://b".to_string(),
                models: vec!["model".to_string()],
                priority: 1,
                enabled: true,
                model_map: None,
            },
        ]);

        let router = ProviderRouter::new(runtime);
        let providers = router.select_providers("claude").await.unwrap();

        // 按 priority 排序：B(1) 在 A(2) 前
        assert_eq!(providers.len(), 2);
        assert_eq!(providers[0].name, "B");
        assert_eq!(providers[1].name, "A");
    }

    #[tokio::test]
    async fn test_no_providers_returns_error() {
        let runtime = make_runtime_with_providers(vec![]);
        let router = ProviderRouter::new(runtime);
        let result = router.select_providers("claude").await;
        assert!(result.is_err());
    }
}