// SPDX-FileCopyrightText: 2026 AprilNEA LLC
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

use gpui::{App, Global};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::Provider;

/// Global application settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub providers: Vec<Provider>,
    pub active_provider_id: Option<String>,
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
        }
    }
}

impl AppSettings {
    /// Get the configuration file path
    fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("chatgpui")
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
