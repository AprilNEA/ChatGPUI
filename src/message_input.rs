// SPDX-FileCopyrightText: 2026 AprilNEA LLC
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::{
    ActiveTheme, Disableable, IconName, IconNamed, Selectable, Sizable,
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputEvent, InputState},
    menu::{DropdownMenu, PopupMenuItem},
    v_flex,
};

use crate::message::Attachment;

const MAX_IMAGE_SIZE: usize = 20 * 1024 * 1024; // 20MB

/// 推理级别
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReasoningLevel {
    #[default]
    Off,
    Low,
    Medium,
    High,
}

impl ReasoningLevel {
    pub fn label(&self) -> &'static str {
        match self {
            ReasoningLevel::Off => "关闭",
            ReasoningLevel::Low => "低",
            ReasoningLevel::Medium => "中",
            ReasoningLevel::High => "高",
        }
    }

    #[allow(dead_code)]
    pub fn icon(&self) -> IconName {
        match self {
            ReasoningLevel::Off => IconName::CircleX,
            ReasoningLevel::Low => IconName::Loader,
            ReasoningLevel::Medium => IconName::Loader,
            ReasoningLevel::High => IconName::Loader,
        }
    }
}

pub struct MessageInput {
    input_state: Entity<InputState>,
    is_loading: bool,
    pending_newline: bool,
    attachments: Vec<Attachment>,
    // 工具栏状态
    web_search_enabled: bool,
    artifacts_enabled: bool,
    reasoning_level: ReasoningLevel,
    image_gen_enabled: bool,
    mcp_enabled: bool,
}

#[allow(dead_code)]
pub struct SubmitEvent {
    pub content: String,
    pub attachments: Vec<Attachment>,
    pub web_search: bool,
    pub artifacts: bool,
    pub reasoning_level: ReasoningLevel,
    pub image_gen: bool,
}

impl EventEmitter<SubmitEvent> for MessageInput {}

impl MessageInput {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input_state = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Type your message...")
                .clean_on_escape()
                .auto_grow(1, 10)
        });

        cx.subscribe_in(
            &input_state,
            window,
            |this, _, event: &InputEvent, window, cx| {
                if let InputEvent::PressEnter { secondary } = event {
                    if *secondary {
                        // Cmd+Enter: insert newline
                    } else if this.pending_newline {
                        this.pending_newline = false;
                    } else {
                        // Plain Enter: submit
                        this.input_state.update(cx, |state, cx| {
                            let value = state.value().to_string();
                            if value.ends_with('\n') {
                                let trimmed = value[..value.len() - 1].to_string();
                                state.set_value(trimmed, window, cx);
                            }
                        });
                        this.handle_submit(window, cx);
                    }
                }
            },
        )
        .detach();

        Self {
            input_state,
            is_loading: false,
            pending_newline: false,
            attachments: Vec::new(),
            web_search_enabled: false,
            artifacts_enabled: false,
            reasoning_level: ReasoningLevel::Off,
            image_gen_enabled: false,
            mcp_enabled: false,
        }
    }

    fn handle_shift_enter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.pending_newline = true;
        self.input_state.update(cx, |state, cx| {
            state.insert("\n", window, cx);
        });
        cx.notify();
    }

    pub fn set_loading(&mut self, loading: bool, cx: &mut Context<Self>) {
        self.is_loading = loading;
        cx.notify();
    }

    pub fn clear(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.input_state.update(cx, |state, cx| {
            state.set_value("", window, cx);
        });
        self.attachments.clear();
    }

    fn handle_submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_loading {
            return;
        }

        let value = self.input_state.read(cx).value().to_string();
        if value.trim().is_empty() && self.attachments.is_empty() {
            return;
        }

        let attachments = std::mem::take(&mut self.attachments);
        cx.emit(SubmitEvent {
            content: value,
            attachments,
            web_search: self.web_search_enabled,
            artifacts: self.artifacts_enabled,
            reasoning_level: self.reasoning_level,
            image_gen: self.image_gen_enabled,
        });
        self.clear(window, cx);
    }

    fn handle_attachment_click(&mut self, cx: &mut Context<Self>) {
        let dialog = rfd::AsyncFileDialog::new()
            .add_filter("Images", &["png", "jpg", "jpeg", "gif", "webp"])
            .pick_files();

        cx.spawn(async move |this, cx| {
            if let Some(files) = dialog.await {
                for file in files {
                    let data = file.read().await;
                    let name = file.file_name();

                    if data.len() > MAX_IMAGE_SIZE {
                        tracing::warn!("Image too large: {} ({} bytes)", name, data.len());
                        continue;
                    }

                    let mime_type = mime_from_extension(&name);
                    let attachment = Attachment::new_image(name, mime_type, data);

                    let _ = cx.update(|app| {
                        let _ = this.update(app, |this, cx| {
                            this.attachments.push(attachment);
                            cx.notify();
                        });
                    });
                }
            }
        })
        .detach();
    }

    fn remove_attachment(&mut self, id: uuid::Uuid, cx: &mut Context<Self>) {
        self.attachments.retain(|a| a.id != id);
        cx.notify();
    }

    fn toggle_web_search(&mut self, cx: &mut Context<Self>) {
        self.web_search_enabled = !self.web_search_enabled;
        cx.notify();
    }

    fn toggle_artifacts(&mut self, cx: &mut Context<Self>) {
        self.artifacts_enabled = !self.artifacts_enabled;
        cx.notify();
    }

    fn set_reasoning_level(&mut self, level: ReasoningLevel, cx: &mut Context<Self>) {
        self.reasoning_level = level;
        cx.notify();
    }

    fn toggle_image_gen(&mut self, cx: &mut Context<Self>) {
        self.image_gen_enabled = !self.image_gen_enabled;
        cx.notify();
    }

    fn toggle_mcp(&mut self, cx: &mut Context<Self>) {
        self.mcp_enabled = !self.mcp_enabled;
        cx.notify();
    }
}

