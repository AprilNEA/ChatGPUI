// SPDX-FileCopyrightText: 2026 AprilNEA LLC
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

//! Syntax highlighting for code blocks using syntect

use gpui::*;
use gpui_component::v_flex;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, LazyLock, RwLock};
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style, ThemeSet};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

/// 全局 SyntaxSet 缓存（只加载一次）
static SYNTAX_SET: LazyLock<Arc<SyntaxSet>> =
    LazyLock::new(|| Arc::new(SyntaxSet::load_defaults_newlines()));

/// 全局 ThemeSet 缓存（只加载一次）
static THEME_SET: LazyLock<Arc<ThemeSet>> = LazyLock::new(|| Arc::new(ThemeSet::load_defaults()));

/// 语法高亮结果缓存
static HIGHLIGHT_CACHE: LazyLock<RwLock<HashMap<u64, Vec<Vec<(Hsla, String, bool, bool)>>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

fn hash_code(code: &str, language: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    let mut hasher = DefaultHasher::new();
    code.hash(&mut hasher);
    language.hash(&mut hasher);
    hasher.finish()
}

/// Syntax highlighter for code blocks
pub struct SyntaxHighlighter {
    syntax_set: Arc<SyntaxSet>,
    theme_set: Arc<ThemeSet>,
    theme_name: String,
}

impl Default for SyntaxHighlighter {
    fn default() -> Self {
        Self::new()
    }
}

impl SyntaxHighlighter {
    /// Create a new syntax highlighter with default themes
    pub fn new() -> Self {
        Self {
            syntax_set: SYNTAX_SET.clone(),
            theme_set: THEME_SET.clone(),
            theme_name: "base16-ocean.dark".to_string(),
        }
    }

    /// Set the highlighting theme
    pub fn with_theme(mut self, theme_name: impl Into<String>) -> Self {
        self.theme_name = theme_name.into();
        self
    }

    /// Highlight code and return GPUI elements
    pub fn highlight(&self, code: &str, language: &str) -> AnyElement {
        let hash = hash_code(code, language);

        // 检查缓存
        if let Ok(cache) = HIGHLIGHT_CACHE.read() {
            if let Some(cached_lines) = cache.get(&hash) {
                return self.render_cached_lines(cached_lines);
            }
        }

        // 缓存未命中，进行高亮处理
        let normalized_lang = normalize_language(language);

        let syntax = self
            .syntax_set
            .find_syntax_by_token(normalized_lang)
            .or_else(|| self.syntax_set.find_syntax_by_extension(normalized_lang))
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());

        let theme = self
            .theme_set
            .themes
            .get(&self.theme_name)
            .unwrap_or_else(|| self.theme_set.themes.values().next().unwrap());

        let mut highlighter = HighlightLines::new(syntax, theme);

        let mut cached_lines: Vec<Vec<(Hsla, String, bool, bool)>> = Vec::new();

        for line in LinesWithEndings::from(code) {
            match highlighter.highlight_line(line, &self.syntax_set) {
                Ok(ranges) => {
                    let line_data: Vec<(Hsla, String, bool, bool)> = ranges
                        .iter()
                        .map(|(style, text)| {
                            let color = self.style_to_hsla(style);
                            let is_bold = style
                                .font_style
                                .contains(syntect::highlighting::FontStyle::BOLD);
                            let is_italic = style
                                .font_style
                                .contains(syntect::highlighting::FontStyle::ITALIC);
                            (color, text.to_string(), is_bold, is_italic)
                        })
                        .collect();
                    cached_lines.push(line_data);
                }
                Err(_) => {
                    cached_lines.push(vec![(hsla(0.0, 0.0, 0.8, 1.0), line.to_string(), false, false)]);
                }
            }
        }

        // 存入缓存
        if let Ok(mut cache) = HIGHLIGHT_CACHE.write() {
            if cache.len() > 500 {
                cache.clear();
            }
            cache.insert(hash, cached_lines.clone());
        }

        self.render_cached_lines(&cached_lines)
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
                div().flex().flex_row().children(spans).into_any_element()
            })
            .collect();

        v_flex().children(lines).into_any_element()
    }

    fn style_to_hsla(&self, style: &Style) -> Hsla {
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
