//! Custom endpoints management

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
