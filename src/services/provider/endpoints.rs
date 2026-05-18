//! Custom endpoints management
//!
//! Handles CRUD operations for provider custom endpoints.

use std::time::{SystemTime, UNIX_EPOCH};

use crate::app_config::AppType;
use crate::error::AppError;
use crate::settings::CustomEndpoint;
use crate::store::AppState;

/// Get custom endpoints list for a provider
pub fn get_custom_endpoints(
    state: &AppState,
    app_type: AppType,
    provider_id: &str,
) -> Result<Vec<CustomEndpoint>, AppError> {
    let providers = state.runtime.providers_by_app.get(app_type.as_str()).cloned().unwrap_or_default();
    let Some(provider) = providers.iter().find(|p| p.id == provider_id) else {
        return Ok(vec![]);
    };
    let Some(meta) = provider.meta.as_ref() else {
        return Ok(vec![]);
    };
    if meta.custom_endpoints.is_empty() {
        return Ok(vec![]);
    }

    let mut result: Vec<_> = meta.custom_endpoints.values().cloned().collect();
    result.sort_by_key(|ep| std::cmp::Reverse(ep.added_at));
    Ok(result)
}

/// Add a custom endpoint to a provider
pub fn add_custom_endpoint(
    state: &AppState,
    app_type: AppType,
    provider_id: &str,
    url: String,
) -> Result<(), AppError> {
    let normalized = url.trim().trim_end_matches('/').to_string();
    if normalized.is_empty() {
        return Err(AppError::localized(
            "provider.endpoint.url_required",
            "URL 不能为空",
            "URL cannot be empty",
        ));
    }

    unimplemented!("DB removed");
    Ok(())
}

/// Remove a custom endpoint from a provider
pub fn remove_custom_endpoint(
    state: &AppState,
    app_type: AppType,
    provider_id: &str,
    url: String,
) -> Result<(), AppError> {
    let normalized = url.trim().trim_end_matches('/').to_string();
    unimplemented!("DB removed");
    Ok(())
}

/// Update endpoint last used timestamp
pub fn update_endpoint_last_used(
    _state: &AppState,
    _app_type: AppType,
    _provider_id: &str,
    _url: String,
) -> Result<(), AppError> {
    unimplemented!("DB removed")
}

/// Get current timestamp in milliseconds
fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}
