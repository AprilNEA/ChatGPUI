// SPDX-FileCopyrightText: 2026 AprilNEA LLC
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{prelude::FluentBuilder, *};
use gpui_component::{ActiveTheme, v_flex};
use gpui_tokio_bridge::Tokio;
use uuid::Uuid;

use crate::{
    conversation_cache::ConversationCache,
    database::{self, attachment as db_attachment, message as db_message},
    llm::{self, LlmProvider, StreamEvent},
    message::{Attachment, AttachmentType, ChatMessage, Message, MessageStatus, Role},
    message_input::{MessageInput, SubmitEvent},
    message_list::MessageList,
    settings::get_settings,
    storage,
};

/// Debounce interval for streaming updates (ms).
const STREAM_DEBOUNCE_MS: u64 = 50;

/// Event emitted when a conversation is created or updated
#[allow(dead_code)]
pub struct ConversationUpdatedEvent {
    pub conversation_id: Uuid,
}

/// Event emitted when a background stream finishes.
pub struct BackgroundStreamFinishedEvent {
    pub conversation_id: Uuid,
    pub error: Option<String>,
}

struct ActiveStream {
    message_id: Uuid,
    conversation_id: Option<Uuid>,
    content: String,
    thinking_content: String,
    thinking_start_time: Option<Instant>,
    thinking_duration_ms: Option<u64>,
}

struct PendingStreamResult {
    message_id: Uuid,
    content: String,
    thinking_content: Option<String>,
    thinking_duration_ms: Option<u64>,
    error: Option<String>,
    ui_applied: bool,
}

pub struct ChatView {
    /// Messages for the current conversation (kept for API calls)
    messages: Vec<Arc<Message>>,
    /// LRU cache of MessageList entities by conversation ID.
    /// Following Zed's pattern: each conversation has its own Entity.
    conversation_cache: ConversationCache,
    /// Currently active MessageList Entity (switched on conversation change).
    current_message_list: Option<Entity<MessageList>>,
    message_input: Entity<MessageInput>,
    llm_provider: Option<Arc<dyn LlmProvider>>,
    current_model_id: String,
    is_generating: bool,
    current_conversation_id: Option<Uuid>,
    _subscription: Subscription,
    /// Debounce task to avoid frequent rerenders during streaming.
    debounce_task: Option<Task<()>>,
    /// Buffered streaming content accumulated during the debounce window.
    pending_stream_chunk: String,
    active_stream: Option<ActiveStream>,
    pending_stream_result: Option<PendingStreamResult>,
}

impl EventEmitter<ConversationUpdatedEvent> for ChatView {}
impl EventEmitter<BackgroundStreamFinishedEvent> for ChatView {}

