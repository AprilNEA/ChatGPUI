// SPDX-FileCopyrightText: 2026 AprilNEA LLC
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

//! GPUI Markdown rendering component

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{LazyLock, RwLock};

use gpui::*;
use gpui_component::{h_flex, scroll::ScrollableElement, v_flex, ActiveTheme};

use crate::parser::{MarkdownElement, MarkdownParser};

#[cfg(feature = "syntax-highlighting")]
use crate::syntax::SyntaxHighlighter;

/// 全局 Markdown 解析缓存
static PARSE_CACHE: LazyLock<RwLock<HashMap<u64, Vec<MarkdownElement>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

/// 计算字符串的哈希值
fn hash_content(content: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    hasher.finish()
}

/// 从缓存获取或解析 Markdown
fn parse_with_cache(content: &str) -> Vec<MarkdownElement> {
    let hash = hash_content(content);

    // 先尝试从缓存读取
    if let Ok(cache) = PARSE_CACHE.read() {
        if let Some(elements) = cache.get(&hash) {
            return elements.clone();
        }
    }

    // 缓存未命中，解析并存储
    let parser = MarkdownParser::new();
    let elements = parser.parse(content);

    if let Ok(mut cache) = PARSE_CACHE.write() {
        // 限制缓存大小，防止内存泄漏
        if cache.len() > 1000 {
            cache.clear();
        }
        cache.insert(hash, elements.clone());
    }

    elements
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
            text_size: px(14.),
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
    style: MarkdownStyle,
    #[cfg(feature = "syntax-highlighting")]
    highlighter: Option<SyntaxHighlighter>,
}

impl Markdown {
    /// Create a new Markdown component with the given content
    pub fn new(content: impl Into<SharedString>) -> Self {
        Self {
            content: content.into(),
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

    #[cfg(feature = "syntax-highlighting")]
    /// Disable syntax highlighting
    pub fn without_syntax_highlighting(mut self) -> Self {
        self.highlighter = None;
        self
    }
}

impl RenderOnce for Markdown {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        // 使用缓存的解析结果
        let elements = parse_with_cache(&self.content);

        let style = ResolvedStyle {
            text_size: self.style.text_size,
            code_bg: self.style.code_bg.unwrap_or(theme.muted),
            code_fg: self.style.code_fg.unwrap_or(theme.foreground),
            inline_code_bg: self.style.inline_code_bg.unwrap_or(theme.muted),
            link_color: self.style.link_color.unwrap_or(theme.link),
            blockquote_border: self.style.blockquote_border.unwrap_or(theme.border),
            muted_foreground: theme.muted_foreground,
        };

        #[cfg(feature = "syntax-highlighting")]
        let highlighter = self.highlighter;

        let rendered: Vec<AnyElement> = elements
            .into_iter()
            .map(|el| {
                render_element(
                    el,
                    &style,
                    #[cfg(feature = "syntax-highlighting")]
                    &highlighter,
                )
            })
            .collect();

        v_flex().gap_2().text_size(style.text_size).children(rendered)
    }
}

#[derive(Clone)]
struct ResolvedStyle {
    text_size: Pixels,
    code_bg: Hsla,
    code_fg: Hsla,
    inline_code_bg: Hsla,
    link_color: Hsla,
    blockquote_border: Hsla,
    muted_foreground: Hsla,
}

fn render_element(
    element: MarkdownElement,
    style: &ResolvedStyle,
    #[cfg(feature = "syntax-highlighting")] highlighter: &Option<SyntaxHighlighter>,
) -> AnyElement {
    match element {
        MarkdownElement::Paragraph(content) => {
            let children = render_inline_children(
                content,
                style,
                #[cfg(feature = "syntax-highlighting")]
                highlighter,
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
            );

            h_flex()
                .flex_wrap()
                .text_size(text_size)
                .font_weight(FontWeight::BOLD)
                .children(children)
                .into_any_element()
        }

        MarkdownElement::CodeBlock { language, code } => {
            #[cfg(feature = "syntax-highlighting")]
            let code_element = if let Some(hl) = highlighter {
                if let Some(lang) = &language {
                    hl.highlight(&code, lang)
                } else {
                    div().child(code.clone()).into_any_element()
                }
            } else {
                div().child(code.clone()).into_any_element()
            };

            #[cfg(not(feature = "syntax-highlighting"))]
            let code_element = div().child(code.clone()).into_any_element();

            let lang_label = language.clone();
            let mut code_block = v_flex()
                .w_full()
                .bg(style.code_bg)
                .text_color(style.code_fg)
                .rounded_md()
                .p_3()
                .text_sm()
                .font_family("monospace")
                .overflow_x_scrollbar()
                .overflow_y_hidden();

            if let Some(lang) = lang_label {
                code_block = code_block.child(
                    div()
                        .text_xs()
                        .text_color(style.muted_foreground)
                        .mb_2()
                        .child(lang),
                );
            }

            code_block.child(code_element).into_any_element()
        }

        MarkdownElement::Blockquote(content) => {
            let children: Vec<AnyElement> = content
                .into_iter()
                .map(|el| {
                    render_element(
                        el,
                        style,
                        #[cfg(feature = "syntax-highlighting")]
                        highlighter,
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
                .into_iter()
                .map(|item| {
                    let item_children: Vec<AnyElement> = item
                        .into_iter()
                        .map(|el| {
                            render_element(
                                el,
                                style,
                                #[cfg(feature = "syntax-highlighting")]
                                highlighter,
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
                .into_iter()
                .enumerate()
                .map(|(i, item)| {
                    let item_children: Vec<AnyElement> = item
                        .into_iter()
                        .map(|el| {
                            render_element(
                                el,
                                style,
                                #[cfg(feature = "syntax-highlighting")]
                                highlighter,
                            )
                        })
                        .collect();

                    h_flex()
                        .gap_2()
                        .items_start()
                        .child(
                            div()
                                .child(format!("{}.", start as usize + i))
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
                .into_iter()
                .map(|el| {
                    render_element(
                        el,
                        style,
                        #[cfg(feature = "syntax-highlighting")]
                        highlighter,
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
            .child(text)
            .into_any_element(),

        MarkdownElement::InlineCode(code) => div()
            .flex()
            .bg(style.inline_code_bg)
            .rounded_sm()
            .px_1()
            .font_family("monospace")
            .text_sm()
            .child(code)
            .into_any_element(),

        MarkdownElement::Strong(content) => {
            let children = render_inline_children(
                content,
                style,
                #[cfg(feature = "syntax-highlighting")]
                highlighter,
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
    elements: Vec<MarkdownElement>,
    style: &ResolvedStyle,
    #[cfg(feature = "syntax-highlighting")] highlighter: &Option<SyntaxHighlighter>,
) -> Vec<AnyElement> {
    elements
        .into_iter()
        .map(|el| {
            render_element(
                el,
                style,
                #[cfg(feature = "syntax-highlighting")]
                highlighter,
            )
        })
        .collect()
}
