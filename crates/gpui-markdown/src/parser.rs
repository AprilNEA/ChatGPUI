// SPDX-FileCopyrightText: 2026 AprilNEA LLC
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

//! Markdown parser that converts markdown text into an intermediate representation.

use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

/// Parsed markdown element
#[derive(Debug, Clone)]
pub enum MarkdownElement {
    /// Plain text
    Text(String),
    /// Bold/strong text
    Strong(Vec<MarkdownElement>),
    /// Italic/emphasis text
    Emphasis(Vec<MarkdownElement>),
    /// Strikethrough text
    Strikethrough(Vec<MarkdownElement>),
    /// Inline code
    InlineCode(String),
    /// Code block with optional language
    CodeBlock {
        language: Option<String>,
        code: String,
    },
    /// Heading with level (1-6)
    Heading {
        level: u8,
        content: Vec<MarkdownElement>,
    },
    /// Paragraph
    Paragraph(Vec<MarkdownElement>),
    /// Blockquote
    Blockquote(Vec<MarkdownElement>),
    /// Unordered list
    UnorderedList(Vec<Vec<MarkdownElement>>),
    /// Ordered list with starting number
    OrderedList {
        start: u64,
        items: Vec<Vec<MarkdownElement>>,
    },
    /// List item
    ListItem(Vec<MarkdownElement>),
    /// Link with URL and optional title
    Link {
        url: String,
        title: Option<String>,
        content: Vec<MarkdownElement>,
    },
    /// Image with URL and alt text
    Image {
        url: String,
        alt: String,
        title: Option<String>,
    },
    /// Horizontal rule / thematic break
    ThematicBreak,
    /// Hard line break
    HardBreak,
    /// Soft line break
    SoftBreak,
}

/// Owned version of pulldown_cmark::Event for storage
#[derive(Debug, Clone)]
enum OwnedEvent {
    Start(OwnedTag),
    End(TagEnd),
    Text(String),
    Code(String),
    SoftBreak,
    HardBreak,
    Rule,
    Other,
}

#[derive(Debug, Clone)]
enum OwnedTag {
    Paragraph,
    Heading { level: HeadingLevel },
    Strong,
    Emphasis,
    Strikethrough,
    BlockQuote,
    CodeBlock { language: Option<String> },
    List { start: Option<u64> },
    Item,
    Link { dest_url: String, title: String },
    Image { dest_url: String, title: String },
    Other,
}

impl OwnedEvent {
    fn from_event(event: &Event) -> Self {
        match event {
            Event::Start(tag) => OwnedEvent::Start(OwnedTag::from_tag(tag)),
            Event::End(tag) => OwnedEvent::End(*tag),
            Event::Text(text) => OwnedEvent::Text(text.to_string()),
            Event::Code(code) => OwnedEvent::Code(code.to_string()),
            Event::SoftBreak => OwnedEvent::SoftBreak,
            Event::HardBreak => OwnedEvent::HardBreak,
            Event::Rule => OwnedEvent::Rule,
            _ => OwnedEvent::Other,
        }
    }
}

impl OwnedTag {
    fn from_tag(tag: &Tag) -> Self {
        match tag {
            Tag::Paragraph => OwnedTag::Paragraph,
            Tag::Heading { level, .. } => OwnedTag::Heading { level: *level },
            Tag::Strong => OwnedTag::Strong,
            Tag::Emphasis => OwnedTag::Emphasis,
            Tag::Strikethrough => OwnedTag::Strikethrough,
            Tag::BlockQuote(_) => OwnedTag::BlockQuote,
            Tag::CodeBlock(kind) => {
                let language = match kind {
                    CodeBlockKind::Fenced(lang) if !lang.is_empty() => Some(lang.to_string()),
                    _ => None,
                };
                OwnedTag::CodeBlock { language }
            }
            Tag::List(start) => OwnedTag::List { start: *start },
            Tag::Item => OwnedTag::Item,
            Tag::Link {
                dest_url, title, ..
            } => OwnedTag::Link {
                dest_url: dest_url.to_string(),
                title: title.to_string(),
            },
            Tag::Image {
                dest_url, title, ..
            } => OwnedTag::Image {
                dest_url: dest_url.to_string(),
                title: title.to_string(),
            },
            _ => OwnedTag::Other,
        }
    }

