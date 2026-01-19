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
mod storage;

use std::path::PathBuf;

use gpui::*;
use gpui_component::{Root, Theme, ThemeRegistry};

use crate::assets::Assets;

use about::{OpenAbout, open_about_window};
use settings::{OpenSettings, open_settings_window};

actions!(
    app,
    [Quit, NewChat, Undo, Redo, Cut, Copy, Paste, SelectAll]
);

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

        // Load themes from themes directory
        let theme_name = SharedString::from("macOS Classic Dark");
        if let Err(err) = ThemeRegistry::watch_dir(PathBuf::from("./themes"), cx, move |cx| {
            if let Some(theme) = ThemeRegistry::global(cx).themes().get(&theme_name).cloned() {
                Theme::global_mut(cx).apply_config(&theme);
            }
        }) {
            tracing::error!("Failed to watch themes directory: {}", err);
        }

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
            KeyBinding::new("cmd-n", NewChat, None),
        ]);

        // Set up menu bar
        cx.set_menus(vec![
            // Application menu (macOS)
            Menu {
                name: t!("app.name").to_string().into(),
                items: vec![
                    MenuItem::action(t!("menu.about").to_string(), OpenAbout),
                    MenuItem::separator(),
                    MenuItem::action(t!("menu.settings").to_string(), OpenSettings),
                    MenuItem::separator(),
                    MenuItem::os_submenu(t!("menu.services").to_string(), SystemMenuType::Services),
                    MenuItem::separator(),
                    MenuItem::action(t!("menu.quit").to_string(), Quit),
                ],
            },
            // File menu
            Menu {
                name: t!("menu.file").to_string().into(),
                items: vec![MenuItem::action(t!("menu.new_chat").to_string(), NewChat)],
            },
            // Edit menu
            Menu {
                name: t!("menu.edit").to_string().into(),
                items: vec![
                    MenuItem::os_action(t!("menu.undo").to_string(), Undo, OsAction::Undo),
                    MenuItem::os_action(t!("menu.redo").to_string(), Redo, OsAction::Redo),
                    MenuItem::separator(),
                    MenuItem::os_action(t!("menu.cut").to_string(), Cut, OsAction::Cut),
                    MenuItem::os_action(t!("menu.copy").to_string(), Copy, OsAction::Copy),
                    MenuItem::os_action(t!("menu.paste").to_string(), Paste, OsAction::Paste),
                    MenuItem::os_action(
                        t!("menu.select_all").to_string(),
                        SelectAll,
                        OsAction::SelectAll,
                    ),
                ],
            },
            // Help menu
            Menu {
                name: t!("menu.help").to_string().into(),
                items: vec![MenuItem::action(
                    t!("menu.documentation").to_string(),
                    OpenAbout,
                )],
            },
        ]);

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
                        traffic_light_position: Some(point(px(14.), px(14.))),
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
