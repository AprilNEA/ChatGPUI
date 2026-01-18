// SPDX-FileCopyrightText: 2026 AprilNEA LLC
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

use gpui::*;
use gpui_component::{
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputState},
    label::Label,
    list::ListItem,
    scroll::ScrollableElement,
    v_flex, ActiveTheme, Icon, IconName, Sizable,
};

/// Event emitted when sidebar toggle button is clicked
pub struct SidebarToggleEvent;

/// Mock chat history item for UI display
#[derive(Clone)]
struct ChatHistoryItem {
    id: usize,
    title: String,
    provider_icon: String,
}

/// Chat history sidebar component
pub struct ChatSidebar {
    search_input: Entity<InputState>,
    // Mock data for UI display
    history_items: Vec<(String, Vec<ChatHistoryItem>)>,
    selected_item: Option<usize>,
}

impl EventEmitter<SidebarToggleEvent> for ChatSidebar {}

impl ChatSidebar {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search_input = cx.new(|cx| InputState::new(window, cx).placeholder(t!("sidebar.search").to_string()));

        // Mock history data grouped by date
        let history_items = vec![
            (
                t!("sidebar.favorites").to_string(),
                vec![
                    ChatHistoryItem {
                        id: 1,
                        title: "Rust 隧道中转项目目录结构设计".to_string(),
                        provider_icon: "A\\".to_string(),
                    },
                    ChatHistoryItem {
                        id: 2,
                        title: "DevUtility: Rust与Tauri构建...".to_string(),
                        provider_icon: "A\\".to_string(),
                    },
                ],
            ),
            (
                t!("sidebar.today").to_string(),
                vec![
                    ChatHistoryItem {
                        id: 3,
                        title: "如何升级 Rust".to_string(),
                        provider_icon: "A\\".to_string(),
                    },
                    ChatHistoryItem {
                        id: 4,
                        title: "GPUI 组件开发".to_string(),
                        provider_icon: "A\\".to_string(),
                    },
                ],
            ),
            (
                t!("sidebar.yesterday").to_string(),
                vec![
                    ChatHistoryItem {
                        id: 5,
                        title: "GCP Anycast 路由优化问题".to_string(),
                        provider_icon: "A\\".to_string(),
                    },
                    ChatHistoryItem {
                        id: 6,
                        title: "sea-orm PostgreSQL mTLS支持".to_string(),
                        provider_icon: "A\\".to_string(),
                    },
                ],
            ),
        ];

        Self {
            search_input,
            history_items,
            selected_item: Some(3), // Default select first "today" item
        }
    }

    fn select_item(&mut self, id: usize, cx: &mut Context<Self>) {
        self.selected_item = Some(id);
        cx.notify();
    }

    fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        cx.emit(SidebarToggleEvent);
    }

    fn render_toolbar(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .w_full()
            .pl(px(78.)) // Leave space for traffic lights
            .pr_2()
            .py_2()
            .justify_end()
            .child(
                h_flex()
                    .gap_1()
                    .child(
                        Button::new("toggle-sidebar")
                            .icon(IconName::PanelLeft)
                            .ghost()
                            .xsmall()
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.toggle_sidebar(cx);
                            })),
                    )
                    .child(
                        Button::new("new-chat")
                            .icon(IconName::Plus)
                            .ghost()
                            .xsmall(),
                    ),
            )
    }

    fn render_search(&mut self, _cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .w_full()
            .px_3()
            .pb_2()
            .child(
                Input::new(&self.search_input)
                    .prefix(Icon::new(IconName::Search).size_4())
                    .xsmall()
                    .appearance(false),
            )
    }

    fn render_quick_tags(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        h_flex()
            .w_full()
            .px_3()
            .pb_3()
            .gap_2()
            .flex_wrap()
            .child(self.render_tag("Company", "🏢", &theme))
            .child(self.render_tag("Rust", "⚙", &theme))
            .child(self.render_tag("Linear", "◆", &theme))
            .child(self.render_tag(&t!("sidebar.default_tag"), "😊", &theme))
    }

    fn render_tag(&self, label: &str, icon: &str, theme: &gpui_component::theme::Theme) -> impl IntoElement {
        h_flex()
            .px_2()
            .py_1()
            .gap_1()
            .rounded_md()
            .bg(theme.muted)
            .text_xs()
            .items_center()
            .child(icon.to_string())
            .child(Label::new(label.to_string()).text_xs())
    }

    fn render_history_list(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let selected = self.selected_item;
        let history = self.history_items.clone();

        v_flex()
            .flex_1()
            .overflow_y_scrollbar()
            .children(history.into_iter().map(|(group_name, items)| {
                v_flex()
                    .w_full()
                    .child(
                        h_flex()
                            .px_3()
                            .py_1()
                            .child(
                                Label::new(group_name)
                                    .text_xs()
                                    .text_color(theme.muted_foreground),
                            ),
                    )
                    .children(items.into_iter().map(|item| {
                        let is_selected = selected == Some(item.id);
                        let item_id = item.id;

                        ListItem::new(("history", item.id))
                            .px_3()
                            .py_2()
                            .mx_2()
                            .my_px()
                            .rounded_md()
                            .selected(is_selected)
                            .on_click(cx.listener(move |this, _, _window, cx| {
                                this.select_item(item_id, cx);
                            }))
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .overflow_hidden()
                                    .child(
                                        h_flex()
                                            .size_4()
                                            .items_center()
                                            .justify_center()
                                            .text_xs()
                                            .child(item.provider_icon),
                                    )
                                    .child(
                                        Label::new(item.title)
                                            .text_sm()
                                            .truncate(),
                                    ),
                            )
                    }))
            }))
    }

    fn render_user_info(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        h_flex()
            .w_full()
            .px_3()
            .py_3()
            .gap_2()
            .items_center()
            .border_t_1()
            .border_color(theme.border)
            .child(
                h_flex()
                    .size_6()
                    .rounded_full()
                    .bg(theme.primary)
                    .items_center()
                    .justify_center()
                    .text_xs()
                    .text_color(theme.primary_foreground)
                    .child("A"),
            )
            .child(Label::new("AprilNEA").text_sm())
    }
}

impl Render for ChatSidebar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        v_flex()
            .h_full()
            .w(px(260.))
            .min_w(px(260.))
            .bg(theme.sidebar)
            .border_r_1()
            .border_color(theme.border)
            .child(self.render_toolbar(cx))
            .child(self.render_search(cx))
            .child(self.render_quick_tags(cx))
            .child(self.render_history_list(cx))
            .child(self.render_user_info(cx))
    }
}
