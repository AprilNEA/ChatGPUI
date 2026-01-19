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

/// Google AI (Gemini) API provider
pub struct GoogleAIProvider {
    client: Client,
    config: ProviderConfig,
    /// Static/fallback models list
    static_models: Vec<Model>,
    /// Cached models from API (populated on first fetch)
    cached_models: Arc<RwLock<Option<Vec<Model>>>>,
    default_model_index: usize,
}

/// Response from Google AI /models API
#[derive(Debug, Deserialize)]
struct ModelsResponse {
    models: Vec<ApiModel>,
}

/// Model info from Google AI API
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiModel {
    name: String,
    display_name: String,
    supported_generation_methods: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GoogleAIRequest {
    contents: Vec<GoogleAIContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system_instruction: Option<GoogleAIContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    generation_config: Option<GenerationConfig>,
}

#[derive(Debug, Serialize, Deserialize)]
struct GoogleAIContent {
    role: String,
    parts: Vec<GoogleAIPart>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum GoogleAIPart {
    Text { text: String },
    InlineData { inline_data: GoogleAIInlineData },
}

#[derive(Debug, Serialize, Deserialize)]
struct GoogleAIInlineData {
    mime_type: String,
    data: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GenerationConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    max_output_tokens: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct GoogleAIStreamResponse {
    candidates: Option<Vec<GoogleAICandidate>>,
}

#[derive(Debug, Deserialize)]
struct GoogleAICandidate {
    content: Option<GoogleAIContent>,
    #[serde(rename = "finishReason")]
    finish_reason: Option<String>,
}

impl GoogleAIProvider {
    /// Default Google AI API base URL
    #[allow(dead_code)]
    pub const DEFAULT_BASE_URL: &'static str = "https://generativelanguage.googleapis.com/v1beta";

    /// Create a new Google AI provider with the given configuration
    pub fn new(config: ProviderConfig) -> Self {
        let static_models = Self::available_models();
        Self {
            client: Client::new(),
            config,
            static_models,
            cached_models: Arc::new(RwLock::new(None)),
            default_model_index: 0, // gemini-2.0-flash as default
        }
    }

    /// Fetch models from the Google AI API
    async fn fetch_models_from_api(&self) -> Result<Vec<Model>> {
        let url = format!(
            "{}/models?key={}",
            self.config.base_url.trim_end_matches('/'),
            self.config.api_key
        );

        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow!("Failed to fetch models: {}", error_text));
        }

        let models_response: ModelsResponse = response.json().await?;

        // Filter to Gemini models that support generateContent
        let models: Vec<Model> = models_response
            .models
            .into_iter()
            .filter(|m| {
                m.supported_generation_methods
                    .contains(&"generateContent".to_string())
                    && m.name.contains("gemini")
            })
            .map(|api_model| {
                // Extract model ID from full name (e.g., "models/gemini-pro" -> "gemini-pro")
                let model_id = api_model
                    .name
                    .strip_prefix("models/")
                    .unwrap_or(&api_model.name)
                    .to_string();

                // Try to match with our static models to get additional metadata
                let static_model = self.static_models.iter().find(|m| m.id == model_id);

                if let Some(sm) = static_model {
                    Model {
                        id: model_id,
                        name: api_model.display_name,
                        ..sm.clone()
                    }
                } else {
                    Model::new(model_id, api_model.display_name)
                }
            })
            .collect();

        Ok(models)
    }

    /// Returns all available Google AI models
    pub fn available_models() -> Vec<Model> {
        vec![
            // Gemini 2.0 models (latest)
            Model::new("gemini-2.0-flash", "Gemini 2.0 Flash")
                .with_description("Fast and versatile multimodal model")
                .with_context_window(1_048_576)
                .with_max_output_tokens(8_192)
                .with_vision()
                .with_tools(),
            Model::new("gemini-2.0-flash-lite", "Gemini 2.0 Flash Lite")
                .with_description("Cost-effective for high-volume tasks")
                .with_context_window(1_048_576)
                .with_max_output_tokens(8_192)
                .with_vision(),
            // Gemini 1.5 models
            Model::new("gemini-1.5-pro", "Gemini 1.5 Pro")
                .with_description("Best for complex reasoning tasks")
                .with_context_window(2_097_152)
                .with_max_output_tokens(8_192)
                .with_vision()
                .with_tools()
                .with_pricing(1.25, 5.0),
            Model::new("gemini-1.5-flash", "Gemini 1.5 Flash")
                .with_description("Fast and efficient for most tasks")
                .with_context_window(1_048_576)
                .with_max_output_tokens(8_192)
                .with_vision()
                .with_tools()
                .with_pricing(0.075, 0.3),
            Model::new("gemini-1.5-flash-8b", "Gemini 1.5 Flash 8B")
                .with_description("Smallest and fastest Flash model")
                .with_context_window(1_048_576)
                .with_max_output_tokens(8_192)
                .with_vision()
                .with_pricing(0.0375, 0.15),
            // Legacy model
            Model::new("gemini-pro", "Gemini Pro")
                .with_description("Previous generation model")
                .with_context_window(32_760)
                .with_max_output_tokens(8_192)
                .with_tools(),
        ]
    }

    /// Find a model by ID
    pub fn find_model(&self, model_id: &str) -> Option<Model> {
        self.static_models
            .iter()
            .find(|m| m.id == model_id)
            .cloned()
    }
}

#[async_trait]
impl LlmProvider for GoogleAIProvider {
    fn id(&self) -> &str {
        "google_ai"
    }

    fn name(&self) -> &str {
        "Google AI"
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
            "{}/models/{}:streamGenerateContent?alt=sse&key={}",
            self.config.base_url.trim_end_matches('/'),
            model_id,
            self.config.api_key
        );

        // Extract system message if present
        let system_instruction =
            messages
                .iter()
                .find(|m| m.role == Role::System)
                .map(|m| GoogleAIContent {
                    role: "user".to_string(), // Google uses "user" role for system instructions
                    parts: vec![GoogleAIPart::Text {
                        text: m.content.clone(),
                    }],
                });

        // Convert messages to Google AI format (excluding system)
        let contents: Vec<GoogleAIContent> = messages
            .iter()
            .filter(|m| m.role != Role::System)
            .map(|m| {
                let mut parts = Vec::new();
                // Add images first
                for att in &m.attachments {
                    parts.push(GoogleAIPart::InlineData {
                        inline_data: GoogleAIInlineData {
                            mime_type: att.mime_type.clone(),
                            data: att.base64_data(),
                        },
                    });
                }
                // Then add text
                if !m.content.is_empty() {
                    parts.push(GoogleAIPart::Text {
                        text: m.content.clone(),
                    });
                }

                GoogleAIContent {
                    role: match m.role {
                        Role::User => "user".to_string(),
                        Role::Assistant => "model".to_string(),
                        Role::System => unreachable!(),
                    },
                    parts,
                }
            })
            .collect();

        // Get max tokens from model or use default
        let max_output_tokens = self.find_model(model_id).and_then(|m| m.max_output_tokens);

        let request = GoogleAIRequest {
            contents,
            system_instruction,
            generation_config: max_output_tokens.map(|max| GenerationConfig {
                max_output_tokens: Some(max),
            }),
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
        tracing::debug!(
            "Making Google AI API request to {} with key: {}",
            url.split('?').next().unwrap_or(&url),
            key_preview
        );

        let response = self
            .client
            .post(&url)
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
                if let Some(data) = line.strip_prefix("data: ") {
                    if data.trim().is_empty() {
                        continue;
                    }

                    if let Ok(response) = serde_json::from_str::<GoogleAIStreamResponse>(data)
                        && let Some(candidates) = response.candidates
                    {
                        for candidate in candidates {
                            if let Some(content) = candidate.content {
                                for part in content.parts {
                                    if let GoogleAIPart::Text { text } = part
                                        && !text.is_empty()
                                    {
                                        tx.send(StreamEvent::Delta(text)).await.ok();
                                    }
                                }
                            }
                            if candidate.finish_reason.is_some() {
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
