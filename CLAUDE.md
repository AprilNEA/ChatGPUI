# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

ChatGPUI is a GPU-accelerated native LLM chat client built with GPUI (the rendering engine from Zed editor). It supports multiple LLM providers with streaming responses, conversation persistence, and image attachments.

## Build & Development Commands

```bash
# Development
cargo run                    # Run in dev mode
mise run dev                 # Alternative via mise

# Build
cargo build --release        # Release build
mise run build              # Alternative via mise

# Code Quality
cargo fmt                    # Format code
cargo clippy                 # Lint check

# Database Cleanup
mise run clean              # Clean PostgreSQL processes and shared memory
mise run clean-all          # Delete all data including conversations
```

**Toolchain**: Rust nightly (`nightly-2026-01-18`), Edition 2024

## Architecture

### Module Structure

```
src/
├── main.rs              # Entry point, menu bar, window creation
├── app.rs               # ChatApp - main application component
├── chat/                # Chat functionality module
│   ├── view.rs          # ChatView - main chat container
│   ├── sidebar.rs       # ChatSidebar - conversation history
│   ├── message_list.rs  # MessageList - virtual list of messages
│   ├── message_input.rs # MessageInput - input with attachments
│   ├── message.rs       # Message data structures
│   └── scroll_manager.rs
├── model_selector.rs    # LLM provider/model picker
├── icons/               # Icon assets and helpers
│   ├── app_icon.rs      # AppIcon enum
│   ├── llm_provider.rs  # LlmProvider icon mapping
│   └── language.rs      # Programming language icons
├── settings/            # Settings management
│   ├── state.rs         # Settings state and persistence
│   ├── view.rs          # Settings window UI
│   └── provider.rs      # Provider configuration
├── windows/             # Standalone windows
│   └── about.rs         # About window
├── llm/                 # LLM provider implementations
├── database/            # Database layer
└── components/          # Shared UI components
```

### Event-Driven Communication

Components communicate via GPUI's `EventEmitter` pattern:
- `ModelSelectorChangedEvent` → ChatView reloads LLM client
- `ConversationSelectedEvent` → ChatView loads conversation
- `ConversationUpdatedEvent` → Sidebar refreshes list

### LLM Provider Integration

Providers implement the `LlmProvider` trait in `src/llm/`:
- `stream_chat()` - Streaming chat completion
- `fetch_models()` - Dynamic model list (optional)
- `models()` - Static model fallback

Supported: Anthropic, OpenAI, Google AI. Implementation pattern: reqwest streaming → async channel → GPUI update loop.

### Database Layer

- **Embedded PostgreSQL** via `postgresql_embedded`
- **ORM**: SeaORM with entities in `crates/entity/`
- **Migrations**: `crates/migration/`
- **Global Service**: `DatabaseService` (GPUI Global trait)

Tables: `conversations`, `messages`, `attachments`

### Workspace Crates

| Crate | Purpose |
|-------|---------|
| `entity` | SeaORM database entities |
| `migration` | Database migrations |

## Performance Patterns

1. **Virtual Lists**: MessageList and ChatSidebar use `v_virtual_list` for large datasets
2. **Streaming Debounce**: 50ms debounce for LLM streaming updates
3. **Measurement Caching**: Height estimates cached, significant changes (>100px) trigger remeasure
4. **Debug Optimization**: shadow-rs disabled in debug builds for faster incremental compilation

## Code Conventions

- **License Header**: All files include SPDX headers (AGPL-3.0-only OR LicenseRef-Commercial)
- **i18n**: Default locale zh-CN, translations in `locales/`
- **Assets**: Embedded via rust-embed, icons in `assets/icons/`

## Adding a New LLM Provider

1. Create `src/llm/{provider_name}.rs` implementing `LlmProvider`
2. Add to `src/llm/mod.rs` factory function
3. Add provider config to `src/settings/provider.rs`
4. Add icon to `assets/icons/llm_provider/` and register in `src/icons/llm_provider.rs`

## Key Dependencies

- `gpui` / `gpui-component` - UI framework and components (git dependencies)
- `gpui-tokio-bridge` - Bridges Tokio async runtime to GPUI
- `sea-orm` - Database ORM
