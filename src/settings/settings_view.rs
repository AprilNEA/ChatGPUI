// SPDX-FileCopyrightText: 2026 AprilNEA LLC
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

use gpui::{
    App, AppContext, Bounds, Context, Entity, FontWeight, IntoElement, ParentElement, Render,
    Styled, Subscription, TitlebarOptions, Window, WindowBounds, WindowKind, WindowOptions,
    actions, prelude::FluentBuilder, px, size,
};
use gpui_component::{
    ActiveTheme, Icon, IconName, Root, Sizable,
    button::{Button, ButtonVariants},
    divider::Divider,
    h_flex,
    input::{Input, InputEvent, InputState},
    label::Label,
    list::ListItem,
    scroll::ScrollableElement,
    v_flex,
};

use super::{AuthMethod, get_settings, update_settings};

/// Settings navigation category
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsCategory {
    General,
    Appearance,
    Provider,
    Prompts,
    Advanced,
}

impl SettingsCategory {
    pub fn label(&self) -> String {
        match self {
            SettingsCategory::General => t!("settings.general"),
            SettingsCategory::Appearance => t!("settings.appearance"),
            SettingsCategory::Provider => t!("settings.provider"),
            SettingsCategory::Prompts => t!("settings.prompts"),
            SettingsCategory::Advanced => t!("settings.advanced"),
        }
        .to_string()
    }

    pub fn icon(&self) -> IconName {
        match self {
            SettingsCategory::General => IconName::Settings,
            SettingsCategory::Appearance => IconName::Palette,
            SettingsCategory::Provider => IconName::Globe,
            SettingsCategory::Prompts => IconName::BookOpen,
            SettingsCategory::Advanced => IconName::Settings2,
        }
    }

    pub fn all() -> Vec<SettingsCategory> {
        vec![
            SettingsCategory::General,
            SettingsCategory::Appearance,
            SettingsCategory::Provider,
            SettingsCategory::Prompts,
            SettingsCategory::Advanced,
        ]
    }
}

/// Settings window view
pub struct SettingsView {
    current_category: SettingsCategory,
    selected_provider_id: Option<String>,
    // Provider form states
    api_key_input: Entity<InputState>,
    base_url_input: Entity<InputState>,
    show_api_key: bool,
    // Subscriptions for auto-save
    _api_key_subscription: Subscription,
    _base_url_subscription: Subscription,
}

