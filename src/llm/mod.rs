// SPDX-FileCopyrightText: 2026 AprilNEA LLC
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

mod anthropic;
mod cache;
mod model;
mod provider;

pub use anthropic::AnthropicProvider;
pub use cache::{fetch_models_cached, get_model_cache};
pub use model::Model;
pub use provider::{LlmProvider, ProviderConfig, StreamEvent};

use std::sync::Arc;

use anyhow::{anyhow, Result};

use crate::settings::{get_settings, Provider as SettingsProvider};

/// Create an LLM provider from application settings
pub fn create_provider_from_settings(cx: &gpui::App) -> Result<Arc<dyn LlmProvider>> {
    let settings = get_settings(cx);
    let provider = settings
        .active_provider()
        .ok_or_else(|| anyhow!("No active provider configured"))?;

    create_provider(provider)
}

/// Create an LLM provider from a settings Provider configuration
pub fn create_provider(provider: &SettingsProvider) -> Result<Arc<dyn LlmProvider>> {
    if provider.api_key.is_empty() {
        return Err(anyhow!("API key not configured for {}", provider.name));
    }

    let config = ProviderConfig::new(&provider.api_key, &provider.base_url);

    match provider.id.as_str() {
        "anthropic" => Ok(Arc::new(AnthropicProvider::new(config))),
        // Future providers can be added here:
        // "openai" => Ok(Arc::new(OpenAIProvider::new(config))),
        // "deepseek" => Ok(Arc::new(DeepSeekProvider::new(config))),
        _ => Err(anyhow!(
            "Provider '{}' is not yet implemented",
            provider.id
        )),
    }
}

/// Get the list of models for a provider by ID (sync, returns static/cached data)
pub fn get_models_for_provider(provider_id: &str) -> Vec<Model> {
    match provider_id {
        "anthropic" => AnthropicProvider::available_models(),
        // Add other providers here as they are implemented
        _ => vec![],
    }
}

/// Fetch models for a provider asynchronously (from API with caching)
pub async fn fetch_models_for_provider(provider: &SettingsProvider) -> Result<Vec<Model>> {
    let provider_instance = create_provider(provider)?;

    fetch_models_cached(&provider.id, async move {
        provider_instance.fetch_models().await
    })
    .await
}
