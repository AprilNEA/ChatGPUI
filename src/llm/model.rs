// SPDX-FileCopyrightText: 2026 AprilNEA LLC
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

use serde::{Deserialize, Serialize};

/// Represents an LLM model with its metadata
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Model {
    /// Unique model identifier (e.g., "claude-sonnet-4-20250514")
    pub id: String,
    /// Human-readable display name (e.g., "Claude Sonnet 4")
    pub name: String,
    /// Model description
    pub description: Option<String>,
    /// Context window size in tokens
    pub context_window: Option<u32>,
    /// Maximum output tokens
    pub max_output_tokens: Option<u32>,
    /// Whether the model supports vision/images
    pub supports_vision: bool,
    /// Whether the model supports tool/function calling
    pub supports_tools: bool,
    /// Whether the model supports streaming
    pub supports_streaming: bool,
    /// Input price per million tokens (in USD)
    pub input_price_per_million: Option<f64>,
    /// Output price per million tokens (in USD)
    pub output_price_per_million: Option<f64>,
}

impl Model {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: None,
            context_window: None,
            max_output_tokens: None,
            supports_vision: false,
            supports_tools: false,
            supports_streaming: true,
            input_price_per_million: None,
            output_price_per_million: None,
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn with_context_window(mut self, tokens: u32) -> Self {
        self.context_window = Some(tokens);
        self
    }

    pub fn with_max_output_tokens(mut self, tokens: u32) -> Self {
        self.max_output_tokens = Some(tokens);
        self
    }

    pub fn with_vision(mut self) -> Self {
        self.supports_vision = true;
        self
    }

    pub fn with_tools(mut self) -> Self {
        self.supports_tools = true;
        self
    }

    pub fn with_pricing(mut self, input: f64, output: f64) -> Self {
        self.input_price_per_million = Some(input);
        self.output_price_per_million = Some(output);
        self
    }
}
