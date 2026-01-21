// SPDX-FileCopyrightText: 2026 AprilNEA LLC
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

use std::time::Duration;

use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::{
    ActiveTheme,
    button::{Button, ButtonVariants},
    h_flex,
    notification::{Notification, NotificationList, NotificationType},
    v_flex,
};

use crate::assets::{AppIcon, ButtonAppIconExt};
use gpui_tokio_bridge::Tokio;

use crate::chat_sidebar::{ChatSidebar, ConversationDeletedEvent, ConversationSelectedEvent};
use crate::chat_view::{BackgroundStreamFinishedEvent, ChatView, ConversationUpdatedEvent};
use crate::database;
use crate::model_selector::{ModelSelector, ModelSelectorChangedEvent};

// Sidebar constants
const SIDEBAR_DEFAULT_WIDTH: f32 = 260.0;
const SIDEBAR_MIN_WIDTH: f32 = 200.0;
const SIDEBAR_MAX_WIDTH: f32 = 400.0;
const SIDEBAR_ANIMATION_DURATION: Duration = Duration::from_millis(150);

/// Drag state for sidebar resizing
#[derive(Clone)]
struct SidebarResizeDrag;

/// Empty view for drag visual (invisible)
struct EmptyDragView;

impl Render for EmptyDragView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

pub struct ChatApp {
    sidebar: Entity<ChatSidebar>,
    model_selector: Entity<ModelSelector>,
    chat_view: Entity<ChatView>,
    notification_list: Entity<NotificationList>,
    sidebar_collapsed: bool,
    sidebar_width: f32,
    /// Animation trigger counter - increments on each toggle to reset animation
    animation_trigger: usize,
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
        let model_selector = cx.new(ModelSelector::new);
        let chat_view = cx.new(|cx| ChatView::new(window, cx));
        let notification_list = cx.new(|cx| NotificationList::new(window, cx));

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

        // Subscribe to conversation deletion events (to clean up cache)
        subscriptions.push(cx.subscribe_in(
            &sidebar,
            window,
            |this, _, event: &ConversationDeletedEvent, _window, cx| {
                this.chat_view.update(cx, |view, _cx| {
                    view.remove_from_cache(event.conversation_id);
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

        // Subscribe to background stream completion for toast notifications
        subscriptions.push(cx.subscribe_in(
            &chat_view,
            window,
            |this, _, event: &BackgroundStreamFinishedEvent, window, cx| {
                let conversation_id = event.conversation_id;
                let title = this
                    .sidebar
                    .read(cx)
                    .conversation_title(conversation_id)
                    .unwrap_or_else(|| t!("toast.conversation_fallback").to_string());
                let (type_, message) = match &event.error {
                    Some(error) => (
                        NotificationType::Error,
                        format!("{}: {}", t!("toast.stream_failed"), error),
                    ),
                    None => (
                        NotificationType::Success,
                        t!("toast.stream_complete").to_string(),
                    ),
                };
                let sidebar = this.sidebar.clone();
                let notification = Notification::new()
                    .title(title)
                    .message(message)
                    .with_type(type_)
                    .on_click(move |_event, _window, cx| {
                        sidebar.update(cx, |sidebar, cx| {
                            sidebar.select_conversation(Some(conversation_id), cx);
                        });
                    });
                this.notification_list.update(cx, |list, cx| {
                    list.push(notification, window, cx);
                });
            },
        ));

        Self {
            sidebar,
            model_selector,
            chat_view,
            notification_list,
            sidebar_collapsed: false,
            sidebar_width: SIDEBAR_DEFAULT_WIDTH,
            animation_trigger: 0,
            _subscriptions: subscriptions,
        }
    }

    fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        self.sidebar_collapsed = !self.sidebar_collapsed;
        self.animation_trigger += 1;
        let collapsed = self.sidebar_collapsed;
        self.model_selector.update(cx, |selector, cx| {
            selector.set_sidebar_collapsed(collapsed, cx);
        });
        cx.notify();
    }

    fn resize_sidebar(&mut self, new_width: f32, cx: &mut Context<Self>) {
        self.sidebar_width = new_width.clamp(SIDEBAR_MIN_WIDTH, SIDEBAR_MAX_WIDTH);
        cx.notify();
    }

    fn new_chat(&mut self, cx: &mut Context<Self>) {
        self.sidebar.update(cx, |sidebar, cx| {
            sidebar.select_conversation(None, cx);
        });
        self.chat_view.update(cx, |view, cx| {
            view.new_chat(cx);
        });
    }
}

impl Render for ChatApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let collapsed = self.sidebar_collapsed;

        div()
            .size_full()
            .relative()
            .bg(theme.background)
            .child(
                h_flex()
                    .size_full()
                    // Animated sidebar container
                    .child(self.render_sidebar(cx))
                    // Resize handle (only when expanded)
                    .when(!collapsed, |el| el.child(self.render_resize_handle(cx)))
                    // Main content area
                    .child(
                        v_flex()
                            .id("main-content")
                            .flex_1()
                            .h_full()
                            .min_w_0()
                            .min_h_0()
                            .overflow_hidden()
                            .child(self.model_selector.clone())
                            .child(self.chat_view.clone()),
                    ),
            )
            // Absolutely positioned toolbar buttons (fixed position regardless of sidebar state)
            .child(
                h_flex()
                    .absolute()
                    .top(px(8.))
                    .left(px(80.)) // After traffic lights
                    .gap_0()
                    .child(
                        Button::new("toggle-sidebar")
                            .app_icon(AppIcon::PanelLeft)
                            .ghost()
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.toggle_sidebar(cx);
                            })),
                    )
                    .child(
                        Button::new("new-chat")
                            .app_icon(AppIcon::Plus)
                            .ghost()
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.new_chat(cx);
                            })),
                    ),
            )
            .child(self.notification_list.clone())
    }
}

impl ChatApp {
    /// Render sidebar with collapse/expand animation
    fn render_sidebar(&self, _cx: &Context<Self>) -> impl IntoElement {
        let collapsed = self.sidebar_collapsed;
        let expanded_width = self.sidebar_width;
        let animation_trigger = self.animation_trigger;

        // Animation parameters
        let (start_width, end_width) = if collapsed {
            (expanded_width, 0.0)
        } else {
            (0.0, expanded_width)
        };

        div()
            .id("sidebar-container")
            .h_full()
            .flex_shrink_0()
            .overflow_hidden()
            .child(self.sidebar.clone())
            .with_animation(
                ("sidebar-anim", animation_trigger),
                Animation::new(SIDEBAR_ANIMATION_DURATION).with_easing(ease_in_out),
                move |el, delta| {
                    let width = start_width + (end_width - start_width) * delta;
                    el.w(px(width))
                },
            )
    }

    /// Render resize handle for dragging sidebar width
    fn render_resize_handle(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        div()
            .id("sidebar-resize-handle")
            .w(px(1.0))
            .h_full()
            .flex_shrink_0()
            .cursor(CursorStyle::ResizeLeftRight)
            .bg(theme.border)
            .hover(|el| el.bg(theme.primary))
            .on_drag(SidebarResizeDrag, |_, _, _, cx| cx.new(|_| EmptyDragView))
            .on_drag_move::<SidebarResizeDrag>(cx.listener(
                |this, event: &DragMoveEvent<SidebarResizeDrag>, _window, cx| {
                    let new_width: f32 = event.event.position.x.into();
                    this.resize_sidebar(new_width, cx);
                },
            ))
    }
}
