// SPDX-FileCopyrightText: 2026 AprilNEA LLC
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

use gpui::*;
use gpui_component::{v_flex, ActiveTheme};

use gpui_tokio_bridge::Tokio;

use crate::{
    llm_client::{LlmClient, StreamEventResult},
    message::{ChatMessage, Message, MessageStatus, Role},
    message_input::{MessageInput, SubmitEvent},
    message_list::MessageList,
};

pub struct ChatView {
    messages: Vec<Message>,
    message_list: Entity<MessageList>,
    message_input: Entity<MessageInput>,
    llm_client: Option<LlmClient>,
    is_generating: bool,
    _subscription: Subscription,
}

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

        let _subscription =
            cx.subscribe_in(&message_input, window, |this, _, event: &SubmitEvent, window, cx| {
                this.handle_user_message(event.0.clone(), window, cx);
            });

        let mut messages = Vec::new();
        messages.push(Message::system("You are a helpful assistant."));

        Self {
            messages,
            message_list,
            message_input,
            llm_client,
            is_generating: false,
            _subscription,
        }
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

        self.messages.push(Message::user(&content));
        self.update_message_list(cx);

        self.generate_response(window, cx);
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
