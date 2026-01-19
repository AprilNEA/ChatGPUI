// SPDX-FileCopyrightText: 2026 AprilNEA LLC
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

use std::sync::Arc;
use std::time::Duration;

use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::{ActiveTheme, h_flex, label::Label, skeleton::Skeleton, v_flex};
use gpui_markdown::Markdown;

use crate::message::{Message, MessageStatus, Role};

pub struct MessageList {
    /// 历史消息（使用 Arc 避免克隆）
    messages: Vec<Arc<Message>>,
    /// 当前流式消息（可变，独立更新）
    streaming_message: Option<Message>,
    scroll_handle: ScrollHandle,
    /// 是否需要在下次渲染时滚动到底部
    pending_scroll_to_bottom: bool,
}

impl MessageList {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            messages: Vec::new(),
            streaming_message: None,
            scroll_handle: ScrollHandle::new(),
            pending_scroll_to_bottom: false,
        }
    }

    /// 设置历史消息（非流式消息）
    pub fn set_messages(&mut self, messages: Vec<Message>, cx: &mut Context<Self>) {
        // 分离流式消息和历史消息
        let (streaming, history): (Vec<_>, Vec<_>) = messages
            .into_iter()
            .partition(|m| matches!(m.status, MessageStatus::Streaming));

        let new_message_added = history.len() > self.messages.len();

        // 历史消息使用 Arc 包装
        self.messages = history.into_iter().map(Arc::new).collect();

        // 流式消息单独存储
        self.streaming_message = streaming.into_iter().next();

        // 如果有新消息，标记需要滚动到底部
        if new_message_added || self.streaming_message.is_some() {
            self.pending_scroll_to_bottom = true;
        }

        cx.notify();
    }

    /// 仅更新流式消息内容（避免克隆所有消息）
    pub fn update_streaming_content(&mut self, content: &str, cx: &mut Context<Self>) {
        if let Some(ref mut msg) = self.streaming_message {
            msg.content.push_str(content);
            self.pending_scroll_to_bottom = true;
            cx.notify();
        }
    }

    /// 完成流式消息，将其移入历史消息
    pub fn finish_streaming(&mut self, cx: &mut Context<Self>) {
        if let Some(mut msg) = self.streaming_message.take() {
            msg.status = MessageStatus::Done;
            self.messages.push(Arc::new(msg));
            cx.notify();
        }
    }

    /// 设置流式消息错误
    pub fn set_streaming_error(&mut self, error: &str, cx: &mut Context<Self>) {
        if let Some(mut msg) = self.streaming_message.take() {
            msg.status = MessageStatus::Error(error.to_string());
            self.messages.push(Arc::new(msg));
            cx.notify();
        }
    }

    /// 开始新的流式消息
    pub fn start_streaming(&mut self, cx: &mut Context<Self>) {
        self.streaming_message = Some(Message::assistant_streaming());
        self.pending_scroll_to_bottom = true;
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

        // 历史消息（使用 Arc，不需要克隆整个 Message）
        let history_items = self
            .messages
            .iter()
            .map(|msg| MessageItem::from_arc(msg.clone()));

        // 流式消息（如果有的话）
        let streaming_item = self
            .streaming_message
            .as_ref()
            .map(|msg| MessageItem::from_streaming(msg));

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
                    .children(history_items)
                    .children(streaming_item),
            )
    }
}

/// 消息数据的来源
enum MessageSource {
    /// 使用 Arc 包装的历史消息（无需克隆）
    Arc(Arc<Message>),
    /// 流式消息的快照（仅克隆内容字符串）
    Snapshot {
        role: Role,
        content: SharedString,
        status: MessageStatus,
    },
}

#[derive(IntoElement)]
pub struct MessageItem {
    source: MessageSource,
}

impl MessageItem {
    /// 从 Arc 创建（历史消息，无需克隆整个 Message）
    pub fn from_arc(message: Arc<Message>) -> Self {
        Self {
            source: MessageSource::Arc(message),
        }
    }

    /// 从流式消息引用创建快照（仅克隆内容字符串）
    pub fn from_streaming(message: &Message) -> Self {
        Self {
            source: MessageSource::Snapshot {
                role: message.role,
                content: message.content.clone().into(),
                status: message.status.clone(),
            },
        }
    }

    fn role(&self) -> Role {
        match &self.source {
            MessageSource::Arc(msg) => msg.role,
            MessageSource::Snapshot { role, .. } => *role,
        }
    }

    fn content(&self) -> SharedString {
        match &self.source {
            MessageSource::Arc(msg) => msg.content.clone().into(),
            MessageSource::Snapshot { content, .. } => content.clone(),
        }
    }

    fn status(&self) -> &MessageStatus {
        match &self.source {
            MessageSource::Arc(msg) => &msg.status,
            MessageSource::Snapshot { status, .. } => status,
        }
    }
}

impl RenderOnce for MessageItem {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let role = self.role();
        let content = self.content();
        let status = self.status().clone();
        let is_user = role == Role::User;
        let is_streaming = matches!(status, MessageStatus::Streaming);

        if is_user {
            // User messages: bubble style, right-aligned
            let bubble = v_flex()
                .max_w(rems(32.))
                .px_4()
                .py_3()
                .rounded_lg()
                .bg(theme.accent)
                .text_color(theme.accent_foreground)
                .child(v_flex().text_sm().child(Label::new(content)));

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
                        .when(content.is_empty() && is_streaming, |this| {
                            this.child(Skeleton::new().h_4().w_48())
                        })
                        .when(!content.is_empty(), |this| {
                            this.child(Markdown::new(content.clone()))
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
                .when(matches!(status, MessageStatus::Error(_)), |this| {
                    if let MessageStatus::Error(ref err) = status {
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
                })
        }
        .into_any_element()
    }
}
