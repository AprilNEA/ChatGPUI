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
    menu::PopupMenu,
    v_flex, v_virtual_list,
};

use crate::settings::OpenSettings;
use gpui_tokio_bridge::Tokio;
use uuid::Uuid;

use crate::database::{self, conversation};

// Actions for conversation context menu
actions!(
    conversation_context,
    [
        RenameConversation,
        ToggleFavorite,
        GenerateTitle,
        CloneConversation,
        ToggleIcon,
        CopyConversationText,
        CopyConversationId,
        CopyAppLink,
        ExportJson,
        ExportMarkdown,
        ExportText,
        DeleteConversation,
        DeleteAllConversations,
        SetTitleLines1,
        SetTitleLines2,
        SetTitleLines3,
    ]
);

/// Event emitted when a conversation is selected
pub struct ConversationSelectedEvent {
    pub conversation_id: Option<Uuid>,
}

/// Context menu state for a conversation
#[allow(dead_code)]
struct ContextMenuState {
    conversation_id: Uuid,
    position: Point<Pixels>,
    menu: Entity<PopupMenu>,
    /// Subscription to dismiss event
    _subscription: Subscription,
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
        provider_id: String,
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
    /// Context menu state
    context_menu: Option<ContextMenuState>,
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
            context_menu: None,
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
                provider_id: conv.provider_id.clone(),
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

