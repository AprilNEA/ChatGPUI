// SPDX-FileCopyrightText: 2026 AprilNEA LLC
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::hash_map::DefaultHasher;
use std::hash::Hasher;
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::{
    ActiveTheme, VirtualListScrollHandle, h_flex, label::Label, skeleton::Skeleton, v_flex,
    v_virtual_list,
};
use gpui_markdown::Markdown;
use uuid::Uuid;

use crate::message::{Message, MessageStatus, Role};
use crate::settings::get_settings;

const AUTO_SCROLL_THRESHOLD: Pixels = px(24.);
const DEFAULT_CONTENT_WIDTH: Pixels = px(640.);
const LIST_HORIZONTAL_PADDING: Pixels = px(16.);
const USER_BUBBLE_MAX_WIDTH: Pixels = px(512.);
const USER_BUBBLE_PADDING_X: Pixels = px(16.);
const USER_BUBBLE_PADDING_Y: Pixels = px(12.);
const ASSISTANT_LABEL_HEIGHT: Pixels = px(14.);
const ASSISTANT_LABEL_GAP: Pixels = px(8.);
const STREAMING_DOTS_HEIGHT: Pixels = px(16.);
const STREAMING_DOTS_GAP: Pixels = px(8.);
const ERROR_ROW_HEIGHT: Pixels = px(20.);
const ERROR_ROW_GAP: Pixels = px(8.);
const ESTIMATED_TEXT_LINE_HEIGHT: Pixels = px(18.);
const ESTIMATED_CHAR_WIDTH: f32 = 7.0;
// Code block extra height: header (~28px) + vertical padding (~24px) + borders (~4px)
const CODE_BLOCK_EXTRA_HEIGHT: Pixels = px(56.);
// Code block uses text_sm which is ~14px line height
const CODE_LINE_HEIGHT: Pixels = px(16.);
// Larger epsilon to avoid frequent rebuilds during sidebar animation
const WIDTH_CHANGE_EPSILON: f32 = 20.0;
const SCROLL_CHANGE_EPSILON: f32 = 1.0;
const REMEASURE_INTERVAL_MS: u64 = 250;

fn max_pixels(a: Pixels, b: Pixels) -> Pixels {
    if f32::from(a) >= f32::from(b) { a } else { b }
}

fn min_pixels(a: Pixels, b: Pixels) -> Pixels {
    if f32::from(a) <= f32::from(b) { a } else { b }
}

fn pixels_changed(a: Pixels, b: Pixels) -> bool {
    (f32::from(a) - f32::from(b)).abs() > 0.5
}

fn layout_hash(role: Role, status: &MessageStatus, content: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    let role_tag = match role {
        Role::System => 0,
        Role::User => 1,
        Role::Assistant => 2,
    };
    hasher.write_u8(role_tag);
    match status {
        MessageStatus::Pending => hasher.write_u8(0),
        MessageStatus::Streaming => hasher.write_u8(1),
        MessageStatus::Done => hasher.write_u8(2),
        MessageStatus::Error(err) => {
            hasher.write_u8(3);
            hasher.write(err.as_bytes());
        }
    }
    hasher.write(content.as_bytes());
    hasher.finish()
}

struct MeasureEntry {
    height: Pixels,
    hash: u64,
    measured: bool,
    last_measured_at: Option<Instant>,
}

struct LayoutMetrics {
    id: Uuid,
    hash: u64,
    estimated_height: Pixels,
    is_streaming: bool,
    force_remeasure: bool,
}

pub struct MessageList {
    /// History items keyed by message id for stable identity.
    message_index: HashMap<Uuid, Entity<MessageItem>>,
    /// Ordered history items.
    message_items: Vec<Entity<MessageItem>>,
    /// Current streaming item (mutable, updated independently).
    streaming_item: Option<Entity<MessageItem>>,
    /// Flattened list used for virtualization.
    virtual_items: Vec<Entity<MessageItem>>,
    item_sizes: Rc<Vec<Size<Pixels>>>,
    scroll_handle: VirtualListScrollHandle,
    /// Whether to scroll to bottom on next render.
    pending_scroll_to_bottom: bool,
    /// Whether the view should keep following the bottom.
    stick_to_bottom: bool,
    size_cache: HashMap<Uuid, MeasureEntry>,
    content_width: Option<Pixels>,
    last_scroll_offset: Pixels,
    last_max_offset: Pixels,
}