    fn matches(&self, other: &OwnedTag) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }

    fn to_end_tag(&self) -> TagEnd {
        match self {
            OwnedTag::Paragraph => TagEnd::Paragraph,
            OwnedTag::Heading { level } => TagEnd::Heading(*level),
            OwnedTag::Strong => TagEnd::Strong,
            OwnedTag::Emphasis => TagEnd::Emphasis,
            OwnedTag::Strikethrough => TagEnd::Strikethrough,
            OwnedTag::BlockQuote => TagEnd::BlockQuote(None),
            OwnedTag::CodeBlock { .. } => TagEnd::CodeBlock,
            OwnedTag::List { start } => TagEnd::List(start.is_some()),
            OwnedTag::Item => TagEnd::Item,
            OwnedTag::Link { .. } => TagEnd::Link,
            OwnedTag::Image { .. } => TagEnd::Image,
            OwnedTag::Other => TagEnd::Paragraph,
        }
    }
}

/// Markdown parser that converts markdown text to elements
pub struct MarkdownParser {
    options: Options,
}

impl Default for MarkdownParser {
    fn default() -> Self {
        Self::new()
    }
}

impl MarkdownParser {
    pub fn new() -> Self {
        let mut options = Options::empty();
        options.insert(Options::ENABLE_STRIKETHROUGH);
        options.insert(Options::ENABLE_TABLES);
        options.insert(Options::ENABLE_FOOTNOTES);
        options.insert(Options::ENABLE_TASKLISTS);
        options.insert(Options::ENABLE_SMART_PUNCTUATION);

        Self { options }
    }

    /// Parse markdown text into elements
    pub fn parse(&self, text: &str) -> Vec<MarkdownElement> {
        let parser = Parser::new_ext(text, self.options);
        let events: Vec<OwnedEvent> = parser.map(|e| OwnedEvent::from_event(&e)).collect();
        self.events_to_elements(&events)
    }

    fn events_to_elements(&self, events: &[OwnedEvent]) -> Vec<MarkdownElement> {
        let mut elements = Vec::new();
        let mut index = 0;

        while index < events.len() {
            if let Some((element, consumed)) = self.parse_element(&events[index..]) {
                elements.push(element);
                index += consumed;
            } else {
                index += 1;
            }
        }

        elements
    }

    fn parse_element(&self, events: &[OwnedEvent]) -> Option<(MarkdownElement, usize)> {
        match events.first()? {
            OwnedEvent::Start(tag) => self.parse_tag(tag.clone(), events),
            OwnedEvent::Text(text) => Some((MarkdownElement::Text(text.clone()), 1)),
            OwnedEvent::Code(code) => Some((MarkdownElement::InlineCode(code.clone()), 1)),
            OwnedEvent::SoftBreak => Some((MarkdownElement::SoftBreak, 1)),
            OwnedEvent::HardBreak => Some((MarkdownElement::HardBreak, 1)),
            OwnedEvent::Rule => Some((MarkdownElement::ThematicBreak, 1)),
            _ => None,
        }
    }

