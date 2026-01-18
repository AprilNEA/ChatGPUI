// SPDX-FileCopyrightText: 2026 AprilNEA LLC
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

use gpui::*;
use gpui_component::{
    ActiveTheme, Icon, IconName, Sizable,
    button::{Button, ButtonVariants},
    h_flex,
    label::Label,
};

use crate::settings::{get_settings, update_settings};

pub struct ModelSelectorChangedEvent {
    pub provider_id: String,
    pub model: String,
}

/// Model selector header component
pub struct ModelSelector {
    selected_provider_id: Option<String>,
    selected_model: Option<String>,
}

impl EventEmitter<ModelSelectorChangedEvent> for ModelSelector {}

impl ModelSelector {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let settings = get_settings(cx);
        let selected_provider_id = settings.active_provider_id.clone();
        let selected_model = settings.active_provider().map(|p| p.default_model.clone());

        Self {
            selected_provider_id,
            selected_model,
        }
    }

    fn get_provider_display(&self, cx: &App) -> (String, String) {
        let settings = get_settings(cx);

        if let Some(provider_id) = &self.selected_provider_id {
            if let Some(provider) = settings.providers.iter().find(|p| &p.id == provider_id) {
                let model_name = self
                    .selected_model
                    .clone()
                    .unwrap_or_else(|| provider.default_model.clone());
                return (model_name, provider.name.clone());
            }
        }

        (
            t!("menu.select_model").to_string(),
            t!("menu.not_configured").to_string(),
        )
    }
}

impl Render for ModelSelector {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let (model_name, provider_name) = self.get_provider_display(cx);

        h_flex()
            .w_full()
            .h(px(52.))
            .px_4()
            .justify_between()
            .items_center()
            .border_b_1()
            .border_color(theme.border)
            .bg(theme.background)
            .child(
                // Left side: Model selector
                h_flex()
                    .gap_1()
                    .items_center()
                    .child(
                        Button::new("model-trigger").ghost().child(
                            h_flex()
                                .gap_1()
                                .items_center()
                                .child(
                                    Label::new(model_name)
                                        .text_base()
                                        .font_weight(FontWeight::MEDIUM),
                                )
                                .child(Icon::new(IconName::ChevronDown).size_4()),
                        ),
                    )
                    .child(
                        Label::new(provider_name)
                            .text_sm()
                            .text_color(theme.muted_foreground),
                    ),
            )
            .child(
                // Right side: Actions
                h_flex().gap_1().child(
                    Button::new("toggle-panel")
                        .icon(IconName::PanelRight)
                        .ghost()
                        .small(),
                ),
            )
    }
}
