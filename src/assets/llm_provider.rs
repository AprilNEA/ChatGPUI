use std::borrow::Cow;

use strum::{Display, EnumString};

use gpui::{AnyElement, IntoElement, SharedString};
use gpui_component::IconNamed;

use crate::assets::IntoIcon;

/// LLM Provider enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, EnumString, Display)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
pub enum LlmProvider {
    Anthropic,
    #[strum(serialize = "openai")]
    OpenAI,
    #[strum(serialize = "azure_openai")]
    AzureOpenAI,
    #[strum(serialize = "deepseek")]
    DeepSeek,
    #[strum(serialize = "google_ai", serialize = "gemini")]
    GoogleAI,
    Mistral,
    Ollama,
    #[strum(serialize = "openrouter")]
    OpenRouter,
    #[strum(serialize = "xai", serialize = "grok")]
    Xai,
    Perplexity,
    #[strum(serialize = "kimi", serialize = "moonshot")]
    Kimi,
    Groq,
    #[strum(serialize = "together", serialize = "together_ai")]
    TogetherAI,
    Cohere,
    #[strum(serialize = "fireworks", serialize = "fireworks_ai")]
    FireworksAI,
}

impl LlmProvider {
    /// Get the display name for this provider
    #[allow(dead_code)]
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Anthropic => "Anthropic",
            Self::OpenAI => "OpenAI",
            Self::AzureOpenAI => "Azure OpenAI",
            Self::DeepSeek => "DeepSeek",
            Self::GoogleAI => "Google AI",
            Self::Mistral => "Mistral",
            Self::Ollama => "Ollama",
            Self::OpenRouter => "OpenRouter",
            Self::Xai => "xAI",
            Self::Perplexity => "Perplexity",
            Self::Kimi => "Kimi",
            Self::Groq => "Groq",
            Self::TogetherAI => "Together AI",
            Self::Cohere => "Cohere",
            Self::FireworksAI => "Fireworks AI",
        }
    }

    /// Get the default base URL for this provider
    #[allow(dead_code)]
    pub fn default_base_url(&self) -> &'static str {
        match self {
            Self::Anthropic => "https://api.anthropic.com/v1",
            Self::OpenAI => "https://api.openai.com/v1",
            Self::AzureOpenAI => "", // Requires custom endpoint
            Self::DeepSeek => "https://api.deepseek.com/v1",
            Self::GoogleAI => "https://generativelanguage.googleapis.com/v1beta",
            Self::Mistral => "https://api.mistral.ai/v1",
            Self::Ollama => "http://localhost:11434/api",
            Self::OpenRouter => "https://openrouter.ai/api/v1",
            Self::Xai => "https://api.x.ai/v1",
            Self::Perplexity => "https://api.perplexity.ai",
            Self::Kimi => "https://api.moonshot.cn/v1",
            Self::Groq => "https://api.groq.com/openai/v1",
            Self::TogetherAI => "https://api.together.xyz/v1",
            Self::Cohere => "https://api.cohere.ai/v1",
            Self::FireworksAI => "https://api.fireworks.ai/inference/v1",
        }
    }
}

impl IconNamed for LlmProvider {
    fn path(self) -> SharedString {
        let name: Cow<str> = match self {
            Self::OpenAI | Self::AzureOpenAI => "openai".into(),
            Self::GoogleAI => "google".into(),
            Self::Mistral => "mistral-ai".into(),
            Self::TogetherAI => "together".into(),
            Self::FireworksAI => "fireworks".into(),
            _ => self.to_string().to_lowercase().into(),
        };
        format!("icons/llm_provider/{}.svg", name).into()
    }
}

impl IntoElement for LlmProvider {
    type Element = AnyElement;

    fn into_element(self) -> Self::Element {
        self.icon().into_any_element()
    }
}
