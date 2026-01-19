// SPDX-FileCopyrightText: 2026 AprilNEA LLC
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

use shadow_rs::ShadowBuilder;

fn main() {
    // Declare rerun conditions to prevent unnecessary rebuilds
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=build.rs");

    let is_release = std::env::var("PROFILE").unwrap_or_default() != "debug";

    if is_release {
        // Only read metadata in release mode (for bundle identifier)
        let identifier = cargo_metadata::MetadataCommand::new()
            .no_deps()
            .exec()
            .ok()
            .and_then(|metadata| {
                metadata
                    .packages
                    .into_iter()
                    .find(|p| p.name == "chatgpui")
            })
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
