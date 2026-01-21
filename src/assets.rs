// SPDX-FileCopyrightText: 2026 AprilNEA LLC
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

use std::borrow::Cow;

use gpui::{AnyElement, AssetSource, IntoElement, ParentElement, Pixels, SharedString, Styled, px};
use gpui_component::{Icon, IconNamed, button::Button};
use rust_embed::RustEmbed;
use strum::{Display, EnumString};

// Re-export gpui-symbols on macOS
#[cfg(target_os = "macos")]
pub use gpui_symbols::Icon as SfIcon;
#[cfg(target_os = "macos")]
pub use gpui_symbols::sfsymbols::SfSymbol;

// =============================================================================
// Platform Icon Abstraction
// =============================================================================

/// App-specific icons with SF Symbol support on macOS.
///
/// Each icon variant maps to:
/// - An SF Symbol name (for macOS 11+)
/// - An SVG path (cross-platform fallback)
///
/// Currently uses SVG rendering. When GPUI adds native image support,
/// we can switch to SF Symbols on macOS for a more native look.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(dead_code)]
pub enum AppIcon {
    // Navigation & Actions
    ArrowRight,
    ArrowLeft,
    ArrowUp,
    ArrowDown,
    ChevronDown,
    ChevronRight,
    ChevronUp,
    ChevronLeft,

    // Common Actions
    Plus,
    Minus,
    Close,
    Check,
    Search,
    Copy,
    Delete,
    Settings,
    Settings2,

    // Panels & Layout
    PanelLeft,
    PanelRight,
    PanelBottom,

    // Content & Files
    File,
    Folder,
    BookOpen,
    Globe,
    ExternalLink,

    // UI States
    Eye,
    EyeOff,
    Star,
    StarOff,
    Loader,
    CircleX,
    CircleCheck,
    Info,
    TriangleAlert,

    // Theme
    Sun,
    Moon,
    Palette,

    // Communication & AI
    Bot,
    User,
    Message,

    // Developer
    SquareTerminal,
    Frame,

    // Custom/App-specific
    Mcp,
}

impl AppIcon {
    /// Returns the type-safe SF Symbol enum for this icon (macOS only).
    ///
    /// Uses the unified `SfSymbol` enum from sfsymbols 0.2+ which combines
    /// all SF Symbols versions into a single type.
    #[cfg(target_os = "macos")]
    pub fn sf_symbol(self) -> SfSymbol {
        use SfSymbol::*;
        match self {
            // Navigation
            Self::ArrowRight => ArrowRight,
            Self::ArrowLeft => ArrowLeft,
            Self::ArrowUp => ArrowUp,
            Self::ArrowDown => ArrowDown,
            Self::ChevronDown => ChevronDown,
            Self::ChevronRight => ChevronRight,
            Self::ChevronUp => ChevronUp,
            Self::ChevronLeft => ChevronLeft,

            // Common Actions
            Self::Plus => Plus,
            Self::Minus => Minus,
            Self::Close => Xmark,
            Self::Check => Checkmark,
            Self::Search => Magnifyingglass,
            Self::Copy => DocOnDoc,
            Self::Delete => Trash,
            Self::Settings => Gearshape,
            Self::Settings2 => Gearshape2,

            // Panels
            Self::PanelLeft => SidebarLeft,
            Self::PanelRight => SidebarRight,
            Self::PanelBottom => RectangleBottomthirdInsetFilled, // sidebar.bottom doesn't exist

            // Content
            Self::File => Doc,
            Self::Folder => Folder,
            Self::BookOpen => Book,
            Self::Globe => Globe,
            Self::ExternalLink => ArrowUpRightSquare,

            // States
            Self::Eye => Eye,
            Self::EyeOff => EyeSlash,
            Self::Star => Star,
            Self::StarOff => StarSlash,
            Self::Loader => ArrowTriangle2Circlepath,
            Self::CircleX => XmarkCircle,
            Self::CircleCheck => CheckmarkCircle,
            Self::Info => InfoCircle,
            Self::TriangleAlert => ExclamationmarkTriangle,

            // Theme
            Self::Sun => SunMax,
            Self::Moon => Moon,
            Self::Palette => Paintpalette,

            // Communication
            Self::Bot => BubbleLeftAndTextBubbleRight,
            Self::User => Person,
            Self::Message => Message,

            // Developer
            Self::SquareTerminal => Terminal,
            Self::Frame => SquareOnSquare,

            // Custom
            Self::Mcp => SquareStack3DUp, // closest approximation
        }
    }

    /// Create a platform-native icon element.
    /// - macOS: Uses SF Symbols for a native look
    /// - Other platforms: Uses SVG icons
    pub fn with_size(self, size: Pixels) -> AppIconElement {
        AppIconElement { icon: self, size }
    }

