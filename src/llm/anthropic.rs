// SPDX-FileCopyrightText: 2026 AprilNEA LLC
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

use std::sync::Arc;

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use futures::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::message::{ChatMessage, Role};

use super::model::Model;
use super::provider::{LlmProvider, ProviderConfig, StreamEvent};

/// Anthropic API provider - first-class LLM provider
pub struct AnthropicProvider {
    client: Client,
    config: ProviderConfig,
    /// Static/fallback models list
    static_models: Vec<Model>,
    /// Cached models from API (populated on first fetch)
    cached_models: Arc<RwLock<Option<Vec<Model>>>>,
    default_model_index: usize,
}

/// Response from Anthropic /v1/models API
#[derive(Debug, Deserialize)]
struct ModelsResponse {
    data: Vec<ApiModel>,
}

/// Model info from Anthropic API
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct ApiModel {
    id: String,
    display_name: String,
    #[serde(rename = "type")]
    model_type: String,
    created_at: String,
}

#[derive(Debug, Serialize)]
struct AnthropicRequest {
    model: String,
    messages: Vec<AnthropicMessage>,
    max_tokens: u32,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
}

#[derive(Debug, Serialize)]
struct AnthropicMessage {
    role: String,
    content: AnthropicContent,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum AnthropicContent {
    Text(String),
    Blocks(Vec<AnthropicContentBlock>),
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
enum AnthropicContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image { source: AnthropicImageSource },
}

#[derive(Debug, Serialize)]
struct AnthropicImageSource {
    #[serde(rename = "type")]
    source_type: String,
    media_type: String,
    data: String,
}

#[derive(Debug, Deserialize)]
struct AnthropicStreamEvent {
    #[serde(rename = "type")]
    event_type: String,
    delta: Option<AnthropicDelta>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct AnthropicDelta {
    #[serde(rename = "type")]
    delta_type: Option<String>,
    text: Option<String>,
}

impl AnthropicProvider {
    /// Default Anthropic API base URL
    #[allow(dead_code)]
    pub const DEFAULT_BASE_URL: &'static str = "https://api.anthropic.com/v1";

    /// Create a new Anthropic provider with the given configuration
    pub fn new(config: ProviderConfig) -> Self {
        let static_models = Self::available_models();
        Self {
            client: Client::new(),
            config,
            static_models,
            cached_models: Arc::new(RwLock::new(None)),
            default_model_index: 0, // claude-sonnet-4 as default
        }
    }

    /// Create from API key with default base URL
    #[allow(dead_code)]
    pub fn with_api_key(api_key: impl Into<String>) -> Self {
        Self::new(ProviderConfig::new(api_key, Self::DEFAULT_BASE_URL))
    }

    /// Create from environment variables
    #[allow(dead_code)]
    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .map_err(|_| anyhow!("ANTHROPIC_API_KEY environment variable not set"))?;
        let base_url = std::env::var("ANTHROPIC_BASE_URL")
            .unwrap_or_else(|_| Self::DEFAULT_BASE_URL.to_string());

        Ok(Self::new(ProviderConfig::new(api_key, base_url)))
    }

    /// Fetch models from the Anthropic API
    async fn fetch_models_from_api(&self) -> Result<Vec<Model>> {
        let url = format!("{}/models", self.config.base_url.trim_end_matches('/'));

        let response = self
            .client
            .get(&url)
            .header("x-api-key", &self.config.api_key)
            .header("anthropic-version", "2023-06-01")
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow!("Failed to fetch models: {}", error_text));
        }

        let models_response: ModelsResponse = response.json().await?;

        // Convert API models to our Model format
        let models: Vec<Model> = models_response
            .data
            .into_iter()
            .filter(|m| m.model_type == "model") // Filter to actual models
            .map(|api_model| {
                // Try to match with our static models to get additional metadata
                let static_model = self
                    .static_models
                    .iter()
                    .find(|m| m.id == api_model.id);

                if let Some(sm) = static_model {
                    // Use static model data with API display name
                    Model {
                        id: api_model.id,
                        name: api_model.display_name,
                        ..sm.clone()
                    }
                } else {
                    // Create basic model from API data
                    Model::new(api_model.id, api_model.display_name)
                        .with_vision()
                        .with_tools()
                }
            })
            .collect();

        Ok(models)
    }

    /// Returns all available Anthropic models
    pub fn available_models() -> Vec<Model> {
        vec![
            // Claude 4 models (latest)
            Model::new("claude-sonnet-4-20250514", "Claude Sonnet 4")
                .with_description("Most intelligent model, best for complex tasks")
                .with_context_window(200_000)
                .with_max_output_tokens(64_000)
                .with_vision()
                .with_tools()
                .with_pricing(3.0, 15.0),
            Model::new("claude-opus-4-20250514", "Claude Opus 4")
                .with_description("Highest capability model for the most demanding tasks")
                .with_context_window(200_000)
                .with_max_output_tokens(32_000)
                .with_vision()
                .with_tools()
                .with_pricing(15.0, 75.0),
            // Claude 3.5 models
            Model::new("claude-3-5-sonnet-20241022", "Claude 3.5 Sonnet")
                .with_description("High intelligence and speed")
                .with_context_window(200_000)
                .with_max_output_tokens(8_192)
                .with_vision()
                .with_tools()
                .with_pricing(3.0, 15.0),
            Model::new("claude-3-5-haiku-20241022", "Claude 3.5 Haiku")
                .with_description("Fast and cost-effective")
                .with_context_window(200_000)
                .with_max_output_tokens(8_192)
                .with_vision()
                .with_tools()
                .with_pricing(0.8, 4.0),
            // Claude 3 models
            Model::new("claude-3-opus-20240229", "Claude 3 Opus")
                .with_description("Previous generation top model")
                .with_context_window(200_000)
                .with_max_output_tokens(4_096)
                .with_vision()
                .with_tools()
                .with_pricing(15.0, 75.0),
            Model::new("claude-3-sonnet-20240229", "Claude 3 Sonnet")
                .with_description("Balanced intelligence and speed")
                .with_context_window(200_000)
                .with_max_output_tokens(4_096)
                .with_vision()
                .with_tools()
                .with_pricing(3.0, 15.0),
            Model::new("claude-3-haiku-20240307", "Claude 3 Haiku")
                .with_description("Fast responses for simple tasks")
                .with_context_window(200_000)
                .with_max_output_tokens(4_096)
                .with_vision()
                .with_tools()
                .with_pricing(0.25, 1.25),
        ]
    }

    /// Find a model by ID
    pub fn find_model(&self, model_id: &str) -> Option<Model> {
        // First check cached models, then fallback to static
        // Note: This is sync, so we can't use async here
        self.static_models.iter().find(|m| m.id == model_id).cloned()
    }
}

