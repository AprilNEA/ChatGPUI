// SPDX-FileCopyrightText: 2026 AprilNEA LLC
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

use shadow_rs::ShadowBuilder;
use std::fs;
use std::path::Path;

/// List of themes to download from gpui-component repository
const THEMES: &[&str] = &[
    "adventure",
    "alduin",
    "ayu",
    "catppuccin",
    "everforest",
    "fahrenheit",
    "flexoki",
    "gruvbox",
    "harper",
    "hybrid",
    "jellybeans",
    "kibble",
    "macos-classic",
    "matrix",
    "mellifluous",
    "molokai",
    "solarized",
    "spaceduck",
    "tokyonight",
    "twilight",
];

const THEME_BASE_URL: &str =
    "https://raw.githubusercontent.com/longbridge/gpui-component/main/themes";

fn download_themes() {
    let themes_dir = Path::new("themes");

    // Create themes directory if it doesn't exist
    if !themes_dir.exists() {
        fs::create_dir_all(themes_dir).expect("Failed to create themes directory");
    }

    for theme_name in THEMES {
        let theme_file = themes_dir.join(format!("{}.json", theme_name));

        // Skip if file already exists
        if theme_file.exists() {
            continue;
        }

        let url = format!("{}/{}.json", THEME_BASE_URL, theme_name);
        println!("cargo:warning=Downloading theme: {}", theme_name);

        match ureq::get(&url).call() {
            Ok(response) => {
                let body = response
                    .into_string()
                    .expect("Failed to read theme response");
                fs::write(&theme_file, body).expect("Failed to write theme file");
            }
            Err(e) => {
                println!("cargo:warning=Failed to download {}: {}", theme_name, e);
            }
        }
    }
}

fn main() {
    // Declare rerun conditions to prevent unnecessary rebuilds
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=themes");

    // Download themes if missing
    download_themes();

    let is_release = std::env::var("PROFILE").unwrap_or_default() != "debug";

    if is_release {
        // Only read metadata in release mode (for bundle identifier)
        let identifier = cargo_metadata::MetadataCommand::new()
            .no_deps()
            .exec()
            .ok()
            .and_then(|metadata| metadata.packages.into_iter().find(|p| p.name == "chatgpui"))
            .and_then(|package| {
                package
                    .metadata
                    .get("bundle")
                    .and_then(|b| b.get("identifier"))
                    .and_then(|i| i.as_str())
                    .map(|s| s.to_string())
            })
            .unwrap_or_else(|| "com.aprilnea.chatgpui".to_string());

        println!("cargo:rustc-env=APP_IDENTIFIER={}", identifier);

        // Build shadow-rs for git info, build time, etc.
        println!("cargo:rerun-if-changed=.git/HEAD");
        println!("cargo:rerun-if-changed=.git/index");
        ShadowBuilder::builder().build().unwrap();
    } else {
        // Debug mode: use hardcoded identifier, skip shadow-rs
        println!("cargo:rustc-env=APP_IDENTIFIER=com.aprilnea.chatgpui.dev");
    }
}
