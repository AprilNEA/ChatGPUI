// SPDX-FileCopyrightText: 2026 AprilNEA LLC
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

//! GPUI Markdown rendering component

use std::future::Future;
use std::sync::Arc;

use gpui::*;
use gpui_component::{ActiveTheme, h_flex, v_flex};

use crate::parser::{MarkdownElement, MarkdownParser};

#[cfg(feature = "syntax-highlighting")]
use crate::syntax::SyntaxHighlighter;

#[cfg(feature = "syntax-highlighting")]
use crate::theme_registry::CodeThemeRegistry;

struct MarkdownParseAsset;

#[allow(clippy::manual_async_fn)]
impl Asset for MarkdownParseAsset {
    type Source = SharedString;
    type Output = Arc<Vec<MarkdownElement>>;

    fn load(
        source: Self::Source,
        _cx: &mut App,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        async move {
            let parser = MarkdownParser::new();
            Arc::new(parser.parse(&source))
        }
    }
}

struct MarkdownCache {
    elements: Option<Arc<Vec<MarkdownElement>>>,
}

/// Style configuration for Markdown rendering
#[derive(Clone)]
pub struct MarkdownStyle {
    /// Base text size
    pub text_size: Pixels,
    /// Code block background color
    pub code_bg: Option<Hsla>,
    /// Code block text color
    pub code_fg: Option<Hsla>,
    /// Inline code background color
    pub inline_code_bg: Option<Hsla>,
    /// Link color
    pub link_color: Option<Hsla>,
    /// Blockquote border color
    pub blockquote_border: Option<Hsla>,
}

impl Default for MarkdownStyle {
    fn default() -> Self {
        Self {
            text_size: px(15.),
            code_bg: None,
            code_fg: None,
            inline_code_bg: None,
            link_color: None,
            blockquote_border: None,
        }
    }
}

impl MarkdownStyle {
    pub fn text_size(mut self, size: Pixels) -> Self {
        self.text_size = size;
        self
    }

    pub fn code_bg(mut self, color: Hsla) -> Self {
        self.code_bg = Some(color);
        self
    }

    pub fn code_fg(mut self, color: Hsla) -> Self {
        self.code_fg = Some(color);
        self
    }
}

/// Markdown rendering component for GPUI
#[derive(IntoElement)]
pub struct Markdown {
    content: SharedString,
    cache_key: ElementId,
    allow_highlighting: bool,
    style: MarkdownStyle,
    #[cfg(feature = "syntax-highlighting")]
    highlighter: Option<SyntaxHighlighter>,
}

impl Markdown {
    /// Create a new Markdown component with the given content and cache key
    pub fn new(content: impl Into<SharedString>, cache_key: impl Into<ElementId>) -> Self {
        Self {
            content: content.into(),
            cache_key: cache_key.into(),
            allow_highlighting: true,
            style: MarkdownStyle::default(),
            #[cfg(feature = "syntax-highlighting")]
            highlighter: Some(SyntaxHighlighter::new()),
        }
    }

    /// Set custom style
    pub fn style(mut self, style: MarkdownStyle) -> Self {
        self.style = style;
        self
    }

    /// Set text size
    pub fn text_size(mut self, size: Pixels) -> Self {
        self.style.text_size = size;
        self
    }

    /// Enable or disable syntax highlighting
    pub fn allow_syntax_highlighting(mut self, allow: bool) -> Self {
        self.allow_highlighting = allow;
        self
    }

    #[cfg(feature = "syntax-highlighting")]
    /// Disable syntax highlighting
    pub fn without_syntax_highlighting(mut self) -> Self {
        self.highlighter = None;
        self
    }
}

impl RenderOnce for Markdown {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let cache = window.use_keyed_state(
            ElementId::from((self.cache_key.clone(), "markdown-cache")),
            cx,
            |_window, _cx| MarkdownCache { elements: None },
        );
        let theme = cx.theme();
        let mut context = RenderContext {
            cache_key: self.cache_key.clone(),
            code_block_index: 0,
            allow_highlighting: self.allow_highlighting,
        };

        // Get code theme colors from the registry (when syntax highlighting is enabled)
        #[cfg(feature = "syntax-highlighting")]
        let (code_theme_bg, code_theme_fg) = {
            let registry = CodeThemeRegistry::global(cx);
            (registry.current_background(), registry.current_foreground())
        };

