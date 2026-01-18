#![feature(box_patterns)]

// SPDX-FileCopyrightText: 2026 AprilNEA LLC
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

#[macro_use]
extern crate rust_i18n;

i18n!("locales", fallback = "en");

mod app;
mod chat_sidebar;
mod chat_view;
mod llm_client;
mod message;
mod message_input;
mod message_list;
mod model_selector;
mod settings;

use gpui::*;
use gpui_component::Root;
use gpui_component_assets::Assets;

use settings::{open_settings_window, OpenSettings};

actions!(app, [Quit]);

fn main() {
    tracing_subscriber::fmt::init();

    // Set default locale
    rust_i18n::set_locale("zh-CN");

    let app = Application::new().with_assets(Assets);

    app.run(move |cx: &mut App| {
        gpui_component::init(cx);
        gpui_tokio_bridge::init(cx);
        settings::init(cx);

        // Register global actions
        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.on_action(|_: &OpenSettings, cx| open_settings_window(cx));

        // Bind keyboard shortcuts
        cx.bind_keys([
            KeyBinding::new("cmd-q", Quit, None),
            KeyBinding::new("cmd-,", OpenSettings, None),
        ]);

        // Set up menu bar
        cx.set_menus(vec![
            Menu {
                name: t!("app.name").to_string().into(),
                items: vec![
                    MenuItem::action(t!("app.about").to_string(), Quit),
                    MenuItem::separator(),
                    MenuItem::action(t!("app.settings").to_string(), OpenSettings),
                    MenuItem::separator(),
                    MenuItem::action(t!("app.quit").to_string(), Quit),
                ],
            },
        ]);

        cx.spawn(async move |cx| {
            cx.update(|cx| {
                let options = WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                        None,
                        size(px(900.), px(700.)),
                        cx,
                    ))),
                    titlebar: Some(TitlebarOptions {
                        appears_transparent: true,
                        traffic_light_position: Some(point(px(9.), px(9.))),
                        ..Default::default()
                    }),
                    ..Default::default()
                };

                cx.open_window(options, |window, cx| {
                    let view = cx.new(|cx| app::ChatApp::new(window, cx));
                    cx.new(|cx| Root::new(view, window, cx))
                })
                .expect("Failed to open window");

                cx.activate(true);
            })
        })
        .detach();
    });
}
