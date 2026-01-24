// SPDX-FileCopyrightText: 2026 AprilNEA LLC
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

use chrono::Local;
use std::rc::Rc;

use gpui::*;
use gpui_component::{
    ActiveTheme, Icon, Sizable, Theme, ThemeMode, VirtualListScrollHandle,
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputState},
    label::Label,
    list::ListItem,
    menu::PopupMenu,
    v_flex, v_virtual_list,
};

use crate::database::{self, conversation};
use crate::icons::{AppIcon, ButtonAppIconExt};
use crate::settings::OpenSettings;
use gpui_tokio_bridge::Tokio;
use uuid::Uuid;

/// Event emitted when a conversation is selected
pub struct ConversationSelectedEvent {
    pub conversation_id: Option<Uuid>,
}

/// Event emitted when a conversation is deleted
pub struct ConversationDeletedEvent {
    pub conversation_id: Uuid,
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

/// Context menu state
struct ContextMenuState {
    /// The menu entity
    menu: Entity<PopupMenu>,
    /// Position where the menu was triggered
    position: Point<Pixels>,
    /// Subscription to dismiss event
    _subscription: Subscription,
}

/// State for inline editing a conversation title
struct EditingState {
    conversation_id: Uuid,
    input: Entity<InputState>,
}

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
    /// Context menu state (if open)
    context_menu: Option<ContextMenuState>,
    /// Inline editing state
    editing: Option<EditingState>,
}

