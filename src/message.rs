// SPDX-FileCopyrightText: 2026 AprilNEA LLC
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

use base64::{Engine, engine::general_purpose::STANDARD};
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

/// 附件类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttachmentType {
    Image,
}

/// 消息附件
#[derive(Debug, Clone)]
pub struct Attachment {
    pub id: Uuid,
    #[allow(dead_code)]
    pub attachment_type: AttachmentType,
    pub name: String,
    pub mime_type: String,
    pub data: Vec<u8>,
}

impl Attachment {
    pub fn new_image(name: String, mime_type: String, data: Vec<u8>) -> Self {
        Self {
            id: Uuid::now_v7(),
            attachment_type: AttachmentType::Image,
            name,
            mime_type,
            data,
        }
    }

    /// 返回 base64 编码的数据
    pub fn base64_data(&self) -> String {
        STANDARD.encode(&self.data)
    }
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
    pub attachments: Vec<Attachment>,
    pub status: MessageStatus,
    #[allow(dead_code)]
    pub created_at: DateTime<Utc>,
}

impl Message {
    pub fn new(role: Role, content: impl Into<String>) -> Self {
        Self {
            id: Uuid::now_v7(),
            role,
            content: content.into(),
            attachments: Vec::new(),
            status: MessageStatus::Done,
            created_at: Utc::now(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self::new(Role::User, content)
    }

    pub fn user_with_attachments(content: impl Into<String>, attachments: Vec<Attachment>) -> Self {
        Self {
            id: Uuid::now_v7(),
            role: Role::User,
            content: content.into(),
            attachments,
            status: MessageStatus::Done,
            created_at: Utc::now(),
        }
    }

    #[allow(dead_code)]
    pub fn assistant(content: impl Into<String>) -> Self {
        Self::new(Role::Assistant, content)
    }

    pub fn assistant_streaming() -> Self {
        Self {
            id: Uuid::now_v7(),
            role: Role::Assistant,
            content: String::new(),
            attachments: Vec::new(),
            status: MessageStatus::Streaming,
            created_at: Utc::now(),
        }
    }

    pub fn system(content: impl Into<String>) -> Self {
        Self::new(Role::System, content)
    }
}

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: Role,
    pub content: String,
    pub attachments: Vec<Attachment>,
}

impl From<&Message> for ChatMessage {
    fn from(msg: &Message) -> Self {
        Self {
            role: msg.role,
            content: msg.content.clone(),
            attachments: msg.attachments.clone(),
        }
    }
}
