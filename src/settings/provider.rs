// SPDX-FileCopyrightText: 2026 AprilNEA LLC
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

use serde::{Deserialize, Serialize};

use crate::llm::{self, Model};

/// Authentication method for API providers
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthMethod {
    ApiKey,
    Bearer,
    None,
}

impl AuthMethod {
    pub fn label(&self) -> String {
        match self {
            AuthMethod::ApiKey => t!("auth.api_key"),
            AuthMethod::Bearer => "Bearer Token".into(),
            AuthMethod::None => t!("auth.none"),
        }
        .to_string()
    }

    #[allow(dead_code)]
    pub fn all() -> Vec<AuthMethod> {
        vec![AuthMethod::ApiKey, AuthMethod::Bearer, AuthMethod::None]
    }
}

/// LLM Provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provider {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    pub auth_method: AuthMethod,
    pub api_key: String,
    pub base_url: String,
    pub default_model: String,
    pub enabled: bool,
}

impl Provider {
    pub fn anthropic() -> Self {
        Self {
            id: "anthropic".to_string(),
            name: "Anthropic".to_string(),
            icon: None,
            auth_method: AuthMethod::ApiKey,
            api_key: String::new(),
            base_url: "https://api.anthropic.com/v1".to_string(),
            default_model: "claude-sonnet-4-20250514".to_string(),
            enabled: true,
        }
    }

    pub fn openai() -> Self {
        Self {
            id: "openai".to_string(),
            name: "OpenAI".to_string(),
            icon: None,
            auth_method: AuthMethod::Bearer,
            api_key: String::new(),
            base_url: "https://api.openai.com/v1".to_string(),
            default_model: "gpt-4o".to_string(),
            enabled: true,
        }
    }

    pub fn azure_openai() -> Self {
        Self {
            id: "azure_openai".to_string(),
            name: "Azure OpenAI".to_string(),
            icon: None,
            auth_method: AuthMethod::ApiKey,
            api_key: String::new(),
            base_url: String::new(),
            default_model: "gpt-4o".to_string(),
            enabled: true,
        }
    }

    pub fn deepseek() -> Self {
        Self {
            id: "deepseek".to_string(),
            name: "DeepSeek".to_string(),
            icon: None,
            auth_method: AuthMethod::Bearer,
            api_key: String::new(),
            base_url: "https://api.deepseek.com/v1".to_string(),
            default_model: "deepseek-chat".to_string(),
            enabled: true,
        }
    }

    pub fn google_ai() -> Self {
        Self {
            id: "google_ai".to_string(),
            name: "Google AI".to_string(),
            icon: None,
            auth_method: AuthMethod::ApiKey,
            api_key: String::new(),
            base_url: "https://generativelanguage.googleapis.com/v1beta".to_string(),
            default_model: "gemini-pro".to_string(),
            enabled: true,
        }
    }

    pub fn groq() -> Self {
        Self {
            id: "groq".to_string(),
            name: "Groq".to_string(),
            icon: None,
            auth_method: AuthMethod::Bearer,
            api_key: String::new(),
            base_url: "https://api.groq.com/openai/v1".to_string(),
            default_model: "llama-3.3-70b-versatile".to_string(),
            enabled: true,
        }
    }

    pub fn mistral() -> Self {
        Self {
            id: "mistral".to_string(),
            name: "Mistral".to_string(),
            icon: None,
            auth_method: AuthMethod::Bearer,
            api_key: String::new(),
            base_url: "https://api.mistral.ai/v1".to_string(),
            default_model: "mistral-large-latest".to_string(),
            enabled: true,
        }
    }

    pub fn ollama() -> Self {
        Self {
            id: "ollama".to_string(),
            name: "Ollama".to_string(),
            icon: None,
            auth_method: AuthMethod::None,
            api_key: String::new(),
            base_url: "http://localhost:11434/api".to_string(),
            default_model: "llama3".to_string(),
            enabled: true,
        }
    }

    pub fn openrouter() -> Self {
        Self {
            id: "openrouter".to_string(),
            name: "OpenRouter".to_string(),
            icon: None,
            auth_method: AuthMethod::Bearer,
            api_key: String::new(),
            base_url: "https://openrouter.ai/api/v1".to_string(),
            default_model: "anthropic/claude-3.5-sonnet".to_string(),
            enabled: true,
        }
    }

    /// Get the list of available models for this provider
    #[allow(dead_code)]
    pub fn models(&self) -> Vec<Model> {
        llm::get_models_for_provider(&self.id)
    }

    /// Check if this provider has a native implementation
    #[allow(dead_code)]
    pub fn is_implemented(&self) -> bool {
        matches!(self.id.as_str(), "anthropic")
    }
}
