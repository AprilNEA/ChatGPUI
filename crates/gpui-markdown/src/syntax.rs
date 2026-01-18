// SPDX-FileCopyrightText: 2026 AprilNEA LLC
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

//! Syntax highlighting for code blocks using syntect

use gpui::*;
use gpui_component::v_flex;
use std::sync::Arc;
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style, ThemeSet};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

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
            syntax_set: Arc::new(SyntaxSet::load_defaults_newlines()),
            theme_set: Arc::new(ThemeSet::load_defaults()),
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

        let mut lines: Vec<AnyElement> = Vec::new();

        for line in LinesWithEndings::from(code) {
            match highlighter.highlight_line(line, &self.syntax_set) {
                Ok(ranges) => {
                    let line_element = self.render_highlighted_line(&ranges);
                    lines.push(line_element);
                }
                Err(_) => {
                    // Fallback to plain text if highlighting fails
                    lines.push(div().child(line.to_string()).into_any_element());
                }
            }
        }

        v_flex().children(lines).into_any_element()
    }

    fn render_highlighted_line(&self, ranges: &[(Style, &str)]) -> AnyElement {
        let spans: Vec<AnyElement> = ranges
            .iter()
            .map(|(style, text)| {
                let color = self.style_to_hsla(style);
                let is_bold = style
                    .font_style
                    .contains(syntect::highlighting::FontStyle::BOLD);
                let is_italic = style
                    .font_style
                    .contains(syntect::highlighting::FontStyle::ITALIC);

                let mut el = div().text_color(color).child(text.to_string());

                if is_bold {
                    el = el.font_weight(FontWeight::BOLD);
                }
                if is_italic {
                    el = el.italic();
                }

                el.into_any_element()
            })
            .collect();

        div().flex().flex_row().children(spans).into_any_element()
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
