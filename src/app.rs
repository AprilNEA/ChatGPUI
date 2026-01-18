// SPDX-FileCopyrightText: 2026 AprilNEA LLC
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

use std::time::Duration;

use gpui::*;
use gpui_component::{ActiveTheme, h_flex, v_flex};
use gpui_tokio_bridge::Tokio;

use crate::chat_sidebar::{
    ChatSidebar, ConversationSelectedEvent, NewChatEvent, SidebarToggleEvent,
};
use crate::chat_view::{ChatView, ConversationUpdatedEvent};
use crate::database;
use crate::model_selector::{ModelSelector, ModelSelectorChangedEvent};

// Sidebar constants
const SIDEBAR_DEFAULT_WIDTH: f32 = 260.0;
const SIDEBAR_COLLAPSED_WIDTH: f32 = 0.0;
const SIDEBAR_ANIMATION_DURATION: Duration = Duration::from_millis(200);

pub struct ChatApp {
    sidebar: Entity<ChatSidebar>,
    model_selector: Entity<ModelSelector>,
    chat_view: Entity<ChatView>,
    sidebar_collapsed: bool,
    sidebar_width: f32,
    _subscriptions: Vec<Subscription>,
}

impl ChatApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        // Initialize database asynchronously
        let db = database::get_db(cx).clone();
        Tokio::spawn(cx, async move {
            if let Err(e) = db.initialize().await {
                tracing::error!("Failed to initialize database: {}", e);
            }
        })
        .detach();

        let sidebar = cx.new(|cx| ChatSidebar::new(window, cx));
        let model_selector = cx.new(|cx| ModelSelector::new(cx));
        let chat_view = cx.new(|cx| ChatView::new(window, cx));

        let mut subscriptions = Vec::new();

        // Subscribe to model selector changes
        subscriptions.push(cx.subscribe_in(
            &model_selector,
            window,
            |this, _, _event: &ModelSelectorChangedEvent, _window, cx| {
                this.chat_view.update(cx, |view, cx| {
                    view.reload_llm_client(cx);
                });
            },
        ));

        // Subscribe to sidebar toggle events
        subscriptions.push(cx.subscribe_in(
            &sidebar,
            window,
            |this, _, _event: &SidebarToggleEvent, _window, cx| {
                this.toggle_sidebar(cx);
            },
        ));

        // Subscribe to conversation selection events
        subscriptions.push(cx.subscribe_in(
            &sidebar,
            window,
            |this, _, event: &ConversationSelectedEvent, _window, cx| {
                if let Some(conv_id) = event.conversation_id {
                    this.chat_view.update(cx, |view, cx| {
                        view.load_conversation(conv_id, cx);
                    });
                }
            },
        ));

        // Subscribe to new chat events
        subscriptions.push(cx.subscribe_in(
            &sidebar,
            window,
            |this, _, _event: &NewChatEvent, _window, cx| {
                this.chat_view.update(cx, |view, cx| {
                    view.new_chat(cx);
                });
            },
        ));

        // Subscribe to conversation updated events (to refresh sidebar)
        subscriptions.push(cx.subscribe_in(
            &chat_view,
            window,
            |this, _, _event: &ConversationUpdatedEvent, _window, cx| {
                this.sidebar.update(cx, |sidebar, cx| {
                    sidebar.reload(cx);
                });
            },
        ));

        Self {
            sidebar,
            model_selector,
            chat_view,
            sidebar_collapsed: false,
            sidebar_width: SIDEBAR_DEFAULT_WIDTH,
            _subscriptions: subscriptions,
        }
    }

    fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        self.sidebar_collapsed = !self.sidebar_collapsed;
        cx.notify();
    }
}

impl Render for ChatApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let collapsed = self.sidebar_collapsed;
        let expanded_width = self.sidebar_width;
        let collapsed_width = SIDEBAR_COLLAPSED_WIDTH;

        h_flex()
            .size_full()
            .bg(theme.background)
            // Left: Chat history sidebar with animation
            .child(
                div()
                    .id("sidebar-container")
                    .h_full()
                    .flex_shrink_0()
                    .overflow_hidden()
                    .child(self.sidebar.clone())
                    .with_animation(
                        ElementId::Name(
                            format!("sidebar-{}", if collapsed { "collapse" } else { "expand" })
                                .into(),
                        ),
                        Animation::new(SIDEBAR_ANIMATION_DURATION),
                        move |el, delta| {
                            let (start, end) = if collapsed {
                                (expanded_width, collapsed_width)
                            } else {
                                (collapsed_width, expanded_width)
                            };
                            el.w(px(start + delta * (end - start)))
                        },
                    ),
            )
            // Right: Main chat area
            .child(
                v_flex()
                    .flex_1()
                    .h_full()
                    .child(self.model_selector.clone())
                    .child(self.chat_view.clone()),
            )
    }
}
