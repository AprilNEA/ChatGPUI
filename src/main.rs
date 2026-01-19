#![feature(box_patterns)]

// SPDX-FileCopyrightText: 2026 AprilNEA LLC
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

#[macro_use]
extern crate rust_i18n;

i18n!("locales", fallback = "en");

mod about;
mod app;
mod assets;
mod chat_sidebar;
mod chat_view;
mod database;
mod llm;
mod message;
mod message_input;
mod message_list;
mod model_selector;
mod settings;

use gpui::*;
use gpui_component::Root;

use crate::assets::Assets;

use about::{OpenAbout, open_about_window};
use settings::{OpenSettings, open_settings_window};

actions!(app, [Quit]);

/// Set the application dock icon on macOS.
/// This is needed for `cargo run` since there's no app bundle with Info.plist.
#[cfg(target_os = "macos")]
fn set_dock_icon() {
    use cocoa::appkit::{NSApplication, NSImage};
    use cocoa::base::{id, nil};
    use cocoa::foundation::NSString;

    unsafe {
        let icon_path = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|p| p.to_path_buf()))
            .and_then(|dir| {
                // Check relative to executable first (for bundled app)
                let bundled = dir.join("../Resources/AppIcon.icns");
                if bundled.exists() {
                    return Some(bundled);
                }
                // Then check in project directory (for cargo run)
                let project = dir.join("../../bundle/AppIcon.icns");
                if project.exists() {
                    return Some(project);
                }
                None
            });

        if let Some(path) = icon_path {
            let path_str = path.to_string_lossy();
            let ns_path = NSString::alloc(nil).init_str(&path_str);
            let image: id = NSImage::alloc(nil).initWithContentsOfFile_(ns_path);

            if image != nil {
                let app = NSApplication::sharedApplication(nil);
                app.setApplicationIconImage_(image);
                tracing::debug!("Dock icon set from: {}", path_str);
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn set_dock_icon() {}

fn main() {
    tracing_subscriber::fmt::init();

    #[cfg(feature = "sentry")]
    let _guard = std::env::var("SENTRY_DSN").ok().map(|dsn| {
        sentry::init((
            dsn,
            sentry::ClientOptions {
                release: sentry::release_name!(),
                send_default_pii: false,
                ..Default::default()
            },
        ))
    });

    // Set default locale
    rust_i18n::set_locale("zh-CN");

    let app = Application::new().with_assets(Assets);

    app.run(move |cx: &mut App| {
        // Set dock icon for cargo run (no app bundle)
        set_dock_icon();

        gpui_component::init(cx);
        gpui_tokio_bridge::init(cx);
        settings::init(cx);
        database::init(cx);

        // Register global actions
        cx.on_action(|_: &Quit, cx| {
            // Shutdown database synchronously before quitting
            let db = database::get_db(cx).clone();
            db.shutdown_sync();
            cx.quit();
        });
        cx.on_action(|_: &OpenSettings, cx| open_settings_window(cx));
        cx.on_action(|_: &OpenAbout, cx| open_about_window(cx));

        // Bind keyboard shortcuts
        cx.bind_keys([
            KeyBinding::new("cmd-q", Quit, None),
            KeyBinding::new("cmd-,", OpenSettings, None),
        ]);

        // Set up menu bar
        cx.set_menus(vec![Menu {
            name: t!("app.name").to_string().into(),
            items: vec![
                MenuItem::action(t!("app.about").to_string(), OpenAbout),
                MenuItem::separator(),
                MenuItem::action(t!("app.settings").to_string(), OpenSettings),
                MenuItem::separator(),
                MenuItem::action(t!("app.quit").to_string(), Quit),
            ],
        }]);

        cx.spawn(async move |cx| {
            cx.update(|cx| {
                let options = WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                        None,
                        size(px(1200.), px(800.)),
                        cx,
                    ))),
                    titlebar: Some(TitlebarOptions {
                        appears_transparent: true,
                        traffic_light_position: Some(point(px(14.), px(12.))),
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