    fn parse_tag(&self, tag: OwnedTag, events: &[OwnedEvent]) -> Option<(MarkdownElement, usize)> {
        let end_tag = tag.to_end_tag();
        let (inner_events, total_consumed) = self.collect_until_end(events, &end_tag, &tag)?;

        let element = match tag {
            OwnedTag::Paragraph => {
                let content = self.events_to_elements(&inner_events);
                MarkdownElement::Paragraph(content)
            }
            OwnedTag::Heading { level } => {
                let content = self.events_to_elements(&inner_events);
                MarkdownElement::Heading {
                    level: Self::heading_level_to_u8(level),
                    content,
                }
            }
            OwnedTag::Strong => {
                let content = self.events_to_elements(&inner_events);
                MarkdownElement::Strong(content)
            }
            OwnedTag::Emphasis => {
                let content = self.events_to_elements(&inner_events);
                MarkdownElement::Emphasis(content)
            }
            OwnedTag::Strikethrough => {
                let content = self.events_to_elements(&inner_events);
                MarkdownElement::Strikethrough(content)
            }
            OwnedTag::BlockQuote => {
                let content = self.events_to_elements(&inner_events);
                MarkdownElement::Blockquote(content)
            }
            OwnedTag::CodeBlock { language } => {
                let code = self.extract_text(&inner_events);
                MarkdownElement::CodeBlock { language, code }
            }
            OwnedTag::List { start } => {
                let items = self.parse_list_items(&inner_events);
                match start {
                    Some(n) => MarkdownElement::OrderedList { start: n, items },
                    None => MarkdownElement::UnorderedList(items),
                }
            }
            OwnedTag::Item => {
                let content = self.events_to_elements(&inner_events);
                MarkdownElement::ListItem(content)
            }
            OwnedTag::Link { dest_url, title } => {
                let content = self.events_to_elements(&inner_events);
                MarkdownElement::Link {
                    url: dest_url,
                    title: if title.is_empty() { None } else { Some(title) },
                    content,
                }
            }
            OwnedTag::Image { dest_url, title } => {
                let alt = self.extract_text(&inner_events);
                MarkdownElement::Image {
                    url: dest_url,
                    alt,
                    title: if title.is_empty() { None } else { Some(title) },
                }
            }
            OwnedTag::Other => return None,
        };

        Some((element, total_consumed))
    }

    fn collect_until_end(
        &self,
        events: &[OwnedEvent],
        end_tag: &TagEnd,
        start_tag: &OwnedTag,
    ) -> Option<(Vec<OwnedEvent>, usize)> {
        let mut depth = 1;
        let mut inner_events = Vec::new();

        for (i, event) in events.iter().enumerate().skip(1) {
            match event {
                OwnedEvent::Start(tag) if tag.matches(start_tag) => {
                    depth += 1;
                    inner_events.push(event.clone());
                }
                OwnedEvent::End(tag) if tag == end_tag => {
                    depth -= 1;
                    if depth == 0 {
                        return Some((inner_events, i + 1));
                    }
                    inner_events.push(event.clone());
                }
                _ => {
                    inner_events.push(event.clone());
                }
            }
        }

        None
    }

    fn parse_list_items(&self, events: &[OwnedEvent]) -> Vec<Vec<MarkdownElement>> {
        let mut items = Vec::new();
        let mut index = 0;

        while index < events.len() {
            if let OwnedEvent::Start(OwnedTag::Item) = &events[index]
                && let Some((inner, consumed)) =
                    self.collect_until_end(&events[index..], &TagEnd::Item, &OwnedTag::Item)
            {
                items.push(self.events_to_elements(&inner));
                index += consumed;
                continue;
            }
            index += 1;
        }

        items
    }

    fn extract_text(&self, events: &[OwnedEvent]) -> String {
        let mut text = String::new();
        for event in events {
            if let OwnedEvent::Text(t) = event {
                text.push_str(t);
            }
        }
        text
    }

    fn heading_level_to_u8(level: HeadingLevel) -> u8 {
        match level {
            HeadingLevel::H1 => 1,
            HeadingLevel::H2 => 2,
            HeadingLevel::H3 => 3,
            HeadingLevel::H4 => 4,
            HeadingLevel::H5 => 5,
            HeadingLevel::H6 => 6,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_text() {
        let parser = MarkdownParser::new();
        let elements = parser.parse("Hello, world!");

        assert_eq!(elements.len(), 1);
        match &elements[0] {
            MarkdownElement::Paragraph(content) => {
                assert_eq!(content.len(), 1);
                match &content[0] {
                    MarkdownElement::Text(text) => assert_eq!(text, "Hello, world!"),
                    _ => panic!("Expected Text"),
                }
            }
            _ => panic!("Expected Paragraph"),
        }
    }

    #[test]
    fn test_parse_code_block() {
        let parser = MarkdownParser::new();
        let elements = parser.parse("```rust\nfn main() {}\n```");

        assert_eq!(elements.len(), 1);
        match &elements[0] {
            MarkdownElement::CodeBlock { language, code } => {
                assert_eq!(language.as_deref(), Some("rust"));
                assert_eq!(code.trim(), "fn main() {}");
            }
            _ => panic!("Expected CodeBlock"),
        }
    }
}
