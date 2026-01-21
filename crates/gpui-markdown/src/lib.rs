// SPDX-FileCopyrightText: 2026 AprilNEA LLC
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

//! GPUI Markdown rendering component
//!
//! This crate provides Markdown parsing and rendering for GPUI applications.
//! It uses pulldown-cmark for parsing and renders to native GPUI elements.

mod parser;
mod render;

#[cfg(feature = "syntax-highlighting")]
mod syntax;
#[cfg(feature = "syntax-highlighting")]
mod theme_registry;

pub use parser::MarkdownParser;
pub use render::{Markdown, MarkdownStyle};

#[cfg(feature = "syntax-highlighting")]
pub use syntax::SyntaxHighlighter;
#[cfg(feature = "syntax-highlighting")]
pub use theme_registry::{CodeThemeEntry, CodeThemeRegistry, CodeThemeSource};