impl EventEmitter<ConversationSelectedEvent> for ChatSidebar {}
impl EventEmitter<ConversationDeletedEvent> for ChatSidebar {}

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
                cx.update(|app| {
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
            editing: None,
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
                cx.update(|app| {
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

    /// Start renaming a conversation
    fn start_rename(&mut self, conversation_id: Uuid, window: &mut Window, cx: &mut Context<Self>) {
        // Get current title
        let current_title = self
            .conversations
            .iter()
            .find(|c| c.id == conversation_id)
            .map(|c| c.title.clone())
            .unwrap_or_default();

        // Create input state with current title
        let input = cx.new(|cx| {
            let mut state = InputState::new(window, cx);
            state.set_value(current_title, window, cx);
            state
        });

        // Focus the input on next frame
        let focus_handle = input.focus_handle(cx);
        window.on_next_frame(move |window, cx| {
            focus_handle.focus(window, cx);
        });

        self.editing = Some(EditingState {
            conversation_id,
            input,
        });
        cx.notify();
    }

    /// Save the renamed conversation title
    fn save_rename(&mut self, cx: &mut Context<Self>) {
        let Some(editing) = self.editing.take() else {
            return;
        };

        let new_title = editing.input.read(cx).value().to_string();
        let conversation_id = editing.conversation_id;

        // Don't save empty titles
        if new_title.trim().is_empty() {
            cx.notify();
            return;
        }

        // Update local list
        if let Some(conv) = self
            .conversations
            .iter_mut()
            .find(|c| c.id == conversation_id)
        {
            conv.title = new_title.clone();
        }
        self.rebuild_flat_list();
        cx.notify();

        // Update database asynchronously
        let db = database::get_db(cx).clone();
        Tokio::spawn(cx, async move {
            if let Err(e) = db
                .update_conversation_title(conversation_id, new_title)
                .await
            {
                tracing::error!("Failed to update conversation title: {}", e);
            }
        })
        .detach();
    }

    /// Cancel renaming
    fn cancel_rename(&mut self, cx: &mut Context<Self>) {
        self.editing = None;
        cx.notify();
    }

    /// Delete a conversation
    fn delete_conversation(&mut self, conversation_id: Uuid, cx: &mut Context<Self>) {
        let db = database::get_db(cx).clone();

        // If deleting current conversation, deselect it
        if self.selected_conversation == Some(conversation_id) {
            self.selected_conversation = None;
            cx.emit(ConversationSelectedEvent {
                conversation_id: None,
            });
        }

        // Remove from local list immediately for responsiveness
        self.conversations.retain(|c| c.id != conversation_id);
        self.rebuild_flat_list();
        cx.notify();

        // Emit deleted event
        cx.emit(ConversationDeletedEvent { conversation_id });

        // Delete from database asynchronously
        Tokio::spawn(cx, async move {
            if let Err(e) = db.delete_conversation(conversation_id).await {
                tracing::error!("Failed to delete conversation: {}", e);
            }
        })
        .detach();
    }

    /// Delete all conversations
    fn delete_all_conversations(&mut self, cx: &mut Context<Self>) {
        let db = database::get_db(cx).clone();
        let conversation_ids: Vec<Uuid> = self.conversations.iter().map(|c| c.id).collect();

        // Deselect current conversation
        self.selected_conversation = None;
        cx.emit(ConversationSelectedEvent {
            conversation_id: None,
        });

        // Clear local list immediately
        self.conversations.clear();
        self.rebuild_flat_list();
        cx.notify();

        // Delete from database asynchronously
        Tokio::spawn(cx, async move {
            for id in conversation_ids {
                if let Err(e) = db.delete_conversation(id).await {
                    tracing::error!("Failed to delete conversation {}: {}", id, e);
                }
            }
        })
        .detach();
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

        let entity = cx.entity().clone();
        let id_for_copy_id = conversation_id.to_string();
        let id_for_copy_link = format!("chatgpui://conversation/{}", conversation_id);

        let menu = PopupMenu::build(window, cx, move |menu, window, cx| {
            let entity_for_rename = entity.clone();
            let entity_for_delete = entity.clone();
            let entity_for_delete_all = entity.clone();

            menu.item(
                PopupMenuItem::new(t!("context.rename"))
                    .icon(Icon::new(AppIcon::Settings2))
                    .on_click(move |_, window, cx| {
                        entity_for_rename.update(cx, |this, cx| {
                            this.start_rename(conversation_id, window, cx);
                        });
                    }),
            )
            .item(
                PopupMenuItem::new(t!("context.favorite"))
                    .icon(Icon::new(AppIcon::Star))
                    .on_click(move |_, _, _cx| {
                        tracing::info!("Toggle favorite: {}", conversation_id);
                    }),
            )
            .item(
                PopupMenuItem::new(t!("context.generate_title"))
                    .icon(Icon::new(AppIcon::Bot))
                    .on_click(move |_, _, _cx| {
                        tracing::info!("Generate title: {}", conversation_id);
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
                    .icon(Icon::new(AppIcon::Copy))
                    .on_click(move |_, _, _cx| {
                        tracing::info!("Clone conversation: {}", conversation_id);
                    }),
            )
            .item(
                PopupMenuItem::new(t!("context.hide_icon"))
                    .icon(Icon::new(AppIcon::EyeOff))
                    .on_click(move |_, _, _cx| {
                        tracing::info!("Toggle icon: {}", conversation_id);
                    }),
            )
            .separator()
            .item(
                PopupMenuItem::new(t!("context.copy_text"))
                    .icon(Icon::new(AppIcon::File))
                    .on_click(move |_, _, _cx| {
                        tracing::info!("Copy text: {}", conversation_id);
                    }),
            )
            .item(
                PopupMenuItem::new(t!("context.copy_id"))
                    .icon(Icon::new(AppIcon::Copy))
                    .on_click({
                        let id_str = id_for_copy_id.clone();
                        move |_, _, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(id_str.clone()));
                        }
                    }),
            )
            .item(
                PopupMenuItem::new(t!("context.copy_link"))
                    .icon(Icon::new(AppIcon::ExternalLink))
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
                        .icon(Icon::new(AppIcon::File))
                        .on_click(move |_, _, _cx| {
                            tracing::info!("Export JSON: {}", conversation_id);
                        }),
                )
                .item(
                    PopupMenuItem::new(t!("context.export_markdown"))
                        .icon(Icon::new(AppIcon::File))
                        .on_click(move |_, _, _cx| {
                            tracing::info!("Export Markdown: {}", conversation_id);
                        }),
                )
                .item(
                    PopupMenuItem::new(t!("context.export_txt"))
                        .icon(Icon::new(AppIcon::File))
                        .on_click(move |_, _, _cx| {
                            tracing::info!("Export Text: {}", conversation_id);
                        }),
                )
            })
            .separator()
            .item(
                PopupMenuItem::new(t!("context.delete"))
                    .icon(Icon::new(AppIcon::Delete))
                    .on_click(move |_, _, cx| {
                        entity_for_delete.update(cx, |this, cx| {
                            this.delete_conversation(conversation_id, cx);
                        });
                    }),
            )
            .item(
                PopupMenuItem::new(t!("context.delete_all"))
                    .icon(Icon::new(AppIcon::Delete))
                    .on_click(move |_, _, cx| {
                        entity_for_delete_all.update(cx, |this, cx| {
                            this.delete_all_conversations(cx);
                        });
                    }),
            )
        });

        // Subscribe to dismiss event with double-frame focus scheduling (Zed's pattern)
        let focus_handle = menu.focus_handle(cx);
        let subscription =
            cx.subscribe_in(&menu, window, |this, _, _: &DismissEvent, _window, cx| {
                this.context_menu = None;
                cx.notify();
            });

        // Double-frame focus scheduling for smooth menu appearance
        window.on_next_frame(move |window, _cx| {
            window.on_next_frame(move |window, cx| {
                focus_handle.focus(window, cx);
            });
        });

        self.context_menu = Some(ContextMenuState {
            menu,
            position,
            _subscription: subscription,
        });
        cx.notify();
    }

    fn render_search(&mut self, _cx: &mut Context<Self>) -> impl IntoElement {
        h_flex().w_full().px_3().pb_2().child(
            Input::new(&self.search_input)
                .prefix(Icon::new(AppIcon::Search).size_4())
                .small(),
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
        let editing_id = self.editing.as_ref().map(|e| e.conversation_id);
        let editing_input = self.editing.as_ref().map(|e| e.input.clone());

        v_flex()
            .flex_1()
            .child(
                v_virtual_list(
                    cx.entity().clone(),
                    "conversation-list",
                    item_sizes,
                    move |_this, visible_range, _scroll_handle, cx| {
                        use crate::icons::{IntoIcon, LlmProvider};
                        use std::str::FromStr;

                        let theme = cx.theme();
                        let editing_input = editing_input.clone();

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
                                        let provider = LlmProvider::from_str(provider_id).ok();
                                        let is_editing = editing_id == Some(item_id);

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
                                                            // Cancel editing if clicking elsewhere
                                                            if this.editing.is_some() {
                                                                this.cancel_rename(cx);
                                                            }
                                                            this.select_conversation(
                                                                Some(item_id),
                                                                cx,
                                                            );
                                                        },
                                                    ))
                                                    .child(
                                                        h_flex()
                                                            .w_full()
                                                            .gap_2()
                                                            .items_center()
                                                            .overflow_hidden()
                                                            .child(if let Some(p) = provider {
                                                                p.icon()
                                                                    .size_4()
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
                                                            .child(if is_editing {
                                                                if let Some(ref input) = editing_input {
                                                                    // Wrap input in div for key handling
                                                                    div()
                                                                        .flex_1()
                                                                        .min_w_0()
                                                                        .on_key_down(cx.listener(
                                                                            |this, event: &KeyDownEvent, _, cx| {
                                                                                if event.keystroke.key == "enter" {
                                                                                    this.save_rename(cx);
                                                                                } else if event.keystroke.key == "escape" {
                                                                                    this.cancel_rename(cx);
                                                                                }
                                                                            },
                                                                        ))
                                                                        .child(
                                                                            Input::new(input)
                                                                                .appearance(false)
                                                                                .text_sm(),
                                                                        )
                                                                        .into_any_element()
                                                                } else {
                                                                    div()
                                                                        .flex_1()
                                                                        .min_w_0()
                                                                        .truncate()
                                                                        .child(Label::new(title.clone()).text_sm())
                                                                        .into_any_element()
                                                                }
                                                            } else {
                                                                div()
                                                                    .flex_1()
                                                                    .min_w_0()
                                                                    .truncate()
                                                                    .child(Label::new(title.clone()).text_sm())
                                                                    .into_any_element()
                                                            }),
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
                                Icon::new(AppIcon::Sun)
                            } else {
                                Icon::new(AppIcon::Moon)
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
                            .app_icon(AppIcon::Settings)
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
            .pt(px(44.)) // Space for traffic lights + toolbar buttons
            .child(self.render_search(cx))
            .child(self.render_history_list(cx))
            .child(self.render_user_info(cx))
            // Render context menu if open
            .children(context_menu_element)
    }
}
