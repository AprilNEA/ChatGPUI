// SPDX-FileCopyrightText: 2026 AprilNEA LLC
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

use std::time::Duration;

use gpui::*;
use gpui_component::{h_flex, v_flex, ActiveTheme};

use crate::chat_sidebar::{ChatSidebar, SidebarToggleEvent};
use crate::chat_view::ChatView;
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
    _model_subscription: Subscription,
    _sidebar_subscription: Subscription,
}

impl ChatApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let sidebar = cx.new(|cx| ChatSidebar::new(window, cx));
        let model_selector = cx.new(|cx| ModelSelector::new(cx));
        let chat_view = cx.new(|cx| ChatView::new(window, cx));

        // Subscribe to model selector changes
        let _model_subscription = cx.subscribe_in(
            &model_selector,
            window,
            |this, _, _event: &ModelSelectorChangedEvent, _window, cx| {
                this.chat_view.update(cx, |view, cx| {
                    view.reload_llm_client(cx);
                });
            },
        );

        // Subscribe to sidebar toggle events
        let _sidebar_subscription = cx.subscribe_in(
            &sidebar,
            window,
            |this, _, _event: &SidebarToggleEvent, _window, cx| {
                this.toggle_sidebar(cx);
            },
        );

        Self {
            sidebar,
            model_selector,
            chat_view,
            sidebar_collapsed: false,
            sidebar_width: SIDEBAR_DEFAULT_WIDTH,
            _model_subscription,
            _sidebar_subscription,
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
                            format!("sidebar-{}", if collapsed { "collapse" } else { "expand" }).into(),
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
