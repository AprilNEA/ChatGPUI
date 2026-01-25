// SPDX-FileCopyrightText: 2026 AprilNEA LLC
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

use gpui::{
    App, AppContext, Bounds, Context, IntoElement, ParentElement, Render, Styled, TitlebarOptions,
    Window, WindowBounds, WindowKind, WindowOptions, actions, px, size,
};
use gpui_component::{ActiveTheme, Root, h_flex, label::Label, v_flex};

#[cfg(not(debug_assertions))]
shadow_rs::shadow!(build);

#[cfg(debug_assertions)]
mod build {
    pub const PKG_VERSION: &str = env!("CARGO_PKG_VERSION");
    pub const SHORT_COMMIT: &str = "dev";
    pub const BUILD_TIME_3339: &str = "debug build";
}

actions!(about, [OpenAbout]);

/// About window view
pub struct AboutView;

impl AboutView {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self
    }
}

impl Render for AboutView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        v_flex()
            .size_full()
            .bg(theme.background)
            .items_center()
            .justify_center()
            .gap_4()
            // Logo
            .child(
                h_flex()
                    .size(px(80.))
                    .items_center()
                    .justify_center()
                    .child(gpui::img("chatgpui.png").size(px(80.))),
            )
            // App name
            .child(
                Label::new("ChatGPUI")
                    .text_xl()
                    .text_color(theme.foreground),
            )
            // Version
            .child(
                Label::new(format!("{} {}", t!("about.version"), build::PKG_VERSION))
                    .text_sm()
                    .text_color(theme.muted_foreground),
            )
            // Build info
            .child(
                v_flex()
                    .gap_1()
                    .items_center()
                    .child(
                        Label::new(format!(
                            "{}: {}",
                            t!("about.commit"),
                            &build::SHORT_COMMIT[..7.min(build::SHORT_COMMIT.len())]
                        ))
                        .text_xs()
                        .text_color(theme.muted_foreground),
                    )
                    .child(
                        Label::new(format!(
                            "{}: {}",
                            t!("about.build_time"),
                            build::BUILD_TIME_3339
                        ))
                        .text_xs()
                        .text_color(theme.muted_foreground),
                    ),
            )
            // Copyright
            .child(
                Label::new(t!("about.copyright").to_string())
                    .text_xs()
                    .text_color(theme.muted_foreground),
            )
    }
}

/// Open the about window
pub fn open_about_window(cx: &mut App) {
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
            None,
            size(px(320.), px(280.)),
            cx,
        ))),
        titlebar: Some(TitlebarOptions {
            title: Some(t!("app.about").to_string().into()),
            ..Default::default()
        }),
        kind: WindowKind::Normal,
        ..Default::default()
    };

    cx.open_window(options, |window, cx| {
        let view = cx.new(|cx| AboutView::new(window, cx));
        cx.new(|cx| Root::new(view, window, cx))
    })
    .expect("Failed to open about window");
}
