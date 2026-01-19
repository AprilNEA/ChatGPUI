// SPDX-FileCopyrightText: 2026 AprilNEA LLC
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

use sea_orm::entity::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(16))")]
pub enum MessageRole {
    #[sea_orm(string_value = "system")]
    System,
    #[sea_orm(string_value = "user")]
    User,
    #[sea_orm(string_value = "assistant")]
    Assistant,
}

#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(16))")]
pub enum MessageStatus {
    #[sea_orm(string_value = "pending")]
    Pending,
    #[sea_orm(string_value = "streaming")]
    Streaming,
    #[sea_orm(string_value = "done")]
    Done,
    #[sea_orm(string_value = "error")]
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "messages")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub role: MessageRole,
    #[sea_orm(column_type = "Text")]
    pub content: String,
    pub status: MessageStatus,
    pub error_message: Option<String>,
    pub created_at: DateTimeUtc,
    /// Extended thinking content (for Claude models)
    #[sea_orm(column_type = "Text", nullable)]
    pub thinking_content: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::conversation::Entity",
        from = "Column::ConversationId",
        to = "super::conversation::Column::Id"
    )]
    Conversation,
}

impl Related<super::conversation::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Conversation.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
