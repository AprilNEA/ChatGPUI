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
    database::{self, message as db_message},
    llm::{self, LlmProvider, StreamEvent},
    message::{ChatMessage, Message, MessageStatus, Role},
    message_input::{MessageInput, SubmitEvent},
    message_list::MessageList,
    settings::get_settings,
};

/// 流式更新的防抖间隔（毫秒）
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
    /// 防抖任务：避免流式更新时频繁重渲染
    debounce_task: Option<Task<()>>,
    /// 是否有待处理的 UI 更新
    pending_ui_update: bool,
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
                this.handle_user_message(event.0.clone(), window, cx);
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
            pending_ui_update: false,
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
        // Try to reload provider if not available (user may have configured it)
        if self.llm_provider.is_none() {
            if let Ok(provider) = llm::create_provider_from_settings(cx) {
                self.current_model_id = provider.default_model().id.clone();
                self.llm_provider = Some(provider);
            }
        }

        let Some(provider) = self.llm_provider.clone() else {
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

        // 开始流式消息（在 MessageList 中单独处理）
        self.message_list.update(cx, |list, cx| {
            list.start_streaming(cx);
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
            // 累积内容，用于最终保存到数据库
            let mut accumulated_content = String::new();

            while let Ok(event) = rx.recv().await {
                match event {
                    StreamEvent::Delta(content) => {
                        accumulated_content.push_str(&content);
                        let _ = cx.update(|app| {
                            let _ = this.update(app, |this, cx| {
                                // 直接更新 MessageList 的流式消息，无需克隆历史消息
                                this.message_list.update(cx, |list, cx| {
                                    list.update_streaming_content(&content, cx);
                                });
                                // 使用防抖机制（这里主要是为了滚动）
                                this.schedule_debounced_update(cx);
                            });
                        });
                    }
                    StreamEvent::Done => {
                        let content = accumulated_content.clone();
                        let _ = cx.update(|app| {
                            let _ = this.update(app, |this, cx| {
                                // 完成流式消息
                                this.message_list.update(cx, |list, cx| {
                                    list.finish_streaming(cx);
                                });
                                // 保存到本地消息列表（用于发送 API 请求）
                                this.messages.push(Message::new(Role::Assistant, &content));
                                this.finish_generating_with_content(content, cx);
                            });
                        });
                        break;
                    }
                    StreamEvent::Error(err) => {
                        let _ = cx.update(|app| {
                            let _ = this.update(app, |this, cx| {
                                this.message_list.update(cx, |list, cx| {
                                    list.set_streaming_error(&err, cx);
                                });
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
        // 取消防抖任务
        self.debounce_task = None;
        self.pending_ui_update = false;
        cx.notify();
    }

    fn finish_generating_with_content(&mut self, content: String, cx: &mut Context<Self>) {
        self.is_generating = false;
        self.message_input.update(cx, |input, cx| {
            input.set_loading(false, cx);
        });

        // 取消防抖任务
        self.debounce_task = None;
        self.pending_ui_update = false;

        // Save assistant message to database
        self.save_message_to_db(Uuid::new_v4(), Role::Assistant, content, cx);

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

    /// 防抖通知 - 流式消息时使用，避免每个 token 都触发重渲染
    fn schedule_debounced_update(&mut self, cx: &mut Context<Self>) {
        self.pending_ui_update = true;

        // 如果已经有防抖任务在运行，不需要再创建新的
        if self.debounce_task.is_some() {
            return;
        }

        self.debounce_task = Some(cx.spawn(async move |this, cx| {
            // 等待防抖间隔
            cx.background_executor()
                .timer(Duration::from_millis(STREAM_DEBOUNCE_MS))
                .await;

            let _ = cx.update(|app| {
                let _ = this.update(app, |this, cx| {
                    if this.pending_ui_update {
                        this.pending_ui_update = false;
                        // 只通知刷新，不需要重新设置消息列表
                        cx.notify();
                    }
                    this.debounce_task = None;
                });
            });
        }));
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
