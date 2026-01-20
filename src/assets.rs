// SPDX-FileCopyrightText: 2026 AprilNEA LLC
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

use std::borrow::Cow;

use gpui::{AnyElement, AssetSource, IntoElement, SharedString};
use gpui_component::{Icon, IconNamed};
use rust_embed::RustEmbed;
use strum::{Display, EnumString};

/// Helper trait to convert IconNamed types to Icon
pub trait IntoIcon: IconNamed + Copy {
    fn icon(self) -> Icon {
        Icon::new(self)
    }
}

impl<T: IconNamed + Copy> IntoIcon for T {}

#[derive(RustEmbed)]
#[folder = "assets"]
#[include = "icons/**/*.svg"]
pub struct Assets;

impl Assets {
    fn rewrite_path(path: &str) -> Cow<'_, str> {
        if path.starts_with("icons/") && path.matches('/').count() == 1 {
            Cow::Owned(path.replacen("icons/", "icons/ui/", 1))
        } else {
            Cow::Borrowed(path)
        }
    }
}

impl AssetSource for Assets {
    fn load(&self, path: &str) -> gpui::Result<Option<std::borrow::Cow<'static, [u8]>>> {
        if path.is_empty() {
            return Ok(None);
        }

        Self::get(&Self::rewrite_path(path))
            .map(|f| Some(f.data))
            .ok_or_else(|| anyhow::anyhow!("asset not found: {}", path))
    }

    fn list(&self, path: &str) -> gpui::Result<Vec<SharedString>> {
        Ok(Self::iter()
            .filter(|p| p.starts_with(path))
            .map(SharedString::from)
            .collect())
    }
}

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

/// Programming language enum for code blocks
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, EnumString, Display)]
#[strum(serialize_all = "lowercase", ascii_case_insensitive)]
#[allow(dead_code)]
pub enum Language {
    // Shell/Terminal
    #[strum(
        serialize = "bash",
        serialize = "sh",
        serialize = "shell",
        serialize = "zsh"
    )]
    Bash,
    #[strum(serialize = "powershell", serialize = "ps1")]
    PowerShell,

    // C family
    C,
    #[strum(
        serialize = "cpp",
        serialize = "c++",
        serialize = "cxx",
        serialize = "cc"
    )]
    Cpp,
    #[strum(serialize = "csharp", serialize = "c#", serialize = "cs")]
    CSharp,

    // Web
    #[strum(serialize = "javascript", serialize = "js", serialize = "jsx")]
    JavaScript,
    #[strum(serialize = "typescript", serialize = "ts", serialize = "tsx")]
    TypeScript,
    #[strum(serialize = "html", serialize = "htm")]
    Html,
    Css,
    #[strum(serialize = "sass", serialize = "scss")]
    Sass,
    #[strum(serialize = "graphql", serialize = "gql")]
    GraphQL,

    // Systems
    #[strum(serialize = "rust", serialize = "rs")]
    Rust,
    #[strum(serialize = "go", serialize = "golang")]
    Go,
    Zig,

    // JVM
    Java,
    #[strum(serialize = "kotlin", serialize = "kt", serialize = "kts")]
    Kotlin,
    Scala,

    // Scripting
    #[strum(serialize = "python", serialize = "py")]
    Python,
    #[strum(serialize = "ruby", serialize = "rb")]
    Ruby,
    Lua,
    R,

    // Mobile
    Swift,
    Dart,

    // Functional
    #[strum(serialize = "haskell", serialize = "hs")]
    Haskell,
    Gleam,

    // Data/Config
    #[strum(serialize = "json", serialize = "jsonc")]
    Json,
    #[strum(serialize = "yaml", serialize = "yml")]
    Yaml,
    Toml,
    #[strum(serialize = "markdown", serialize = "md")]
    Markdown,

    // Scientific
    #[strum(serialize = "julia", serialize = "jl")]
    Julia,
    #[strum(serialize = "matlab", serialize = "m")]
    Matlab,
    #[strum(serialize = "fortran", serialize = "f90", serialize = "f95")]
    Fortran,

    // Other
    #[strum(serialize = "cobol", serialize = "cob")]
    Cobol,
    #[strum(serialize = "solidity", serialize = "sol")]
    Solidity,
    #[strum(serialize = "terraform", serialize = "tf", serialize = "hcl")]
    Terraform,
    Sql,
}

impl IconNamed for Language {
    fn path(self) -> SharedString {
        let name: Cow<str> = match self {
            Self::Cpp => "c-plusplus".into(),
            _ => self.to_string().to_lowercase().into(),
        };
        format!("icons/language/{}.svg", name).into()
    }
}

impl IntoElement for Language {
    type Element = AnyElement;

    fn into_element(self) -> Self::Element {
        self.icon().into_any_element()
    }
}