impl MessageList {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            message_index: HashMap::new(),
            message_items: Vec::new(),
            streaming_item: None,
            virtual_items: Vec::new(),
            item_sizes: Rc::new(Vec::new()),
            scroll_handle: VirtualListScrollHandle::new(),
            pending_scroll_to_bottom: false,
            stick_to_bottom: true,
            size_cache: HashMap::new(),
            content_width: None,
            last_scroll_offset: Pixels::ZERO,
            last_max_offset: Pixels::ZERO,
        }
    }

    /// Set historical messages (non-streaming).
    pub fn set_messages(&mut self, messages: Vec<Arc<Message>>, cx: &mut Context<Self>) {
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
                cx.new(|_cx| MessageItem::from_arc(message.clone()))
            };
            next_index.insert(message_id, item.clone());
            next_items.push(item);
        }

        self.message_index = next_index;
        self.message_items = next_items;

        // Store streaming message separately.
        self.streaming_item = streaming.into_iter().next().map(|message| {
            let message = (*message).clone();
            cx.new(|_cx| MessageItem::from_streaming(message))
        });

        self.rebuild_virtual_items();
        self.rebuild_item_sizes(cx);

        // If a new message arrives, mark scroll to bottom.
        if (new_message_added || self.streaming_item.is_some())
            && (self.stick_to_bottom || self.was_near_bottom())
        {
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
            self.refresh_streaming_size(cx);
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
            self.rebuild_virtual_items();
            self.rebuild_item_sizes(cx);
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
            self.rebuild_virtual_items();
            self.rebuild_item_sizes(cx);
            cx.notify();
        }
    }

    /// Start a new streaming message.
    pub fn start_streaming(&mut self, message: Message, cx: &mut Context<Self>) {
        self.streaming_item = Some(cx.new(|_cx| MessageItem::from_streaming(message)));
        if self.stick_to_bottom || self.was_near_bottom() {
            self.pending_scroll_to_bottom = true;
        }
        self.rebuild_virtual_items();
        self.rebuild_item_sizes(cx);
        cx.notify();
    }

    /// Force the list to scroll to the bottom on the next render.
    pub fn force_scroll_to_bottom(&mut self) {
        self.pending_scroll_to_bottom = true;
        self.stick_to_bottom = true;
        self.last_scroll_offset = Pixels::ZERO;
        self.last_max_offset = Pixels::ZERO;
    }

    fn is_near_bottom(&self) -> bool {
        let max_offset = self.scroll_handle.max_offset().height;
        if max_offset <= Pixels::ZERO {
            return true;
        }
        let offset = self.scroll_handle.offset().y;
        (offset + max_offset).abs() <= AUTO_SCROLL_THRESHOLD
    }

    fn was_near_bottom(&self) -> bool {
        let max_offset = self.last_max_offset;
        if max_offset <= Pixels::ZERO {
            return true;
        }
        let offset = self.last_scroll_offset;
        (offset + max_offset).abs() <= AUTO_SCROLL_THRESHOLD
    }

    fn update_content_width(&mut self, cx: &mut Context<Self>) {
        let list_width = self.scroll_handle.bounds().size.width;
        if list_width <= Pixels::ZERO {
            return;
        }
        let content_width = max_pixels(px(1.), list_width - LIST_HORIZONTAL_PADDING * 2);
        let width_changed = self.content_width.is_none_or(|prev| {
            (f32::from(prev) - f32::from(content_width)).abs() > WIDTH_CHANGE_EPSILON
        });
        if width_changed {
            let old_width = self.content_width;
            self.content_width = Some(content_width);
            // Only invalidate measurements for significant width changes (not animation)
            // Small changes during animation can use existing measurements
            let significant_change = old_width
                .is_none_or(|prev| (f32::from(prev) - f32::from(content_width)).abs() > 100.0);
            if significant_change {
                for entry in self.size_cache.values_mut() {
                    entry.measured = false;
                }
            }
            self.rebuild_item_sizes(cx);
            cx.notify();
        }
    }

    fn update_scroll_follow(&mut self, auto_scroll: bool) {
        let offset = self.scroll_handle.offset().y;
        let max_offset = self.scroll_handle.max_offset().height;
        let offset_delta = f32::from(offset) - f32::from(self.last_scroll_offset);
        let max_delta = (f32::from(max_offset) - f32::from(self.last_max_offset)).abs();
        let content_size_changed = max_delta > SCROLL_CHANGE_EPSILON;
        let user_scrolled_up = offset_delta > SCROLL_CHANGE_EPSILON && !content_size_changed;
        let user_scrolled_down = offset_delta < -SCROLL_CHANGE_EPSILON && !content_size_changed;

        if !auto_scroll {
            self.stick_to_bottom = false;
        } else if self.pending_scroll_to_bottom || (content_size_changed && self.was_near_bottom())
        {
            self.stick_to_bottom = true;
        } else if self.stick_to_bottom {
            if user_scrolled_up {
                self.stick_to_bottom = false;
            }
        } else if user_scrolled_down && self.is_near_bottom() {
            self.stick_to_bottom = true;
        }

        self.last_scroll_offset = offset;
        self.last_max_offset = max_offset;
    }

    fn rebuild_virtual_items(&mut self) {
        self.virtual_items.clear();
        self.virtual_items
            .extend(self.message_items.iter().cloned());
        if let Some(item) = &self.streaming_item {
            self.virtual_items.push(item.clone());
        }
    }

    fn rebuild_item_sizes(&mut self, cx: &mut Context<Self>) {
        let content_width = self.content_width.unwrap_or(DEFAULT_CONTENT_WIDTH);
        let mut sizes = Vec::with_capacity(self.virtual_items.len());
        let mut active_ids = HashSet::with_capacity(self.virtual_items.len());

        for item in &self.virtual_items {
            let metrics = {
                let item_ref = item.read(cx);
                item_ref.layout_metrics(content_width)
            };

            let entry = self.size_cache.entry(metrics.id).or_insert(MeasureEntry {
                height: metrics.estimated_height,
                hash: metrics.hash,
                measured: false,
                last_measured_at: None,
            });

            if entry.hash != metrics.hash {
                entry.hash = metrics.hash;
                entry.measured = false;
                entry.last_measured_at = None;
                entry.height = if metrics.is_streaming {
                    max_pixels(entry.height, metrics.estimated_height)
                } else {
                    metrics.estimated_height
                };
            } else if !entry.measured {
                entry.height = if metrics.is_streaming {
                    max_pixels(entry.height, metrics.estimated_height)
                } else {
                    metrics.estimated_height
                };
            }

            sizes.push(size(px(0.), entry.height));
            active_ids.insert(metrics.id);
        }

        self.size_cache.retain(|id, _| active_ids.contains(id));
        self.item_sizes = Rc::new(sizes);
    }

    fn refresh_streaming_size(&mut self, cx: &mut Context<Self>) {
        let Some(item) = self.streaming_item.as_ref() else {
            return;
        };
        let content_width = self.content_width.unwrap_or(DEFAULT_CONTENT_WIDTH);
        let metrics = {
            let item_ref = item.read(cx);
            item_ref.layout_metrics(content_width)
        };

        let entry = self.size_cache.entry(metrics.id).or_insert(MeasureEntry {
            height: metrics.estimated_height,
            hash: metrics.hash,
            measured: false,
            last_measured_at: None,
        });

        if entry.hash != metrics.hash {
            entry.hash = metrics.hash;
            entry.measured = false;
            entry.last_measured_at = None;
            entry.height = max_pixels(entry.height, metrics.estimated_height);
        } else if !entry.measured {
            entry.height = max_pixels(entry.height, metrics.estimated_height);
        }
        let updated_height = entry.height;

        let mut sizes = (*self.item_sizes).clone();
        let expected_len = self.virtual_items.len();
        if sizes.len() == expected_len && !sizes.is_empty() {
            let last_index = sizes.len() - 1;
            sizes[last_index] = size(px(0.), updated_height);
        } else {
            sizes = self
                .virtual_items
                .iter()
                .map(|item| {
                    let id = item.read(cx).id();
                    let height = self
                        .size_cache
                        .get(&id)
                        .map(|entry| entry.height)
                        .unwrap_or(ESTIMATED_TEXT_LINE_HEIGHT);
                    size(px(0.), height)
                })
                .collect();
        }
        self.item_sizes = Rc::new(sizes);
    }

    fn measure_visible_items(
        &mut self,
        visible_range: Range<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.virtual_items.is_empty() {
            return;
        }

        let content_width = self.content_width.unwrap_or(DEFAULT_CONTENT_WIDTH);
        let available_space = size(
            AvailableSpace::Definite(content_width),
            AvailableSpace::MinContent,
        );
        let mut updated = false;

        for ix in visible_range {
            let Some(item) = self.virtual_items.get(ix).cloned() else {
                continue;
            };
            let metrics = {
                let item_ref = item.read(cx);
                item_ref.layout_metrics(content_width)
            };

            let entry = self.size_cache.entry(metrics.id).or_insert(MeasureEntry {
                height: metrics.estimated_height,
                hash: metrics.hash,
                measured: false,
                last_measured_at: None,
            });

            if entry.hash != metrics.hash {
                entry.hash = metrics.hash;
                entry.measured = false;
                entry.last_measured_at = None;
                entry.height = if metrics.is_streaming {
                    max_pixels(entry.height, metrics.estimated_height)
                } else {
                    metrics.estimated_height
                };
            }

            let should_remeasure = if metrics.force_remeasure {
                entry
                    .last_measured_at
                    .is_none_or(|at| at.elapsed() >= Duration::from_millis(REMEASURE_INTERVAL_MS))
            } else {
                false
            };

            if entry.measured && entry.hash == metrics.hash && !should_remeasure {
                continue;
            }

            let mut element = item.into_any_element();
            let measured_size = element.layout_as_root(available_space, window, cx);
            let new_height = if metrics.is_streaming {
                max_pixels(entry.height, measured_size.height)
            } else {
                measured_size.height
            };

            let height_changed = !entry.measured
                || pixels_changed(entry.height, new_height)
                || entry.hash != metrics.hash;

            if height_changed {
                entry.height = new_height;
                entry.hash = metrics.hash;
                entry.last_measured_at = Some(Instant::now());
                updated = true;
            } else {
                entry.last_measured_at = Some(Instant::now());
            }
            entry.measured = true;
        }

        if updated {
            self.rebuild_item_sizes(cx);
            cx.notify();
        }
    }
}