#[async_trait]
impl LlmProvider for AnthropicProvider {
    fn id(&self) -> &str {
        "anthropic"
    }

    fn name(&self) -> &str {
        "Anthropic"
    }

    fn models(&self) -> Vec<Model> {
        // Return static models for sync access
        // Use fetch_models() for fresh data
        self.static_models.clone()
    }

    async fn fetch_models(&self) -> Result<Vec<Model>> {
        // Check cache first
        {
            let cache = self.cached_models.read().await;
            if let Some(models) = cache.as_ref() {
                return Ok(models.clone());
            }
        }

        // Fetch from API
        match self.fetch_models_from_api().await {
            Ok(models) => {
                // Update cache
                let mut cache = self.cached_models.write().await;
                *cache = Some(models.clone());
                Ok(models)
            }
            Err(e) => {
                tracing::warn!("Failed to fetch models from API, using static list: {}", e);
                // Return static models as fallback
                Ok(self.static_models.clone())
            }
        }
    }

    fn default_model(&self) -> &Model {
        &self.static_models[self.default_model_index]
    }

    async fn stream_chat(
        &self,
        model_id: &str,
        messages: Vec<ChatMessage>,
        tx: async_channel::Sender<StreamEvent>,
    ) -> Result<()> {
        let url = format!("{}/messages", self.config.base_url.trim_end_matches('/'));

        // Extract system message if present
        let system_msg = messages
            .iter()
            .find(|m| m.role == Role::System)
            .map(|m| m.content.clone());

        // Convert messages to Anthropic format (excluding system)
        let anthropic_messages: Vec<AnthropicMessage> = messages
            .iter()
            .filter(|m| m.role != Role::System)
            .map(|m| {
                let content = if m.attachments.is_empty() {
                    AnthropicContent::Text(m.content.clone())
                } else {
                    let mut blocks = Vec::new();
                    // Add images first
                    for att in &m.attachments {
                        blocks.push(AnthropicContentBlock::Image {
                            source: AnthropicImageSource {
                                source_type: "base64".to_string(),
                                media_type: att.mime_type.clone(),
                                data: att.base64_data(),
                            },
                        });
                    }
                    // Then add text
                    if !m.content.is_empty() {
                        blocks.push(AnthropicContentBlock::Text {
                            text: m.content.clone(),
                        });
                    }
                    AnthropicContent::Blocks(blocks)
                };

                AnthropicMessage {
                    role: match m.role {
                        Role::User => "user".to_string(),
                        Role::Assistant => "assistant".to_string(),
                        Role::System => unreachable!(),
                    },
                    content,
                }
            })
            .collect();

        // Get max tokens from model or use default
        let max_tokens = self
            .find_model(model_id)
            .and_then(|m| m.max_output_tokens)
            .unwrap_or(4096);

        let request = AnthropicRequest {
            model: model_id.to_string(),
            messages: anthropic_messages,
            max_tokens,
            stream: true,
            system: system_msg,
        };

        // Debug: log masked API key info
        let key_len = self.config.api_key.len();
        let key_preview = if key_len > 8 {
            format!(
                "{}...{} (len={})",
                &self.config.api_key[..4],
                &self.config.api_key[key_len - 4..],
                key_len
            )
        } else {
            format!("(too short, len={})", key_len)
        };
        tracing::debug!("Making API request to {} with key: {}", url, key_preview);

        let response = self
            .client
            .post(&url)
            .header("x-api-key", &self.config.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            tx.send(StreamEvent::Error(format!("API error: {}", error_text)))
                .await
                .ok();
            return Err(anyhow!("API error: {}", error_text));
        }

        let mut stream = response.bytes_stream();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            let text = String::from_utf8_lossy(&chunk);

            for line in text.lines() {
                if line.starts_with("data: ") {
                    let data = &line[6..];
                    if data.trim().is_empty() {
                        continue;
                    }

                    tracing::debug!("SSE data: {}", data);

                    if let Ok(event) = serde_json::from_str::<AnthropicStreamEvent>(data) {
                        match event.event_type.as_str() {
                            "content_block_delta" => {
                                if let Some(delta) = event.delta {
                                    if let Some(text) = delta.text {
                                        if !text.is_empty() {
                                            tx.send(StreamEvent::Delta(text)).await.ok();
                                        }
                                    }
                                }
                            }
                            "message_stop" => {
                                tx.send(StreamEvent::Done).await.ok();
                                return Ok(());
                            }
                            "error" => {
                                tx.send(StreamEvent::Error(
                                    t!("error.stream_error").to_string(),
                                ))
                                .await
                                .ok();
                                return Ok(());
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        tx.send(StreamEvent::Done).await.ok();
        Ok(())
    }
}