        // Use code theme background if available, otherwise fall back to UI theme
        #[cfg(feature = "syntax-highlighting")]
        let default_code_bg = code_theme_bg.unwrap_or_else(|| {
            if theme.mode.is_dark() {
                theme.muted
            } else {
                hsla(
                    theme.muted.h,
                    theme.muted.s * 0.5,
                    (theme.muted.l + 1.0) / 2.0,
                    theme.muted.a,
                )
            }
        });

        #[cfg(not(feature = "syntax-highlighting"))]
        let default_code_bg = if theme.mode.is_dark() {
            theme.muted
        } else {
            hsla(
                theme.muted.h,
                theme.muted.s * 0.5,
                (theme.muted.l + 1.0) / 2.0,
                theme.muted.a,
            )
        };

        // Use code theme foreground if available
        #[cfg(feature = "syntax-highlighting")]
        let default_code_fg = code_theme_fg.unwrap_or(theme.foreground);

        #[cfg(not(feature = "syntax-highlighting"))]
        let default_code_fg = theme.foreground;

        let default_inline_code_bg = if theme.mode.is_dark() {
            theme.muted
        } else {
            hsla(
                theme.muted.h,
                theme.muted.s * 0.6,
                (theme.muted.l + 1.0) / 2.0,
                theme.muted.a,
            )
        };

        let code_bg = self.style.code_bg.unwrap_or(default_code_bg);
        // Header background: slightly different from code body for visual separation
        let code_header_bg = {
            let is_dark = code_bg.l < 0.5;
            if is_dark {
                // Dark theme: make header slightly lighter
                hsla(code_bg.h, code_bg.s, (code_bg.l + 0.05).min(1.0), code_bg.a)
            } else {
                // Light theme: make header slightly darker
                hsla(code_bg.h, code_bg.s, (code_bg.l - 0.03).max(0.0), code_bg.a)
            }
        };

        let style = ResolvedStyle {
            text_size: self.style.text_size,
            code_bg,
            code_header_bg,
            code_fg: self.style.code_fg.unwrap_or(default_code_fg),
            inline_code_bg: self.style.inline_code_bg.unwrap_or(default_inline_code_bg),
            link_color: self.style.link_color.unwrap_or(theme.link),
            blockquote_border: self.style.blockquote_border.unwrap_or(theme.border),
            muted_foreground: theme.muted_foreground,
            code_border: theme.border,
        };

        #[cfg(feature = "syntax-highlighting")]
        let highlighter = self.highlighter;

        let elements = match window.use_asset::<MarkdownParseAsset>(&self.content, cx) {
            Some(elements) => {
                let should_update = cache
                    .read(cx)
                    .elements
                    .as_ref()
                    .is_none_or(|cached| !Arc::ptr_eq(cached, &elements));
                if should_update {
                    cache.update(cx, |state, _| {
                        state.elements = Some(elements.clone());
                    });
                }
                Some(elements)
            }
            None => cache.read(cx).elements.clone(),
        };

        let Some(elements) = elements else {
            return v_flex()
                .gap_2()
                .text_size(style.text_size)
                .child(div().child(self.content))
                .into_any_element();
        };

        let rendered: Vec<AnyElement> = elements
            .iter()
            .map(|el| {
                render_element(
                    el,
                    &style,
                    #[cfg(feature = "syntax-highlighting")]
                    &highlighter,
                    &mut context,
                    window,
                    cx,
                )
            })
            .collect();

        v_flex()
            .gap_2()
            .text_size(style.text_size)
            .children(rendered)
            .into_any_element()
    }
}

#[derive(Clone)]
struct ResolvedStyle {
    text_size: Pixels,
    code_bg: Hsla,
    code_header_bg: Hsla,
    code_fg: Hsla,
    inline_code_bg: Hsla,
    link_color: Hsla,
    blockquote_border: Hsla,
    muted_foreground: Hsla,
    code_border: Hsla,
}

struct RenderContext {
    cache_key: ElementId,
    code_block_index: usize,
    allow_highlighting: bool,
}

impl RenderContext {
    fn next_code_block_index(&mut self) -> usize {
        let index = self.code_block_index;
        self.code_block_index += 1;
        index
    }

    fn code_block_cache_key(&self, index: usize) -> ElementId {
        ElementId::from((self.cache_key.clone(), format!("code-block-{}", index)))
    }

    fn code_block_scroll_id(&self, index: usize) -> ElementId {
        ElementId::from((
            self.cache_key.clone(),
            format!("code-block-scroll-{}", index),
        ))
    }

