//! Usage Logger - 记录 API 请求使用情况
//!
//! YAML 模式下改为结构化 JSON 输出到 stdout（由 Docker 日志驱动管理）。

use super::calculator::{CostBreakdown, CostCalculator, ModelPricing};
use super::parser::TokenUsage;
use crate::error::AppError;
use rust_decimal::Decimal;
use serde::Serialize;

/// 请求日志
#[derive(Debug, Clone)]
pub struct RequestLog {
    pub request_id: String,
    pub provider_id: String,
    pub app_type: String,
    pub model: String,
    pub request_model: String,
    pub usage: TokenUsage,
    pub cost: Option<CostBreakdown>,
    pub latency_ms: u64,
    pub first_token_ms: Option<u64>,
    pub status_code: u16,
    pub error_message: Option<String>,
    pub session_id: Option<String>,
    /// 供应商类型 (claude, claude_auth, codex, gemini, gemini_cli, openrouter)
    pub provider_type: Option<String>,
    /// 是否为流式请求
    pub is_streaming: bool,
    /// 成本倍数
    pub cost_multiplier: String,
}

/// JSON 日志输出格式
#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
struct JsonLogEntry {
    log_type: &'static str,
    request_id: String,
    provider_id: String,
    app_type: String,
    model: String,
    request_model: String,
    input_tokens: u32,
    output_tokens: u32,
    cache_read_tokens: u32,
    cache_creation_tokens: u32,
    input_cost_usd: String,
    output_cost_usd: String,
    total_cost_usd: String,
    latency_ms: u64,
    first_token_ms: Option<u64>,
    status_code: u16,
    error_message: Option<String>,
    session_id: Option<String>,
    provider_type: Option<String>,
    is_streaming: bool,
    cost_multiplier: String,
}

/// 使用量记录器（日志输出到 stdout）
pub struct UsageLogger;

impl UsageLogger {
    pub fn new() -> Self {
        Self
    }

    /// 记录成功的请求 — 输出结构化 JSON 到 stdout
    pub fn log_request(&self, log: &RequestLog) -> Result<(), AppError> {
        let (input_cost, output_cost, total_cost) = if let Some(cost) = &log.cost {
            (
                cost.input_cost.to_string(),
                cost.output_cost.to_string(),
                cost.total_cost.to_string(),
            )
        } else {
            ("0".to_string(), "0".to_string(), "0".to_string())
        };

        let entry = JsonLogEntry {
            log_type: "usage",
            request_id: log.request_id.clone(),
            provider_id: log.provider_id.clone(),
            app_type: log.app_type.clone(),
            model: log.model.clone(),
            request_model: log.request_model.clone(),
            input_tokens: log.usage.input_tokens,
            output_tokens: log.usage.output_tokens,
            cache_read_tokens: log.usage.cache_read_tokens,
            cache_creation_tokens: log.usage.cache_creation_tokens,
            input_cost_usd: input_cost,
            output_cost_usd: output_cost,
            total_cost_usd: total_cost,
            latency_ms: log.latency_ms,
            first_token_ms: log.first_token_ms,
            status_code: log.status_code,
            error_message: log.error_message.clone(),
            session_id: log.session_id.clone(),
            provider_type: log.provider_type.clone(),
            is_streaming: log.is_streaming,
            cost_multiplier: log.cost_multiplier.clone(),
        };

        if let Ok(json) = serde_json::to_string(&entry) {
            log::info!("[USG] {}", json);
        }

        Ok(())
    }

    /// 记录失败的请求
    #[allow(dead_code, clippy::too_many_arguments)]
    pub fn log_error(
        &self,
        request_id: String,
        provider_id: String,
        app_type: String,
        model: String,
        status_code: u16,
        error_message: String,
        latency_ms: u64,
    ) -> Result<(), AppError> {
        let request_model = model.clone();
        let log = RequestLog {
            request_id,
            provider_id,
            app_type,
            model,
            request_model,
            usage: TokenUsage::default(),
            cost: None,
            latency_ms,
            first_token_ms: None,
            status_code,
            error_message: Some(error_message),
            session_id: None,
            provider_type: None,
            is_streaming: false,
            cost_multiplier: "1.0".to_string(),
        };

        self.log_request(&log)
    }

    /// 记录失败的请求（带更多上下文信息）
    #[allow(clippy::too_many_arguments)]
    pub fn log_error_with_context(
        &self,
        request_id: String,
        provider_id: String,
        app_type: String,
        model: String,
        status_code: u16,
        error_message: String,
        latency_ms: u64,
        is_streaming: bool,
        session_id: Option<String>,
        provider_type: Option<String>,
    ) -> Result<(), AppError> {
        let request_model = model.clone();
        let log = RequestLog {
            request_id,
            provider_id,
            app_type,
            model,
            request_model,
            usage: TokenUsage::default(),
            cost: None,
            latency_ms,
            first_token_ms: None,
            status_code,
            error_message: Some(error_message),
            session_id,
            provider_type,
            is_streaming,
            cost_multiplier: "1.0".to_string(),
        };

        self.log_request(&log)
    }

    /// 获取模型定价 — 简化版，使用硬编码的默认定价
    ///
    /// YAML 模式下没有 model_pricing 表，返回 None 让成本记录为 0
    pub fn get_model_pricing(&self, _model_id: &str) -> Result<Option<ModelPricing>, AppError> {
        Ok(None)
    }

    /// 获取有效的倍率与计费模式来源 — 简化版，固定返回默认值
    pub fn resolve_pricing_config(&self, _provider_id: &str, _app_type: &str) -> (Decimal, String) {
        // YAML 模式下默认倍率 1.0，计费模式 response
        (Decimal::from(1), "response".to_string())
    }

    /// 计算并记录请求
    #[allow(clippy::too_many_arguments)]
    pub fn log_with_calculation(
        &self,
        request_id: String,
        provider_id: String,
        app_type: String,
        model: String,
        request_model: String,
        pricing_model: String,
        usage: TokenUsage,
        cost_multiplier: Decimal,
        latency_ms: u64,
        first_token_ms: Option<u64>,
        status_code: u16,
        session_id: Option<String>,
        provider_type: Option<String>,
        is_streaming: bool,
    ) -> Result<(), AppError> {
        let pricing = self.get_model_pricing(&pricing_model)?;

        let has_usage = usage.input_tokens > 0
            || usage.output_tokens > 0
            || usage.cache_read_tokens > 0
            || usage.cache_creation_tokens > 0;

        if pricing.is_none() && has_usage {
            log::debug!("[USG-002] 模型定价未找到，成本将记录为 0: {pricing_model}");
        }

        let cost = CostCalculator::try_calculate_for_app(
            &app_type,
            &usage,
            pricing.as_ref(),
            cost_multiplier,
        );

        let log = RequestLog {
            request_id,
            provider_id,
            app_type,
            model,
            request_model,
            usage,
            cost,
            latency_ms,
            first_token_ms,
            status_code,
            error_message: None,
            session_id,
            provider_type,
            is_streaming,
            cost_multiplier: cost_multiplier.to_string(),
        };

        self.log_request(&log)
    }
}