impl ChatView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        // Create initial message list for new chat (no conversation ID)
        let message_list = cx.new(MessageList::new);
        let message_input = cx.new(|cx| MessageInput::new(window, cx));

        // Try to create LLM provider from settings
        let (llm_provider, current_model_id) = match llm::create_provider_from_settings(cx) {
            Ok(provider) => {
                let model_id = provider.default_model().id.clone();
                (Some(provider), model_id)
            }
            Err(e) => {
                tracing::warn!(
                    "No LLM provider configured: {}. Please set up in Settings.",
                    e
                );
                (None, String::new())
            }
        };

        let _subscription = cx.subscribe_in(
            &message_input,
            window,
            |this, _, event: &SubmitEvent, window, cx| {
                this.handle_user_message(
                    event.content.clone(),
                    event.attachments.clone(),
                    window,
                    cx,
                );
            },
        );

        let messages = vec![Arc::new(Message::system("You are a helpful assistant."))];

        Self {
            messages,
            conversation_cache: ConversationCache::new(),
            current_message_list: Some(message_list),
            message_input,
            llm_provider,
            current_model_id,
            is_generating: false,
            current_conversation_id: None,
            _subscription,
            debounce_task: None,
            pending_stream_chunk: String::new(),
            active_stream: None,
            pending_stream_result: None,
        }
    }

    /// Start a new chat (clear messages and conversation)
    pub fn new_chat(&mut self, cx: &mut Context<Self>) {
        // Cache current MessageList if it belongs to a conversation
        self.cache_current_message_list();

        self.messages.clear();
        self.messages
            .push(Arc::new(Message::system("You are a helpful assistant.")));
        self.current_conversation_id = None;
        self.clear_streaming_ui_buffer();

        // Create a new MessageList for the new chat (no conversation ID)
        self.current_message_list = Some(cx.new(MessageList::new));
        self.update_message_list(cx);
    }

    /// Load an existing conversation
    pub fn load_conversation(&mut self, conversation_id: Uuid, cx: &mut Context<Self>) {
        // Don't reload if already on this conversation
        if self.current_conversation_id == Some(conversation_id) {
            return;
        }

        // Cache current MessageList if it belongs to a conversation
        self.cache_current_message_list();

        self.current_conversation_id = Some(conversation_id);
        self.messages.clear();
        self.messages
            .push(Arc::new(Message::system("You are a helpful assistant.")));
        self.clear_streaming_ui_buffer();

        // Try to get MessageList from cache (zero-copy switch)
        if let Some(cached_list) = self.conversation_cache.take(conversation_id) {
            tracing::debug!("Cache hit for conversation {}", conversation_id);
            self.current_message_list = Some(cached_list);
            // Messages still need to be loaded for API context
            self.load_messages_for_context(conversation_id, cx);
            cx.notify();
            return;
        }

        // Cache miss: create new MessageList and load from database
        tracing::debug!("Cache miss for conversation {}", conversation_id);
        self.current_message_list =
            Some(cx.new(|cx| MessageList::new_for_conversation(conversation_id, cx)));

        let db = database::get_db(cx).clone();
        let (tx, rx) = async_channel::unbounded();

        Tokio::spawn(cx, async move {
            let messages_result = db.list_messages(conversation_id).await;

            if let Ok(db_messages) = messages_result {
                let mut messages_with_attachments = Vec::new();

                for msg in db_messages {
                    // Load attachments for this message
                    let attachments = if let Ok(db_attachments) = db.list_attachments(msg.id).await
                    {
                        let mut loaded_attachments = Vec::new();
                        for db_att in db_attachments {
                            // Load file data from storage
                            if let Ok(data) = storage::load_attachment(&db_att.file_path).await {
                                let att_type = match db_att.attachment_type {
                                    db_attachment::AttachmentType::Image => AttachmentType::Image,
                                };
                                loaded_attachments.push(Attachment {
                                    id: db_att.id,
                                    attachment_type: att_type,
                                    name: db_att.name,
                                    mime_type: db_att.mime_type,
                                    data,
                                });
                            }
                        }
                        loaded_attachments
                    } else {
                        Vec::new()
                    };

                    messages_with_attachments.push((msg, attachments));
                }

                let _ = tx.send(Ok(messages_with_attachments)).await;
            } else {
                let _ = tx.send(Err(messages_result.unwrap_err())).await;
            }
        })
        .detach();

        cx.spawn(async move |this, cx| {
            if let Ok(Ok(messages_with_attachments)) = rx.recv().await {
                let _ = cx.update(|app| {
                    let _ = this.update(app, |this, cx| {
                        for (msg, attachments) in messages_with_attachments {
                            let role = match msg.role {
                                db_message::MessageRole::System => Role::System,
                                db_message::MessageRole::User => Role::User,
                                db_message::MessageRole::Assistant => Role::Assistant,
                            };
                            let status = match msg.status {
                                db_message::MessageStatus::Pending => MessageStatus::Pending,
                                db_message::MessageStatus::Streaming => MessageStatus::Streaming,
                                db_message::MessageStatus::Done => MessageStatus::Done,
                                db_message::MessageStatus::Error => {
                                    MessageStatus::Error(msg.error_message.unwrap_or_default())
                                }
                            };
                            this.messages.push(Arc::new(Message {
                                id: msg.id,
                                role,
                                content: msg.content,
                                attachments,
                                status,
                                created_at: msg.created_at,
                                thinking_content: msg.thinking_content,
                                thinking_duration_ms: None,
                            }));
                        }
                        // Update the message list with loaded messages
                        this.update_message_list(cx);
                    });
                });
            }
        })
        .detach();
    }

    /// Cache the current MessageList into the LRU cache.
    fn cache_current_message_list(&mut self) {
        if let (Some(conv_id), Some(list)) = (
            self.current_conversation_id,
            self.current_message_list.take(),
        ) {
            self.conversation_cache.insert(conv_id, list);
        }
    }

    /// Load messages for API context only (when switching to a cached MessageList).
    fn load_messages_for_context(&mut self, conversation_id: Uuid, cx: &mut Context<Self>) {
        let db = database::get_db(cx).clone();
        let (tx, rx) = async_channel::unbounded();

        Tokio::spawn(cx, async move {
            let messages_result = db.list_messages(conversation_id).await;
            if let Ok(db_messages) = messages_result {
                let mut messages_with_attachments = Vec::new();
                for msg in db_messages {
                    let attachments = if let Ok(db_attachments) = db.list_attachments(msg.id).await
                    {
                        let mut loaded_attachments = Vec::new();
                        for db_att in db_attachments {
                            if let Ok(data) = storage::load_attachment(&db_att.file_path).await {
                                let att_type = match db_att.attachment_type {
                                    db_attachment::AttachmentType::Image => AttachmentType::Image,
                                };
                                loaded_attachments.push(Attachment {
                                    id: db_att.id,
                                    attachment_type: att_type,
                                    name: db_att.name,
                                    mime_type: db_att.mime_type,
                                    data,
                                });
                            }
                        }
                        loaded_attachments
                    } else {
                        Vec::new()
                    };
                    messages_with_attachments.push((msg, attachments));
                }
                let _ = tx.send(Ok(messages_with_attachments)).await;
            } else {
                let _ = tx.send(Err(messages_result.unwrap_err())).await;
            }
        })
        .detach();

        cx.spawn(async move |this, cx| {
            if let Ok(Ok(messages_with_attachments)) = rx.recv().await {
                let _ = cx.update(|app| {
                    let _ = this.update(app, |this, _cx| {
                        for (msg, attachments) in messages_with_attachments {
                            let role = match msg.role {
                                db_message::MessageRole::System => Role::System,
                                db_message::MessageRole::User => Role::User,
                                db_message::MessageRole::Assistant => Role::Assistant,
                            };
                            let status = match msg.status {
                                db_message::MessageStatus::Pending => MessageStatus::Pending,
                                db_message::MessageStatus::Streaming => MessageStatus::Streaming,
                                db_message::MessageStatus::Done => MessageStatus::Done,
                                db_message::MessageStatus::Error => {
                                    MessageStatus::Error(msg.error_message.unwrap_or_default())
                                }
                            };
                            this.messages.push(Arc::new(Message {
                                id: msg.id,
                                role,
                                content: msg.content,
                                attachments,
                                status,
                                created_at: msg.created_at,
                                thinking_content: msg.thinking_content,
                                thinking_duration_ms: None,
                            }));
                        }
                        // No UI update needed - MessageList is already populated from cache
                    });
                });
            }
        })
        .detach();
    }

    /// Remove a conversation from the cache (called when conversation is deleted).
    pub fn remove_from_cache(&mut self, conversation_id: Uuid) {
        self.conversation_cache.remove(conversation_id);
    }

    fn handle_user_message(
        &mut self,
        content: String,
        attachments: Vec<Attachment>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if content.trim().is_empty() && attachments.is_empty() || self.is_generating {
            return;
        }

        let user_message = if attachments.is_empty() {
            Message::user(&content)
        } else {
            Message::user_with_attachments(&content, attachments.clone())
        };
        self.messages.push(Arc::new(user_message.clone()));
        self.update_message_list(cx);

        // Create conversation if this is the first message
        if self.current_conversation_id.is_none() {
            self.create_conversation_and_save_message(
                content.clone(),
                user_message.id,
                attachments,
                cx,
            );
        } else {
            self.save_message_to_db(user_message.id, Role::User, content.clone(), cx);
            self.save_attachments_to_storage(user_message.id, attachments, cx);
        }

        self.generate_response(window, cx);
    }

    fn save_attachments_to_storage(
        &self,
        message_id: Uuid,
        attachments: Vec<Attachment>,
        cx: &mut Context<Self>,
    ) {
        if attachments.is_empty() {
            return;
        }

        let db = database::get_db(cx).clone();

        Tokio::spawn(cx, async move {
            for att in attachments {
                // Save file to storage
                let extension = storage::extension_from_mime(&att.mime_type);
                let file_size = att.data.len() as i64;

                match storage::save_attachment(att.id, &att.data, extension).await {
                    Ok(file_path) => {
                        // Save record to database
                        if let Err(e) = db
                            .create_attachment(
                                att.id,
                                message_id,
                                db_attachment::AttachmentType::Image,
                                att.name,
                                att.mime_type,
                                file_path,
                                file_size,
                            )
                            .await
                        {
                            tracing::error!("Failed to save attachment record: {}", e);
                        }
                    }
                    Err(e) => {
                        tracing::error!("Failed to save attachment file: {}", e);
                    }
                }
            }
        })
        .detach();
    }

    fn create_conversation_and_save_message(
        &mut self,
        first_message: String,
        message_id: Uuid,
        attachments: Vec<Attachment>,
        cx: &mut Context<Self>,
    ) {
        let db = database::get_db(cx).clone();
        let settings = get_settings(cx);

        // Get provider info from settings
        let (provider_id, model) = settings
            .active_provider()
            .map(|p| (p.id.clone(), p.default_model.clone()))
            .unwrap_or_else(|| ("unknown".to_string(), "unknown".to_string()));

        // Use first few chars of message as title
        let title = first_message.chars().take(50).collect::<String>();
        let title = if first_message.len() > 50 {
            format!("{}...", title)
        } else {
            title
        };

        let (tx, rx) = async_channel::unbounded();
        let first_message_clone = first_message.clone();

        Tokio::spawn(cx, async move {
            // Create conversation
            let conv_result = db
                .create_conversation(title, provider_id, model, None)
                .await;

            if let Ok(conv) = conv_result {
                // Save the first message
                let _ = db
                    .create_message(
                        message_id,
                        conv.id,
                        db_message::MessageRole::User,
                        first_message_clone,
                        db_message::MessageStatus::Done,
                    )
                    .await;

                // Save attachments
                for att in attachments {
                    let extension = storage::extension_from_mime(&att.mime_type);
                    let file_size = att.data.len() as i64;

                    if let Ok(file_path) =
                        storage::save_attachment(att.id, &att.data, extension).await
                    {
                        let _ = db
                            .create_attachment(
                                att.id,
                                message_id,
                                db_attachment::AttachmentType::Image,
                                att.name,
                                att.mime_type,
                                file_path,
                                file_size,
                            )
                            .await;
                    }
                }

                let _ = tx.send(conv.id).await;
            }
        })
        .detach();

        cx.spawn(async move |this, cx| {
            if let Ok(conversation_id) = rx.recv().await {
                let _ = cx.update(|app| {
                    let _ = this.update(app, |this, cx| {
                        if this.current_conversation_id.is_none() {
                            this.current_conversation_id = Some(conversation_id);
                        }
                        if let Some(stream) = this.active_stream.as_mut()
                            && stream.conversation_id.is_none()
                        {
                            stream.conversation_id = Some(conversation_id);
                        }
                        this.finalize_pending_stream(conversation_id, cx);
                        cx.emit(ConversationUpdatedEvent { conversation_id });
                    });
                });
            }
        })
        .detach();
    }

    fn save_message_to_db(
        &self,
        message_id: Uuid,
        role: Role,
        content: String,
        cx: &mut Context<Self>,
    ) {
        let Some(conversation_id) = self.current_conversation_id else {
            return;
        };
        self.save_message_to_db_for(conversation_id, message_id, role, content, cx);
    }

    fn save_message_to_db_for(
        &self,
        conversation_id: Uuid,
        message_id: Uuid,
        role: Role,
        content: String,
        cx: &mut Context<Self>,
    ) {
        let db = database::get_db(cx).clone();
        let db_role = match role {
            Role::System => db_message::MessageRole::System,
            Role::User => db_message::MessageRole::User,
            Role::Assistant => db_message::MessageRole::Assistant,
        };

        Tokio::spawn(cx, async move {
            let _ = db
                .create_message(
                    message_id,
                    conversation_id,
                    db_role,
                    content,
                    db_message::MessageStatus::Done,
                )
                .await;
        })
        .detach();
    }

    fn save_message_with_thinking_to_db(
        &self,
        conversation_id: Uuid,
        message_id: Uuid,
        role: Role,
        content: String,
        thinking_content: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let db = database::get_db(cx).clone();
        let db_role = match role {
            Role::System => db_message::MessageRole::System,
            Role::User => db_message::MessageRole::User,
            Role::Assistant => db_message::MessageRole::Assistant,
        };

        Tokio::spawn(cx, async move {
            let _ = db
                .create_message_with_thinking(
                    message_id,
                    conversation_id,
                    db_role,
                    content,
                    db_message::MessageStatus::Done,
                    thinking_content,
                )
                .await;
        })
        .detach();
    }

    fn generate_response(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        // Try to reload provider if not available (user may have configured it)
        if self.llm_provider.is_none()
            && let Ok(provider) = llm::create_provider_from_settings(cx)
        {
            self.current_model_id = provider.default_model().id.clone();
            self.llm_provider = Some(provider);
        }

        let Some(provider) = self.llm_provider.clone() else {
            self.messages.push(Arc::new(Message {
                id: Uuid::now_v7(),
                role: Role::Assistant,
                content: "Error: No LLM provider configured. Please go to Settings (⌘,) to set up a provider.".to_string(),
                attachments: Vec::new(),
                status: MessageStatus::Error("Provider not configured".to_string()),
                created_at: chrono::Utc::now(),
                thinking_content: None,
                thinking_duration_ms: None,
            }));
            self.update_message_list(cx);
            return;
        };

        self.is_generating = true;
        self.message_input.update(cx, |input, cx| {
            input.set_loading(true, cx);
        });
        self.debounce_task = None;
        self.pending_stream_chunk.clear();
        self.pending_stream_result = None;

        // Start streaming message (handled separately in MessageList).
        let streaming_message = Message::assistant_streaming();
        self.active_stream = Some(ActiveStream {
            message_id: streaming_message.id,
            conversation_id: self.current_conversation_id,
            content: String::new(),
            thinking_content: String::new(),
            thinking_start_time: None,
            thinking_duration_ms: None,
        });
        if let Some(message_list) = &self.current_message_list {
            message_list.update(cx, |list, cx| {
                list.start_streaming(streaming_message, cx);
            });
        }

        let messages: Vec<ChatMessage> = self
            .messages
            .iter()
            .filter(|m| !matches!(m.status, MessageStatus::Streaming))
            .filter(|m| m.role != Role::System || !m.content.is_empty())
            .map(|m| m.as_ref().into())
            .collect();

        let (tx, rx) = async_channel::unbounded();
        let model_id = self.current_model_id.clone();

        // Spawn the HTTP request on Tokio runtime
        Tokio::spawn(cx, async move {
            if let Err(e) = provider.stream_chat(&model_id, messages, tx).await {
                tracing::error!("Stream error: {}", e);
            }
        })
        .detach();

        // Process stream events on GPUI
        cx.spawn(async move |this, cx| {
            while let Ok(event) = rx.recv().await {
                match event {
                    StreamEvent::ThinkingDelta(content) => {
                        let _ = cx.update(|app| {
                            let _ = this.update(app, |this, cx| {
                                if let Some(stream) = this.active_stream.as_mut() {
                                    // Start thinking timer on first thinking delta
                                    if stream.thinking_start_time.is_none() {
                                        stream.thinking_start_time = Some(Instant::now());
                                    }
                                    stream.thinking_content.push_str(&content);
                                }
                                if this.stream_is_current() {
                                    // Update thinking content in UI
                                    if let Some(message_list) = &this.current_message_list {
                                        message_list.update(cx, |list, cx| {
                                            list.update_thinking_content(&content, cx);
                                        });
                                    }
                                }
                            });
                        });
                    }
                    StreamEvent::ThinkingDone => {
                        let _ = cx.update(|app| {
                            let _ = this.update(app, |this, cx| {
                                if let Some(stream) = this.active_stream.as_mut() {
                                    // Calculate thinking duration
                                    if let Some(start_time) = stream.thinking_start_time.take() {
                                        stream.thinking_duration_ms =
                                            Some(start_time.elapsed().as_millis() as u64);
                                    }
                                }
                                if this.stream_is_current() {
                                    // Mark thinking as done in UI
                                    let duration = this
                                        .active_stream
                                        .as_ref()
                                        .and_then(|s| s.thinking_duration_ms);
                                    if let Some(message_list) = &this.current_message_list {
                                        message_list.update(cx, |list, cx| {
                                            list.finish_thinking(duration, cx);
                                        });
                                    }
                                }
                            });
                        });
                    }
                    StreamEvent::Delta(content) => {
                        let _ = cx.update(|app| {
                            let _ = this.update(app, |this, cx| {
                                if let Some(stream) = this.active_stream.as_mut() {
                                    stream.content.push_str(&content);
                                }
                                if this.stream_is_current() {
                                    this.pending_stream_chunk.push_str(&content);
                                    // Debounce: batch updates into MessageList.
                                    this.schedule_debounced_update(cx);
                                }
                            });
                        });
                    }
                    StreamEvent::Done => {
                        let _ = cx.update(|app| {
                            let _ = this.update(app, |this, cx| {
                                this.flush_pending_stream_chunk(cx);
                                this.finish_active_stream(None, cx);
                            });
                        });
                        break;
                    }
                    StreamEvent::Error(err) => {
                        let _ = cx.update(|app| {
                            let _ = this.update(app, |this, cx| {
                                this.flush_pending_stream_chunk(cx);
                                this.finish_active_stream(Some(err), cx);
                            });
                        });
                        break;
                    }
                }
            }
        })
        .detach();
    }

    fn finish_generating(&mut self, cx: &mut Context<Self>) {
        self.is_generating = false;
        self.message_input.update(cx, |input, cx| {
            input.set_loading(false, cx);
        });
        // Clear debounce task.
        self.debounce_task = None;
        self.pending_stream_chunk.clear();
        self.active_stream = None;
        cx.notify();
    }

    fn finish_generating_with_content_for(
        &mut self,
        conversation_id: Uuid,
        message_id: Uuid,
        content: String,
        thinking_content: Option<String>,
        cx: &mut Context<Self>,
    ) {
        self.is_generating = false;
        self.message_input.update(cx, |input, cx| {
            input.set_loading(false, cx);
        });

        // Clear debounce task.
        self.debounce_task = None;
        self.pending_stream_chunk.clear();
        self.active_stream = None;

        // Save assistant message to database (with thinking content)
        self.save_message_with_thinking_to_db(
            conversation_id,
            message_id,
            Role::Assistant,
            content,
            thinking_content,
            cx,
        );

        cx.notify();
    }

    fn is_current_conversation(&self, conversation_id: Option<Uuid>) -> bool {
        match (conversation_id, self.current_conversation_id) {
            (None, None) => true,
            (Some(left), Some(right)) => left == right,
            _ => false,
        }
    }

    fn stream_is_current(&self) -> bool {
        self.active_stream
            .as_ref()
            .is_some_and(|stream| self.is_current_conversation(stream.conversation_id))
    }

    fn clear_streaming_ui_buffer(&mut self) {
        self.pending_stream_chunk.clear();
        self.debounce_task = None;
    }

    fn sync_streaming_ui(&mut self, cx: &mut Context<Self>) {
        if !self.stream_is_current() {
            return;
        }

        let Some(stream) = &self.active_stream else {
            return;
        };

        let streaming_message = Message {
            id: stream.message_id,
            role: Role::Assistant,
            content: stream.content.clone(),
            attachments: Vec::new(),
            status: MessageStatus::Streaming,
            created_at: chrono::Utc::now(),
            thinking_content: None,
            thinking_duration_ms: None,
        };

        if let Some(message_list) = &self.current_message_list {
            message_list.update(cx, |list, cx| {
                list.start_streaming(streaming_message, cx);
            });
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn complete_stream_result(
        &mut self,
        conversation_id: Uuid,
        message_id: Uuid,
        content: String,
        thinking_content: Option<String>,
        thinking_duration_ms: Option<u64>,
        error: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let is_current = self.current_conversation_id == Some(conversation_id);

        if let Some(error) = error {
            if is_current && let Some(message_list) = &self.current_message_list {
                message_list.update(cx, |list, cx| {
                    list.set_streaming_error(&error, cx);
                });
            }
            self.finish_generating(cx);
            if !is_current {
                cx.emit(BackgroundStreamFinishedEvent {
                    conversation_id,
                    error: Some(error),
                });
            }
            return;
        }

        if is_current {
            if let Some(message_list) = &self.current_message_list {
                message_list.update(cx, |list, cx| {
                    list.finish_streaming(cx);
                });
            }
            self.messages.push(Arc::new(Message {
                id: message_id,
                role: Role::Assistant,
                content: content.clone(),
                attachments: Vec::new(),
                status: MessageStatus::Done,
                created_at: chrono::Utc::now(),
                thinking_content: thinking_content.clone(),
                thinking_duration_ms,
            }));
        }

        self.finish_generating_with_content_for(
            conversation_id,
            message_id,
            content,
            thinking_content,
            cx,
        );

        if !is_current {
            cx.emit(BackgroundStreamFinishedEvent {
                conversation_id,
                error: None,
            });
        }
    }

    fn finish_active_stream(&mut self, error: Option<String>, cx: &mut Context<Self>) {
        let Some(stream) = self.active_stream.take() else {
            return;
        };

        let conversation_id = stream.conversation_id;
        let message_id = stream.message_id;
        let content = stream.content;
        let thinking_content = if stream.thinking_content.is_empty() {
            None
        } else {
            Some(stream.thinking_content)
        };
        let thinking_duration_ms = stream.thinking_duration_ms;

        let is_current = self.is_current_conversation(conversation_id);

        if let Some(conversation_id) = conversation_id {
            self.complete_stream_result(
                conversation_id,
                message_id,
                content,
                thinking_content,
                thinking_duration_ms,
                error,
                cx,
            );
            return;
        }

        if let Some(ref error) = error {
            if is_current && let Some(message_list) = &self.current_message_list {
                message_list.update(cx, |list, cx| {
                    list.set_streaming_error(error, cx);
                });
            }
        } else if is_current {
            if let Some(message_list) = &self.current_message_list {
                message_list.update(cx, |list, cx| {
                    list.finish_streaming(cx);
                });
            }
            self.messages.push(Arc::new(Message {
                id: message_id,
                role: Role::Assistant,
                content: content.clone(),
                attachments: Vec::new(),
                status: MessageStatus::Done,
                created_at: chrono::Utc::now(),
                thinking_content: thinking_content.clone(),
                thinking_duration_ms,
            }));
        }

        self.pending_stream_result = Some(PendingStreamResult {
            message_id,
            content,
            thinking_content,
            thinking_duration_ms,
            error,
            ui_applied: is_current,
        });
        self.finish_generating(cx);
    }

    fn finalize_pending_stream(&mut self, conversation_id: Uuid, cx: &mut Context<Self>) {
        let Some(result) = self.pending_stream_result.take() else {
            return;
        };

        if result.ui_applied {
            if result.error.is_none() {
                self.finish_generating_with_content_for(
                    conversation_id,
                    result.message_id,
                    result.content,
                    result.thinking_content,
                    cx,
                );
            }
            return;
        }

        self.complete_stream_result(
            conversation_id,
            result.message_id,
            result.content,
            result.thinking_content,
            result.thinking_duration_ms,
            result.error,
            cx,
        );
    }

    /// Reload LLM provider from settings (called when provider/model changes)
    pub fn reload_llm_client(&mut self, cx: &mut Context<Self>) {
        match llm::create_provider_from_settings(cx) {
            Ok(provider) => {
                self.current_model_id = provider.default_model().id.clone();
                self.llm_provider = Some(provider);
                tracing::info!("LLM provider reloaded successfully.");
            }
            Err(e) => {
                self.llm_provider = None;
                tracing::warn!("No LLM provider configured after reload: {}", e);
            }
        }
        cx.notify();
    }

    /// Set the current model to use
    #[allow(dead_code)]
    pub fn set_model(&mut self, model_id: String, _cx: &mut Context<Self>) {
        self.current_model_id = model_id;
    }

    /// Get available models from the current provider
    #[allow(dead_code)]
    pub fn available_models(&self) -> Vec<llm::Model> {
        self.llm_provider
            .as_ref()
            .map(|p| p.models())
            .unwrap_or_default()
    }

    fn update_message_list(&mut self, cx: &mut Context<Self>) {
        let messages: Vec<Arc<Message>> = self
            .messages
            .iter()
            .filter(|m| m.role != Role::System)
            .cloned()
            .collect();

        if let Some(message_list) = &self.current_message_list {
            message_list.update(cx, |list, cx| {
                list.set_messages(messages, cx);
            });
        }
        self.sync_streaming_ui(cx);
        cx.notify();
    }

    /// Debounced flush for streaming updates to avoid rerendering per token.
    fn schedule_debounced_update(&mut self, cx: &mut Context<Self>) {
        // If a debounce task is already running, do not create another.
        if self.debounce_task.is_some() {
            return;
        }

        self.debounce_task = Some(cx.spawn(async move |this, cx| {
            // Wait for debounce interval.
            cx.background_executor()
                .timer(Duration::from_millis(STREAM_DEBOUNCE_MS))
                .await;

            let _ = cx.update(|app| {
                let _ = this.update(app, |this, cx| {
                    this.flush_pending_stream_chunk(cx);
                    this.debounce_task = None;
                });
            });
        }));
    }

    fn flush_pending_stream_chunk(&mut self, cx: &mut Context<Self>) {
        if self.pending_stream_chunk.is_empty() {
            return;
        }

        if !self.stream_is_current() {
            self.pending_stream_chunk.clear();
            return;
        }

        let chunk = std::mem::take(&mut self.pending_stream_chunk);
        if let Some(message_list) = &self.current_message_list {
            message_list.update(cx, move |list, cx| {
                list.update_streaming_content(&chunk, cx);
            });
        }
    }
}

impl Render for ChatView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        v_flex()
            .id("chat-view")
            .flex_1()
            .w_full()
            .min_h_0()
            .overflow_hidden()
            .bg(theme.background)
            // Message list takes all available space
            .when_some(self.current_message_list.clone(), |el, list| el.child(list))
            // Input stays at bottom
            .child(
                div()
                    .id("message-input-container")
                    .flex_shrink_0()
                    .w_full()
                    .child(self.message_input.clone()),
            )
    }
}
