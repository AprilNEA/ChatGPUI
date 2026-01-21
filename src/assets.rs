// SPDX-FileCopyrightText: 2026 AprilNEA LLC
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

use std::borrow::Cow;

use gpui::{AnyElement, AssetSource, IntoElement, SharedString};
use gpui_component::{Icon, IconNamed};
use rust_embed::RustEmbed;
use strum::{Display, EnumString};

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
    /// Returns the SF Symbol name for this icon (macOS 11+).
    ///
    /// SF Symbols: https://developer.apple.com/sf-symbols/
    #[allow(dead_code)]
    pub fn sf_symbol_name(self) -> &'static str {
        match self {
            // Navigation
            Self::ArrowRight => "arrow.right",
            Self::ArrowLeft => "arrow.left",
            Self::ArrowUp => "arrow.up",
            Self::ArrowDown => "arrow.down",
            Self::ChevronDown => "chevron.down",
            Self::ChevronRight => "chevron.right",
            Self::ChevronUp => "chevron.up",
            Self::ChevronLeft => "chevron.left",

            // Common Actions
            Self::Plus => "plus",
            Self::Minus => "minus",
            Self::Close => "xmark",
            Self::Check => "checkmark",
            Self::Search => "magnifyingglass",
            Self::Copy => "doc.on.doc",
            Self::Delete => "trash",
            Self::Settings => "gearshape",
            Self::Settings2 => "gearshape.2",

            // Panels
            Self::PanelLeft => "sidebar.left",
            Self::PanelRight => "sidebar.right",
            Self::PanelBottom => "sidebar.bottom",

            // Content
            Self::File => "doc",
            Self::Folder => "folder",
            Self::BookOpen => "book",
            Self::Globe => "globe",
            Self::ExternalLink => "arrow.up.right.square",

            // States
            Self::Eye => "eye",
            Self::EyeOff => "eye.slash",
            Self::Star => "star",
            Self::StarOff => "star.slash",
            Self::Loader => "arrow.triangle.2.circlepath",
            Self::CircleX => "xmark.circle",
            Self::CircleCheck => "checkmark.circle",
            Self::Info => "info.circle",
            Self::TriangleAlert => "exclamationmark.triangle",

            // Theme
            Self::Sun => "sun.max",
            Self::Moon => "moon",
            Self::Palette => "paintpalette",

            // Communication
            Self::Bot => "bubble.left.and.text.bubble.right",
            Self::User => "person",
            Self::Message => "message",

            // Developer
            Self::SquareTerminal => "terminal",
            Self::Frame => "square.on.square",

            // Custom (no SF Symbol equivalent, uses custom SVG)
            Self::Mcp => "square.stack.3d.up", // closest approximation
        }
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
        self.icon().into_any_element()
    }
}

// =============================================================================
// SF Symbols Support (macOS only)
// =============================================================================

