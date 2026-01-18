// SPDX-FileCopyrightText: 2026 AprilNEA LLC
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

use std::collections::HashMap;

use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::{
    ActiveTheme, Icon, IconName, Sizable,
    button::{Button, ButtonVariants},
    h_flex,
    label::Label,
    menu::{DropdownMenu, PopupMenuItem},
};
use gpui_tokio_bridge::Tokio;

use crate::llm::{self, Model};
use crate::settings::{get_settings, update_settings};

/// Event emitted when the model selection changes
pub struct ModelSelectorChangedEvent {
    pub provider_id: String,
    pub model_id: String,
}

/// Model selector header component with dropdown
pub struct ModelSelector {
    sidebar_collapsed: bool,
    /// Cached models per provider (provider_id -> models)
    cached_models: HashMap<String, Vec<Model>>,
    /// Whether we're currently fetching models
    is_fetching: bool,
}

impl EventEmitter<ModelSelectorChangedEvent> for ModelSelector {}

impl ModelSelector {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let mut this = Self {
            sidebar_collapsed: false,
            cached_models: HashMap::new(),
            is_fetching: false,
        };

        // Pre-populate with static models
        let settings = get_settings(cx);
        for provider in &settings.providers {
            if provider.enabled && !provider.api_key.is_empty() {
                let models = llm::get_models_for_provider(&provider.id);
                this.cached_models.insert(provider.id.clone(), models);
            }
        }

        // Start async fetch for fresh models
        this.fetch_all_models(cx);