impl Render for MessageList {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.update_content_width(cx);
        let auto_scroll = get_settings(cx).auto_scroll;
        self.update_scroll_follow(auto_scroll);

        if auto_scroll && (self.stick_to_bottom || self.pending_scroll_to_bottom) {
            let max_offset = self.scroll_handle.max_offset().height;
            let current_x = self.scroll_handle.offset().x;
            let target_y = if max_offset > Pixels::ZERO {
                -max_offset
            } else {
                Pixels::ZERO
            };
            self.scroll_handle.set_offset(point(current_x, target_y));
        }
        self.pending_scroll_to_bottom = false;

        let view = cx.entity();
        v_virtual_list(
            view,
            "message-list",
            self.item_sizes.clone(),
            |this, visible_range, window, cx| {
                this.update_content_width(cx);
                this.measure_visible_items(visible_range.clone(), window, cx);
                let mut items = Vec::with_capacity(visible_range.len());
                for ix in visible_range {
                    if let Some(item) = this.virtual_items.get(ix) {
                        items.push(item.clone());
                    }
                }
                items
            },
        )
        .flex_1()
        .w_full()
        .min_h_0()
        .p_4()
        .gap_6()
        .overflow_x_hidden()
        .track_scroll(&self.scroll_handle)
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

    fn content_str(&self) -> &str {
        match &self.source {
            MessageSource::Arc(msg) => msg.content.as_str(),
            MessageSource::Snapshot { content, .. } => content.as_str(),
        }
    }

