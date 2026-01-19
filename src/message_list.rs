// SPDX-FileCopyrightText: 2026 AprilNEA LLC
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::{ActiveTheme, h_flex, label::Label, skeleton::Skeleton, v_flex};
use gpui_markdown::Markdown;
use uuid::Uuid;

use crate::message::{Message, MessageStatus, Role};
use crate::settings::get_settings;

const AUTO_SCROLL_THRESHOLD: Pixels = px(24.);

pub struct MessageList {
    /// History items keyed by message id for stable identity.
    message_index: HashMap<Uuid, Entity<MessageItem>>,
    /// Ordered history items.
    message_items: Vec<Entity<MessageItem>>,
    /// Current streaming item (mutable, updated independently).
    streaming_item: Option<Entity<MessageItem>>,
    scroll_handle: ScrollHandle,
    /// Whether to scroll to bottom on next render.
    pending_scroll_to_bottom: bool,
    /// Whether the view should keep following the bottom.
    stick_to_bottom: bool,
    /// Maximum scroll height observed during streaming to avoid jitter on reflow.
    max_scroll_height: Pixels,
}

impl MessageList {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            message_index: HashMap::new(),
            message_items: Vec::new(),
            streaming_item: None,
            scroll_handle: ScrollHandle::new(),
            pending_scroll_to_bottom: false,
            stick_to_bottom: true,
            max_scroll_height: Pixels::ZERO,
        }
    }

    /// Set historical messages (non-streaming).
    pub fn set_messages(&mut self, messages: Vec<Message>, cx: &mut Context<Self>) {
        // Split streaming and history messages.
        let (streaming, history): (Vec<_>, Vec<_>) = messages
            .into_iter()
            .partition(|m| matches!(m.status, MessageStatus::Streaming));

        let new_message_added = history.len() > self.message_items.len();

        let mut next_index = HashMap::with_capacity(history.len());
        let mut next_items = Vec::with_capacity(history.len());

        for message in history {
            let message_id = message.id;
            let item = if let Some(existing) = self.message_index.get(&message_id) {
                existing.clone()
            } else {
                cx.new(|_cx| MessageItem::from_arc(Arc::new(message)))
            };
            next_index.insert(message_id, item.clone());
            next_items.push(item);
        }

        self.message_index = next_index;
        self.message_items = next_items;

        // Store streaming message separately.
        self.streaming_item = streaming
            .into_iter()
            .next()
            .map(|message| cx.new(|_cx| MessageItem::from_streaming(message)));

        // If a new message arrives, mark scroll to bottom.
        if new_message_added || self.streaming_item.is_some() {
            self.pending_scroll_to_bottom = true;
        }

        cx.notify();
    }

    /// Update streaming content only (avoid cloning all messages).
    pub fn update_streaming_content(&mut self, content: &str, cx: &mut Context<Self>) {
        if let Some(item) = &self.streaming_item {
            let chunk = content.to_string();
            item.update(cx, move |item, cx| {
                item.append_content(&chunk, cx);
            });
            self.pending_scroll_to_bottom = true;
            cx.notify();
        }
    }

    /// Finish streaming message and move it into history.
    pub fn finish_streaming(&mut self, cx: &mut Context<Self>) {
        if let Some(item) = self.streaming_item.take() {
            item.update(cx, |item, cx| {
                item.set_status(MessageStatus::Done, cx);
            });
            let message_id = item.read(cx).id();
            self.message_items.push(item.clone());
            self.message_index.insert(message_id, item);
            cx.notify();
        }
    }

    /// Set error state for streaming message.
    pub fn set_streaming_error(&mut self, error: &str, cx: &mut Context<Self>) {
        if let Some(item) = self.streaming_item.take() {
            let error = error.to_string();
            item.update(cx, move |item, cx| {
                item.set_status(MessageStatus::Error(error), cx);
            });
            let message_id = item.read(cx).id();
            self.message_items.push(item.clone());
            self.message_index.insert(message_id, item);
            cx.notify();
        }
    }

    /// Start a new streaming message.
    pub fn start_streaming(&mut self, message: Message, cx: &mut Context<Self>) {
        self.streaming_item = Some(cx.new(|_cx| MessageItem::from_streaming(message)));
        self.pending_scroll_to_bottom = true;
        cx.notify();
    }

    fn is_near_bottom(&self) -> bool {
        let max_offset = self.scroll_handle.max_offset().height;
        if max_offset <= Pixels::ZERO {
            return true;
        }
        let offset = self.scroll_handle.offset().y;
        (offset + max_offset).abs() <= AUTO_SCROLL_THRESHOLD
    }
}

