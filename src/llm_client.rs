// SPDX-FileCopyrightText: 2026 AprilNEA LLC
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

use anyhow::{Result, anyhow};
use futures::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::message::{ChatMessage, Role};

#[derive(Debug, Clone)]
pub struct LlmClient {
    client: Client,
    api_key: String,
    base_url: String,
    model: String,
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
    content: String,
}

#[derive(Debug, Deserialize)]
struct StreamEvent {
    #[serde(rename = "type")]
    event_type: String,
    delta: Option<Delta>,
}

#[derive(Debug, Deserialize)]
struct Delta {
    #[serde(rename = "type")]
    delta_type: Option<String>,
    text: Option<String>,
}

pub enum StreamEventResult {
    Delta(String),
    Done,
    Error(String),
}

impl LlmClient {
    pub fn new(
        api_key: impl Into<String>,
        base_url: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        Self {
            client: Client::new(),
            api_key: api_key.into(),
            base_url: base_url.into(),
            model: model.into(),
        }
    }

    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .map_err(|_| anyhow!("ANTHROPIC_API_KEY environment variable not set"))?;
        let base_url = std::env::var("ANTHROPIC_BASE_URL")
            .unwrap_or_else(|_| "https://api.anthropic.com/v1".to_string());
        let model = std::env::var("ANTHROPIC_MODEL")
            .unwrap_or_else(|_| "claude-sonnet-4-20250514".to_string());

        Ok(Self::new(api_key, base_url, model))
    }

    /// Create a client from app settings
    pub fn from_settings(cx: &gpui::App) -> Result<Self> {
        use crate::settings::get_settings;

        let settings = get_settings(cx);
        let provider = settings
            .active_provider()
            .ok_or_else(|| anyhow!("No active provider configured"))?;

        if provider.api_key.is_empty() {
            return Err(anyhow!("API key not configured for {}", provider.name));
        }

        Ok(Self::new(
            &provider.api_key,
            &provider.base_url,
            &provider.default_model,
        ))
    }

    pub async fn stream_chat(
        &self,
        messages: Vec<ChatMessage>,
        tx: async_channel::Sender<StreamEventResult>,
    ) -> Result<()> {
        let url = format!("{}/messages", self.base_url.trim_end_matches('/'));

        // Extract system message if present
        let system_msg = messages
            .iter()
            .find(|m| m.role == Role::System)
            .map(|m| m.content.clone());

        // Convert messages to Anthropic format (excluding system)
        let anthropic_messages: Vec<AnthropicMessage> = messages
            .iter()
            .filter(|m| m.role != Role::System)
            .map(|m| AnthropicMessage {
                role: match m.role {
                    Role::User => "user".to_string(),
                    Role::Assistant => "assistant".to_string(),
                    Role::System => unreachable!(),
                },
                content: m.content.clone(),
            })
            .collect();

        let request = AnthropicRequest {
            model: self.model.clone(),
            messages: anthropic_messages,
            max_tokens: 4096,
            stream: true,
            system: system_msg,
        };

        let response = self
            .client
            .post(url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            tx.send(StreamEventResult::Error(format!(
                "API error: {}",
                error_text
            )))
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

                    if let Ok(event) = serde_json::from_str::<StreamEvent>(data) {
                        match event.event_type.as_str() {
                            "content_block_delta" => {
                                if let Some(delta) = event.delta {
                                    if let Some(text) = delta.text {
                                        if !text.is_empty() {
                                            tx.send(StreamEventResult::Delta(text)).await.ok();
                                        }
                                    }
                                }
                            }
                            "message_stop" => {
                                tx.send(StreamEventResult::Done).await.ok();
                                return Ok(());
                            }
                            "error" => {
                                tx.send(StreamEventResult::Error(
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

        tx.send(StreamEventResult::Done).await.ok();
        Ok(())
    }
}
