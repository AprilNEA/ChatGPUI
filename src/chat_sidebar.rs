// SPDX-FileCopyrightText: 2026 AprilNEA LLC
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

use chrono::Local;
use std::rc::Rc;

use gpui::*;
use gpui_component::{
    ActiveTheme, Icon, IconName, Theme, ThemeMode, VirtualListScrollHandle,
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputState},
    label::Label,
    list::ListItem,
    v_flex, v_virtual_list,
};

use crate::settings::OpenSettings;
use gpui_tokio_bridge::Tokio;
use uuid::Uuid;

use crate::database::{self, conversation};

/// Event emitted when a conversation is selected
pub struct ConversationSelectedEvent {
    pub conversation_id: Option<Uuid>,
}

/// Virtual list item type
#[derive(Clone)]
enum ListItemKind {
    /// Group header (Today, Yesterday, Older)
    GroupHeader(String),
    /// Conversation item
    Conversation {
        id: Uuid,
        title: String,
        provider_icon: &'static str,
    },
}

/// Fixed heights for virtual list items
const GROUP_HEADER_HEIGHT: f32 = 28.0;
const CONVERSATION_ITEM_HEIGHT: f32 = 40.0;

/// Chat history sidebar component
pub struct ChatSidebar {
    search_input: Entity<InputState>,
    conversations: Vec<conversation::Model>,
    selected_conversation: Option<Uuid>,
    db_ready: bool,
    /// Flattened list items for virtual list
    flat_items: Vec<ListItemKind>,
    /// Pre-calculated sizes for virtual list
    item_sizes: Rc<Vec<Size<Pixels>>>,
    scroll_handle: VirtualListScrollHandle,
}

