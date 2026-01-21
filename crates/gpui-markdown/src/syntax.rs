// SPDX-FileCopyrightText: 2026 AprilNEA LLC
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

//! Syntax highlighting for code blocks using syntect

use crate::theme_registry::CodeThemeRegistry;
use gpui::*;
use gpui_component::v_flex;
use std::collections::hash_map::DefaultHasher;
use std::future::Future;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, LazyLock};
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style, Theme};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

/// Global SyntaxSet cache (loaded once).
static SYNTAX_SET: LazyLock<Arc<SyntaxSet>> =
    LazyLock::new(|| Arc::new(SyntaxSet::load_defaults_newlines()));

type HighlightedLines = Vec<Vec<(Hsla, String, bool, bool)>>;

#[derive(Clone, Hash, PartialEq, Eq)]
struct HighlightSource {
    code: SharedString,
    language: SharedString,
    theme_name: SharedString,
}

// Store theme in thread-local for async access
thread_local! {
    static CURRENT_THEME: std::cell::RefCell<Option<Arc<Theme>>> = const { std::cell::RefCell::new(None) };
}

struct HighlightAsset;

#[allow(clippy::manual_async_fn)]
impl Asset for HighlightAsset {
    type Source = HighlightSource;
    type Output = Arc<HighlightedLines>;

    fn load(
        source: Self::Source,
        _cx: &mut App,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        // Get theme from thread-local (set before calling use_asset)
        let theme = CURRENT_THEME.with(|t| t.borrow().clone());
        async move {
            if let Some(theme) = theme {
                Arc::new(highlight_code(&source, &theme))
            } else {
                // Fallback: return empty highlighting
                Arc::new(Vec::new())
            }
        }
    }
}

fn set_current_theme(theme: Arc<Theme>) {
    CURRENT_THEME.with(|t| {
        *t.borrow_mut() = Some(theme);
    });
}

struct HighlightCache {
    lines: Option<Arc<HighlightedLines>>,
    source_hash: Option<u64>,
}

/// Syntax highlighter for code blocks
pub struct SyntaxHighlighter {
    /// Override theme name (if None, uses global registry's current theme)
    theme_name_override: Option<SharedString>,
}

impl Default for SyntaxHighlighter {
    fn default() -> Self {
        Self::new()
    }
}

impl SyntaxHighlighter {
    /// Create a new syntax highlighter that uses the global theme registry
    pub fn new() -> Self {
        Self {
            theme_name_override: None,
        }
    }

    /// Set an override theme (bypasses global registry)
    pub fn with_theme(mut self, theme_name: impl Into<String>) -> Self {
        self.theme_name_override = Some(SharedString::from(theme_name.into()));
        self
    }

    /// Get the theme to use for highlighting
    fn get_theme(&self, cx: &App) -> (SharedString, Arc<Theme>) {
        if let Some(override_name) = &self.theme_name_override {
            // Use override theme
            let registry = CodeThemeRegistry::global(cx);
            if let Some(theme) = registry.get_theme(override_name.as_ref()) {
                return (override_name.clone(), theme);
            }
        }

        // Use global registry's current theme
        let registry = CodeThemeRegistry::global(cx);
        let name = registry.current_theme_name();
        let theme = registry.current_theme();
        (SharedString::from(name), theme)
    }

    /// Highlight code and return GPUI elements
    pub fn highlight(
        &self,
        code: &str,
        language: &str,
        cache_key: ElementId,
        allow_highlighting: bool,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        let cache = window.use_keyed_state(
            ElementId::from((cache_key, "highlight-cache")),
            cx,
            |_window, _cx| HighlightCache {
                lines: None,
                source_hash: None,
            },
        );
        let normalized = normalize_language(language);
        let (theme_name, theme) = self.get_theme(cx);

        let source = HighlightSource {
            code: SharedString::from(code.to_string()),
            language: SharedString::from(normalized.to_string()),
            theme_name,
        };
        let source_hash = hash_source(&source);

        // Set theme in thread-local for async asset loading
        set_current_theme(theme);

        let lines = match window.use_asset::<HighlightAsset>(&source, cx) {
            Some(cached_lines) => {
                let should_update = cache
                    .read(cx)
                    .lines
                    .as_ref()
                    .is_none_or(|cached| !Arc::ptr_eq(cached, &cached_lines));
                if should_update {
                    cache.update(cx, |state, _| {
                        state.lines = Some(cached_lines.clone());
                        state.source_hash = Some(source_hash);
                    });
                }
                Some(cached_lines)
            }
            None => {
                let cache_state = cache.read(cx);
                if cache_state.source_hash == Some(source_hash) {
                    cache_state.lines.clone()
                } else {
                    None
                }
            }
        };

        if allow_highlighting {
            if let Some(cached_lines) = lines {
                self.render_cached_lines(&cached_lines)
            } else {
                self.render_plain_lines(code)
            }
        } else {
            self.render_plain_lines(code)
        }
    }

