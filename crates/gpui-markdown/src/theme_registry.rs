// SPDX-FileCopyrightText: 2026 AprilNEA LLC
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

//! Code theme registry for syntax highlighting
//!
//! Supports loading themes from:
//! - Built-in syntect themes
//! - .tmTheme files (TextMate format)
//! - VS Code JSON theme files

use gpui::{App, Global, Hsla, hsla};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::{Arc, RwLock};
use syntect::highlighting::{Theme, ThemeSet};

/// A code theme entry with metadata
#[derive(Clone)]
pub struct CodeThemeEntry {
    pub name: String,
    pub theme: Arc<Theme>,
    pub is_dark: bool,
    pub source: CodeThemeSource,
}

/// Source of a code theme
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CodeThemeSource {
    /// Built-in syntect theme
    Builtin,
    /// Loaded from .tmTheme file
    TmTheme,
    /// Loaded from VS Code JSON file
    VsCode,
}

/// Global registry for code syntax highlighting themes
pub struct CodeThemeRegistry {
    themes: RwLock<HashMap<String, CodeThemeEntry>>,
    current_theme: RwLock<String>,
}

impl Global for CodeThemeRegistry {}

impl Default for CodeThemeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl CodeThemeRegistry {
    /// Create a new theme registry with built-in themes
    pub fn new() -> Self {
        let mut themes = HashMap::new();

        // Load built-in syntect themes
        let theme_set = ThemeSet::load_defaults();
        for (name, theme) in theme_set.themes {
            let is_dark = is_theme_dark(&theme);
            themes.insert(
                name.clone(),
                CodeThemeEntry {
                    name: name.clone(),
                    theme: Arc::new(theme),
                    is_dark,
                    source: CodeThemeSource::Builtin,
                },
            );
        }

        Self {
            themes: RwLock::new(themes),
            current_theme: RwLock::new("base16-ocean.dark".to_string()),
        }
    }

    /// Get the global theme registry
    pub fn global(cx: &App) -> &Self {
        cx.global::<Self>()
    }

    /// Get a mutable reference to the global theme registry
    pub fn global_mut(cx: &mut App) -> &mut Self {
        cx.global_mut::<Self>()
    }

    /// Initialize the global theme registry
    pub fn init(cx: &mut App) {
        cx.set_global(Self::new());
    }

    /// Get all available theme names
    pub fn theme_names(&self) -> Vec<String> {
        let themes = self.themes.read().unwrap();
        let mut names: Vec<_> = themes.keys().cloned().collect();
        names.sort();
        names
    }

    /// Get all themes
    pub fn themes(&self) -> HashMap<String, CodeThemeEntry> {
        self.themes.read().unwrap().clone()
    }

    /// Get a theme by name
    pub fn get_theme(&self, name: &str) -> Option<Arc<Theme>> {
        self.themes
            .read()
            .unwrap()
            .get(name)
            .map(|e| e.theme.clone())
    }

    /// Get theme entry by name
    pub fn get_theme_entry(&self, name: &str) -> Option<CodeThemeEntry> {
        self.themes.read().unwrap().get(name).cloned()
    }

    /// Get the current theme name
    pub fn current_theme_name(&self) -> String {
        self.current_theme.read().unwrap().clone()
    }

    /// Get the current theme
    pub fn current_theme(&self) -> Arc<Theme> {
        let name = self.current_theme.read().unwrap().clone();
        self.get_theme(&name)
            .unwrap_or_else(|| self.get_theme("base16-ocean.dark").unwrap())
    }

    /// Set the current theme by name
    pub fn set_current_theme(&self, name: &str) {
        if self.themes.read().unwrap().contains_key(name) {
            *self.current_theme.write().unwrap() = name.to_string();
        }
    }

