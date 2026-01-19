// SPDX-FileCopyrightText: 2026 AprilNEA LLC
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

use gpui::{AssetSource, SharedString};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "assets"]
#[include = "icons/**/*.svg"]
#[include = "icons/**/*.png"]
#[include = "*.svg"]
#[include = "*.png"]
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> gpui::Result<Option<std::borrow::Cow<'static, [u8]>>> {
        if path.is_empty() {
            return Ok(None);
        }
        Self::get(path)
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

/// Brand icons for LLM providers
#[derive(RustEmbed)]
#[folder = "assets/icons/brand"]
#[include = "*.svg"]
pub struct BrandAssets;

impl BrandAssets {
    /// Get the icon path for a provider ID
    /// Returns the appropriate icon based on provider and theme
    pub fn provider_icon(provider_id: &str, is_dark: bool) -> Option<&'static str> {
        match provider_id {
            "anthropic" => Some("icons/brand/anthropic.svg"),
            "openai" | "azure_openai" => Some("icons/brand/openai.svg"),
            "deepseek" => Some("icons/brand/deepseek.svg"),
            "google_ai" => Some("icons/brand/google.svg"),
            "mistral" => Some("icons/brand/mistral-ai.svg"),
            "ollama" => Some(if is_dark {
                "icons/brand/ollama-dark.svg"
            } else {
                "icons/brand/ollama-light.svg"
            }),
            "openrouter" => Some(if is_dark {
                "icons/brand/openrouter-dark.svg"
            } else {
                "icons/brand/openrouter-light.svg"
            }),
            "xai" | "grok" => Some(if is_dark {
                "icons/brand/xai-dark.svg"
            } else {
                "icons/brand/xai-light.svg"
            }),
            "perplexity" => Some("icons/brand/perplexity.svg"),
            "kimi" | "moonshot" => Some("icons/brand/kimi-icon.svg"),
            _ => None,
        }
    }

    /// Check if a provider has a brand icon
    #[allow(dead_code)]
    pub fn has_icon(provider_id: &str) -> bool {
        matches!(
            provider_id,
            "anthropic"
                | "openai"
                | "azure_openai"
                | "deepseek"
                | "google_ai"
                | "mistral"
                | "ollama"
                | "openrouter"
                | "xai"
                | "grok"
                | "perplexity"
                | "kimi"
                | "moonshot"
        )
    }
}

/// Language icons for code blocks
#[derive(RustEmbed)]
#[folder = "assets/icons/language"]
#[include = "*.svg"]
#[allow(dead_code)]
pub struct LanguageAssets;

#[allow(dead_code)]
impl LanguageAssets {
    /// Get the icon path for a language
    /// Returns the appropriate icon based on language and theme
    pub fn language_icon(language: &str, is_dark: bool) -> Option<&'static str> {
        let lang = language.to_lowercase();
        match lang.as_str() {
            // Shell/Terminal
            "bash" | "sh" | "shell" | "zsh" => Some(if is_dark {
                "icons/language/bash_dark.svg"
            } else {
                "icons/language/bash.svg"
            }),
            "powershell" | "ps1" => Some("icons/language/powershell.svg"),

            // C family
            "c" => Some("icons/language/c.svg"),
            "cpp" | "c++" | "cxx" | "cc" => Some("icons/language/c-plusplus.svg"),
            "csharp" | "c#" | "cs" => Some("icons/language/csharp.svg"),

            // Web
            "javascript" | "js" | "jsx" => Some("icons/language/javascript.svg"),
            "typescript" | "ts" | "tsx" => Some("icons/language/typescript.svg"),
            "html" | "htm" => Some("icons/language/html5.svg"),
            "css" => Some("icons/language/css.svg"),
            "sass" | "scss" => Some("icons/language/sass.svg"),
            "graphql" | "gql" => Some("icons/language/graphql.svg"),

            // Systems
            "rust" | "rs" => Some(if is_dark {
                "icons/language/rust_dark.svg"
            } else {
                "icons/language/rust.svg"
            }),
            "go" | "golang" => Some(if is_dark {
                "icons/language/golang_dark.svg"
            } else {
                "icons/language/golang.svg"
            }),
            "zig" => Some("icons/language/zig.svg"),

            // JVM
            "java" => Some("icons/language/java.svg"),
            "kotlin" | "kt" | "kts" => Some("icons/language/kotlin.svg"),
            "scala" => Some("icons/language/scala.svg"),

            // Scripting
            "python" | "py" => Some("icons/language/python.svg"),
            "ruby" | "rb" => Some("icons/language/ruby.svg"),
            "lua" => Some("icons/language/lua.svg"),
            "r" => Some(if is_dark {
                "icons/language/r_dark.svg"
            } else {
                "icons/language/r.svg"
            }),

            // Mobile
            "swift" => Some("icons/language/swift.svg"),
            "dart" => Some("icons/language/dart.svg"),

            // Functional
            "haskell" | "hs" => Some("icons/language/haskell.svg"),
            "gleam" => Some("icons/language/gleam.svg"),

            // Data/Config
            "json" | "jsonc" => Some("icons/language/json.svg"),
            "markdown" | "md" => Some(if is_dark {
                "icons/language/markdown-dark.svg"
            } else {
                "icons/language/markdown-light.svg"
            }),

            // Scientific
            "julia" | "jl" => Some("icons/language/julia.svg"),
            "matlab" | "m" => Some("icons/language/matlab.svg"),
            "fortran" | "f90" | "f95" => Some("icons/language/fortran.svg"),

            // Other
            "cobol" | "cob" => Some("icons/language/cobol.svg"),
            "solidity" | "sol" => Some("icons/language/solidity.svg"),
            "terraform" | "tf" | "hcl" => Some("icons/language/terraform.svg"),

            _ => None,
        }
    }

    /// Check if a language has an icon
    pub fn has_icon(language: &str) -> bool {
        Self::language_icon(language, false).is_some()
    }
}
