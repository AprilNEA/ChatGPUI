// SPDX-FileCopyrightText: 2026 AprilNEA LLC
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

use std::sync::Arc;
use std::time::Duration;

use gpui::*;
use gpui_component::{ActiveTheme, v_flex};
use gpui_tokio_bridge::Tokio;
use uuid::Uuid;

use crate::{
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

pub struct ChatView {
    messages: Vec<Message>,
    message_list: Entity<MessageList>,
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
    /// Current streaming message id for persistence.
    streaming_message_id: Option<Uuid>,
}

impl EventEmitter<ConversationUpdatedEvent> for ChatView {}

impl ChatView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let message_list = cx.new(|cx| MessageList::new(cx));
        let message_input = cx.new(|cx| MessageInput::new(window, cx));

        // Try to create LLM provider from settings
        let (llm_provider, current_model_id) = match llm::create_provider_from_settings(cx) {
            Ok(provider) => {
                let model_id = provider.default_model().id.clone();
                (Some(provider), model_id)
            }
            Err(e) => {
                tracing::warn!("No LLM provider configured: {}. Please set up in Settings.", e);
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

        let mut messages = Vec::new();
        messages.push(Message::system("You are a helpful assistant."));

        Self {
            messages,
            message_list,
            message_input,
            llm_provider,
            current_model_id,
            is_generating: false,
            current_conversation_id: None,
            _subscription,
            debounce_task: None,
            pending_stream_chunk: String::new(),
            streaming_message_id: None,
        }
    }

    /// Start a new chat (clear messages and conversation)
    pub fn new_chat(&mut self, cx: &mut Context<Self>) {
        self.messages.clear();
        self.messages
            .push(Message::system("You are a helpful assistant."));
        self.current_conversation_id = None;
        self.streaming_message_id = None;
        self.update_message_list(cx);
    }

    /// Load an existing conversation
    pub fn load_conversation(&mut self, conversation_id: Uuid, cx: &mut Context<Self>) {
        self.current_conversation_id = Some(conversation_id);
        self.messages.clear();
        self.messages
            .push(Message::system("You are a helpful assistant."));

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
                let _ = tx
                    .send(Err(messages_result.unwrap_err()))
                    .await;
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
                            this.messages.push(Message {
                                id: msg.id,
                                role,
                                content: msg.content,
                                attachments,
                                status,
                                created_at: msg.created_at,
                            });
                        }
                        this.update_message_list(cx);
                    });
                });
            }
        })
        .detach();
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
        self.messages.push(user_message.clone());
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

                    if let Ok(file_path) = storage::save_attachment(att.id, &att.data, extension).await {
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
                        this.current_conversation_id = Some(conversation_id);
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

    fn generate_response(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        // Try to reload provider if not available (user may have configured it)
        if self.llm_provider.is_none() {
            if let Ok(provider) = llm::create_provider_from_settings(cx) {
                self.current_model_id = provider.default_model().id.clone();
                self.llm_provider = Some(provider);
            }
        }

        let Some(provider) = self.llm_provider.clone() else {
            self.messages.push(Message {
                id: Uuid::now_v7(),
                role: Role::Assistant,
                content: "Error: No LLM provider configured. Please go to Settings (⌘,) to set up a provider.".to_string(),
                attachments: Vec::new(),
                status: MessageStatus::Error("Provider not configured".to_string()),
                created_at: chrono::Utc::now(),
            });
            self.update_message_list(cx);
            return;
        };

        self.is_generating = true;
        self.message_input.update(cx, |input, cx| {
            input.set_loading(true, cx);
        });

        // Start streaming message (handled separately in MessageList).
        let streaming_message = Message::assistant_streaming();
        self.streaming_message_id = Some(streaming_message.id);
        self.message_list.update(cx, |list, cx| {
            list.start_streaming(streaming_message, cx);
        });

        let messages: Vec<ChatMessage> = self
            .messages
            .iter()
            .filter(|m| !matches!(m.status, MessageStatus::Streaming))
            .filter(|m| m.role != Role::System || !m.content.is_empty())
            .map(|m| m.into())
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
            // Accumulate content for final persistence.
            let mut accumulated_content = String::new();

            while let Ok(event) = rx.recv().await {
                match event {
                    StreamEvent::Delta(content) => {
                        accumulated_content.push_str(&content);
                        let _ = cx.update(|app| {
                            let _ = this.update(app, |this, cx| {
                                this.pending_stream_chunk.push_str(&content);
                                // Debounce: batch updates into MessageList.
                                this.schedule_debounced_update(cx);
                            });
                        });
                    }
                    StreamEvent::Done => {
                        let content = accumulated_content.clone();
                        let _ = cx.update(|app| {
                            let _ = this.update(app, |this, cx| {
                                let message_id = this
                                    .streaming_message_id
                                    .take()
                                    .unwrap_or_else(Uuid::now_v7);
                                this.flush_pending_stream_chunk(cx);
                                // Finish streaming message.
                                this.message_list.update(cx, |list, cx| {
                                    list.finish_streaming(cx);
                                });
                                // Persist to local list (used for future API requests).
                                this.messages.push(Message {
                                    id: message_id,
                                    role: Role::Assistant,
                                    content: content.clone(),
                                    attachments: Vec::new(),
                                    status: MessageStatus::Done,
                                    created_at: chrono::Utc::now(),
                                });
                                this.finish_generating_with_content(message_id, content, cx);
                            });
                        });
                        break;
                    }
                    StreamEvent::Error(err) => {
                        let _ = cx.update(|app| {
                            let _ = this.update(app, |this, cx| {
                                this.flush_pending_stream_chunk(cx);
                                this.message_list.update(cx, |list, cx| {
                                    list.set_streaming_error(&err, cx);
                                });
                                this.streaming_message_id = None;
                                this.finish_generating(cx);
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
        self.streaming_message_id = None;
        cx.notify();
    }

    fn finish_generating_with_content(
        &mut self,
        message_id: Uuid,
        content: String,
        cx: &mut Context<Self>,
    ) {
        self.is_generating = false;
        self.message_input.update(cx, |input, cx| {
            input.set_loading(false, cx);
        });

        // Clear debounce task.
        self.debounce_task = None;
        self.pending_stream_chunk.clear();
        self.streaming_message_id = None;

        // Save assistant message to database
        self.save_message_to_db(message_id, Role::Assistant, content, cx);

        cx.notify();
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
        let messages: Vec<Message> = self
            .messages
            .iter()
            .filter(|m| m.role != Role::System)
            .cloned()
            .collect();

        self.message_list.update(cx, |list, cx| {
            list.set_messages(messages, cx);
        });
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

        let chunk = std::mem::take(&mut self.pending_stream_chunk);
        self.message_list.update(cx, move |list, cx| {
            list.update_streaming_content(&chunk, cx);
        });
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
            .child(self.message_list.clone())
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
