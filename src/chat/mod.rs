// SPDX-FileCopyrightText: 2026 AprilNEA LLC
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

mod conversation_cache;
mod message;
mod message_input;
mod message_list;
mod scroll_manager;
mod sidebar;
mod view;

// Re-export public types used by other modules
pub use message::{ChatMessage, Role};
pub use sidebar::{ChatSidebar, ConversationDeletedEvent, ConversationSelectedEvent};
pub use view::{BackgroundStreamFinishedEvent, ChatView, ConversationUpdatedEvent};
