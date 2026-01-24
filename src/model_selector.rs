// SPDX-FileCopyrightText: 2026 AprilNEA LLC
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

use std::collections::HashMap;

use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::{
    ActiveTheme, Sizable,
    button::{Button, ButtonVariants},
    h_flex,
    label::Label,
    popover::Popover,
    v_flex,
};
use gpui_tokio_bridge::Tokio;

use crate::icons::{AppIcon, ButtonAppIconExt};
use crate::llm::{self, Model};
use crate::settings::{OpenSettings, get_settings, update_settings};

/// Event emitted when the model selection changes
#[allow(dead_code)]
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
    /// Currently hovered model for details panel
    hovered_model: Option<Model>,
}

impl EventEmitter<ModelSelectorChangedEvent> for ModelSelector {}

impl ModelSelector {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let mut this = Self {
            sidebar_collapsed: false,
            cached_models: HashMap::new(),
            is_fetching: false,
            hovered_model: None,
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
    #[allow(dead_code)]
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

    #[allow(dead_code)]
    fn get_models_for_provider(&self, provider_id: &str) -> Vec<Model> {
        self.cached_models
            .get(provider_id)
            .cloned()
            .unwrap_or_else(|| llm::get_models_for_provider(provider_id))
    }

    fn render_provider_icon(provider_id: &str, foreground: Hsla) -> Div {
        use crate::icons::{IntoIcon, LlmProvider};
        use std::str::FromStr;

        let provider = LlmProvider::from_str(provider_id).ok();

        if let Some(p) = provider {
            div()
                .size_6()
                .flex()
                .items_center()
                .justify_center()
                .child(p.icon().size_5().text_color(foreground))
        } else {
            // Fallback for unknown providers
            let first_char = provider_id
                .chars()
                .next()
                .unwrap_or('?')
                .to_uppercase()
                .to_string();
            div()
                .size_6()
                .rounded_sm()
                .bg(hsla(0.0, 0.0, 0.5, 1.0))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    Label::new(first_char)
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(white()),
                )
        }
    }

    /// Format a number with K/M suffix
    fn format_number(n: u32) -> String {
        if n >= 1_000_000 {
            format!("{}M", n / 1_000_000)
        } else if n >= 1_000 {
            format!("{}K", n / 1_000)
        } else {
            n.to_string()
        }
    }