    /// Show context menu for a conversation
    fn show_context_menu(
        &mut self,
        conversation_id: Uuid,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        use gpui_component::menu::PopupMenuItem;

        // Create copies for each closure that needs the id
        let id_for_rename = conversation_id;
        let id_for_favorite = conversation_id;
        let id_for_generate = conversation_id;
        let id_for_clone = conversation_id;
        let id_for_icon = conversation_id;
        let id_for_copy_text = conversation_id;
        let id_for_copy_id = conversation_id.to_string();
        let id_for_copy_link = format!("chatgpui://conversation/{}", conversation_id);
        let id_for_export_json = conversation_id;
        let id_for_export_md = conversation_id;
        let id_for_export_txt = conversation_id;
        let id_for_delete = conversation_id;

        let menu = PopupMenu::build(window, cx, move |menu, window, cx| {
            menu.item(
                PopupMenuItem::new(t!("context.rename"))
                    .icon(IconName::Settings2)
                    .on_click(move |_, _, _cx| {
                        // TODO: Implement rename dialog
                        tracing::info!("Rename conversation: {}", id_for_rename);
                    }),
            )
            .item(
                PopupMenuItem::new(t!("context.favorite"))
                    .icon(IconName::Star)
                    .on_click(move |_, _, _cx| {
                        // TODO: Implement favorite toggle
                        tracing::info!("Toggle favorite: {}", id_for_favorite);
                    }),
            )
            .item(
                PopupMenuItem::new(t!("context.generate_title"))
                    .icon(IconName::Bot)
                    .on_click(move |_, _, _cx| {
                        // TODO: Implement title generation
                        tracing::info!("Generate title: {}", id_for_generate);
                    }),
            )
            .submenu(t!("context.title_max_lines"), window, cx, |menu, _, _| {
                menu.item(PopupMenuItem::new(t!("context.lines_1")).on_click(|_, _, _| {}))
                    .item(PopupMenuItem::new(t!("context.lines_2")).on_click(|_, _, _| {}))
                    .item(PopupMenuItem::new(t!("context.lines_3")).on_click(|_, _, _| {}))
            })
            .separator()
            .item(
                PopupMenuItem::new(t!("context.clone"))
                    .icon(IconName::Copy)
                    .on_click(move |_, _, _cx| {
                        // TODO: Implement clone
                        tracing::info!("Clone conversation: {}", id_for_clone);
                    }),
            )
            .item(
                PopupMenuItem::new(t!("context.hide_icon"))
                    .icon(IconName::EyeOff)
                    .on_click(move |_, _, _cx| {
                        // TODO: Implement hide icon
                        tracing::info!("Toggle icon: {}", id_for_icon);
                    }),
            )
            .separator()
            .item(
                PopupMenuItem::new(t!("context.copy_text"))
                    .icon(IconName::File)
                    .on_click(move |_, _, _cx| {
                        // TODO: Implement copy text
                        tracing::info!("Copy text: {}", id_for_copy_text);
                    }),
            )
            .item(
                PopupMenuItem::new(t!("context.copy_id"))
                    .icon(IconName::Copy)
                    .on_click({
                        let id_str = id_for_copy_id.clone();
                        move |_, _, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(id_str.clone()));
                        }
                    }),
            )
            .item(
                PopupMenuItem::new(t!("context.copy_link"))
                    .icon(IconName::ExternalLink)
                    .on_click({
                        let link = id_for_copy_link.clone();
                        move |_, _, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(link.clone()));
                        }
                    }),
            )
            .separator()
            .submenu(t!("context.export"), window, cx, move |menu, _, _| {
                menu.item(
                    PopupMenuItem::new(t!("context.export_json"))
                        .icon(IconName::File)
                        .on_click(move |_, _, _cx| {
                            // TODO: Implement JSON export
                            tracing::info!("Export JSON: {}", id_for_export_json);
                        }),
                )
                .item(
                    PopupMenuItem::new(t!("context.export_markdown"))
                        .icon(IconName::File)
                        .on_click(move |_, _, _cx| {
                            // TODO: Implement Markdown export
                            tracing::info!("Export Markdown: {}", id_for_export_md);
                        }),
                )
                .item(
                    PopupMenuItem::new(t!("context.export_txt"))
                        .icon(IconName::File)
                        .on_click(move |_, _, _cx| {
                            // TODO: Implement text export
                            tracing::info!("Export Text: {}", id_for_export_txt);
                        }),
                )
            })
            .separator()
            .item(
                PopupMenuItem::new(t!("context.delete"))
                    .icon(IconName::Delete)
                    .on_click(move |_, _, _cx| {
                        // TODO: Implement delete
                        tracing::info!("Delete conversation: {}", id_for_delete);
                    }),
            )
            .item(
                PopupMenuItem::new(t!("context.delete_all"))
                    .icon(IconName::Delete)
                    .on_click(|_, _, _cx| {
                        // TODO: Implement delete all
                        tracing::info!("Delete all conversations");
                    }),
            )
        });

        // Subscribe to dismiss event - store subscription to keep it alive
        let subscription = cx.subscribe_in(&menu, window, |this, _, _: &DismissEvent, _, cx| {
            this.context_menu = None;
            cx.notify();
        });

        // Focus the menu after two frames to ensure deferred rendering is complete
        // This is critical for smooth menu appearance (Zed's pattern)
        let focus_handle = menu.focus_handle(cx);
        window.on_next_frame(move |window, _app| {
            window.on_next_frame(move |window, _app| {
                focus_handle.focus(window);
            });
        });

        self.context_menu = Some(ContextMenuState {
            conversation_id,
            position,
            menu,
            _subscription: subscription,
        });

        cx.notify();
    }

    /// Close the context menu
    #[allow(dead_code)]
    fn close_context_menu(&mut self, cx: &mut Context<Self>) {
        self.context_menu = None;
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
                        use crate::assets::BrandAssets;

                        let theme = cx.theme();
                        let is_dark = theme.mode.is_dark();

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
                                        provider_id,
                                    } => {
                                        let is_selected = selected == Some(*id);
                                        let item_id = *id;
                                        let icon_path =
                                            BrandAssets::provider_icon(provider_id, is_dark);

                                        div()
                                            .id(SharedString::from(format!("conv-{}", item_id)))
                                            .w_full()
                                            .h(px(CONVERSATION_ITEM_HEIGHT))
                                            .px_2()
                                            .on_mouse_down(
                                                MouseButton::Right,
                                                cx.listener(
                                                    move |this, event: &MouseDownEvent, window, cx| {
                                                        this.show_context_menu(
                                                            item_id,
                                                            event.position,
                                                            window,
                                                            cx,
                                                        );
                                                    },
                                                ),
                                            )
                                            .child(
                                                ListItem::new(("history", ix))
                                                    .w_full()
                                                    .h_full()
                                                    .px_3()
                                                    .py_2()
                                                    .rounded_md()
                                                    .selected(is_selected)
                                                    .on_click(cx.listener(
                                                        move |this, _event: &ClickEvent, _window, cx| {
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
                                                            .child(if let Some(path) = icon_path {
                                                                svg()
                                                                    .path(path)
                                                                    .size(px(16.))
                                                                    .flex_shrink_0()
                                                                    .text_color(theme.foreground)
                                                                    .into_any_element()
                                                            } else {
                                                                h_flex()
                                                                    .size(px(16.))
                                                                    .rounded_sm()
                                                                    .bg(theme.muted)
                                                                    .items_center()
                                                                    .justify_center()
                                                                    .flex_shrink_0()
                                                                    .text_xs()
                                                                    .text_color(
                                                                        theme.muted_foreground,
                                                                    )
                                                                    .child(
                                                                        provider_id
                                                                            .chars()
                                                                            .next()
                                                                            .unwrap_or('?')
                                                                            .to_uppercase()
                                                                            .to_string(),
                                                                    )
                                                                    .into_any_element()
                                                            })
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

impl Render for ChatSidebar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        // Get context menu position and menu entity if open
        // Using deferred rendering with occlude() for better performance (Zed's pattern)
        let context_menu_element = self.context_menu.as_ref().map(|state| {
            let position = state.position;
            let menu = state.menu.clone();
            deferred(
                anchored()
                    .snap_to_window_with_margin(px(8.))
                    .anchor(Corner::TopLeft)
                    .position(position)
                    .child(
                        div()
                            .occlude() // Prevents rendering content behind the menu
                            .font_family(theme.font_family.clone())
                            .cursor_default()
                            .child(menu),
                    ),
            )
            .with_priority(1)
        });

        v_flex()
            .size_full()
            .bg(theme.sidebar)
            .border_r_1()
            .border_color(theme.border)
            .pt(px(44.)) // Space for traffic lights + toolbar buttons
            .child(self.render_search(cx))
            .child(self.render_history_list(cx))
            .child(self.render_user_info(cx))
            // Render context menu if open
            .children(context_menu_element)
    }
}