    fn render_cached_lines(&self, cached_lines: &[Vec<(Hsla, String, bool, bool)>]) -> AnyElement {
        let lines: Vec<AnyElement> = cached_lines
            .iter()
            .map(|line_data| {
                let spans: Vec<AnyElement> = line_data
                    .iter()
                    .map(|(color, text, is_bold, is_italic)| {
                        let mut el = div().text_color(*color).child(text.clone());
                        if *is_bold {
                            el = el.font_weight(FontWeight::BOLD);
                        }
                        if *is_italic {
                            el = el.italic();
                        }
                        el.into_any_element()
                    })
                    .collect();
                div()
                    .flex()
                    .flex_row()
                    .whitespace_nowrap()
                    .flex_shrink_0()
                    .children(spans)
                    .into_any_element()
            })
            .collect();

        v_flex().flex_shrink_0().children(lines).into_any_element()
    }

    fn render_plain_lines(&self, code: &str) -> AnyElement {
        // Use .lines() instead of LinesWithEndings to avoid trailing newlines causing extra spacing
        let lines: Vec<AnyElement> = code
            .lines()
            .map(|line| {
                div()
                    .flex()
                    .flex_row()
                    .whitespace_nowrap()
                    .flex_shrink_0()
                    .child(line.to_string())
                    .into_any_element()
            })
            .collect();

        v_flex().flex_shrink_0().children(lines).into_any_element()
    }
}

fn highlight_code(source: &HighlightSource, theme: &Theme) -> HighlightedLines {
    let syntax_set = SYNTAX_SET.clone();
    let normalized_lang = normalize_language(source.language.as_ref());

    let syntax = syntax_set
        .find_syntax_by_token(normalized_lang)
        .or_else(|| syntax_set.find_syntax_by_extension(normalized_lang))
        .unwrap_or_else(|| syntax_set.find_syntax_plain_text());

    let mut highlighter = HighlightLines::new(syntax, theme);
    let mut cached_lines: HighlightedLines = Vec::new();

    // LinesWithEndings preserves the trailing '\n' on each line.
    // Since we render each line as a separate div in a vertical flex container,
    // we must strip the newline to prevent double line spacing (one from the div
    // layout and another from the embedded newline character).
    for line in LinesWithEndings::from(source.code.as_ref()) {
        match highlighter.highlight_line(line, &syntax_set) {
            Ok(ranges) => {
                let line_data: Vec<(Hsla, String, bool, bool)> = ranges
                    .iter()
                    .map(|(style, text)| {
                        let color = style_to_hsla(style);
                        let is_bold = style
                            .font_style
                            .contains(syntect::highlighting::FontStyle::BOLD);
                        let is_italic = style
                            .font_style
                            .contains(syntect::highlighting::FontStyle::ITALIC);
                        (
                            color,
                            text.trim_end_matches('\n').to_string(),
                            is_bold,
                            is_italic,
                        )
                    })
                    .collect();
                cached_lines.push(line_data);
            }
            Err(_) => {
                cached_lines.push(vec![(
                    hsla(0.0, 0.0, 0.8, 1.0),
                    line.trim_end_matches('\n').to_string(),
                    false,
                    false,
                )]);
            }
        }
    }

    cached_lines
}

fn style_to_hsla(style: &Style) -> Hsla {
    // Convert RGB to HSL
    let r = style.foreground.r as f32 / 255.0;
    let g = style.foreground.g as f32 / 255.0;
    let b = style.foreground.b as f32 / 255.0;
    let a = style.foreground.a as f32 / 255.0;

    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;

    if (max - min).abs() < f32::EPSILON {
        return hsla(0.0, 0.0, l, a);
    }

    let d = max - min;
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };

    let h = if (max - r).abs() < f32::EPSILON {
        ((g - b) / d + if g < b { 6.0 } else { 0.0 }) / 6.0
    } else if (max - g).abs() < f32::EPSILON {
        ((b - r) / d + 2.0) / 6.0
    } else {
        ((r - g) / d + 4.0) / 6.0
    };

    hsla(h, s, l, a)
}

fn hash_source(source: &HighlightSource) -> u64 {
    let mut hasher = DefaultHasher::new();
    source.hash(&mut hasher);
    hasher.finish()
}

/// Map common language aliases to syntect syntax names
fn normalize_language(lang: &str) -> &str {
    match lang {
        "js" | "javascript" => "js",
        "ts" | "typescript" => "ts",
        "py" | "python" => "py",
        "rb" | "ruby" => "rb",
        "rs" | "rust" => "rs",
        "go" | "golang" => "go",
        "cpp" | "c++" => "cpp",
        "cs" | "csharp" | "c#" => "cs",
        "sh" | "bash" | "shell" => "sh",
        "yml" | "yaml" => "yaml",
        "md" | "markdown" => "md",
        other => other,
    }
}