impl Render for MessageList {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let auto_scroll = get_settings(cx).auto_scroll;
        let near_bottom = self.is_near_bottom();
        if !auto_scroll {
            self.stick_to_bottom = false;
        } else if near_bottom {
            self.stick_to_bottom = true;
        }

        let should_follow = auto_scroll && self.stick_to_bottom;
        let streaming_active = self.streaming_item.is_some();
        let current_max = self.scroll_handle.max_offset().height;
        let (spacer_height, has_spacer) = if should_follow && streaming_active {
            let current_max_f: f32 = current_max.into();
            let max_f: f32 = self.max_scroll_height.into();
            let target_max = max_f.max(current_max_f);
            self.max_scroll_height = Pixels::from(target_max);
            if target_max > current_max_f {
                (Pixels::from(target_max - current_max_f), true)
            } else {
                (Pixels::ZERO, false)
            }
        } else {
            self.max_scroll_height = current_max;
            (Pixels::ZERO, false)
        };

        // Keep following the bottom while enabled to avoid jitter from async layout updates.
        if should_follow {
            self.scroll_handle.scroll_to_bottom();
        }
        self.pending_scroll_to_bottom = false;

        // History items (entities with stable identity).
        let history_items = self
            .message_items
            .iter()
            .cloned();

        // Streaming message (if any).
        let streaming_item = self.streaming_item.clone();
        let view = cx.weak_entity();

        div()
            .id("message-list")
            .flex_1()
            .w_full()
            .min_h_0()
            .overflow_x_hidden()
            .overflow_y_scroll()
            .track_scroll(&self.scroll_handle)
            .on_scroll_wheel(move |event, window, cx| {
                let delta = event.delta.pixel_delta(window.line_height());
                if delta.y != Pixels::ZERO {
                    if let Some(view) = view.upgrade() {
                        view.update(cx, |this, cx| {
                            this.stick_to_bottom = false;
                            this.pending_scroll_to_bottom = false;
                            cx.notify();
                        });
                    }
                }
            })
            .child(
                v_flex()
                    .w_full()
                    .p_4()
                    .gap_6()
                    .children(history_items)
                    .children(streaming_item)
                    .when(has_spacer, |this| this.child(div().h(spacer_height))),
            )
    }
}

/// Source of message data.
enum MessageSource {
    /// Historical message wrapped in Arc (no clone).
    Arc(Arc<Message>),
    /// Snapshot of streaming message (clone content only).
    Snapshot {
        role: Role,
        content: String,
        status: MessageStatus,
    },
}

pub struct MessageItem {
    id: Uuid,
    element_id: SharedString,
    source: MessageSource,
}

impl MessageItem {
    fn new(id: Uuid, source: MessageSource) -> Self {
        Self {
            id,
            element_id: SharedString::from(format!("message-{}", id)),
            source,
        }
    }

    /// Build from Arc (history message, no full clone).
    pub fn from_arc(message: Arc<Message>) -> Self {
        Self::new(message.id, MessageSource::Arc(message))
    }

    /// Build snapshot from streaming message (clone content only).
    pub fn from_streaming(message: Message) -> Self {
        let id = message.id;
        Self::new(
            id,
            MessageSource::Snapshot {
                role: message.role,
                content: message.content,
                status: message.status,
            },
        )
    }

    pub fn id(&self) -> Uuid {
        self.id
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
            MessageSource::Snapshot { content, .. } => content.clone().into(),
        }
    }

    fn status(&self) -> &MessageStatus {
        match &self.source {
            MessageSource::Arc(msg) => &msg.status,
            MessageSource::Snapshot { status, .. } => status,
        }
    }

    fn append_content(&mut self, chunk: &str, cx: &mut Context<Self>) {
        if let MessageSource::Snapshot { content, .. } = &mut self.source {
            content.push_str(chunk);
            cx.notify();
        }
    }

    fn set_status(&mut self, status: MessageStatus, cx: &mut Context<Self>) {
        if let MessageSource::Snapshot {
            status: current_status,
            ..
        } = &mut self.source
        {
            *current_status = status;
            cx.notify();
        }
    }
}

impl Render for MessageItem {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let role = self.role();
        let content = self.content();
        let status = self.status().clone();
        let is_user = role == Role::User;
        let is_streaming = matches!(status, MessageStatus::Streaming);
        let message_id = self.id;

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

            h_flex()
                .w_full()
                .justify_end()
                .child(bubble)
                .id(self.element_id.clone())
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
                            this.child(
                                Markdown::new(content.clone(), message_id)
                                    .allow_syntax_highlighting(!is_streaming),
                            )
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
                .id(self.element_id.clone())
        }
        .into_any_element()
    }
}
