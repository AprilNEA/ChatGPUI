// SPDX-FileCopyrightText: 2026 AprilNEA LLC
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

use anyhow::Result;
use async_trait::async_trait;

use crate::message::ChatMessage;

use super::model::Model;

/// Result type for streaming events
#[derive(Debug, Clone)]
pub enum StreamEvent {
    /// Incremental text delta
    Delta(String),
    /// Stream completed successfully
    Done,
    /// Error occurred during streaming
    Error(String),
}

/// Configuration for an LLM provider
#[derive(Debug, Clone)]
pub struct ProviderConfig {
    pub api_key: String,
    pub base_url: String,
}

impl ProviderConfig {
    pub fn new(api_key: impl Into<String>, base_url: impl Into<String>) -> Self {
        Self {
            // Trim whitespace from API key to handle copy-paste issues
            api_key: api_key.into().trim().to_string(),
            base_url: base_url.into().trim().to_string(),
        }
    }
}

/// Trait that all LLM providers must implement
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Returns the unique identifier for this provider (e.g., "anthropic")
    fn id(&self) -> &str;

    /// Returns the display name for this provider (e.g., "Anthropic")
    fn name(&self) -> &str;

    /// Returns the cached/static list of available models for this provider
    fn models(&self) -> Vec<Model>;

    /// Fetches the list of available models from the API
    /// Returns cached data if available, otherwise fetches from API
    async fn fetch_models(&self) -> Result<Vec<Model>>;

    /// Returns the default model for this provider
    fn default_model(&self) -> &Model;

    /// Streams a chat completion response
    async fn stream_chat(
        &self,
        model_id: &str,
        messages: Vec<ChatMessage>,
        tx: async_channel::Sender<StreamEvent>,
    ) -> Result<()>;
}