fn mime_from_extension(filename: &str) -> String {
    let ext = filename
        .rsplit('.')
        .next()
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        _ => "application/octet-stream",
    }
    .to_string()
}

impl Render for MessageInput {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let is_loading = self.is_loading;
        let has_attachments = !self.attachments.is_empty();

        v_flex()
            .gap_2()
            .border_t_1()
            .border_color(theme.border)
            .bg(theme.background)
            .p_4()
            // 附件预览
            .when(has_attachments, |this: Div| {
                this.child(self.render_attachment_preview(cx))
            })
            // 输入框
            .child(
                div()
                    .w_full()
                    .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                        if event.keystroke.key == "enter" && event.keystroke.modifiers.shift {
                            this.handle_shift_enter(window, cx);
                        }
                    }))
                    .child(Input::new(&self.input_state).w_full().disabled(is_loading)),
            )
            // 底部工具栏
            .child(self.render_toolbar(cx))
    }
}

impl MessageInput {
    fn render_toolbar(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let is_loading = self.is_loading;

        h_flex()
            .justify_between()
            .items_center()
            .child(
                h_flex()
                    .gap_1()
                    // 附件按钮
                    .child(
                        Button::new("attachment")
                            .icon(IconName::Plus)
                            .ghost()
                            .disabled(is_loading)
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.handle_attachment_click(cx);
                            })),
                    )
                    // 搜索开关
                    .child(self.render_toggle_button(
                        "web-search",
                        IconName::Globe,
                        self.web_search_enabled,
                        is_loading,
                        cx.listener(|this, _, _window, cx| {
                            this.toggle_web_search(cx);
                        }),
                    ))
                    // Artifacts 开关
                    .child(self.render_toggle_button(
                        "artifacts",
                        IconName::Frame,
                        self.artifacts_enabled,
                        is_loading,
                        cx.listener(|this, _, _window, cx| {
                            this.toggle_artifacts(cx);
                        }),
                    ))
                    // 推理级别下拉菜单
                    .child(self.render_reasoning_dropdown(cx))
                    // 图像生成开关
                    .child(self.render_toggle_button(
                        "image-gen",
                        IconName::Palette,
                        self.image_gen_enabled,
                        is_loading,
                        cx.listener(|this, _, _window, cx| {
                            this.toggle_image_gen(cx);
                        }),
                    ))
                    // MCP 工具开关（占位符）
                    .child(self.render_toggle_button(
                        "mcp",
                        IconName::SquareTerminal,
                        self.mcp_enabled,
                        is_loading,
                        cx.listener(|this, _, _window, cx| {
                            this.toggle_mcp(cx);
                        }),
                    )),
            )
            // 发送按钮
            .child(
                Button::new("send")
                    .icon(IconName::ArrowRight)
                    .small()
                    .primary()
                    .loading(is_loading)
                    .disabled(is_loading)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.handle_submit(window, cx);
                    })),
            )
    }

    fn render_toggle_button(
        &self,
        id: &'static str,
        icon: IconName,
        enabled: bool,
        is_loading: bool,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> impl IntoElement {
        Button::new(id)
            .icon(icon)
            .ghost()
            .selected(enabled)
            .disabled(is_loading)
            .on_click(handler)
    }

    fn render_reasoning_dropdown(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let current_level = self.reasoning_level;
        let is_enabled = current_level != ReasoningLevel::Off;
        let entity = cx.entity().clone();

        Button::new("reasoning")
            .icon(IconName::Bot)
            .ghost()
            .selected(is_enabled)
            .child(current_level.label())
            .dropdown_menu(move |menu, _window, _cx| {
                let entity_off = entity.clone();
                let entity_low = entity.clone();
                let entity_med = entity.clone();
                let entity_high = entity.clone();

                menu.item(
                    PopupMenuItem::new("关闭")
                        .icon(IconName::CircleX)
                        .checked(current_level == ReasoningLevel::Off)
                        .on_click(move |_, _window, cx| {
                            entity_off.update(cx, |this, cx| {
                                this.set_reasoning_level(ReasoningLevel::Off, cx);
                            });
                        }),
                )
                .item(
                    PopupMenuItem::new("低")
                        .icon(IconName::Loader)
                        .checked(current_level == ReasoningLevel::Low)
                        .on_click(move |_, _window, cx| {
                            entity_low.update(cx, |this, cx| {
                                this.set_reasoning_level(ReasoningLevel::Low, cx);
                            });
                        }),
                )
                .item(
                    PopupMenuItem::new("中")
                        .icon(IconName::Loader)
                        .checked(current_level == ReasoningLevel::Medium)
                        .on_click(move |_, _window, cx| {
                            entity_med.update(cx, |this, cx| {
                                this.set_reasoning_level(ReasoningLevel::Medium, cx);
                            });
                        }),
                )
                .item(
                    PopupMenuItem::new("高")
                        .icon(IconName::Loader)
                        .checked(current_level == ReasoningLevel::High)
                        .on_click(move |_, _window, cx| {
                            entity_high.update(cx, |this, cx| {
                                this.set_reasoning_level(ReasoningLevel::High, cx);
                            });
                        }),
                )
            })
    }

    fn render_attachment_preview(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        h_flex()
            .gap_2()
            .flex_wrap()
            .children(self.attachments.iter().map(|attachment| {
                let id = attachment.id;
                let name = attachment.name.clone();

                div()
                    .id(ElementId::Name(format!("attachment-{}", id).into()))
                    .relative()
                    .rounded_md()
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.muted)
                    .p_2()
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                svg()
                                    .path(IconName::File.path())
                                    .size_4()
                                    .text_color(theme.muted_foreground),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(theme.foreground)
                                    .max_w(px(120.))
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .child(name),
                            )
                            .child(
                                Button::new(SharedString::from(format!("remove-{}", id)))
                                    .icon(IconName::Close)
                                    .ghost()
                                    .on_click(cx.listener(move |this, _, _window, cx| {
                                        this.remove_attachment(id, cx);
                                    })),
                            ),
                    )
            }))
    }
}