    /// Load themes from a directory
    ///
    /// Supports .tmTheme and .json (VS Code format) files
    pub fn load_from_directory(&self, dir: &Path) -> Result<usize, std::io::Error> {
        if !dir.exists() {
            return Ok(0);
        }

        let mut count = 0;

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file()
                && let Some(ext) = path.extension()
            {
                let ext = ext.to_string_lossy().to_lowercase();
                match ext.as_str() {
                    "tmtheme" => {
                        if let Ok(()) = self.load_tmtheme(&path) {
                            count += 1;
                        }
                    }
                    "json" => {
                        if let Ok(()) = self.load_vscode_theme(&path) {
                            count += 1;
                        }
                    }
                    _ => {}
                }
            }
        }

        Ok(count)
    }

    /// Load a .tmTheme file
    pub fn load_tmtheme(&self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let theme = ThemeSet::get_theme(path)?;
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Unknown")
            .to_string();

        let is_dark = is_theme_dark(&theme);

        self.themes.write().unwrap().insert(
            name.clone(),
            CodeThemeEntry {
                name,
                theme: Arc::new(theme),
                is_dark,
                source: CodeThemeSource::TmTheme,
            },
        );

        Ok(())
    }

    /// Load a VS Code JSON theme file
    pub fn load_vscode_theme(&self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let content = fs::read_to_string(path)?;
        self.load_vscode_theme_from_str(&content, path)
    }

    /// Load a VS Code theme from string content
    pub fn load_vscode_theme_from_str(
        &self,
        content: &str,
        path: &Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use std::str::FromStr;
        use vscode_theme_syntect::VscodeTheme;

        let vscode_theme = VscodeTheme::from_str(content)?;
        let theme: Theme = vscode_theme.try_into()?;

        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Unknown")
            .to_string();

        let is_dark = is_theme_dark(&theme);

        self.themes.write().unwrap().insert(
            name.clone(),
            CodeThemeEntry {
                name,
                theme: Arc::new(theme),
                is_dark,
                source: CodeThemeSource::VsCode,
            },
        );

        Ok(())
    }

    /// Get themes matching the current UI mode (dark/light)
    pub fn themes_for_mode(&self, is_dark: bool) -> Vec<String> {
        self.themes
            .read()
            .unwrap()
            .iter()
            .filter(|(_, entry)| entry.is_dark == is_dark)
            .map(|(name, _)| name.clone())
            .collect()
    }

    /// Get the current theme's background color as GPUI Hsla
    pub fn current_background(&self) -> Option<Hsla> {
        let theme = self.current_theme();
        theme
            .settings
            .background
            .map(|c| color_to_hsla(c.r, c.g, c.b, c.a))
    }

    /// Get the current theme's foreground color as GPUI Hsla
    pub fn current_foreground(&self) -> Option<Hsla> {
        let theme = self.current_theme();
        theme
            .settings
            .foreground
            .map(|c| color_to_hsla(c.r, c.g, c.b, c.a))
    }

    /// Check if the current theme is dark
    pub fn is_current_theme_dark(&self) -> bool {
        let name = self.current_theme.read().unwrap().clone();
        self.themes
            .read()
            .unwrap()
            .get(&name)
            .map(|e| e.is_dark)
            .unwrap_or(true)
    }
}

/// Determine if a theme is dark based on its background color
fn is_theme_dark(theme: &Theme) -> bool {
    if let Some(settings) = &theme.settings.background {
        // Calculate relative luminance
        let r = settings.r as f32 / 255.0;
        let g = settings.g as f32 / 255.0;
        let b = settings.b as f32 / 255.0;
        let luminance = 0.2126 * r + 0.7152 * g + 0.0722 * b;
        luminance < 0.5
    } else {
        // Default to dark if no background specified
        true
    }
}

/// Convert RGB color to GPUI Hsla
fn color_to_hsla(r: u8, g: u8, b: u8, a: u8) -> Hsla {
    let r = r as f32 / 255.0;
    let g = g as f32 / 255.0;
    let b = b as f32 / 255.0;
    let a = a as f32 / 255.0;

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