    fn status(&self) -> &MessageStatus {
        match &self.source {
            MessageSource::Arc(msg) => &msg.status,
            MessageSource::Snapshot { status, .. } => status,
        }
    }

    fn layout_metrics(&self, content_width: Pixels) -> LayoutMetrics {
        let role = self.role();
        let status = self.status();
        let content = self.content_str();
        let hash = layout_hash(role, status, content);
        let force_remeasure =
            role == Role::Assistant && (content.contains("```") || content.contains("~~~"));
        let estimated_height = estimate_item_height(role, status, content, content_width);
        let is_streaming = matches!(status, MessageStatus::Streaming);
        LayoutMetrics {
            id: self.id,
            hash,
            estimated_height,
            is_streaming,
            force_remeasure,
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

fn estimate_item_height(
    role: Role,
    status: &MessageStatus,
    content: &str,
    content_width: Pixels,
) -> Pixels {
    match role {
        Role::User => {
            let bubble_width = min_pixels(content_width, USER_BUBBLE_MAX_WIDTH);
            let text_width = max_pixels(px(1.), bubble_width - USER_BUBBLE_PADDING_X * 2);
            let text_height = estimate_text_height(content, text_width);
            text_height + USER_BUBBLE_PADDING_Y * 2
        }
        Role::System | Role::Assistant => {
            let text_height = if content.is_empty() {
                ESTIMATED_TEXT_LINE_HEIGHT
            } else {
                estimate_text_height(content, content_width)
            };
            let mut height = ASSISTANT_LABEL_HEIGHT + ASSISTANT_LABEL_GAP + text_height;
            if matches!(status, MessageStatus::Streaming) {
                height += STREAMING_DOTS_GAP + STREAMING_DOTS_HEIGHT;
            }
            if matches!(status, MessageStatus::Error(_)) {
                height += ERROR_ROW_GAP + ERROR_ROW_HEIGHT;
            }
            height
        }
    }
}

fn estimate_text_height(content: &str, width: Pixels) -> Pixels {
    let width_f: f32 = width.into();
    let chars_per_line = (width_f / ESTIMATED_CHAR_WIDTH).floor().max(1.0) as usize;

    let mut total_height = px(0.);
    let mut in_code_block = false;
    let mut code_block_lines = 0usize;

    if content.is_empty() {
        return ESTIMATED_TEXT_LINE_HEIGHT;
    }

    for line in content.lines() {
        // Check for code block delimiters
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            if in_code_block {
                // End of code block - add accumulated height
                let code_height = CODE_LINE_HEIGHT * code_block_lines.max(1);
                total_height += CODE_BLOCK_EXTRA_HEIGHT + code_height;
                code_block_lines = 0;
            }
            in_code_block = !in_code_block;
            continue;
        }

        if in_code_block {
            // Code lines don't wrap, count as single lines
            code_block_lines += 1;
        } else {
            // Regular text - estimate wrapping
            let len = line.chars().count().max(1);
            let needed = len.div_ceil(chars_per_line);
            total_height += ESTIMATED_TEXT_LINE_HEIGHT * needed.max(1);
        }
    }

    // Handle unclosed code block (streaming)
    if in_code_block && code_block_lines > 0 {
        let code_height = CODE_LINE_HEIGHT * code_block_lines;
        total_height += CODE_BLOCK_EXTRA_HEIGHT + code_height;
    }

    if total_height <= px(0.) {
        total_height = ESTIMATED_TEXT_LINE_HEIGHT;
    }

    total_height
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
                .flex_shrink_0()
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
