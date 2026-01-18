// SPDX-FileCopyrightText: 2026 AprilNEA LLC
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::{
    ActiveTheme, h_flex, label::Label, scroll::ScrollableElement, skeleton::Skeleton, v_flex,
};

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
            .flex_1()
            .overflow_y_scrollbar()
            .child(
                v_flex()
                    .p_4()
                    .gap_4()
                    .children(messages.into_iter().map(|msg| MessageBubble::new(msg))),
            )
    }
}

#[derive(IntoElement)]
pub struct MessageBubble {
    message: Message,
}

impl MessageBubble {
    pub fn new(message: Message) -> Self {
        Self { message }
    }
}

impl RenderOnce for MessageBubble {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let is_user = self.message.role == Role::User;
        let is_streaming = matches!(self.message.status, MessageStatus::Streaming);

        let bubble = v_flex()
            .max_w(rems(32.))
            .px_4()
            .py_3()
            .rounded_lg()
            .when(is_user, |this| {
                this.bg(theme.primary).text_color(theme.primary_foreground)
            })
            .when(!is_user, |this| {
                this.bg(theme.muted).text_color(theme.foreground)
            })
            .child(
                v_flex()
                    .text_sm()
                    .when(self.message.content.is_empty() && is_streaming, |this| {
                        this.child(Skeleton::new().h_4().w_32())
                    })
                    .when(!self.message.content.is_empty(), |this| {
                        this.child(Label::new(self.message.content.clone()))
                    }),
            )
            .when(is_streaming, |this| {
                this.child(
                    Label::new("Generating...")
                        .text_xs()
                        .mt_2()
                        .text_color(theme.muted_foreground),
                )
            })
            .when(
                matches!(self.message.status, MessageStatus::Error(_)),
                |this| {
                    if let MessageStatus::Error(ref err) = self.message.status {
                        this.child(
                            Label::new(format!("Error: {}", err))
                                .text_xs()
                                .mt_2()
                                .text_color(theme.danger),
                        )
                    } else {
                        this
                    }
                },
            );

        h_flex()
            .w_full()
            .when(is_user, |this| this.justify_end())
            .when(!is_user, |this| this.justify_start())
            .child(bubble)
    }
}
