// SPDX-FileCopyrightText: 2026 AprilNEA LLC
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

use std::borrow::Cow;
use strum::{Display, EnumString};

use gpui::{AnyElement, IntoElement, SharedString};
use gpui_component::IconNamed;

use super::IntoIcon;

/// Programming language enum for code blocks
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, EnumString, Display)]
#[strum(serialize_all = "lowercase", ascii_case_insensitive)]
#[allow(dead_code)]
pub enum Language {
    // Shell/Terminal
    #[strum(
        serialize = "bash",
        serialize = "sh",
        serialize = "shell",
        serialize = "zsh"
    )]
    Bash,
    #[strum(serialize = "powershell", serialize = "ps1")]
    PowerShell,

    // C family
    C,
    #[strum(
        serialize = "cpp",
        serialize = "c++",
        serialize = "cxx",
        serialize = "cc"
    )]
    Cpp,
    #[strum(serialize = "csharp", serialize = "c#", serialize = "cs")]
    CSharp,

    // Web
    #[strum(serialize = "javascript", serialize = "js", serialize = "jsx")]
    JavaScript,
    #[strum(serialize = "typescript", serialize = "ts", serialize = "tsx")]
    TypeScript,
    #[strum(serialize = "html", serialize = "htm")]
    Html,
    Css,
    #[strum(serialize = "sass", serialize = "scss")]
    Sass,
    #[strum(serialize = "graphql", serialize = "gql")]
    GraphQL,

    // Systems
    #[strum(serialize = "rust", serialize = "rs")]
    Rust,
    #[strum(serialize = "go", serialize = "golang")]
    Go,
    Zig,

    // JVM
    Java,
    #[strum(serialize = "kotlin", serialize = "kt", serialize = "kts")]
    Kotlin,
    Scala,

    // Scripting
    #[strum(serialize = "python", serialize = "py")]
    Python,
    #[strum(serialize = "ruby", serialize = "rb")]
    Ruby,
    Lua,
    R,

    // Mobile
    Swift,
    Dart,

    // Functional
    #[strum(serialize = "haskell", serialize = "hs")]
    Haskell,
    Gleam,

    // Data/Config
    #[strum(serialize = "json", serialize = "jsonc")]
    Json,
    #[strum(serialize = "yaml", serialize = "yml")]
    Yaml,
    Toml,
    #[strum(serialize = "markdown", serialize = "md")]
    Markdown,

    // Scientific
    #[strum(serialize = "julia", serialize = "jl")]
    Julia,
    #[strum(serialize = "matlab", serialize = "m")]
    Matlab,
    #[strum(serialize = "fortran", serialize = "f90", serialize = "f95")]
    Fortran,

    // Other
    #[strum(serialize = "cobol", serialize = "cob")]
    Cobol,
    #[strum(serialize = "solidity", serialize = "sol")]
    Solidity,
    #[strum(serialize = "terraform", serialize = "tf", serialize = "hcl")]
    Terraform,
    Sql,
}

impl IconNamed for Language {
    fn path(self) -> SharedString {
        let name: Cow<str> = match self {
            Self::Cpp => "c-plusplus".into(),
            _ => self.to_string().to_lowercase().into(),
        };
        format!("icons/language/{}.svg", name).into()
    }
}

impl IntoElement for Language {
    type Element = AnyElement;

    fn into_element(self) -> Self::Element {
        self.icon().into_any_element()
    }
}
