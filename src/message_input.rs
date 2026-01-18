// SPDX-FileCopyrightText: 2026 AprilNEA LLC
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

use gpui::*;
use gpui_component::{
    ActiveTheme, Disableable, IconName, Sizable,
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputEvent, InputState},
    v_flex,
};

pub struct MessageInput {
    input_state: Entity<InputState>,
    is_loading: bool,
}

pub struct SubmitEvent(pub String);

impl EventEmitter<SubmitEvent> for MessageInput {}

impl MessageInput {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input_state = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Type your message...")
                .clean_on_escape()
        });

        cx.subscribe_in(
            &input_state,
            window,
            |this, _, event: &InputEvent, window, cx| {
                if let InputEvent::PressEnter { secondary: _ } = event {
                    this.handle_submit(window, cx);
                }
            },
        )
        .detach();

        Self {
            input_state,
            is_loading: false,
        }
    }

    pub fn set_loading(&mut self, loading: bool, cx: &mut Context<Self>) {
        self.is_loading = loading;
        cx.notify();
    }

    pub fn clear(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.input_state.update(cx, |state, cx| {
            state.set_value("", window, cx);
        });
    }

    fn handle_submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_loading {
            return;
        }

        let value = self.input_state.read(cx).value().to_string();
        if value.trim().is_empty() {
            return;
        }

        cx.emit(SubmitEvent(value));
        self.clear(window, cx);
    }
}

impl Render for MessageInput {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let is_loading = self.is_loading;

        h_flex()
            .p_4()
            .gap_3()
            .border_t_1()
            .border_color(theme.border)
            .bg(theme.background)
            .child(
                v_flex()
                    .flex_1()
                    .child(Input::new(&self.input_state).large().disabled(is_loading)),
            )
            .child(
                Button::new("send")
                    .icon(IconName::ArrowRight)
                    .large()
                    .primary()
                    .loading(is_loading)
                    .disabled(is_loading)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.handle_submit(window, cx);
                    })),
            )
    }
}
