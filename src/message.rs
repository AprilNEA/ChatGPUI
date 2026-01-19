// SPDX-FileCopyrightText: 2026 AprilNEA LLC
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageStatus {
    Pending,
    Streaming,
    Done,
    Error(String),
}

#[derive(Debug, Clone)]
pub struct Message {
    pub id: Uuid,
    pub role: Role,
    pub content: String,
    pub status: MessageStatus,
    #[allow(dead_code)]
    pub created_at: DateTime<Utc>,
}

impl Message {
    pub fn new(role: Role, content: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            role,
            content: content.into(),
            status: MessageStatus::Done,
            created_at: Utc::now(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self::new(Role::User, content)
    }

    #[allow(dead_code)]
    pub fn assistant(content: impl Into<String>) -> Self {
        Self::new(Role::Assistant, content)
    }

    pub fn assistant_streaming() -> Self {
        Self {
            id: Uuid::new_v4(),
            role: Role::Assistant,
            content: String::new(),
            status: MessageStatus::Streaming,
            created_at: Utc::now(),
        }
    }

    pub fn system(content: impl Into<String>) -> Self {
        Self::new(Role::System, content)
    }

}

#[derive(Debug, Clone, Serialize)]
pub struct ChatMessage {
    pub role: Role,
    pub content: String,
}

impl From<&Message> for ChatMessage {
    fn from(msg: &Message) -> Self {
        Self {
            role: msg.role,
            content: msg.content.clone(),
        }
    }
}
