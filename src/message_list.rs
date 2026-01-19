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
    scroll_handle: ScrollHandle,
    /// 是否需要在下次渲染时滚动到底部
    pending_scroll_to_bottom: bool,
}

impl MessageList {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            messages: Vec::new(),
            scroll_handle: ScrollHandle::new(),
            pending_scroll_to_bottom: false,
        }
    }

    pub fn set_messages(&mut self, messages: Vec<Message>, cx: &mut Context<Self>) {
        let new_message_added = messages.len() > self.messages.len();
        self.messages = messages;

        // 如果有新消息，标记需要滚动到底部
        if new_message_added {
            self.pending_scroll_to_bottom = true;
        }

        cx.notify();
    }
}

impl Render for MessageList {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        // 在渲染时执行滚动
        if self.pending_scroll_to_bottom {
            self.scroll_handle.scroll_to_bottom();
            self.pending_scroll_to_bottom = false;
        }

        div()
            .id("message-list")
            .flex_1()
            .w_full()
            .min_h_0()
            .overflow_x_hidden()
            .overflow_y_scroll()
            .track_scroll(&self.scroll_handle)
            .child(
                v_flex()
                    .w_full()
                    .p_4()
                    .gap_6()
                    .children(self.messages.iter().map(|msg| MessageItem::new(msg.clone()))),
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
                .overflow_hidden()
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
                                    .size_4()
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
                                    .size_4()
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
                                    .size_4()
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
                                    .w_full()
                                    .overflow_hidden()
                                    .gap_2()
                                    .items_start()
                                    .child(
                                        div()
                                            .flex_shrink_0()
                                            .size_6()
                                            .rounded_full()
                                            .bg(theme.danger)
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .text_sm()
                                            .text_color(gpui::white())
                                            .child("!"),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .overflow_hidden()
                                            .text_ellipsis()
                                            .child(
                                                Label::new(format!("Error: {}", err))
                                                    .text_sm()
                                                    .text_color(theme.danger),
                                            ),
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