    /// Returns the SVG icon path in the assets bundle.
    fn svg_path(self) -> &'static str {
        match self {
            Self::ArrowRight => "icons/ui/arrow-right.svg",
            Self::ArrowLeft => "icons/ui/arrow-left.svg",
            Self::ArrowUp => "icons/ui/arrow-up.svg",
            Self::ArrowDown => "icons/ui/arrow-down.svg",
            Self::ChevronDown => "icons/ui/chevron-down.svg",
            Self::ChevronRight => "icons/ui/chevron-right.svg",
            Self::ChevronUp => "icons/ui/chevron-up.svg",
            Self::ChevronLeft => "icons/ui/chevron-left.svg",
            Self::Plus => "icons/ui/plus.svg",
            Self::Minus => "icons/ui/minus.svg",
            Self::Close => "icons/ui/close.svg",
            Self::Check => "icons/ui/check.svg",
            Self::Search => "icons/ui/search.svg",
            Self::Copy => "icons/ui/copy.svg",
            Self::Delete => "icons/ui/delete.svg",
            Self::Settings => "icons/ui/settings.svg",
            Self::Settings2 => "icons/ui/settings-2.svg",
            Self::PanelLeft => "icons/ui/panel-left.svg",
            Self::PanelRight => "icons/ui/panel-right.svg",
            Self::PanelBottom => "icons/ui/panel-bottom.svg",
            Self::File => "icons/ui/file.svg",
            Self::Folder => "icons/ui/folder.svg",
            Self::BookOpen => "icons/ui/book-open.svg",
            Self::Globe => "icons/ui/globe.svg",
            Self::ExternalLink => "icons/ui/external-link.svg",
            Self::Eye => "icons/ui/eye.svg",
            Self::EyeOff => "icons/ui/eye-off.svg",
            Self::Star => "icons/ui/star.svg",
            Self::StarOff => "icons/ui/star-off.svg",
            Self::Loader => "icons/ui/loader.svg",
            Self::CircleX => "icons/ui/circle-x.svg",
            Self::CircleCheck => "icons/ui/circle-check.svg",
            Self::Info => "icons/ui/info.svg",
            Self::TriangleAlert => "icons/ui/triangle-alert.svg",
            Self::Sun => "icons/ui/sun.svg",
            Self::Moon => "icons/ui/moon.svg",
            Self::Palette => "icons/ui/palette.svg",
            Self::Bot => "icons/ui/bot.svg",
            Self::User => "icons/ui/user.svg",
            Self::Message => "icons/ui/message.svg",
            Self::SquareTerminal => "icons/ui/square-terminal.svg",
            Self::Frame => "icons/ui/frame.svg",
            Self::Mcp => "icons/ui/mcp.svg",
        }
    }
}

impl IconNamed for AppIcon {
    fn path(self) -> SharedString {
        self.svg_path().into()
    }
}

impl IntoElement for AppIcon {
    type Element = AnyElement;

    fn into_element(self) -> Self::Element {
        // Default size 16px
        self.with_size(px(16.)).into_any_element()
    }
}

// =============================================================================
// Platform-Native Icon Element
// =============================================================================

/// A sized AppIcon element that renders using SF Symbols on macOS.
#[derive(Clone, Copy)]
pub struct AppIconElement {
    icon: AppIcon,
    size: Pixels,
}

impl IntoElement for AppIconElement {
    type Element = AnyElement;

    fn into_element(self) -> Self::Element {
        #[cfg(target_os = "macos")]
        {
            // Use SF Symbols on macOS via gpui-symbols
            SfIcon::from_name(self.icon.sf_symbol())
                .size(self.size)
                .into_any_element()
        }

        #[cfg(not(target_os = "macos"))]
        {
            // Use SVG icons on other platforms
            Icon::new(self.icon).size(self.size).into_any_element()
        }
    }
}

// =============================================================================
// Button Extension for AppIcon
// =============================================================================

/// Extension trait for Button to use AppIcon with platform-native rendering.
///
/// This allows using SF Symbols on macOS and SVG icons on other platforms
/// with a consistent API similar to `Button::icon()`.
///
/// # Example
///
/// ```rust,ignore
/// use crate::assets::{AppIcon, ButtonAppIconExt};
///
/// Button::new("settings")
///     .app_icon(AppIcon::Settings)
///     .ghost()
/// ```
pub trait ButtonAppIconExt {
    /// Set a platform-native icon for this button.
    /// - macOS: Uses SF Symbols
    /// - Other platforms: Uses SVG icons
    fn app_icon(self, icon: AppIcon) -> Self;

    /// Set a platform-native icon with custom size.
    fn app_icon_with_size(self, icon: AppIcon, size: Pixels) -> Self;
}

impl ButtonAppIconExt for Button {
    fn app_icon(self, icon: AppIcon) -> Self {
        self.app_icon_with_size(icon, px(16.))
    }

    fn app_icon_with_size(self, icon: AppIcon, size: Pixels) -> Self {
        #[cfg(target_os = "macos")]
        {
            // Use SF Symbols on macOS
            // Manually set square size to match Icon Button mode (size_8 = 32px)
            self.size_8()
                .flex()
                .items_center()
                .justify_center()
                .child(icon.with_size(size))
        }

        #[cfg(not(target_os = "macos"))]
        {
            // Use Button's native icon() method for proper icon button mode
            self.icon(Icon::new(icon).with_size(size))
        }
    }
}

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
