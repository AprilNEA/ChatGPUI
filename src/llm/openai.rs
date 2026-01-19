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

/// OpenAI API provider
pub struct OpenAIProvider {
    client: Client,
    config: ProviderConfig,
    /// Static/fallback models list
    static_models: Vec<Model>,
    /// Cached models from API (populated on first fetch)
    cached_models: Arc<RwLock<Option<Vec<Model>>>>,
    default_model_index: usize,
}

/// Response from OpenAI /v1/models API
#[derive(Debug, Deserialize)]
struct ModelsResponse {
    data: Vec<ApiModel>,
}

/// Model info from OpenAI API
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct ApiModel {
    id: String,
    owned_by: String,
}

#[derive(Debug, Serialize)]
struct OpenAIRequest {
    model: String,
    messages: Vec<OpenAIMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    stream: bool,
}

#[derive(Debug, Serialize)]
struct OpenAIMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct OpenAIStreamChunk {
    choices: Vec<OpenAIChoice>,
}

#[derive(Debug, Deserialize)]
struct OpenAIChoice {
    delta: OpenAIDelta,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAIDelta {
    content: Option<String>,
}

impl OpenAIProvider {
    /// Default OpenAI API base URL
    #[allow(dead_code)]
    pub const DEFAULT_BASE_URL: &'static str = "https://api.openai.com/v1";

    /// Create a new OpenAI provider with the given configuration
    pub fn new(config: ProviderConfig) -> Self {
        let static_models = Self::available_models();
        Self {
            client: Client::new(),
            config,
            static_models,
            cached_models: Arc::new(RwLock::new(None)),
            default_model_index: 0, // gpt-4o as default
        }
    }

    /// Fetch models from the OpenAI API
    async fn fetch_models_from_api(&self) -> Result<Vec<Model>> {
        let url = format!("{}/models", self.config.base_url.trim_end_matches('/'));

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow!("Failed to fetch models: {}", error_text));
        }

        let models_response: ModelsResponse = response.json().await?;

        // Filter to GPT models and convert to our Model format
        let models: Vec<Model> = models_response
            .data
            .into_iter()
            .filter(|m| {
                m.id.starts_with("gpt-")
                    || m.id.starts_with("o1")
                    || m.id.starts_with("o3")
            })
            .map(|api_model| {
                // Try to match with our static models to get additional metadata
                let static_model = self
                    .static_models
                    .iter()
                    .find(|m| m.id == api_model.id);

                if let Some(sm) = static_model {
                    sm.clone()
                } else {
                    // Create basic model from API data
                    Model::new(api_model.id.clone(), api_model.id)
                }
            })
            .collect();

        Ok(models)
    }

    /// Returns all available OpenAI models
    pub fn available_models() -> Vec<Model> {
        vec![
            // GPT-4o models (latest)
            Model::new("gpt-4o", "GPT-4o")
                .with_description("Most advanced multimodal model")
                .with_context_window(128_000)
                .with_max_output_tokens(16_384)
                .with_vision()
                .with_tools()
                .with_pricing(2.5, 10.0),
            Model::new("gpt-4o-mini", "GPT-4o Mini")
                .with_description("Fast and affordable for simple tasks")
                .with_context_window(128_000)
                .with_max_output_tokens(16_384)
                .with_vision()
                .with_tools()
                .with_pricing(0.15, 0.6),
            // o1 reasoning models
            Model::new("o1", "o1")
                .with_description("Reasoning model for complex problems")
                .with_context_window(200_000)
                .with_max_output_tokens(100_000)
                .with_pricing(15.0, 60.0),
            Model::new("o1-mini", "o1-mini")
                .with_description("Faster reasoning model")
                .with_context_window(128_000)
                .with_max_output_tokens(65_536)
                .with_pricing(3.0, 12.0),
            // o3-mini
            Model::new("o3-mini", "o3-mini")
                .with_description("Latest reasoning model")
                .with_context_window(200_000)
                .with_max_output_tokens(100_000)
                .with_pricing(1.1, 4.4),
            // GPT-4 Turbo
            Model::new("gpt-4-turbo", "GPT-4 Turbo")
                .with_description("Previous generation flagship")
                .with_context_window(128_000)
                .with_max_output_tokens(4_096)
                .with_vision()
                .with_tools()
                .with_pricing(10.0, 30.0),
            // GPT-3.5 Turbo
            Model::new("gpt-3.5-turbo", "GPT-3.5 Turbo")
                .with_description("Fast and economical")
                .with_context_window(16_385)
                .with_max_output_tokens(4_096)
                .with_tools()
                .with_pricing(0.5, 1.5),
        ]
    }

    /// Find a model by ID
    pub fn find_model(&self, model_id: &str) -> Option<Model> {
        self.static_models.iter().find(|m| m.id == model_id).cloned()
    }
}

#[async_trait]
impl LlmProvider for OpenAIProvider {
    fn id(&self) -> &str {
        "openai"
    }

    fn name(&self) -> &str {
        "OpenAI"
    }

    fn models(&self) -> Vec<Model> {
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
        let url = format!(
            "{}/chat/completions",
            self.config.base_url.trim_end_matches('/')
        );

        // Convert messages to OpenAI format
        let openai_messages: Vec<OpenAIMessage> = messages
            .iter()
            .map(|m| OpenAIMessage {
                role: match m.role {
                    Role::User => "user".to_string(),
                    Role::Assistant => "assistant".to_string(),
                    Role::System => "system".to_string(),
                },
                content: m.content.clone(),
            })
            .collect();

        // Get max tokens from model or use default
        let max_tokens = self
            .find_model(model_id)
            .and_then(|m| m.max_output_tokens);

        let request = OpenAIRequest {
            model: model_id.to_string(),
            messages: openai_messages,
            max_tokens,
            stream: true,
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
        tracing::debug!("Making OpenAI API request to {} with key: {}", url, key_preview);

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
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
                    if data.trim() == "[DONE]" {
                        tx.send(StreamEvent::Done).await.ok();
                        return Ok(());
                    }

                    if data.trim().is_empty() {
                        continue;
                    }

                    if let Ok(chunk) = serde_json::from_str::<OpenAIStreamChunk>(data) {
                        for choice in chunk.choices {
                            if let Some(content) = choice.delta.content {
                                if !content.is_empty() {
                                    tx.send(StreamEvent::Delta(content)).await.ok();
                                }
                            }
                            if choice.finish_reason.is_some() {
                                tx.send(StreamEvent::Done).await.ok();
                                return Ok(());
                            }
                        }
                    }
                }
            }
        }

        tx.send(StreamEvent::Done).await.ok();
        Ok(())
    }
}