/// SF Symbols utilities for macOS.
#[cfg(target_os = "macos")]
pub mod sf_symbols {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};

    use gpui::{Hsla, Rgba};
    use objc2::rc::Retained;
    use objc2::{ClassType, msg_send};
    use objc2_app_kit::{NSBitmapImageRep, NSImage};
    use objc2_foundation::{NSData, NSSize, NSString};

    // CGFloat is f64 on 64-bit systems
    type CGFloat = f64;

    static MACOS_VERSION: OnceLock<(u32, u32, u32)> = OnceLock::new();
    static SF_SYMBOL_CACHE: OnceLock<Mutex<HashMap<SfSymbolCacheKey, Vec<u8>>>> = OnceLock::new();

    #[derive(Clone, PartialEq, Eq, Hash)]
    struct SfSymbolCacheKey {
        name: String,
        size: u32,
        color: Option<[u8; 4]>,
    }

    /// Check if SF Symbols are available (macOS 11.0+).
    pub fn is_available() -> bool {
        let version = macos_version();
        version.0 >= 11
    }

    /// Get macOS version as (major, minor, patch).
    pub fn macos_version() -> (u32, u32, u32) {
        *MACOS_VERSION.get_or_init(|| {
            use std::process::Command;

            let output = Command::new("sw_vers")
                .arg("-productVersion")
                .output()
                .ok()
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .unwrap_or_default();

            let parts: Vec<u32> = output
                .trim()
                .split('.')
                .filter_map(|s| s.parse().ok())
                .collect();

            (
                parts.first().copied().unwrap_or(0),
                parts.get(1).copied().unwrap_or(0),
                parts.get(2).copied().unwrap_or(0),
            )
        })
    }

    /// Load an SF Symbol as PNG data.
    pub fn load_symbol(name: &str, size: u32, color: Option<Hsla>) -> Option<Vec<u8>> {
        if !is_available() {
            return None;
        }

        let color_bytes = color.map(|c| {
            let rgba: Rgba = c.into();
            [
                (rgba.r * 255.0) as u8,
                (rgba.g * 255.0) as u8,
                (rgba.b * 255.0) as u8,
                (rgba.a * 255.0) as u8,
            ]
        });

        let cache_key = SfSymbolCacheKey {
            name: name.to_string(),
            size,
            color: color_bytes,
        };

        let cache = SF_SYMBOL_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
        if let Ok(guard) = cache.lock()
            && let Some(cached) = guard.get(&cache_key)
        {
            return Some(cached.clone());
        }

        let png_data = load_symbol_inner(name, size, color_bytes)?;

        if let Ok(mut guard) = cache.lock() {
            guard.insert(cache_key, png_data.clone());
        }

        Some(png_data)
    }

    fn load_symbol_inner(name: &str, size: u32, _color: Option<[u8; 4]>) -> Option<Vec<u8>> {
        unsafe {
            let ns_name = NSString::from_str(name);

            // Load SF Symbol using class method
            let image: Option<Retained<NSImage>> = msg_send![
                NSImage::class(),
                imageWithSystemSymbolName: &*ns_name,
                accessibilityDescription: std::ptr::null::<NSString>()
            ];
            let image = image?;

            // Create symbol configuration with specific point size and weight
            // NSFontWeightRegular = 0.0, NSFontWeightMedium = 0.23
            let config: Option<Retained<objc2::runtime::AnyObject>> = msg_send![
                objc2::class!(NSImageSymbolConfiguration),
                configurationWithPointSize: size as CGFloat,
                weight: 0.0 as CGFloat  // Regular weight
            ];

            // Apply configuration to get properly sized symbol
            let sized_image: Retained<NSImage> = if let Some(cfg) = config {
                let result: Option<Retained<NSImage>> =
                    msg_send![&image, imageWithSymbolConfiguration: &*cfg];
                result.unwrap_or(image)
            } else {
                image
            };

            // Set explicit size
            let target_size = NSSize::new(size as CGFloat, size as CGFloat);
            let _: () = msg_send![&sized_image, setSize: target_size];

            // Get TIFF representation
            let tiff_data: Option<Retained<NSData>> = msg_send![&sized_image, TIFFRepresentation];
            let tiff_data = tiff_data?;

            // Create bitmap rep from TIFF
            let bitmap_rep = NSBitmapImageRep::imageRepWithData(&tiff_data)?;

            // Convert to PNG (NSBitmapImageFileTypePNG = 4)
            let png_data: Option<Retained<NSData>> = msg_send![
                &bitmap_rep,
                representationUsingType: 4u64,
                properties: std::ptr::null::<objc2_foundation::NSDictionary>()
            ];
            let png_data = png_data?;

            let length = png_data.length();
            if length == 0 {
                return None;
            }

            unsafe extern "C" {
                fn CFDataGetBytePtr(data: *const std::ffi::c_void) -> *const u8;
            }

            let cf_data: *const std::ffi::c_void =
                &*png_data as *const objc2_foundation::NSData as *const std::ffi::c_void;
            let bytes_ptr = CFDataGetBytePtr(cf_data);
            if bytes_ptr.is_null() {
                return None;
            }

            let slice = std::slice::from_raw_parts(bytes_ptr, length);
            Some(slice.to_vec())
        }
    }

    /// Clear the SF Symbol cache.
    #[allow(dead_code)]
    pub fn clear_cache() {
        if let Some(cache) = SF_SYMBOL_CACHE.get()
            && let Ok(mut guard) = cache.lock()
        {
            guard.clear();
        }
    }
}

