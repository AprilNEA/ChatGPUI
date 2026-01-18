// SPDX-FileCopyrightText: 2026 AprilNEA LLC
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

use std::time::Duration;

use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::{ActiveTheme, h_flex, label::Label, skeleton::Skeleton, v_flex};
use gpui_markdown::Markdown;

use crate::message::{Message, MessageStatus, Role};

pub struct MessageList {
    messages: Vec<Message>,
}

impl MessageList {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            messages: Vec::new(),
        }
    }

    pub fn set_messages(&mut self, messages: Vec<Message>, cx: &mut Context<Self>) {
        self.messages = messages;
        cx.notify();
    }
}

impl Render for MessageList {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let messages = self.messages.clone();

        v_flex()
            .id("message-list")
            .size_full()
            .overflow_y_scroll()
            .child(
                v_flex()
                    .w_full()
                    .p_4()
                    .gap_6()
                    .children(messages.into_iter().map(|msg| MessageItem::new(msg))),
            )
    }
}

#[derive(IntoElement)]
pub struct MessageItem {
    message: Message,
}

impl MessageItem {
    pub fn new(message: Message) -> Self {
        Self { message }
    }
}

impl RenderOnce for MessageItem {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let is_user = self.message.role == Role::User;
        let is_streaming = matches!(self.message.status, MessageStatus::Streaming);

        if is_user {
            // User messages: bubble style, right-aligned
            let bubble = v_flex()
                .max_w(rems(32.))
                .px_4()
                .py_3()
                .rounded_lg()
                .bg(theme.accent)
                .text_color(theme.accent_foreground)
                .child(
                    v_flex()
                        .text_sm()
                        .child(Label::new(self.message.content.clone())),
                );

            h_flex().w_full().justify_end().child(bubble)
        } else {
            // Assistant messages: full-width, no bubble, direct Markdown rendering
            v_flex()
                .w_full()
                .gap_2()
                .child(
                    // Optional: Add a subtle label for assistant
                    Label::new("Assistant")
                        .text_xs()
                        .text_color(theme.muted_foreground),
                )
                .child(
                    v_flex()
                        .w_full()
                        .when(self.message.content.is_empty() && is_streaming, |this| {
                            this.child(Skeleton::new().h_4().w_48())
                        })
                        .when(!self.message.content.is_empty(), |this| {
                            this.child(Markdown::new(self.message.content.clone()))
                        }),
                )
                .when(is_streaming, |this| {
                    this.child(
                        h_flex()
                            .gap_1()
                            .child(
                                div()
                                    .size_2()
                                    .rounded_full()
                                    .bg(theme.muted_foreground)
                                    .with_animation(
                                        "pulse-1",
                                        Animation::new(Duration::from_secs(1))
                                            .repeat()
                                            .with_easing(pulsating_between(0.4, 1.0)),
                                        |this, delta| this.opacity(delta),
                                    ),
                            )
                            .child(
                                div()
                                    .size_2()
                                    .rounded_full()
                                    .bg(theme.muted_foreground)
                                    .with_animation(
                                        "pulse-2",
                                        Animation::new(Duration::from_secs(1))
                                            .repeat()
                                            .with_easing(pulsating_between(0.4, 1.0)),
                                        |this, delta| this.opacity(delta),
                                    ),
                            )
                            .child(
                                div()
                                    .size_2()
                                    .rounded_full()
                                    .bg(theme.muted_foreground)
                                    .with_animation(
                                        "pulse-3",
                                        Animation::new(Duration::from_secs(1))
                                            .repeat()
                                            .with_easing(pulsating_between(0.4, 1.0)),
                                        |this, delta| this.opacity(delta),
                                    ),
                            ),
                    )
                })
                .when(
                    matches!(self.message.status, MessageStatus::Error(_)),
                    |this| {
                        if let MessageStatus::Error(ref err) = self.message.status {
                            this.child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(
                                        div()
                                            .size_4()
                                            .rounded_full()
                                            .bg(theme.danger)
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .text_xs()
                                            .text_color(gpui::white())
                                            .child("!"),
                                    )
                                    .child(
                                        Label::new(format!("Error: {}", err))
                                            .text_sm()
                                            .text_color(theme.danger),
                                    ),
                            )
                        } else {
                            this
                        }
                    },
                )
        }
        .into_any_element()
    }
}
