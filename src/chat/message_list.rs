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
    ActiveTheme, Icon, IconName, Sizable,
    button::{Button, ButtonVariants},
    h_flex,
    label::Label,
    scroll::Scrollbar,
    skeleton::Skeleton,
    text::{TextView, TextViewStyle},
    v_flex, v_virtual_list,
};
use rust_i18n::t;
use uuid::Uuid;

use crate::icons::{AppIcon, ButtonAppIconExt, LlmProvider};
use crate::settings::get_settings;

use super::message::{Message, MessageStatus, Role};
use super::scroll_manager::ScrollManager;
use std::str::FromStr;

/// Event emitted when user wants to edit a message
pub struct EditMessageEvent {
    pub message_id: Uuid,
    pub content: String,
}

/// Event emitted when user wants to retry from a message
pub struct RetryMessageEvent {
    pub message_id: Uuid,
}

impl EventEmitter<EditMessageEvent> for MessageList {}
impl EventEmitter<RetryMessageEvent> for MessageList {}

const DEFAULT_CONTENT_WIDTH: Pixels = px(640.);
const LIST_HORIZONTAL_PADDING: Pixels = px(16.);
const USER_BUBBLE_MAX_WIDTH: Pixels = px(512.);
const USER_BUBBLE_PADDING_X: Pixels = px(16.);
const USER_BUBBLE_PADDING_Y: Pixels = px(12.);
const ASSISTANT_LABEL_HEIGHT: Pixels = px(14.);
const ASSISTANT_LABEL_GAP: Pixels = px(8.);
const STREAMING_DOT_HEIGHT: Pixels = px(8.);
const STREAMING_DOT_GAP: Pixels = px(8.);
const ERROR_ROW_HEIGHT: Pixels = px(20.);
const ERROR_ROW_GAP: Pixels = px(8.);
const THINKING_HEADER_HEIGHT: Pixels = px(28.); // Header with padding
const THINKING_CONTENT_LINE_HEIGHT: Pixels = px(14.); // text_xs line height
const THINKING_CONTENT_PADDING: Pixels = px(16.); // px_2 + py_2
const THINKING_GAP: Pixels = px(4.); // gap_1
const ESTIMATED_TEXT_LINE_HEIGHT: Pixels = px(18.);
const ESTIMATED_CHAR_WIDTH: f32 = 7.0;
// Code block extra height: header (~28px) + vertical padding (~24px) + borders (~4px)
const CODE_BLOCK_EXTRA_HEIGHT: Pixels = px(56.);
// Code block uses text_sm which is ~14px line height
const CODE_LINE_HEIGHT: Pixels = px(16.);
// Larger epsilon to avoid frequent rebuilds during sidebar animation
const WIDTH_CHANGE_EPSILON: f32 = 20.0;
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

fn layout_hash(
    role: Role,
    status: &MessageStatus,
    content: &str,
    thinking_content: Option<&str>,
    thinking_collapsed: bool,
) -> u64 {
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
    // Include thinking state in hash for accurate height calculation
    if let Some(thinking) = thinking_content {
        hasher.write_u8(1);
        hasher.write(thinking.as_bytes());
        hasher.write_u8(if thinking_collapsed { 1 } else { 0 });
    } else {
        hasher.write_u8(0);
    }
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
    cacheable: bool,
}

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
struct HeightCacheKey {
    id: Uuid,
    width: i32,
    hash: u64,
}

impl HeightCacheKey {
    fn new(id: Uuid, width: Pixels, hash: u64) -> Self {
        let width_key = f32::from(width).round() as i32;
        Self::from_parts(id, width_key, hash)
    }

    fn from_parts(id: Uuid, width: i32, hash: u64) -> Self {
        Self { id, width, hash }
    }
}