impl SettingsView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let api_key_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(t!("settings.api_key_placeholder").to_string())
        });
        let base_url_input = cx.new(|cx| InputState::new(window, cx).placeholder("API Base URL"));

        // Load initial provider
        let selected_provider_id = {
            let settings = get_settings(cx);
            settings.active_provider_id.clone()
        };

        // Subscribe to input changes for auto-save
        let _api_key_subscription = cx.subscribe_in(
            &api_key_input,
            window,
            |this, _, event: &InputEvent, _window, cx| {
                if matches!(event, InputEvent::Change | InputEvent::PressEnter { .. }) {
                    this.save_api_key(cx);
                }
            },
        );

        let _base_url_subscription = cx.subscribe_in(
            &base_url_input,
            window,
            |this, _, event: &InputEvent, _window, cx| {
                if matches!(event, InputEvent::Change | InputEvent::PressEnter { .. }) {
                    this.save_base_url(cx);
                }
            },
        );

        let mut view = Self {
            current_category: SettingsCategory::Provider,
            selected_provider_id,
            api_key_input,
            base_url_input,
            show_api_key: false,
            _api_key_subscription,
            _base_url_subscription,
        };

        // Load provider data into inputs
        view.load_provider_data(window, cx);
        view
    }

    fn load_provider_data(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(provider_id) = &self.selected_provider_id {
            let settings = get_settings(cx);
            if let Some(provider) = settings.providers.iter().find(|p| &p.id == provider_id) {
                let api_key = provider.api_key.clone();
                let base_url = provider.base_url.clone();

                self.api_key_input.update(cx, |state, cx| {
                    state.set_value(&api_key, window, cx);
                });
                self.base_url_input.update(cx, |state, cx| {
                    state.set_value(&base_url, window, cx);
                });
            }
        }
    }

    fn save_api_key(&mut self, cx: &mut Context<Self>) {
        if let Some(provider_id) = &self.selected_provider_id {
            let id = provider_id.clone();
            let value = self.api_key_input.read(cx).value().to_string();
            update_settings(cx, |settings| {
                if let Some(provider) = settings.provider_mut(&id) {
                    provider.api_key = value;
                }
            });
        }
    }

    fn save_base_url(&mut self, cx: &mut Context<Self>) {
        if let Some(provider_id) = &self.selected_provider_id {
            let id = provider_id.clone();
            let value = self.base_url_input.read(cx).value().to_string();
            update_settings(cx, |settings| {
                if let Some(provider) = settings.provider_mut(&id) {
                    provider.base_url = value;
                }
            });
        }
    }

    fn select_category(&mut self, category: SettingsCategory, cx: &mut Context<Self>) {
        self.current_category = category;
        cx.notify();
    }

    fn select_provider(
        &mut self,
        provider_id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Save current values before switching
        self.save_api_key(cx);
        self.save_base_url(cx);

        self.selected_provider_id = Some(provider_id);
        self.load_provider_data(window, cx);
        cx.notify();
    }

    fn toggle_api_key_visibility(&mut self, cx: &mut Context<Self>) {
        self.show_api_key = !self.show_api_key;
        cx.notify();
    }

    fn render_sidebar(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let current = self.current_category;

        v_flex()
            .w(px(180.))
            .h_full()
            .py_2()
            .bg(theme.sidebar)
            .border_r_1()
            .border_color(theme.border)
            .children(
                SettingsCategory::all()
                    .into_iter()
                    .enumerate()
                    .map(|(idx, category)| {
                        let is_selected = category == current;
                        ListItem::new(("category", idx))
                            .px_3()
                            .py_1()
                            .mx_2()
                            .rounded_md()
                            .selected(is_selected)
                            .on_click(cx.listener(move |this, _, _window, cx| {
                                this.select_category(category, cx);
                            }))
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(Icon::new(category.icon()).size_6())
                                    .child(Label::new(category.label())),
                            )
                    }),
            )
    }

    fn render_provider_list(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let settings = get_settings(cx);
        let selected_id = self.selected_provider_id.clone();
        let providers: Vec<_> = settings
            .providers
            .iter()
            .map(|p| (p.id.clone(), p.name.clone()))
            .collect();

        v_flex()
            .w(px(200.))
            .h_full()
            .border_r_1()
            .border_color(theme.border)
            .child(
                h_flex()
                    .px_4()
                    .py_3()
                    .child(Label::new(t!("settings.provider").to_string()).text_sm()),
            )
            .child(Divider::horizontal())
            .child(
                v_flex().flex_1().overflow_y_scrollbar().py_1().children(
                    providers
                        .into_iter()
                        .enumerate()
                        .map(|(idx, (provider_id, provider_name))| {
                            let is_selected = selected_id.as_ref() == Some(&provider_id);
                            let id_clone = provider_id.clone();

                            ListItem::new(("provider", idx))
                                .px_3()
                                .py_2()
                                .mx_2()
                                .my_px()
                                .rounded_md()
                                .selected(is_selected)
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.select_provider(id_clone.clone(), window, cx);
                                }))
                                .child(
                                    h_flex()
                                        .gap_2()
                                        .items_center()
                                        .child(self.provider_icon(&provider_id))
                                        .child(Label::new(provider_name)),
                                )
                        }),
                ),
            )
            .child(Divider::horizontal())
            .child(
                h_flex()
                    .px_2()
                    .py_2()
                    .gap_1()
                    .child(
                        Button::new("add-provider")
                            .icon(IconName::Plus)
                            .ghost()
                            .xsmall(),
                    )
                    .child(
                        Button::new("remove-provider")
                            .icon(IconName::Minus)
                            .ghost()
                            .xsmall(),
                    ),
            )
    }

    fn provider_icon(&self, provider_id: &str) -> impl IntoElement {
        let icon = match provider_id {
            "anthropic" => "A\\",
            "openai" => "◎",
            "azure_openai" => "△",
            "deepseek" => "◈",
            "google_ai" => "G",
            "groq" => "9",
            "mistral" => "M",
            "ollama" => "🦙",
            "openrouter" => "◁",
            _ => "•",
        };
        h_flex()
            .size_8()
            .items_center()
            .justify_center()
            .text_base()
            .child(icon)
    }

    fn render_provider_detail(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let settings = get_settings(cx);

        let provider = self
            .selected_provider_id
            .as_ref()
            .and_then(|id| settings.providers.iter().find(|p| &p.id == id));

        let Some(provider) = provider else {
            return v_flex()
                .flex_1()
                .h_full()
                .items_center()
                .justify_center()
                .child(Label::new(t!("settings.select_provider").to_string()))
                .into_any_element();
        };

        let provider_name = provider.name.clone();
        let auth_method = provider.auth_method.clone();
        let show_api_key = self.show_api_key;

        v_flex()
            .flex_1()
            .h_full()
            .overflow_y_scrollbar()
            .child(
                // Provider header
                h_flex().px_6().py_4().justify_between().child(
                    h_flex().gap_2().items_center().child(
                        Label::new(provider_name)
                            .text_lg()
                            .font_weight(FontWeight::MEDIUM),
                    ),
                ),
            )
            .child(Divider::horizontal())
            .child(
                // Provider settings form
                v_flex()
                    .p_6()
                    .gap_6()
                    // Auth method
                    .child(
                        v_flex()
                            .gap_2()
                            .child(
                                Label::new(t!("settings.auth_method").to_string())
                                    .text_sm()
                                    .text_color(theme.muted_foreground),
                            )
                            .child(
                                h_flex()
                                    .px_3()
                                    .py_2()
                                    .rounded_md()
                                    .border_1()
                                    .border_color(theme.border)
                                    .bg(theme.background)
                                    .child(Label::new(auth_method.label())),
                            ),
                    )
                    // API Key
                    .when(auth_method != AuthMethod::None, |this| {
                        this.child(
                            v_flex()
                                .gap_2()
                                .child(
                                    Label::new(t!("settings.api_key").to_string())
                                        .text_sm()
                                        .text_color(theme.muted_foreground),
                                )
                                .child(
                                    h_flex()
                                        .gap_2()
                                        .child(
                                            v_flex().flex_1().child(
                                                Input::new(&self.api_key_input)
                                                    .appearance(false)
                                                    .cleanable(true),
                                            ),
                                        )
                                        .child(
                                            Button::new("toggle-visibility")
                                                .icon(if show_api_key {
                                                    IconName::EyeOff
                                                } else {
                                                    IconName::Eye
                                                })
                                                .ghost()
                                                .on_click(cx.listener(|this, _, _window, cx| {
                                                    this.toggle_api_key_visibility(cx);
                                                })),
                                        ),
                                ),
                        )
                    })
                    // Base URL
                    .child(
                        v_flex()
                            .gap_2()
                            .child(
                                Label::new("API Base URL")
                                    .text_sm()
                                    .text_color(theme.muted_foreground),
                            )
                            .child(Input::new(&self.base_url_input).cleanable(true)),
                    ),
            )
            .into_any_element()
    }

    fn render_provider_page(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .flex_1()
            .h_full()
            .child(self.render_provider_list(cx))
            .child(self.render_provider_detail(cx))
    }

    fn render_placeholder_page(&self, title: String, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        v_flex()
            .flex_1()
            .h_full()
            .items_center()
            .justify_center()
            .child(Label::new(title))
            .child(
                Label::new(t!("settings.coming_soon").to_string())
                    .text_sm()
                    .text_color(theme.muted_foreground),
            )
    }

    fn render_content(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        match self.current_category {
            SettingsCategory::Provider => self.render_provider_page(cx).into_any_element(),
            SettingsCategory::General => self
                .render_placeholder_page(t!("settings.general_settings").to_string(), cx)
                .into_any_element(),
            SettingsCategory::Appearance => self
                .render_placeholder_page(t!("settings.appearance_settings").to_string(), cx)
                .into_any_element(),
            SettingsCategory::Prompts => self
                .render_placeholder_page(t!("settings.prompts_settings").to_string(), cx)
                .into_any_element(),
            SettingsCategory::Advanced => self
                .render_placeholder_page(t!("settings.advanced_settings").to_string(), cx)
                .into_any_element(),
        }
    }
}

impl Render for SettingsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        h_flex()
            .size_full()
            .bg(theme.background)
            .child(self.render_sidebar(cx))
            .child(self.render_content(cx))
    }
}

actions!(settings, [OpenSettings]);

/// Open the settings window
pub fn open_settings_window(cx: &mut App) {
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
            None,
            size(px(800.), px(600.)),
            cx,
        ))),
        titlebar: Some(TitlebarOptions {
            title: Some(t!("settings.title").to_string().into()),
            ..Default::default()
        }),
        kind: WindowKind::Normal,
        ..Default::default()
    };

    cx.open_window(options, |window, cx| {
        let view = cx.new(|cx| SettingsView::new(window, cx));
        cx.new(|cx| Root::new(view, window, cx))
    })
    .expect("Failed to open settings window");
}