    fn code_block_copy_id(&self, index: usize) -> ElementId {
        ElementId::from((self.cache_key.clone(), format!("code-block-copy-{}", index)))
    }
}

/// Render plain code without syntax highlighting, splitting by lines to avoid extra spacing
fn render_plain_code(code: &str) -> AnyElement {
    // Trim trailing newlines to avoid empty lines at the end
    let code = code.trim_end_matches('\n');
    let lines: Vec<AnyElement> = code
        .lines()
        .map(|line| {
            div()
                .whitespace_nowrap()
                .flex_shrink_0()
                .child(line.to_string())
                .into_any_element()
        })
        .collect();
    v_flex().flex_shrink_0().children(lines).into_any_element()
}

fn render_element(
    element: &MarkdownElement,
    style: &ResolvedStyle,
    #[cfg(feature = "syntax-highlighting")] highlighter: &Option<SyntaxHighlighter>,
    context: &mut RenderContext,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    match element {
        MarkdownElement::Paragraph(content) => {
            let children = render_inline_children(
                content,
                style,
                #[cfg(feature = "syntax-highlighting")]
                highlighter,
                context,
                window,
                cx,
            );
            h_flex().flex_wrap().children(children).into_any_element()
        }

        MarkdownElement::Heading { level, content } => {
            let text_size = match level {
                1 => px(28.),
                2 => px(24.),
                3 => px(20.),
                4 => px(18.),
                5 => px(16.),
                _ => px(14.),
            };

            let children = render_inline_children(
                content,
                style,
                #[cfg(feature = "syntax-highlighting")]
                highlighter,
                context,
                window,
                cx,
            );

            h_flex()
                .flex_wrap()
                .text_size(text_size)
                .font_weight(FontWeight::BOLD)
                .children(children)
                .into_any_element()
        }

        MarkdownElement::CodeBlock { language, code } => {
            let code_block_index = context.next_code_block_index();
            #[cfg(feature = "syntax-highlighting")]
            let code_element = if let Some(hl) = highlighter {
                if let Some(lang) = language.as_deref() {
                    hl.highlight(
                        code,
                        lang,
                        context.code_block_cache_key(code_block_index),
                        context.allow_highlighting,
                        window,
                        cx,
                    )
                } else {
                    render_plain_code(code)
                }
            } else {
                render_plain_code(code)
            };

            #[cfg(not(feature = "syntax-highlighting"))]
            let code_element = render_plain_code(code);

            let theme = cx.theme();
            let is_dark = theme.mode.is_dark();
            let raw_label = language.as_deref().unwrap_or("text").trim();
            let label = raw_label.to_lowercase();
            let display_label = if label.is_empty() {
                "text".to_string()
            } else {
                label.clone()
            };

            let icon_path = language_icon(&label, is_dark);

            let mut header_left = h_flex().items_center().gap_2();
            if let Some(path) = icon_path {
                header_left =
                    header_left.child(svg().path(path).size_4().text_color(style.muted_foreground));
            }
            header_left = header_left.child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(style.muted_foreground)
                    .child(display_label),
            );

            let code_to_copy = code.to_string();
            let copy_button = div()
                .id(context.code_block_copy_id(code_block_index))
                .p_1()
                .rounded_sm()
                .cursor_pointer()
                .hover(|s| s.bg(style.code_bg))
                .on_click(move |_ev, _window, cx: &mut App| {
                    cx.write_to_clipboard(ClipboardItem::new_string(code_to_copy.clone()));
                })
                .child(
                    svg()
                        .path("icons/copy.svg")
                        .size_4()
                        .text_color(style.muted_foreground),
                );

            let header = h_flex()
                .items_center()
                .justify_between()
                .px_3()
                .py_2()
                .bg(style.code_header_bg)
                .border_b_1()
                .border_color(style.code_border)
                .child(header_left)
                .child(copy_button);

            let code_body = div()
                .w_full()
                .px_3()
                .py_3()
                .text_size(theme.mono_font_size)
                .font_family(theme.mono_font_family.clone())
                .text_color(style.code_fg)
                .id(context.code_block_scroll_id(code_block_index))
                .overflow_x_scroll()
                .overflow_y_hidden()
                .child(code_element);

            v_flex()
                .w_full()
                .bg(style.code_bg)
                .border_1()
                .border_color(style.code_border)
                .rounded_lg()
                .overflow_hidden()
                .child(header)
                .child(code_body)
                .into_any_element()
        }

        MarkdownElement::Blockquote(content) => {
            let children: Vec<AnyElement> = content
                .iter()
                .map(|el| {
                    render_element(
                        el,
                        style,
                        #[cfg(feature = "syntax-highlighting")]
                        highlighter,
                        context,
                        window,
                        cx,
                    )
                })
                .collect();

            v_flex()
                .border_l_2()
                .border_color(style.blockquote_border)
                .pl_4()
                .text_color(style.muted_foreground)
                .children(children)
                .into_any_element()
        }

        MarkdownElement::UnorderedList(items) => {
            let list_items: Vec<AnyElement> = items
                .iter()
                .map(|item| {
                    let item_children: Vec<AnyElement> = item
                        .iter()
                        .map(|el| {
                            render_element(
                                el,
                                style,
                                #[cfg(feature = "syntax-highlighting")]
                                highlighter,
                                context,
                                window,
                                cx,
                            )
                        })
                        .collect();

                    h_flex()
                        .gap_2()
                        .items_start()
                        .child(
                            div()
                                .child("•")
                                .text_color(style.muted_foreground)
                                .min_w(px(16.)),
                        )
                        .child(v_flex().flex_1().children(item_children))
                        .into_any_element()
                })
                .collect();

            v_flex().gap_1().children(list_items).into_any_element()
        }

        MarkdownElement::OrderedList { start, items } => {
            let list_items: Vec<AnyElement> = items
                .iter()
                .enumerate()
                .map(|(i, item)| {
                    let item_children: Vec<AnyElement> = item
                        .iter()
                        .map(|el| {
                            render_element(
                                el,
                                style,
                                #[cfg(feature = "syntax-highlighting")]
                                highlighter,
                                context,
                                window,
                                cx,
                            )
                        })
                        .collect();

                    h_flex()
                        .gap_2()
                        .items_start()
                        .child(
                            div()
                                .child(format!("{}.", *start as usize + i))
                                .text_color(style.muted_foreground)
                                .min_w(px(24.)),
                        )
                        .child(v_flex().flex_1().children(item_children))
                        .into_any_element()
                })
                .collect();

            v_flex().gap_1().children(list_items).into_any_element()
        }

        MarkdownElement::ListItem(content) => {
            let children: Vec<AnyElement> = content
                .iter()
                .map(|el| {
                    render_element(
                        el,
                        style,
                        #[cfg(feature = "syntax-highlighting")]
                        highlighter,
                        context,
                        window,
                        cx,
                    )
                })
                .collect();

            v_flex().children(children).into_any_element()
        }

        MarkdownElement::ThematicBreak => div()
            .w_full()
            .h(px(1.))
            .bg(style.blockquote_border)
            .my_4()
            .into_any_element(),

        MarkdownElement::Text(text) => div()
            .flex()
            .flex_shrink_0()
            .child(text.clone())
            .into_any_element(),

        MarkdownElement::InlineCode(code) => div()
            .flex()
            .bg(style.inline_code_bg)
            .rounded_sm()
            .px_1()
            .font_family("monospace")
            .text_sm()
            .child(code.clone())
            .into_any_element(),

        MarkdownElement::Strong(content) => {
            let children = render_inline_children(
                content,
                style,
                #[cfg(feature = "syntax-highlighting")]
                highlighter,
                context,
                window,
                cx,
            );
            h_flex()
                .flex_wrap()
                .font_weight(FontWeight::BOLD)
                .children(children)
                .into_any_element()
        }

        MarkdownElement::Emphasis(content) => {
            let children = render_inline_children(
                content,
                style,
                #[cfg(feature = "syntax-highlighting")]
                highlighter,
                context,
                window,
                cx,
            );
            h_flex()
                .flex_wrap()
                .italic()
                .children(children)
                .into_any_element()
        }

        MarkdownElement::Strikethrough(content) => {
            let children = render_inline_children(
                content,
                style,
                #[cfg(feature = "syntax-highlighting")]
                highlighter,
                context,
                window,
                cx,
            );
            h_flex()
                .flex_wrap()
                .line_through()
                .children(children)
                .into_any_element()
        }

        MarkdownElement::Link { content, .. } => {
            let children = render_inline_children(
                content,
                style,
                #[cfg(feature = "syntax-highlighting")]
                highlighter,
                context,
                window,
                cx,
            );
            h_flex()
                .flex_wrap()
                .text_color(style.link_color)
                .cursor_pointer()
                .underline()
                .children(children)
                .into_any_element()
        }

        MarkdownElement::Image { alt, .. } => {
            // For now, just show alt text as placeholder
            // TODO: Implement actual image loading
            div()
                .bg(style.code_bg)
                .rounded_md()
                .p_2()
                .text_sm()
                .text_color(style.muted_foreground)
                .child(format!("[Image: {}]", alt))
                .into_any_element()
        }

        MarkdownElement::SoftBreak => div().child(" ").into_any_element(),
        MarkdownElement::HardBreak => div().h(px(8.)).w_full().into_any_element(),
    }
}