/// SF Symbols stub for non-macOS platforms.
#[cfg(not(target_os = "macos"))]
pub mod sf_symbols {
    use gpui::Hsla;

    /// SF Symbols are only available on macOS.
    pub fn is_available() -> bool {
        false
    }

    /// Stub for non-macOS platforms.
    pub fn load_symbol(_name: &str, _size: u32, _color: Option<Hsla>) -> Option<Vec<u8>> {
        None
    }
}

// =============================================================================
// SF Symbol Icon Component
// =============================================================================

use gpui::{Hsla, Image, ImageFormat, ParentElement, Styled, img};
use std::sync::Arc;

/// An icon that renders using SF Symbols on macOS, with SVG fallback.
///
/// Note: SF Symbols are converted to bitmaps which may appear slightly less sharp
/// than native SVG icons when scaled. Use `use_sf_symbol(true)` to explicitly
/// enable SF Symbol rendering, or leave as default (false) for SVG.
#[derive(Clone)]
#[allow(dead_code)]
pub struct SfSymbolIcon {
    icon: AppIcon,
    size: u32,
    color: Option<Hsla>,
    use_sf_symbol: bool,
}

#[allow(dead_code)]
impl SfSymbolIcon {
    pub fn new(icon: AppIcon) -> Self {
        Self {
            icon,
            size: 16,
            color: None,
            use_sf_symbol: false, // Default to SVG for best quality
        }
    }

    pub fn size(mut self, size: u32) -> Self {
        self.size = size;
        self
    }

    pub fn color(mut self, color: impl Into<Hsla>) -> Self {
        self.color = Some(color.into());
        self
    }

    /// Enable or disable SF Symbol rendering (macOS only).
    /// When disabled (default), uses SVG icons which are always crisp.
    pub fn use_sf_symbol(mut self, enable: bool) -> Self {
        self.use_sf_symbol = enable;
        self
    }
}

impl SfSymbolIcon {
    fn render_inner(self) -> AnyElement {
        let size = gpui::px(self.size as f32);

        // Try to load SF Symbol on macOS if enabled
        #[cfg(target_os = "macos")]
        if self.use_sf_symbol && sf_symbols::is_available() {
            // Render at 4x resolution for Retina screens
            if let Some(png_data) =
                sf_symbols::load_symbol(self.icon.sf_symbol_name(), self.size * 4, self.color)
            {
                let image = Image::from_bytes(ImageFormat::Png, png_data);
                return gpui::div()
                    .size(size)
                    .child(img(Arc::new(image)).size_full())
                    .into_any_element();
            }
        }

        // Use SVG icon (default, always crisp)
        let mut icon = self.icon.icon().size(size);
        if let Some(color) = self.color {
            icon = icon.text_color(color);
        }
        gpui::div().child(icon).into_any_element()
    }
}

impl IntoElement for SfSymbolIcon {
    type Element = AnyElement;

    fn into_element(self) -> Self::Element {
        self.render_inner()
    }
}

/// Extension trait for AppIcon to create SF Symbol icons
#[allow(dead_code)]
pub trait AppIconExt {
    fn sf_icon(self) -> SfSymbolIcon;
}

impl AppIconExt for AppIcon {
    fn sf_icon(self) -> SfSymbolIcon {
        SfSymbolIcon::new(self)
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