    /// Render the model details panel
    fn render_model_details(model: &Model, cx: &App) -> impl IntoElement {
        let theme = cx.theme();

        v_flex()
            .w(px(280.))
            .p_3()
            .gap_3()
            .bg(theme.popover)
            .border_1()
            .border_color(theme.border)
            .rounded_lg()
            .shadow_lg()
            // Header with model name
            .child(
                v_flex()
                    .gap_1()
                    .child(
                        Label::new(model.name.clone())
                            .text_base()
                            .font_weight(FontWeight::SEMIBOLD),
                    )
                    .child(
                        div().px_2().py_0p5().rounded_sm().bg(theme.muted).child(
                            Label::new(model.id.clone())
                                .text_xs()
                                .text_color(theme.muted_foreground),
                        ),
                    ),
            )
            // Description
            .when(model.description.is_some(), |el| {
                el.child(
                    Label::new(model.description.clone().unwrap_or_default())
                        .text_sm()
                        .text_color(theme.muted_foreground),
                )
            })
            // Pricing
            .when(
                model.input_price_per_million.is_some() || model.output_price_per_million.is_some(),
                |el| {
                    el.child(
                        v_flex()
                            .gap_1()
                            .when(model.input_price_per_million.is_some(), |el| {
                                let price = model.input_price_per_million.unwrap();
                                el.child(
                                    h_flex()
                                        .gap_1()
                                        .child(Label::new("Input:").text_sm())
                                        .child(
                                            Label::new(format!("${:.2}/M tokens", price))
                                                .text_sm()
                                                .font_weight(FontWeight::MEDIUM),
                                        ),
                                )
                            })
                            .when(model.output_price_per_million.is_some(), |el| {
                                let price = model.output_price_per_million.unwrap();
                                el.child(
                                    h_flex()
                                        .gap_1()
                                        .child(Label::new("Output:").text_sm())
                                        .child(
                                            Label::new(format!("${:.2}/M tokens", price))
                                                .text_sm()
                                                .font_weight(FontWeight::MEDIUM),
                                        ),
                                )
                            }),
                    )
                },
            )
            // Context and output lengths
            .child(
                v_flex()
                    .gap_1()
                    .when(model.context_window.is_some(), |el| {
                        let ctx = model.context_window.unwrap();
                        el.child(
                            h_flex()
                                .gap_1()
                                .child(Label::new("Context length:").text_sm())
                                .child(
                                    Label::new(Self::format_number(ctx))
                                        .text_sm()
                                        .font_weight(FontWeight::MEDIUM),
                                ),
                        )
                    })
                    .when(model.max_output_tokens.is_some(), |el| {
                        let max_out = model.max_output_tokens.unwrap();
                        el.child(
                            h_flex()
                                .gap_1()
                                .child(Label::new("Max output length:").text_sm())
                                .child(
                                    Label::new(Self::format_number(max_out))
                                        .text_sm()
                                        .font_weight(FontWeight::MEDIUM),
                                ),
                        )
                    }),
            )
            // Capabilities
            .when(model.supports_vision || model.supports_tools, |el| {
                el.child(
                    h_flex()
                        .gap_2()
                        .when(model.supports_vision, |el| {
                            el.child(
                                h_flex()
                                    .gap_1()
                                    .items_center()
                                    .child(AppIcon::Eye.with_size(px(20.)))
                                    .child(
                                        Label::new("Vision")
                                            .text_xs()
                                            .text_color(theme.muted_foreground),
                                    ),
                            )
                        })
                        .when(model.supports_tools, |el| {
                            el.child(
                                h_flex()
                                    .gap_1()
                                    .items_center()
                                    .child(AppIcon::Settings2.with_size(px(20.)))
                                    .child(
                                        Label::new("Tools")
                                            .text_xs()
                                            .text_color(theme.muted_foreground),
                                    ),
                            )
                        }),
                )
            })
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

        // Get hovered model for details panel
        let hovered_model = self.hovered_model.clone();

        h_flex()
            .w_full()
            .flex_shrink_0()
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
                    .when(collapsed, |el| el.pl(px(128.)))
                    .child(
                        Popover::new("model-selector-popover")
                            .appearance(false)
                            .bg(transparent_black())
                            .p_0()
                            .rounded_none()
                            .shadow_none()
                            .border_0()
                            .trigger(
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
                                            .child(AppIcon::ChevronDown.with_size(px(24.))),
                                    ),
                            )
                            .content(move |_state, _window, cx| {
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
                                    .cloned()
                                    .collect();

                                let theme = cx.theme();

                                // Show empty state with settings button if no providers configured
                                if providers.is_empty() {
                                    return v_flex()
                                        .w(px(280.))
                                        .p_4()
                                        .gap_3()
                                        .bg(theme.popover)
                                        .border_1()
                                        .border_color(theme.border)
                                        .rounded_lg()
                                        .shadow_lg()
                                        .items_center()
                                        .child(AppIcon::Settings.with_size(px(32.)))
                                        .child(
                                            Label::new(t!("menu.no_provider_configured").to_string())
                                                .text_sm()
                                                .text_color(theme.muted_foreground),
                                        )
                                        .child(
                                            Button::new("open-settings")
                                                .label(t!("menu.open_settings").to_string())
                                                .small()
                                                .on_click(|_ev, window, cx| {
                                                    window.dispatch_action(Box::new(OpenSettings), cx);
                                                }),
                                        )
                                        .into_any_element();
                                }

                                // Two-panel layout: models list + details (separated)
                                h_flex()
                                    .gap_2()
                                    .items_start()
                                    // Left panel: models list
                                    .child(
                                        div()
                                            .id("model-list")
                                            .w(px(280.))
                                            .max_h(px(400.))
                                            .bg(theme.popover)
                                            .border_1()
                                            .border_color(theme.border)
                                            .rounded_lg()
                                            .shadow_lg()
                                            .overflow_y_scroll()
                                            .py_1()
                                            .children(providers.iter().flat_map(|provider| {
                                                let models = cached_models
                                                    .get(&provider.id)
                                                    .cloned()
                                                    .unwrap_or_else(|| {
                                                        llm::get_models_for_provider(&provider.id)
                                                    });

                                                let provider_id = provider.id.clone();
                                                let provider_name_display = provider.name.clone();
                                                let foreground = theme.foreground;

                                                // Provider header + models
                                                std::iter::once(
                                                    // Provider header
                                                    h_flex()
                                                        .px_3()
                                                        .py_2()
                                                        .gap_2()
                                                        .items_center()
                                                        .child(Self::render_provider_icon(&provider_id, foreground))
                                                        .child(
                                                            Label::new(provider_name_display)
                                                                .text_sm()
                                                                .font_weight(FontWeight::SEMIBOLD)
                                                                .text_color(theme.foreground),
                                                        )
                                                        .into_any_element(),
                                                )
                                                .chain(models.into_iter().map({
                                                    let current_provider_id = current_provider_id.clone();
                                                    let current_model = current_model.clone();
                                                    let provider_id = provider_id.clone();
                                                    let entity = entity.clone();
                                                    move |model| {
                                                        let model_id = model.id.clone();
                                                        let model_name = model.name.clone();
                                                        let is_selected = current_provider_id == provider_id
                                                            && current_model == model_id;
                                                        let supports_vision = model.supports_vision;
                                                        let supports_tools = model.supports_tools;
                                                        let model_for_hover = model.clone();
                                                        let entity_for_hover = entity.clone();
                                                        let entity_for_click = entity.clone();
                                                        let provider_id_for_click = provider_id.clone();
                                                        let model_id_for_click = model_id.clone();

                                                        div()
                                                            .id(SharedString::from(format!(
                                                                "model-item-{}",
                                                                model_id
                                                            )))
                                                            .w_full()
                                                            .px_3()
                                                            .py_1p5()
                                                            .cursor_pointer()
                                                            .rounded_sm()
                                                            .hover(|s| s.bg(theme.list_active))
                                                            .on_mouse_move({
                                                                let model = model_for_hover.clone();
                                                                let entity = entity_for_hover.clone();
                                                                move |_ev, _window, cx| {
                                                                    entity
                                                                        .update(cx, |this: &mut ModelSelector, cx| {
                                                                            if this.hovered_model.as_ref().map(|m| &m.id) != Some(&model.id) {
                                                                                this.hovered_model =
                                                                                    Some(model.clone());
                                                                                cx.notify();
                                                                            }
                                                                        });
                                                                }
                                                            })
                                                            .on_click({
                                                                let provider_id = provider_id_for_click;
                                                                let model_id = model_id_for_click;
                                                                let entity = entity_for_click;
                                                                move |_ev, _window, cx| {
                                                                    // Update settings
                                                                    update_settings(cx, |settings| {
                                                                        settings.active_provider_id =
                                                                            Some(provider_id.clone());
                                                                        if let Some(provider) = settings
                                                                            .providers
                                                                            .iter_mut()
                                                                            .find(|p| p.id == provider_id)
                                                                        {
                                                                            provider.default_model =
                                                                                model_id.clone();
                                                                        }
                                                                    });

                                                                    // Emit event
                                                                    entity
                                                                        .update(cx, |_this: &mut ModelSelector, cx| {
                                                                            cx.emit(
                                                                                ModelSelectorChangedEvent {
                                                                                    provider_id: provider_id
                                                                                        .clone(),
                                                                                    model_id: model_id
                                                                                        .clone(),
                                                                                },
                                                                            );
                                                                            cx.notify();
                                                                        });
                                                                }
                                                            })
                                                            .child(
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
                                                                                Label::new(model_name)
                                                                                    .text_sm(),
                                                                            ),
                                                                    )
                                                                    .child(
                                                                        h_flex()
                                                                            .gap_1()
                                                                            .items_center()
                                                                            .when(is_selected, |el| {
                                                                                el.child(
                                                                                    AppIcon::Check
                                                                                        .with_size(px(24.)),
                                                                                )
                                                                            })
                                                                            .when(supports_vision, |el| {
                                                                                el.child(
                                                                                    AppIcon::Eye
                                                                                        .with_size(px(20.)),
                                                                                )
                                                                            })
                                                                            .when(supports_tools, |el| {
                                                                                el.child(
                                                                                    AppIcon::Settings2
                                                                                        .with_size(px(20.)),
                                                                                )
                                                                            }),
                                                                    ),
                                                            )
                                                            .into_any_element()
                                                    }
                                                }))
                                            })),
                                    )
                                    // Right panel: model details (shown when hovering)
                                    .when(hovered_model.is_some(), |el| {
                                        let model = hovered_model.clone().unwrap();
                                        el.child(Self::render_model_details(&model, cx))
                                    })
                                    .into_any_element()
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
                        .app_icon(AppIcon::PanelRight)
                        .ghost()
                        .small(),
                ),
            )
    }
}