impl EventEmitter<ConversationSelectedEvent> for ChatSidebar {}

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
                        this.rebuild_flat_list();
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
            flat_items: Vec::new(),
            item_sizes: Rc::new(Vec::new()),
            scroll_handle: VirtualListScrollHandle::new(),
        }
    }

    /// Rebuild flat list and sizes from conversations
    fn rebuild_flat_list(&mut self) {
        let now = Local::now();
        let today = now.date_naive();
        let yesterday = today.pred_opt().unwrap_or(today);

        let mut today_items = Vec::new();
        let mut yesterday_items = Vec::new();
        let mut older_items = Vec::new();

        for conv in &self.conversations {
            let conv_date = conv.updated_at.with_timezone(&Local).date_naive();
            let item = ListItemKind::Conversation {
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

        let mut flat_items = Vec::new();
        let mut sizes = Vec::new();

        if !today_items.is_empty() {
            flat_items.push(ListItemKind::GroupHeader(t!("sidebar.today").to_string()));
            sizes.push(size(px(260.), px(GROUP_HEADER_HEIGHT)));
            for item in today_items {
                flat_items.push(item);
                sizes.push(size(px(260.), px(CONVERSATION_ITEM_HEIGHT)));
            }
        }

        if !yesterday_items.is_empty() {
            flat_items.push(ListItemKind::GroupHeader(
                t!("sidebar.yesterday").to_string(),
            ));
            sizes.push(size(px(260.), px(GROUP_HEADER_HEIGHT)));
            for item in yesterday_items {
                flat_items.push(item);
                sizes.push(size(px(260.), px(CONVERSATION_ITEM_HEIGHT)));
            }
        }

        if !older_items.is_empty() {
            flat_items.push(ListItemKind::GroupHeader(t!("sidebar.older").to_string()));
            sizes.push(size(px(260.), px(GROUP_HEADER_HEIGHT)));
            for item in older_items {
                flat_items.push(item);
                sizes.push(size(px(260.), px(CONVERSATION_ITEM_HEIGHT)));
            }
        }

        self.flat_items = flat_items;
        self.item_sizes = Rc::new(sizes);
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
                        this.rebuild_flat_list();
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

    pub fn conversation_title(&self, id: Uuid) -> Option<String> {
        self.conversations
            .iter()
            .find(|conv| conv.id == id)
            .map(|conv| conv.title.clone())
    }

    /// Select a conversation
    pub fn select_conversation(&mut self, id: Option<Uuid>, cx: &mut Context<Self>) {
        self.selected_conversation = id;
        cx.emit(ConversationSelectedEvent {
            conversation_id: id,
        });
        cx.notify();
    }

    fn render_search(&mut self, _cx: &mut Context<Self>) -> impl IntoElement {
        h_flex().w_full().px_3().pb_2().child(
            Input::new(&self.search_input)
                .prefix(Icon::new(IconName::Search).size_6())
                .appearance(false),
        )
    }

    fn render_history_list(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        if self.flat_items.is_empty() && self.db_ready {
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

        let selected = self.selected_conversation;
        let items = self.flat_items.clone();
        let item_sizes = self.item_sizes.clone();

        v_flex()
            .flex_1()
            .child(
                v_virtual_list(
                    cx.entity().clone(),
                    "conversation-list",
                    item_sizes,
                    move |_this, visible_range, _scroll_handle, cx| {
                        let theme = cx.theme();
                        visible_range
                            .map(|ix| {
                                let item = &items[ix];
                                match item {
                                    ListItemKind::GroupHeader(name) => div()
                                        .w_full()
                                        .h(px(GROUP_HEADER_HEIGHT))
                                        .child(
                                            h_flex().w_full().h_full().px_3().items_center().child(
                                                Label::new(name.clone())
                                                    .text_xs()
                                                    .text_color(theme.muted_foreground),
                                            ),
                                        )
                                        .into_any_element(),
                                    ListItemKind::Conversation {
                                        id,
                                        title,
                                        provider_icon,
                                    } => {
                                        let is_selected = selected == Some(*id);
                                        let item_id = *id;

                                        div()
                                            .w_full()
                                            .h(px(CONVERSATION_ITEM_HEIGHT))
                                            .px_2() // 父容器用 padding 代替子元素 margin
                                            .child(
                                                ListItem::new(("history", ix))
                                                    .w_full()
                                                    .h_full()
                                                    .px_3()
                                                    .py_2()
                                                    .rounded_md()
                                                    .selected(is_selected)
                                                    .on_click(cx.listener(
                                                        move |this, _, _window, cx| {
                                                            this.select_conversation(
                                                                Some(item_id),
                                                                cx,
                                                            );
                                                        },
                                                    ))
                                                    .child(
                                                        h_flex()
                                                            .gap_2()
                                                            .items_center()
                                                            .overflow_hidden()
                                                            .child(
                                                                svg()
                                                                    .path(*provider_icon)
                                                                    .size(px(16.))
                                                                    .flex_shrink_0()
                                                                    .text_color(theme.foreground),
                                                            )
                                                            .child(
                                                                Label::new(title.clone())
                                                                    .text_sm()
                                                                    .truncate(),
                                                            ),
                                                    ),
                                            )
                                            .into_any_element()
                                    }
                                }
                            })
                            .collect()
                    },
                )
                .w_full()
                .flex_1()
                .track_scroll(&self.scroll_handle),
            )
            .into_any_element()
    }

    fn render_user_info(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let is_dark = theme.mode.is_dark();

        h_flex()
            .w_full()
            .px_3()
            .py_3()
            .gap_2()
            .items_center()
            .justify_between()
            .border_t_1()
            .border_color(theme.border)
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(
                        h_flex()
                            .size_8()
                            .rounded_full()
                            .bg(theme.primary)
                            .items_center()
                            .justify_center()
                            .text_sm()
                            .text_color(theme.primary_foreground)
                            .child("A"),
                    )
                    .child(Label::new("AprilNEA").text_sm()),
            )
            .child(
                h_flex()
                    .gap_1()
                    .items_center()
                    // Theme toggle button
                    .child(
                        Button::new("theme-toggle")
                            .icon(if is_dark {
                                IconName::Sun
                            } else {
                                IconName::Moon
                            })
                            .ghost()
                            .on_click(|_, window, cx| {
                                let current_mode = cx.theme().mode;
                                let new_mode = if current_mode.is_dark() {
                                    ThemeMode::Light
                                } else {
                                    ThemeMode::Dark
                                };
                                Theme::change(new_mode, Some(window), cx);
                            }),
                    )
                    // Settings button
                    .child(
                        Button::new("settings")
                            .icon(IconName::Settings)
                            .ghost()
                            .on_click(|_, window, cx| {
                                window.dispatch_action(Box::new(OpenSettings), cx);
                            }),
                    ),
            )
    }
}

/// Get provider icon path based on provider ID
fn get_provider_icon(provider_id: &str) -> &'static str {
    match provider_id {
        "anthropic" => "icons/brand/anthropic.svg",
        "openai" => "icons/brand/openai.svg",
        "google" => "icons/brand/google.svg",
        _ => "chatgpui.svg",
    }
}

impl Render for ChatSidebar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        v_flex()
            .size_full()
            .bg(theme.sidebar)
            .border_r_1()
            .border_color(theme.border)
            .pt(px(44.)) // Space for traffic lights + toolbar buttons
            .child(self.render_search(cx))
            .child(self.render_history_list(cx))
            .child(self.render_user_info(cx))
    }
}