        this
    }

    /// Fetch models from all configured providers
    fn fetch_all_models(&mut self, cx: &mut Context<Self>) {
        if self.is_fetching {
            return;
        }
        self.is_fetching = true;

        let settings = get_settings(cx);
        let providers: Vec<_> = settings
            .providers
            .iter()
            .filter(|p| p.enabled && !p.api_key.is_empty())
            .cloned()
            .collect();

        let (tx, rx) = async_channel::unbounded();

        // Spawn async task to fetch models
        Tokio::spawn(cx, async move {
            let mut results: HashMap<String, Vec<Model>> = HashMap::new();

            for provider in providers {
                match llm::fetch_models_for_provider(&provider).await {
                    Ok(models) => {
                        tracing::info!(
                            "Fetched {} models for provider {}",
                            models.len(),
                            provider.id
                        );
                        results.insert(provider.id.clone(), models);
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Failed to fetch models for {}: {}, using static list",
                            provider.id,
                            e
                        );
                        // Keep static models as fallback
                        results.insert(
                            provider.id.clone(),
                            llm::get_models_for_provider(&provider.id),
                        );
                    }
                }
            }

            let _ = tx.send(results).await;
        })
        .detach();

        // Handle the result when it comes back
        cx.spawn(async move |this, cx| {
            if let Ok(results) = rx.recv().await {
                this.update(cx, |this, cx| {
                    this.cached_models = results;
                    this.is_fetching = false;
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    pub fn set_sidebar_collapsed(&mut self, collapsed: bool, cx: &mut Context<Self>) {
        self.sidebar_collapsed = collapsed;
        cx.notify();
    }

    /// Refresh models from API
    pub fn refresh_models(&mut self, cx: &mut Context<Self>) {
        // Clear cache and re-fetch
        self.cached_models.clear();
        llm::get_model_cache();
        self.fetch_all_models(cx);
    }

    fn get_provider_display(&self, cx: &App) -> (String, String) {
        let settings = get_settings(cx);

        if let Some(provider) = settings.active_provider() {
            let model_id = provider.default_model.clone();

            // Try to find the model name from cached models first
            let model_name = self
                .cached_models
                .get(&provider.id)
                .and_then(|models| models.iter().find(|m| m.id == model_id))
                .map(|m| m.name.clone())
                .unwrap_or_else(|| {
                    // Fallback to static models
                    llm::get_models_for_provider(&provider.id)
                        .into_iter()
                        .find(|m| m.id == model_id)
                        .map(|m| m.name.clone())
                        .unwrap_or(model_id)
                });

            return (model_name, provider.name.clone());
        }

        (
            t!("menu.select_model").to_string(),
            t!("menu.not_configured").to_string(),
        )
    }

    fn get_models_for_provider(&self, provider_id: &str) -> Vec<Model> {
        self.cached_models
            .get(provider_id)
            .cloned()
            .unwrap_or_else(|| llm::get_models_for_provider(provider_id))
    }

    fn render_provider_icon(provider_id: &str) -> Div {
        let (icon_text, bg_color) = match provider_id {
            "anthropic" => ("A", hsla(0.08, 0.8, 0.55, 1.0)),
            "openai" => ("O", hsla(0.45, 0.7, 0.4, 1.0)),
            "google_ai" => ("G", hsla(0.6, 0.8, 0.5, 1.0)),
            "deepseek" => ("D", hsla(0.55, 0.7, 0.5, 1.0)),
            "mistral" => ("M", hsla(0.75, 0.6, 0.5, 1.0)),
            "groq" => ("G", hsla(0.95, 0.7, 0.5, 1.0)),
            _ => ("?", hsla(0.0, 0.0, 0.5, 1.0)),
        };

        div()
            .size_4()
            .rounded_sm()
            .bg(bg_color)
            .flex()
            .items_center()
            .justify_center()
            .child(
                Label::new(icon_text)
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(white()),
            )
    }
}

impl Render for ModelSelector {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let (model_name, provider_name) = self.get_provider_display(cx);
        let collapsed = self.sidebar_collapsed;

        // Get the entity for use in closures
        let entity = cx.entity().clone();

        // Collect models data for use in closure
        let cached_models = self.cached_models.clone();

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
                h_flex()
                    .gap_1()
                    .items_center()
                    .when(collapsed, |el| el.pl(px(96.)))
                    .child(
                        Button::new("model-trigger")
                            .ghost()
                            .child(
                                h_flex()
                                    .gap_1()
                                    .items_center()
                                    .child(
                                        Label::new(model_name)
                                            .text_base()
                                            .font_weight(FontWeight::MEDIUM),
                                    )
                                    .child(Icon::new(IconName::ChevronDown).size_4()),
                            )
                            .dropdown_menu(move |menu, _window, cx| {
                                let settings = get_settings(cx);
                                let current_model = settings
                                    .active_provider()
                                    .map(|p| p.default_model.clone())
                                    .unwrap_or_default();
                                let current_provider_id = settings
                                    .active_provider_id
                                    .clone()
                                    .unwrap_or_default();

                                let providers: Vec<_> = settings
                                    .providers
                                    .iter()
                                    .filter(|p| p.enabled && !p.api_key.is_empty())
                                    .collect();

                                let mut menu = menu.scrollable(true).max_h(px(400.)).min_w(px(280.));

                                for provider in providers {
                                    // Use cached models if available, otherwise static
                                    let models = cached_models
                                        .get(&provider.id)
                                        .cloned()
                                        .unwrap_or_else(|| llm::get_models_for_provider(&provider.id));

                                    let provider_id = provider.id.clone();
                                    let provider_name = provider.name.clone();

                                    // Add provider header (non-clickable label)
                                    menu = menu.item(
                                        PopupMenuItem::element({
                                            let provider_id = provider_id.clone();
                                            move |_window, cx| {
                                                h_flex()
                                                    .gap_2()
                                                    .items_center()
                                                    .child(Self::render_provider_icon(&provider_id))
                                                    .child(
                                                        Label::new(provider_name.clone())
                                                            .text_sm()
                                                            .font_weight(FontWeight::SEMIBOLD)
                                                            .text_color(cx.theme().foreground),
                                                    )
                                            }
                                        })
                                        .disabled(true),
                                    );

                                    // Add models for this provider
                                    for model in models {
                                        let model_id = model.id.clone();
                                        let model_name = model.name.clone();
                                        let provider_id_for_click = provider.id.clone();
                                        let entity_clone = entity.clone();

                                        let is_selected =
                                            current_provider_id == provider.id && current_model == model_id;

                                        menu = menu.item(
                                            PopupMenuItem::element({
                                                let model_name = model_name.clone();
                                                let supports_vision = model.supports_vision;
                                                let supports_tools = model.supports_tools;
                                                move |_window, cx| {
                                                    h_flex()
                                                        .w_full()
                                                        .gap_2()
                                                        .items_center()
                                                        .justify_between()
                                                        .child(
                                                            h_flex()
                                                                .gap_2()
                                                                .items_center()
                                                                .pl_5()
                                                                .child(
                                                                    Label::new(model_name.clone())
                                                                        .text_sm(),
                                                                ),
                                                        )
                                                        .child(
                                                            h_flex()
                                                                .gap_1()
                                                                .items_center()
                                                                .when(is_selected, |el| {
                                                                    el.child(
                                                                        Icon::new(IconName::Check)
                                                                            .size_4()
                                                                            .text_color(cx.theme().accent),
                                                                    )
                                                                })
                                                                .when(supports_vision, |el| {
                                                                    el.child(
                                                                        Icon::new(IconName::Eye)
                                                                            .size_3()
                                                                            .text_color(
                                                                                cx.theme().muted_foreground,
                                                                            ),
                                                                    )
                                                                })
                                                                .when(supports_tools, |el| {
                                                                    el.child(
                                                                        Icon::new(IconName::Settings2)
                                                                            .size_3()
                                                                            .text_color(
                                                                                cx.theme().muted_foreground,
                                                                            ),
                                                                    )
                                                                }),
                                                        )
                                                }
                                            })
                                            .on_click({
                                                let provider_id = provider_id_for_click.clone();
                                                let model_id = model_id.clone();
                                                let entity = entity_clone.clone();
                                                move |_, _window, cx| {
                                                    // Update settings
                                                    update_settings(cx, |settings| {
                                                        settings.active_provider_id =
                                                            Some(provider_id.clone());
                                                        if let Some(provider) = settings
                                                            .providers
                                                            .iter_mut()
                                                            .find(|p| p.id == provider_id)
                                                        {
                                                            provider.default_model = model_id.clone();
                                                        }
                                                    });

                                                    // Emit event
                                                    entity.update(cx, |_this, cx| {
                                                        cx.emit(ModelSelectorChangedEvent {
                                                            provider_id: provider_id.clone(),
                                                            model_id: model_id.clone(),
                                                        });
                                                        cx.notify();
                                                    });
                                                }
                                            }),
                                        );
                                    }
                                }

                                menu
                            }),
                    )
                    .child(
                        Label::new(provider_name)
                            .text_sm()
                            .text_color(theme.muted_foreground),
                    ),
            )
            .child(
                h_flex().gap_1().child(
                    Button::new("toggle-panel")
                        .icon(IconName::PanelRight)
                        .ghost()
                        .small(),
                ),
            )
    }
}