fn render_inline_children(
    elements: &[MarkdownElement],
    style: &ResolvedStyle,
    #[cfg(feature = "syntax-highlighting")] highlighter: &Option<SyntaxHighlighter>,
    context: &mut RenderContext,
    window: &mut Window,
    cx: &mut App,
) -> Vec<AnyElement> {
    elements
        .iter()
        .map(|el| {
            render_element(
                el,
                style,
                #[cfg(feature = "syntax-highlighting")]
                highlighter,
                context,
                window,
                cx,
            )
        })
        .collect()
}

/// Get language icon path based on language name and theme
fn language_icon(language: &str, is_dark: bool) -> Option<&'static str> {
    match language {
        // Shell/Terminal
        "bash" | "sh" | "shell" | "zsh" => Some(if is_dark {
            "icons/language/bash_dark.svg"
        } else {
            "icons/language/bash.svg"
        }),
        "powershell" | "ps1" => Some("icons/language/powershell.svg"),

        // C family
        "c" => Some("icons/language/c.svg"),
        "cpp" | "c++" | "cxx" | "cc" => Some("icons/language/c-plusplus.svg"),
        "csharp" | "c#" | "cs" => Some("icons/language/csharp.svg"),

        // Web
        "javascript" | "js" | "jsx" => Some("icons/language/javascript.svg"),
        "typescript" | "ts" | "tsx" => Some("icons/language/typescript.svg"),
        "html" | "htm" => Some("icons/language/html5.svg"),
        "css" => Some("icons/language/css.svg"),
        "sass" | "scss" => Some("icons/language/sass.svg"),
        "graphql" | "gql" => Some("icons/language/graphql.svg"),

        // Systems
        "rust" | "rs" => Some(if is_dark {
            "icons/language/rust_dark.svg"
        } else {
            "icons/language/rust.svg"
        }),
        "go" | "golang" => Some(if is_dark {
            "icons/language/golang_dark.svg"
        } else {
            "icons/language/golang.svg"
        }),
        "zig" => Some("icons/language/zig.svg"),

        // JVM
        "java" => Some("icons/language/java.svg"),
        "kotlin" | "kt" | "kts" => Some("icons/language/kotlin.svg"),
        "scala" => Some("icons/language/scala.svg"),

        // Scripting
        "python" | "py" => Some("icons/language/python.svg"),
        "ruby" | "rb" => Some("icons/language/ruby.svg"),
        "lua" => Some("icons/language/lua.svg"),
        "r" => Some(if is_dark {
            "icons/language/r_dark.svg"
        } else {
            "icons/language/r.svg"
        }),

        // Mobile
        "swift" => Some("icons/language/swift.svg"),
        "dart" => Some("icons/language/dart.svg"),

        // Functional
        "haskell" | "hs" => Some("icons/language/haskell.svg"),
        "gleam" => Some("icons/language/gleam.svg"),

        // Data/Config
        "json" | "jsonc" => Some("icons/language/json.svg"),
        "markdown" | "md" => Some(if is_dark {
            "icons/language/markdown-dark.svg"
        } else {
            "icons/language/markdown-light.svg"
        }),

        // Scientific
        "julia" | "jl" => Some("icons/language/julia.svg"),
        "matlab" | "m" => Some("icons/language/matlab.svg"),
        "fortran" | "f90" | "f95" => Some("icons/language/fortran.svg"),

        // Other
        "cobol" | "cob" => Some("icons/language/cobol.svg"),
        "solidity" | "sol" => Some("icons/language/solidity.svg"),
        "terraform" | "tf" | "hcl" => Some("icons/language/terraform.svg"),

        _ => None,
    }
}
