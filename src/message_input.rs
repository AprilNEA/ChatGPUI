// SPDX-FileCopyrightText: 2026 AprilNEA LLC
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::{
    ActiveTheme, Disableable, IconName, IconNamed, Sizable,
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputEvent, InputState},
    v_flex,
};

use crate::message::Attachment;

const MAX_IMAGE_SIZE: usize = 20 * 1024 * 1024; // 20MB

pub struct MessageInput {
    input_state: Entity<InputState>,
    is_loading: bool,
    pending_newline: bool,
    attachments: Vec<Attachment>,
}

pub struct SubmitEvent {
    pub content: String,
    pub attachments: Vec<Attachment>,
}

impl EventEmitter<SubmitEvent> for MessageInput {}

impl MessageInput {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input_state = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Type your message...")
                .clean_on_escape()
                .auto_grow(1, 10) // Support multiline display
        });

        cx.subscribe_in(
            &input_state,
            window,
            |this, _, event: &InputEvent, window, cx| {
                if let InputEvent::PressEnter { secondary } = event {
                    if *secondary {
                        // Cmd+Enter: insert newline (component already inserted it)
                        // Do nothing, keep the newline
                    } else if this.pending_newline {
                        // Shift+Enter was pressed, keep the newline
                        this.pending_newline = false;
                    } else {
                        // Plain Enter: submit message
                        // Remove the trailing newline that was just inserted by the component
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
        }
    }

    fn handle_shift_enter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.pending_newline = true;
        // Manually insert newline
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
            .when(has_attachments, |this: Div| {
                this.child(self.render_attachment_preview(cx))
            })
            .child(
                h_flex()
                    .gap_3()
                    .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                        if event.keystroke.key == "enter" && event.keystroke.modifiers.shift {
                            this.handle_shift_enter(window, cx);
                        }
                    }))
                    .child(
                        Button::new("attachment")
                            .icon(IconName::Plus)
                            .large()
                            .ghost()
                            .disabled(is_loading)
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.handle_attachment_click(cx);
                            })),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .child(Input::new(&self.input_state).large().disabled(is_loading)),
                    )
                    .child(
                        Button::new("send")
                            .icon(IconName::ArrowRight)
                            .large()
                            .primary()
                            .loading(is_loading)
                            .disabled(is_loading)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.handle_submit(window, cx);
                            })),
                    ),
            )
    }
}

impl MessageInput {
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
                                    .xsmall()
                                    .on_click(cx.listener(move |this, _, _window, cx| {
                                        this.remove_attachment(id, cx);
                                    })),
                            ),
                    )
            }))
    }
}
