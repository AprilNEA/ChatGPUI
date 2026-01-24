// SPDX-FileCopyrightText: 2026 AprilNEA LLC
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

pub mod app_icon;
pub mod language;
pub mod llm_provider;

pub use app_icon::{AppIcon, ButtonAppIconExt};
// pub use language::Language;
pub use llm_provider::LlmProvider;

use std::borrow::Cow;

use gpui::{AssetSource, SharedString};
use gpui_component::{Icon, IconNamed};
use rust_embed::RustEmbed;

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
