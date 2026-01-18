// SPDX-FileCopyrightText: 2026 AprilNEA LLC
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

use gpui::*;
use gpui_component::{ActiveTheme, v_flex};
use gpui_tokio_bridge::Tokio;
use uuid::Uuid;

use crate::{
    database::{self, message as db_message},
    llm_client::{LlmClient, StreamEventResult},
    message::{ChatMessage, Message, MessageStatus, Role},
    message_input::{MessageInput, SubmitEvent},
    message_list::MessageList,
    settings::get_settings,
};

/// Event emitted when a conversation is created or updated
pub struct ConversationUpdatedEvent {
    pub conversation_id: Uuid,
}

pub struct ChatView {
    messages: Vec<Message>,
    message_list: Entity<MessageList>,
    message_input: Entity<MessageInput>,
    llm_client: Option<LlmClient>,
    is_generating: bool,
    current_conversation_id: Option<Uuid>,
    _subscription: Subscription,
}

impl EventEmitter<ConversationUpdatedEvent> for ChatView {}

impl ChatView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let message_list = cx.new(|cx| MessageList::new(cx));
        let message_input = cx.new(|cx| MessageInput::new(window, cx));

        // Try to create LLM client from settings first, then fallback to env
        let llm_client = LlmClient::from_settings(cx)
            .or_else(|_| LlmClient::from_env())
            .ok();
        if llm_client.is_none() {
            tracing::warn!("No LLM provider configured. Please set up in Settings.");
        }

        let _subscription = cx.subscribe_in(
            &message_input,
            window,
            |this, _, event: &SubmitEvent, window, cx| {
                this.handle_user_message(event.0.clone(), window, cx);
            },
        );

        let mut messages = Vec::new();
        messages.push(Message::system("You are a helpful assistant."));

        Self {
            messages,
            message_list,
            message_input,
            llm_client,
            is_generating: false,
            current_conversation_id: None,
            _subscription,
        }
    }

    /// Start a new chat (clear messages and conversation)
    pub fn new_chat(&mut self, cx: &mut Context<Self>) {
        self.messages.clear();
        self.messages
            .push(Message::system("You are a helpful assistant."));
        self.current_conversation_id = None;
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
            let result = db.list_messages(conversation_id).await;
            let _ = tx.send(result).await;
        })
        .detach();

        cx.spawn(async move |this, cx| {
            if let Ok(Ok(db_messages)) = rx.recv().await {
                let _ = cx.update(|app| {
                    let _ = this.update(app, |this, cx| {
                        for msg in db_messages {
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
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if content.trim().is_empty() || self.is_generating {
            return;
        }

        let user_message = Message::user(&content);
        self.messages.push(user_message.clone());
        self.update_message_list(cx);

        // Create conversation if this is the first message
        if self.current_conversation_id.is_none() {
            self.create_conversation_and_save_message(content.clone(), user_message.id, cx);
        } else {
            self.save_message_to_db(user_message.id, Role::User, content.clone(), cx);
        }

        self.generate_response(window, cx);
    }

    fn create_conversation_and_save_message(
        &mut self,
        first_message: String,
        _message_id: Uuid,
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
                        conv.id,
                        db_message::MessageRole::User,
                        first_message_clone,
                        db_message::MessageStatus::Done,
                    )
                    .await;
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
        _message_id: Uuid,
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
        // Try to reload client if not available (user may have configured it)
        if self.llm_client.is_none() {
            self.llm_client = LlmClient::from_settings(cx)
                .or_else(|_| LlmClient::from_env())
                .ok();
        }

        let Some(client) = self.llm_client.clone() else {
            self.messages.push(Message {
                id: uuid::Uuid::new_v4(),
                role: Role::Assistant,
                content: "Error: No LLM provider configured. Please go to Settings (⌘,) to set up a provider.".to_string(),
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

        self.messages.push(Message::assistant_streaming());
        self.update_message_list(cx);

        let messages: Vec<ChatMessage> = self
            .messages
            .iter()
            .filter(|m| !matches!(m.status, MessageStatus::Streaming))
            .filter(|m| m.role != Role::System || !m.content.is_empty())
            .map(|m| m.into())
            .collect();

        let (tx, rx) = async_channel::unbounded();

        // Spawn the HTTP request on Tokio runtime
        Tokio::spawn(cx, async move {
            if let Err(e) = client.stream_chat(messages, tx).await {
                tracing::error!("Stream error: {}", e);
            }
        })
        .detach();

        // Process stream events on GPUI
        cx.spawn(async move |this, cx| {
            while let Ok(event) = rx.recv().await {
                match event {
                    StreamEventResult::Delta(content) => {
                        let _ = cx.update(|app| {
                            let _ = this.update(app, |this, cx| {
                                if let Some(msg) = this.messages.last_mut() {
                                    if matches!(msg.status, MessageStatus::Streaming) {
                                        msg.append_content(&content);
                                        this.update_message_list(cx);
                                    }
                                }
                            });
                        });
                    }
                    StreamEventResult::Done => {
                        let _ = cx.update(|app| {
                            let _ = this.update(app, |this, cx| {
                                if let Some(msg) = this.messages.last_mut() {
                                    msg.set_done();
                                }
                                this.finish_generating(cx);
                            });
                        });
                        break;
                    }
                    StreamEventResult::Error(err) => {
                        let _ = cx.update(|app| {
                            let _ = this.update(app, |this, cx| {
                                if let Some(msg) = this.messages.last_mut() {
                                    msg.set_error(&err);
                                }
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

        // Save assistant message to database
        if let Some(msg) = self.messages.last() {
            if msg.role == Role::Assistant {
                self.save_message_to_db(msg.id, Role::Assistant, msg.content.clone(), cx);
            }
        }

        self.update_message_list(cx);
    }

    /// Reload LLM client from settings (called when provider/model changes)
    pub fn reload_llm_client(&mut self, cx: &mut Context<Self>) {
        self.llm_client = LlmClient::from_settings(cx)
            .or_else(|_| LlmClient::from_env())
            .ok();

        if self.llm_client.is_none() {
            tracing::warn!("No LLM provider configured after reload.");
        } else {
            tracing::info!("LLM client reloaded successfully.");
        }
        cx.notify();
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
}

impl Render for ChatView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        v_flex()
            .size_full()
            .bg(theme.background)
            .child(v_flex().flex_1().child(self.message_list.clone()))
            .child(self.message_input.clone())
    }
}