pub struct MessageList {
    /// The conversation ID this MessageList is bound to (immutable after creation).
    /// Follows Zed's pattern: Entity binds to data at creation, cannot be reassigned.
    #[allow(dead_code)]
    conversation_id: Option<Uuid>,
    /// History items keyed by message id for stable identity.
    message_index: HashMap<Uuid, Entity<MessageItem>>,
    /// Ordered history items.
    message_items: Vec<Entity<MessageItem>>,
    /// Current streaming item (mutable, updated independently).
    streaming_item: Option<Entity<MessageItem>>,
    /// Flattened list used for virtualization.
    virtual_items: Vec<Entity<MessageItem>>,
    item_sizes: Rc<Vec<Size<Pixels>>>,
    /// Scroll state manager (handles scroll position, auto-scroll)
    scroll_manager: ScrollManager,
    size_cache: HashMap<Uuid, MeasureEntry>,
    height_cache: HashMap<HeightCacheKey, Pixels>,
    content_width: Option<Pixels>,
}

impl MessageList {
    /// Create a new MessageList not bound to any conversation (for new chats).
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            conversation_id: None,
            message_index: HashMap::new(),
            message_items: Vec::new(),
            streaming_item: None,
            virtual_items: Vec::new(),
            item_sizes: Rc::new(Vec::new()),
            scroll_manager: ScrollManager::new(),
            size_cache: HashMap::new(),
            height_cache: HashMap::new(),
            content_width: None,
        }
    }

    /// Create a new MessageList bound to a specific conversation.
    ///
    /// Following Zed's pattern: the conversation_id is set at creation and cannot
    /// be changed. To switch conversations, create a new MessageList Entity.
    pub fn new_for_conversation(conversation_id: Uuid, _cx: &mut Context<Self>) -> Self {
        Self {
            conversation_id: Some(conversation_id),
            message_index: HashMap::new(),
            message_items: Vec::new(),
            streaming_item: None,
            virtual_items: Vec::new(),
            item_sizes: Rc::new(Vec::new()),
            scroll_manager: ScrollManager::new(),
            size_cache: HashMap::new(),
            height_cache: HashMap::new(),
            content_width: None,
        }
    }

    /// Get the conversation ID this MessageList is bound to.
    #[allow(dead_code)]
    pub fn conversation_id(&self) -> Option<Uuid> {
        self.conversation_id
    }

    /// Check if this MessageList has an active streaming message.
    #[allow(dead_code)]
    pub fn has_streaming(&self) -> bool {
        self.streaming_item.is_some()
    }

    /// Get a mutable reference to the scroll manager
    #[allow(dead_code)]
    pub fn scroll_manager_mut(&mut self) -> &mut ScrollManager {
        &mut self.scroll_manager
    }

    /// Get a reference to the scroll manager
    #[allow(dead_code)]
    pub fn scroll_manager(&self) -> &ScrollManager {
        &self.scroll_manager
    }

    /// Subscribe to MessageItem events and forward them as MessageList events
    fn subscribe_message_item(&self, item: &Entity<MessageItem>, cx: &mut Context<Self>) {
        cx.subscribe(item, |this, _, event: &MessageItemEditEvent, cx| {
            cx.emit(EditMessageEvent {
                message_id: event.message_id,
                content: event.content.clone(),
            });
            cx.notify();
            let _ = this; // silence unused warning
        })
        .detach();

        cx.subscribe(item, |this, _, event: &MessageItemRetryEvent, cx| {
            cx.emit(RetryMessageEvent {
                message_id: event.message_id,
            });
            cx.notify();
            let _ = this; // silence unused warning
        })
        .detach();
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
                let item = cx.new(|_cx| MessageItem::from_arc(message.clone()));
                self.subscribe_message_item(&item, cx);
                item
            };
            next_index.insert(message_id, item.clone());
            next_items.push(item);
        }

        self.message_index = next_index;
        self.message_items = next_items;

        // Store streaming message separately.
        self.streaming_item = streaming.into_iter().next().map(|message| {
            let message = (*message).clone();
            let item = cx.new(|_cx| MessageItem::from_streaming(message));
            self.subscribe_message_item(&item, cx);
            item
        });

        self.rebuild_virtual_items();
        self.rebuild_item_sizes(cx);

        // If a new message arrives, mark scroll to bottom.
        if new_message_added || self.streaming_item.is_some() {
            self.scroll_manager.scroll_to_bottom_if_following();
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

    /// Update thinking content for the streaming message.
    pub fn update_thinking_content(&mut self, content: &str, cx: &mut Context<Self>) {
        if let Some(item) = &self.streaming_item {
            let chunk = content.to_string();
            item.update(cx, move |item, cx| {
                item.append_thinking_content(&chunk, cx);
            });
            self.refresh_streaming_size(cx);
            cx.notify();
        }
    }

    /// Mark thinking as done for the streaming message.
    pub fn finish_thinking(&mut self, duration_ms: Option<u64>, cx: &mut Context<Self>) {
        if let Some(item) = &self.streaming_item {
            item.update(cx, move |item, cx| {
                item.set_thinking_done(duration_ms, cx);
            });
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
        let item = cx.new(|_cx| MessageItem::from_streaming(message));
        self.subscribe_message_item(&item, cx);
        self.streaming_item = Some(item);
        self.scroll_manager.scroll_to_bottom_if_following();
        self.rebuild_virtual_items();
        self.rebuild_item_sizes(cx);
        cx.notify();
    }

    #[allow(dead_code)]
    pub fn reset_scroll_tracking(&mut self) {
        self.scroll_manager.reset();
    }

    fn update_content_width(&mut self, cx: &mut Context<Self>) {
        let list_width = self.scroll_manager.bounds().size.width;
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

            let cache_key = HeightCacheKey::new(metrics.id, content_width, metrics.hash);
            let cached_height = self.height_cache.get(&cache_key).copied();

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

            if let Some(height) = cached_height {
                entry.height = height;
                entry.measured = true;
                entry.last_measured_at = Some(Instant::now());
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

            if !metrics.is_streaming && metrics.cacheable {
                let cache_key = HeightCacheKey::new(metrics.id, content_width, metrics.hash);
                self.height_cache.insert(cache_key, entry.height);
            }
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

        // Update scroll tracking and apply pending scroll operations
        let auto_scroll = get_settings(cx).auto_scroll;
        self.scroll_manager.update_scroll_follow(auto_scroll);
        self.scroll_manager.apply_pending_scroll(auto_scroll);

        let view = cx.entity();
        let scroll_handle = self.scroll_manager.handle().clone();

        div()
            .relative()
            .flex_1()
            .w_full()
            .min_h_0()
            .child(
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
                .size_full()
                .p_4()
                .gap_6()
                .overflow_x_hidden()
                .track_scroll(self.scroll_manager.handle()),
            )
            .child(
                div()
                    .absolute()
                    .top_0()
                    .right_0()
                    .bottom_0()
                    .w(px(16.))
                    .child(Scrollbar::vertical(&scroll_handle)),
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
        thinking_content: String,
        thinking_done: bool,
        thinking_duration_ms: Option<u64>,
    },
}

/// Event emitted by MessageItem when edit button is clicked
pub struct MessageItemEditEvent {
    pub message_id: Uuid,
    pub content: String,
}

/// Event emitted by MessageItem when retry button is clicked
pub struct MessageItemRetryEvent {
    pub message_id: Uuid,
}

pub struct MessageItem {
    id: Uuid,
    element_id: SharedString,
    source: MessageSource,
    /// Cached SharedString for content to avoid repeated conversions.
    cached_content: Option<SharedString>,
    /// Whether the thinking section is collapsed
    thinking_collapsed: bool,
}

impl EventEmitter<MessageItemEditEvent> for MessageItem {}
impl EventEmitter<MessageItemRetryEvent> for MessageItem {}

impl MessageItem {
    fn new(id: Uuid, source: MessageSource) -> Self {
        Self {
            id,
            element_id: SharedString::from(format!("message-{}", id)),
            source,
            cached_content: None,
            thinking_collapsed: true, // Default to collapsed
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
                thinking_content: String::new(),
                thinking_done: false,
                thinking_duration_ms: None,
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

    fn content(&mut self) -> SharedString {
        // Return cached content if available
        if let Some(cached) = &self.cached_content {
            return cached.clone();
        }
        // Build and cache the SharedString
        let shared: SharedString = match &self.source {
            MessageSource::Arc(msg) => msg.content.clone().into(),
            MessageSource::Snapshot { content, .. } => content.clone().into(),
        };
        self.cached_content = Some(shared.clone());
        shared
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
        let thinking_content = self.thinking_content();
        let thinking_collapsed = self.thinking_collapsed;
        let hash = layout_hash(role, status, content, thinking_content, thinking_collapsed);
        let force_remeasure =
            role == Role::Assistant && (content.contains("```") || content.contains("~~~"));
        let estimated_height = estimate_item_height(
            role,
            status,
            content,
            thinking_content,
            thinking_collapsed,
            content_width,
        );
        let is_streaming = matches!(status, MessageStatus::Streaming);
        let cacheable = thinking_collapsed;
        LayoutMetrics {
            id: self.id,
            hash,
            estimated_height,
            is_streaming,
            force_remeasure,
            cacheable,
        }
    }

    fn append_content(&mut self, chunk: &str, _cx: &mut Context<Self>) {
        if let MessageSource::Snapshot { content, .. } = &mut self.source {
            content.push_str(chunk);
            // Invalidate cache since content changed
            self.cached_content = None;
            // Note: Caller (MessageList) handles notify to avoid duplicate notifications
        }
    }

    fn append_thinking_content(&mut self, chunk: &str, _cx: &mut Context<Self>) {
        if let MessageSource::Snapshot {
            thinking_content, ..
        } = &mut self.source
        {
            thinking_content.push_str(chunk);
            // Note: Caller (MessageList) handles notify to avoid duplicate notifications
        }
    }

    fn set_thinking_done(&mut self, duration_ms: Option<u64>, cx: &mut Context<Self>) {
        if let MessageSource::Snapshot {
            thinking_done,
            thinking_duration_ms,
            ..
        } = &mut self.source
        {
            *thinking_done = true;
            *thinking_duration_ms = duration_ms;
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

    fn thinking_content(&self) -> Option<&str> {
        match &self.source {
            MessageSource::Arc(msg) => msg.thinking_content.as_deref(),
            MessageSource::Snapshot {
                thinking_content, ..
            } => {
                if thinking_content.is_empty() {
                    None
                } else {
                    Some(thinking_content.as_str())
                }
            }
        }
    }

    fn thinking_duration_ms(&self) -> Option<u64> {
        match &self.source {
            MessageSource::Arc(msg) => msg.thinking_duration_ms,
            MessageSource::Snapshot {
                thinking_duration_ms,
                ..
            } => *thinking_duration_ms,
        }
    }

    fn is_thinking_done(&self) -> bool {
        match &self.source {
            MessageSource::Arc(_) => true, // Historical messages are always done
            MessageSource::Snapshot { thinking_done, .. } => *thinking_done,
        }
    }

    fn toggle_thinking_collapsed(&mut self, cx: &mut Context<Self>) {
        self.thinking_collapsed = !self.thinking_collapsed;
        cx.notify();
    }

    fn emit_edit(&mut self, cx: &mut Context<Self>) {
        let content = self.content_str().to_string();
        cx.emit(MessageItemEditEvent {
            message_id: self.id,
            content,
        });
    }

    fn emit_retry(&mut self, cx: &mut Context<Self>) {
        cx.emit(MessageItemRetryEvent {
            message_id: self.id,
        });
    }
}

fn estimate_item_height(
    role: Role,
    status: &MessageStatus,
    content: &str,
    thinking_content: Option<&str>,
    thinking_collapsed: bool,
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

            // Add thinking section height if present
            if let Some(thinking) = thinking_content {
                // Always add header height
                height += THINKING_GAP + THINKING_HEADER_HEIGHT;

                // Add content height only if expanded
                if !thinking_collapsed && !thinking.is_empty() {
                    let thinking_lines = thinking.lines().count().max(1);
                    let thinking_text_height = THINKING_CONTENT_LINE_HEIGHT * thinking_lines;
                    height += THINKING_GAP + THINKING_CONTENT_PADDING + thinking_text_height;
                }
            }

            if matches!(status, MessageStatus::Streaming) {
                height += STREAMING_DOT_GAP + STREAMING_DOT_HEIGHT;
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
    let mut line_count = 0usize;

    if content.is_empty() {
        return ESTIMATED_TEXT_LINE_HEIGHT;
    }

    for line in content.lines() {
        line_count += 1;
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
            // Empty lines still take one line height
            if line.is_empty() {
                total_height += ESTIMATED_TEXT_LINE_HEIGHT;
            } else {
                let len = line.chars().count();
                let needed = len.div_ceil(chars_per_line);
                total_height += ESTIMATED_TEXT_LINE_HEIGHT * needed.max(1);
            }
        }
    }

    // Handle trailing newline: lines() doesn't yield empty string for trailing \n
    // If content ends with newline and we processed at least one line, add extra line
    if content.ends_with('\n') && line_count > 0 && !in_code_block {
        total_height += ESTIMATED_TEXT_LINE_HEIGHT;
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
        // Extract theme colors to local variables to avoid borrow conflicts
        let theme = cx.theme();
        let accent = theme.accent;
        let accent_foreground = theme.accent_foreground;
        let muted = theme.muted;
        let muted_foreground = theme.muted_foreground;
        let foreground = theme.foreground;
        let background = theme.background;
        let border = theme.border;
        let danger = theme.danger;

        let role = self.role();
        let content = self.content();
        let status = self.status().clone();
        let is_user = role == Role::User;
        let is_streaming = matches!(status, MessageStatus::Streaming);
        let message_id = self.id;

        // Get thinking info
        let thinking_content_opt = self.thinking_content().map(|s| s.to_string());
        let thinking_duration_ms = self.thinking_duration_ms();
        let is_thinking_done = self.is_thinking_done();
        let thinking_collapsed = self.thinking_collapsed;
        let has_thinking = thinking_content_opt.is_some();

        // Action buttons for messages (shown when not streaming)
        let show_actions = !is_streaming;
        let content_for_copy = content.clone();

        if is_user {
            // User messages: bubble style, right-aligned
            let bubble = v_flex()
                .max_w(rems(32.))
                .px_4()
                .py_3()
                .rounded_lg()
                .bg(accent)
                .text_color(accent_foreground)
                .child(v_flex().text_sm().child(Label::new(content)));

            // User action buttons (right-aligned)
            let action_buttons = h_flex()
                .gap_1()
                .justify_end()
                .child(
                    Button::new("copy")
                        .app_icon(AppIcon::Copy)
                        .ghost()
                        .small()
                        .on_click(move |_, _, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(
                                content_for_copy.to_string(),
                            ));
                        }),
                )
                .child(
                    Button::new("edit")
                        .app_icon(AppIcon::Edit)
                        .ghost()
                        .small()
                        .on_click(cx.listener(|this, _, _window, cx| {
                            this.emit_edit(cx);
                        })),
                )
                .child(
                    Button::new("retry")
                        .app_icon(AppIcon::RotateCcw)
                        .ghost()
                        .small()
                        .on_click(cx.listener(|this, _, _window, cx| {
                            this.emit_retry(cx);
                        })),
                )
                .child(
                    Button::new("more")
                        .app_icon(AppIcon::Ellipsis)
                        .ghost()
                        .small()
                        .on_click(cx.listener(|_this, _, _window, _cx| {
                            // TODO: Implement dropdown menu
                        })),
                );

            v_flex()
                .w_full()
                .items_end()
                .gap_1()
                .child(bubble)
                .when(show_actions, |this| this.child(action_buttons))
                .id(self.element_id.clone())
        } else {
            // Assistant messages: with avatar on left, content on right
            // Get the active provider for the avatar icon
            let provider_icon = get_settings(cx)
                .active_provider_id
                .as_ref()
                .and_then(|id| LlmProvider::from_str(id).ok());

            h_flex()
                .w_full()
                .flex_shrink_0()
                .overflow_hidden()
                .gap_3()
                .items_start()
                // Avatar with provider icon
                .child(
                    div()
                        .flex_shrink_0()
                        .size_8()
                        .rounded_full()
                        .bg(muted)
                        .flex()
                        .items_center()
                        .justify_center()
                        .map(|this| {
                            if let Some(provider) = provider_icon {
                                this.child(Icon::new(provider).size_5().text_color(foreground))
                            } else {
                                this.child(AppIcon::Bot.with_size(px(20.)))
                            }
                        }),
                )
                // Message content column
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .gap_2()
                        // Thinking section (collapsible pill button)
                        .when(has_thinking, |this| {
                            let thinking_header_text = if is_thinking_done {
                                if let Some(duration) = thinking_duration_ms {
                                    let seconds = duration as f64 / 1000.0;
                                    format!("{} {:.0} {}", t!("thinking.duration"), seconds, t!("thinking.seconds"))
                                } else {
                                    t!("thinking.duration").to_string()
                                }
                            } else {
                                t!("thinking.in_progress").to_string()
                            };

                            this.child(
                                v_flex()
                                    .w_full()
                                    .gap_2()
                                    .child(
                                        h_flex()
                                            .gap_2()
                                            .items_center()
                                            // Thinking pill button
                                            .child(
                                                div()
                                                    .id("thinking-header")
                                                    .cursor_pointer()
                                                    .px_3()
                                                    .py_1()
                                                    .rounded_full()
                                                    .border_1()
                                                    .border_color(border)
                                                    .bg(background)
                                                    .hover(|this| this.bg(muted.opacity(0.5)))
                                                    .on_click(cx.listener(|this, _, _window, cx| {
                                                        this.toggle_thinking_collapsed(cx);
                                                    }))
                                                    .child(
                                                        h_flex()
                                                            .gap_1p5()
                                                            .items_center()
                                                            .child(
                                                                Label::new(thinking_header_text)
                                                                    .text_xs()
                                                                    .text_color(muted_foreground),
                                                            )
                                                            .when(!is_thinking_done, |this| {
                                                                // Pulsing dot when thinking
                                                                this.child(
                                                                    div()
                                                                        .size(px(6.))
                                                                        .rounded_full()
                                                                        .bg(accent)
                                                                        .with_animation(
                                                                            "thinking-pulse",
                                                                            Animation::new(Duration::from_millis(800))
                                                                                .repeat()
                                                                                .with_easing(pulsating_between(0.3, 1.0)),
                                                                            |this, delta| this.opacity(delta),
                                                                        ),
                                                                )
                                                            })
                                                            .when(is_thinking_done, |this| {
                                                                // Chevron icon
                                                                this.child(
                                                                    if thinking_collapsed {
                                                                        AppIcon::ChevronDown
                                                                    } else {
                                                                        AppIcon::ChevronUp
                                                                    }
                                                                    .with_size(px(12.)),
                                                                )
                                                            }),
                                                    ),
                                            ),
                                    )
                                    // Thinking content (shown when expanded)
                                    .when(!thinking_collapsed, |this| {
                                        if let Some(thinking_text) = &thinking_content_opt {
                                            this.child(
                                                div()
                                                    .w_full()
                                                    .px_3()
                                                    .py_2()
                                                    .rounded_lg()
                                                    .bg(muted.opacity(0.3))
                                                    .border_l_2()
                                                    .border_color(muted_foreground.opacity(0.3))
                                                    .child(
                                                        Label::new(thinking_text.clone())
                                                            .text_xs()
                                                            .text_color(muted_foreground),
                                                    ),
                                            )
                                        } else {
                                            this
                                        }
                                    }),
                            )
                        })
                        // Main content (markdown or skeleton)
                        .when(content.is_empty() && is_streaming, |this| {
                            this.child(Skeleton::new().h_4().w_48())
                        })
                        .when(!content.is_empty(), |this| {
                            this.child(
                                TextView::markdown(
                                    ElementId::from(message_id),
                                    content.clone(),
                                )
                                .style(TextViewStyle {
                                    heading_base_font_size: px(18.),
                                    ..TextViewStyle::default()
                                })
                                .code_block_actions(|code_block, _window, cx| {
                                    let code = code_block.code();
                                    let lang = code_block.lang();
                                    let has_lang = lang.is_some();
                                    h_flex()
                                        .w_full()
                                        .justify_between()
                                        .when_some(lang, |this, l| {
                                            this.child(
                                                Label::new(l.to_string())
                                                    .text_size(px(11.))
                                                    .text_color(cx.theme().muted_foreground)
                                            )
                                        })
                                        .when(!has_lang, |this| this.child(div()))
                                        .child(
                                            Button::new("copy")
                                                .icon(Icon::new(IconName::Copy))
                                                .ghost()
                                                .compact()
                                                .on_click(move |_, _, cx| {
                                                    cx.write_to_clipboard(ClipboardItem::new_string(code.to_string()));
                                                })
                                        )
                                })
                                .selectable(true)
                            )
                        })
                        // Streaming indicator: breathing dot
                        .when(is_streaming, |this| {
                            this.child(
                                div()
                                    .size(px(8.))
                                    .rounded_full()
                                    .bg(accent)
                                    .with_animation(
                                        "breathing",
                                        Animation::new(Duration::from_millis(1500))
                                            .repeat()
                                            .with_easing(pulsating_between(0.3, 1.0)),
                                        |this, delta| this.opacity(delta),
                                    ),
                            )
                        })
                        // Error message
                        .when(matches!(status, MessageStatus::Error(_)), |this| {
                            if let MessageStatus::Error(ref err) = status {
                                this.child(
                                    h_flex()
                                        .w_full()
                                        .overflow_hidden()
                                        .gap_2()
                                        .items_center()
                                        .child(
                                            div()
                                                .flex_shrink_0()
                                                .size_5()
                                                .rounded_full()
                                                .bg(danger)
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
                                                .text_color(danger),
                                        ),
                                )
                            } else {
                                this
                            }
                        })
                        // Assistant action buttons (left-aligned)
                        .when(show_actions, |this| {
                            let content_to_copy = content.clone();
                            this.child(
                                h_flex()
                                    .gap_1()
                                    .child(
                                        Button::new("copy")
                                            .app_icon(AppIcon::Copy)
                                            .ghost()
                                            .small()
                                            .on_click(move |_, _, cx| {
                                                cx.write_to_clipboard(ClipboardItem::new_string(
                                                    content_to_copy.to_string(),
                                                ));
                                            }),
                                    )
                                    .child(
                                        Button::new("retry")
                                            .app_icon(AppIcon::RotateCcw)
                                            .ghost()
                                            .small()
                                            .on_click(cx.listener(|this, _, _window, cx| {
                                                this.emit_retry(cx);
                                            })),
                                    )
                                    .child(
                                        Button::new("edit")
                                            .app_icon(AppIcon::Edit)
                                            .ghost()
                                            .small()
                                            .on_click(cx.listener(|this, _, _window, cx| {
                                                this.emit_edit(cx);
                                            })),
                                    )
                                    .child(
                                        Button::new("more")
                                            .app_icon(AppIcon::Ellipsis)
                                            .ghost()
                                            .small()
                                            .on_click(cx.listener(|_this, _, _window, _cx| {
                                                // TODO: Implement dropdown menu
                                            })),
                                    ),
                            )
                        }),
                )
                .id(self.element_id.clone())
        }
        .into_any_element()
    }
}
