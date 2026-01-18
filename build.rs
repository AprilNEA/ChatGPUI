// SPDX-FileCopyrightText: 2026 AprilNEA LLC
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

use cargo_metadata::MetadataCommand;
use shadow_rs::ShadowBuilder;

fn main() {
    // Read bundle identifier from Cargo.toml metadata
    let metadata = MetadataCommand::new()
        .no_deps()
        .exec()
        .expect("Failed to get cargo metadata");

    let package = metadata
        .packages
        .iter()
        .find(|p| p.name == "chatgpui")
        .expect("Failed to find chatgpui package");

    let identifier = package
        .metadata
        .get("bundle")
        .and_then(|b| b.get("identifier"))
        .and_then(|i| i.as_str())
        .unwrap_or("chatgpui");

    println!("cargo:rustc-env=APP_IDENTIFIER={}", identifier);
    println!("cargo:rerun-if-changed=Cargo.toml");

    // Build shadow-rs
    ShadowBuilder::builder().build().unwrap();
}
