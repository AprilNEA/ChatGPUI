// SPDX-FileCopyrightText: 2026 AprilNEA LLC
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

use gpui::{App, Global, SharedString, Window};
use gpui_component::{Theme, ThemeMode};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::Provider;

/// Send message shortcut
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum SendShortcut {
    #[default]
    Enter,
    CmdEnter,
}

/// Application icon placement
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum IconPlacement {
    Dock,
    MenuBar,
    #[default]
    Both,
}

/// Appearance mode
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum AppearanceMode {
    #[default]
    System,
    Light,
    Dark,
}

/// Accent color mode
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum AccentColor {
    #[default]
    System,
    Blue,
    Purple,
    Pink,
    Red,
    Orange,
    Yellow,
    Green,
}

/// Appearance settings
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppearanceSettings {
    /// Theme name (e.g., "Tokyo Night", "Catppuccin Mocha")
    #[serde(default)]
    pub theme: Option<String>,
    #[serde(default)]
    pub mode: AppearanceMode,
    #[serde(default)]
    pub accent_color: AccentColor,
    /// UI font family name (None means system default)
    #[serde(default)]
    pub ui_font: Option<String>,
    /// Code font family name (None means system default)
    #[serde(default)]
    pub code_font: Option<String>,
    /// Code syntax highlighting theme (e.g., "base16-ocean.dark")
    #[serde(default)]
    pub code_theme: Option<String>,
}

impl AppearanceSettings {
    /// Apply theme configuration
    pub fn apply_theme(&self, cx: &mut App) {
        use gpui_component::ThemeRegistry;

        if let Some(theme_name) = &self.theme {
            let registry = ThemeRegistry::global(cx);
            if let Some(theme_config) = registry.themes().get(theme_name.as_str()).cloned() {
                Theme::global_mut(cx).apply_config(&theme_config);
            }
        }
    }

    /// Apply appearance mode to the theme system
    pub fn apply_mode(&self, window: Option<&mut Window>, cx: &mut App) {
        match self.mode {
            AppearanceMode::System => {
                Theme::sync_system_appearance(window, cx);
            }
            AppearanceMode::Light => {
                Theme::change(ThemeMode::Light, window, cx);
            }
            AppearanceMode::Dark => {
                Theme::change(ThemeMode::Dark, window, cx);
            }
        }
    }

    /// Apply UI font to the theme system
    pub fn apply_ui_font(&self, cx: &mut App) {
        let theme = Theme::global_mut(cx);
        if let Some(font) = &self.ui_font {
            theme.font_family = SharedString::from(font.clone());
        } else {
            // Use system default font
            theme.font_family = SharedString::from(".SystemUIFont");
        }
    }

    /// Apply code font to the theme system
    pub fn apply_code_font(&self, cx: &mut App) {
        let theme = Theme::global_mut(cx);
        if let Some(font) = &self.code_font {
            theme.mono_font_family = SharedString::from(font.clone());
        } else {
            // Use system default monospace font
            theme.mono_font_family = SharedString::from("monospace");
        }
    }

    /// Apply code syntax highlighting theme
    pub fn apply_code_theme(&self, cx: &mut App) {
        use gpui_markdown::CodeThemeRegistry;

        let registry = CodeThemeRegistry::global(cx);
        if let Some(theme_name) = &self.code_theme {
            registry.set_current_theme(theme_name);
        } else {
            // Use default theme based on UI mode
            let is_dark = Theme::global(cx).mode.is_dark();
            let default_theme = if is_dark {
                "base16-ocean.dark"
            } else {
                "base16-ocean.light"
            };
            registry.set_current_theme(default_theme);
        }
    }

    /// Apply all appearance settings
    pub fn apply_all(&self, window: Option<&mut Window>, cx: &mut App) {
        self.apply_theme(cx);
        self.apply_mode(window, cx);
        self.apply_ui_font(cx);
        self.apply_code_font(cx);
        self.apply_code_theme(cx);
    }
}

/// Global application settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    // Provider settings
    pub providers: Vec<Provider>,
    pub active_provider_id: Option<String>,

    // General settings
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default)]
    pub send_shortcut: SendShortcut,
    #[serde(default)]
    pub icon_placement: IconPlacement,
    #[serde(default = "default_auto_scroll")]
    pub auto_scroll: bool,
    #[serde(default)]
    pub proxy: Option<String>,

    // Appearance settings
    #[serde(default)]
    pub appearance: AppearanceSettings,
}

fn default_language() -> String {
    "en".to_string()
}

fn default_auto_scroll() -> bool {
    true
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            providers: vec![
                Provider::anthropic(),
                Provider::openai(),
                Provider::azure_openai(),
                Provider::deepseek(),
                Provider::google_ai(),
                Provider::groq(),
                Provider::mistral(),
                Provider::ollama(),
                Provider::openrouter(),
            ],
            active_provider_id: Some("anthropic".to_string()),
            language: default_language(),
            send_shortcut: SendShortcut::default(),
            icon_placement: IconPlacement::default(),
            auto_scroll: default_auto_scroll(),
            proxy: None,
            appearance: AppearanceSettings::default(),
        }
    }
}

impl AppSettings {
    /// Get the configuration file path
    fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(env!("APP_IDENTIFIER"))
            .join("settings.json")
    }

    /// Load settings from disk
    pub fn load() -> Self {
        let path = Self::config_path();
        if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(content) => match serde_json::from_str(&content) {
                    Ok(settings) => return settings,
                    Err(e) => tracing::warn!("Failed to parse settings: {}", e),
                },
                Err(e) => tracing::warn!("Failed to read settings file: {}", e),
            }
        }
        Self::default()
    }

    /// Save settings to disk
    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Get the active provider
    pub fn active_provider(&self) -> Option<&Provider> {
        self.active_provider_id
            .as_ref()
            .and_then(|id| self.providers.iter().find(|p| &p.id == id))
    }

    /// Get a mutable reference to a provider by ID
    pub fn provider_mut(&mut self, id: &str) -> Option<&mut Provider> {
        self.providers.iter_mut().find(|p| p.id == id)
    }
}

/// Global settings state wrapper
pub struct GlobalSettings(pub AppSettings);

impl Global for GlobalSettings {}

/// Initialize global settings
pub fn init(cx: &mut App) {
    let settings = AppSettings::load();
    cx.set_global(GlobalSettings(settings));
}

/// Get global settings (read-only)
pub fn get_settings(cx: &App) -> &AppSettings {
    &cx.global::<GlobalSettings>().0
}

/// Update global settings
pub fn update_settings<F>(cx: &mut App, f: F)
where
    F: FnOnce(&mut AppSettings),
{
    let settings = cx.global_mut::<GlobalSettings>();
    f(&mut settings.0);
    if let Err(e) = settings.0.save() {
        tracing::error!("Failed to save settings: {}", e);
    }
}

/// Apply appearance settings from the current configuration
pub fn apply_appearance_settings(window: Option<&mut Window>, cx: &mut App) {
    let appearance = get_settings(cx).appearance.clone();
    appearance.apply_all(window, cx);
}
