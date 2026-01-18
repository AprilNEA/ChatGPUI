// SPDX-FileCopyrightText: 2026 AprilNEA LLC
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

use chrono::{Datelike, Local};
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
use gpui_tokio_bridge::Tokio;
use uuid::Uuid;

use crate::database::{self, conversation};

/// Event emitted when sidebar toggle button is clicked
pub struct SidebarToggleEvent;

/// Event emitted when a conversation is selected
pub struct ConversationSelectedEvent {
    pub conversation_id: Option<Uuid>,
}

/// Event emitted when new chat button is clicked
pub struct NewChatEvent;

/// Chat history item for UI display
#[derive(Clone)]
struct ChatHistoryItem {
    id: Uuid,
    title: String,
    provider_icon: String,
}

/// Chat history sidebar component
pub struct ChatSidebar {
    search_input: Entity<InputState>,
    conversations: Vec<conversation::Model>,
    selected_conversation: Option<Uuid>,
    db_ready: bool,
}

impl EventEmitter<SidebarToggleEvent> for ChatSidebar {}
impl EventEmitter<ConversationSelectedEvent> for ChatSidebar {}
impl EventEmitter<NewChatEvent> for ChatSidebar {}

impl ChatSidebar {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search_input =
            cx.new(|cx| InputState::new(window, cx).placeholder(t!("sidebar.search").to_string()));

        // Schedule loading conversations after creation
        let db = database::get_db(cx).clone();
        let (tx, rx) = async_channel::unbounded();

        Tokio::spawn(cx, async move {
            // Wait a bit for database to initialize
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            let result = db.list_conversations().await;
            let _ = tx.send(result).await;
        })
        .detach();

        cx.spawn(async move |this, cx| {
            if let Ok(Ok(conversations)) = rx.recv().await {
                let _ = cx.update(|app| {
                    let _ = this.update(app, |this, cx| {
                        this.conversations = conversations;
                        this.db_ready = true;
                        cx.notify();
                    });
                });
            }
        })
        .detach();

        Self {
            search_input,
            conversations: Vec::new(),
            selected_conversation: None,
            db_ready: false,
        }
    }

    fn load_conversations(&mut self, cx: &mut Context<Self>) {
        let db = database::get_db(cx).clone();
        let (tx, rx) = async_channel::unbounded();

        Tokio::spawn(cx, async move {
            let result = db.list_conversations().await;
            let _ = tx.send(result).await;
        })
        .detach();

        cx.spawn(async move |this, cx| {
            if let Ok(Ok(conversations)) = rx.recv().await {
                let _ = cx.update(|app| {
                    let _ = this.update(app, |this, cx| {
                        this.conversations = conversations;
                        this.db_ready = true;
                        cx.notify();
                    });
                });
            }
        })
        .detach();
    }

    /// Reload conversations from database
    pub fn reload(&mut self, cx: &mut Context<Self>) {
        self.load_conversations(cx);
    }

    /// Select a conversation
    pub fn select_conversation(&mut self, id: Option<Uuid>, cx: &mut Context<Self>) {
        self.selected_conversation = id;
        cx.emit(ConversationSelectedEvent {
            conversation_id: id,
        });
        cx.notify();
    }

    fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        cx.emit(SidebarToggleEvent);
    }

    fn new_chat(&mut self, cx: &mut Context<Self>) {
        self.selected_conversation = None;
        cx.emit(NewChatEvent);
        cx.notify();
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
                            .xsmall()
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.new_chat(cx);
                            })),
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
    }

    fn render_tag(
        &self,
        label: &str,
        icon: &str,
        theme: &gpui_component::theme::Theme,
    ) -> impl IntoElement {
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

    /// Group conversations by date
    fn group_conversations(&self) -> Vec<(String, Vec<ChatHistoryItem>)> {
        let now = Local::now();
        let today = now.date_naive();
        let yesterday = today.pred_opt().unwrap_or(today);

        let mut today_items = Vec::new();
        let mut yesterday_items = Vec::new();
        let mut older_items = Vec::new();

        for conv in &self.conversations {
            let conv_date = conv.updated_at.with_timezone(&Local).date_naive();
            let item = ChatHistoryItem {
                id: conv.id,
                title: conv.title.clone(),
                provider_icon: get_provider_icon(&conv.provider_id),
            };

            if conv_date == today {
                today_items.push(item);
            } else if conv_date == yesterday {
                yesterday_items.push(item);
            } else {
                older_items.push(item);
            }
        }

        let mut groups = Vec::new();

        if !today_items.is_empty() {
            groups.push((t!("sidebar.today").to_string(), today_items));
        }
        if !yesterday_items.is_empty() {
            groups.push((t!("sidebar.yesterday").to_string(), yesterday_items));
        }
        if !older_items.is_empty() {
            groups.push((t!("sidebar.older").to_string(), older_items));
        }

        groups
    }

    fn render_history_list(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let selected = self.selected_conversation;
        let groups = self.group_conversations();

        if groups.is_empty() && self.db_ready {
            // Show empty state
            return v_flex()
                .flex_1()
                .items_center()
                .justify_center()
                .child(
                    Label::new(t!("sidebar.no_conversations").to_string())
                        .text_sm()
                        .text_color(theme.muted_foreground),
                )
                .into_any_element();
        }

        v_flex()
            .flex_1()
            .overflow_y_scrollbar()
            .children(groups.into_iter().map(|(group_name, items)| {
                v_flex()
                    .w_full()
                    .child(
                        h_flex().px_3().py_1().child(
                            Label::new(group_name)
                                .text_xs()
                                .text_color(theme.muted_foreground),
                        ),
                    )
                    .children(items.into_iter().enumerate().map(|(idx, item)| {
                        let is_selected = selected == Some(item.id);
                        let item_id = item.id;

                        ListItem::new(("history", idx))
                            .px_3()
                            .py_2()
                            .mx_2()
                            .my_px()
                            .rounded_md()
                            .selected(is_selected)
                            .on_click(cx.listener(move |this, _, _window, cx| {
                                this.select_conversation(Some(item_id), cx);
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
                                    .child(Label::new(item.title).text_sm().truncate()),
                            )
                    }))
            }))
            .into_any_element()
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

/// Get provider icon based on provider ID
fn get_provider_icon(provider_id: &str) -> String {
    match provider_id {
        "anthropic" => "A\\".to_string(),
        "openai" => "◉".to_string(),
        "google" => "G".to_string(),
        _ => "?".to_string(),
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
            // .child(self.render_quick_tags(cx))
            .child(self.render_history_list(cx))
            .child(self.render_user_info(cx))
    }
}
