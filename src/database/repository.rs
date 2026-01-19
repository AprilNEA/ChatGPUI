// SPDX-FileCopyrightText: 2026 AprilNEA LLC
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

#![allow(dead_code)]

use anyhow::Result;
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};
use uuid::Uuid;

use entity::{conversation, message};

/// Repository for conversation operations
pub struct ConversationRepository;

impl ConversationRepository {
    /// Create a new conversation
    pub async fn create(
        db: &DatabaseConnection,
        title: String,
        provider_id: String,
        model: String,
        system_prompt: Option<String>,
    ) -> Result<conversation::Model> {
        let now = Utc::now();
        let conversation = conversation::ActiveModel {
            id: Set(Uuid::new_v4()),
            title: Set(title),
            provider_id: Set(provider_id),
            model: Set(model),
            system_prompt: Set(system_prompt),
            created_at: Set(now),
            updated_at: Set(now),
        };

        let result = conversation.insert(db).await?;
        Ok(result)
    }

    /// Get all conversations ordered by updated_at descending
    pub async fn list(db: &DatabaseConnection) -> Result<Vec<conversation::Model>> {
        let conversations = conversation::Entity::find()
            .order_by_desc(conversation::Column::UpdatedAt)
            .all(db)
            .await?;
        Ok(conversations)
    }

    /// Get a conversation by ID
    pub async fn find_by_id(
        db: &DatabaseConnection,
        id: Uuid,
    ) -> Result<Option<conversation::Model>> {
        let conversation = conversation::Entity::find_by_id(id).one(db).await?;
        Ok(conversation)
    }

    /// Update conversation title
    pub async fn update_title(
        db: &DatabaseConnection,
        id: Uuid,
        title: String,
    ) -> Result<conversation::Model> {
        let conversation = conversation::Entity::find_by_id(id)
            .one(db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Conversation not found"))?;

        let mut active: conversation::ActiveModel = conversation.into();
        active.title = Set(title);
        active.updated_at = Set(Utc::now());

        let result = active.update(db).await?;
        Ok(result)
    }

    /// Delete a conversation and all its messages
    pub async fn delete(db: &DatabaseConnection, id: Uuid) -> Result<()> {
        conversation::Entity::delete_by_id(id).exec(db).await?;
        Ok(())
    }

    /// Update the updated_at timestamp
    pub async fn touch(db: &DatabaseConnection, id: Uuid) -> Result<()> {
        let conversation = conversation::Entity::find_by_id(id)
            .one(db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Conversation not found"))?;

        let mut active: conversation::ActiveModel = conversation.into();
        active.updated_at = Set(Utc::now());
        active.update(db).await?;
        Ok(())
    }
}

/// Repository for message operations
pub struct MessageRepository;

impl MessageRepository {
    /// Create a new message
    pub async fn create(
        db: &DatabaseConnection,
        conversation_id: Uuid,
        role: message::MessageRole,
        content: String,
        status: message::MessageStatus,
    ) -> Result<message::Model> {
        let msg = message::ActiveModel {
            id: Set(Uuid::new_v4()),
            conversation_id: Set(conversation_id),
            role: Set(role),
            content: Set(content),
            status: Set(status),
            error_message: Set(None),
            created_at: Set(Utc::now()),
        };

        let result = msg.insert(db).await?;

        // Touch conversation to update timestamp
        ConversationRepository::touch(db, conversation_id).await?;

        Ok(result)
    }

    /// Get all messages for a conversation ordered by created_at
    pub async fn list_by_conversation(
        db: &DatabaseConnection,
        conversation_id: Uuid,
    ) -> Result<Vec<message::Model>> {
        let messages = message::Entity::find()
            .filter(message::Column::ConversationId.eq(conversation_id))
            .order_by_asc(message::Column::CreatedAt)
            .all(db)
            .await?;
        Ok(messages)
    }

    /// Update message content (for streaming)
    pub async fn update_content(
        db: &DatabaseConnection,
        id: Uuid,
        content: String,
    ) -> Result<message::Model> {
        let msg = message::Entity::find_by_id(id)
            .one(db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Message not found"))?;

        let mut active: message::ActiveModel = msg.into();
        active.content = Set(content);

        let result = active.update(db).await?;
        Ok(result)
    }

    /// Update message status
    pub async fn update_status(
        db: &DatabaseConnection,
        id: Uuid,
        status: message::MessageStatus,
        error_message: Option<String>,
    ) -> Result<message::Model> {
        let msg = message::Entity::find_by_id(id)
            .one(db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Message not found"))?;

        let mut active: message::ActiveModel = msg.into();
        active.status = Set(status);
        active.error_message = Set(error_message);

        let result = active.update(db).await?;
        Ok(result)
    }

    /// Delete a message
    pub async fn delete(db: &DatabaseConnection, id: Uuid) -> Result<()> {
        message::Entity::delete_by_id(id).exec(db).await?;
        Ok(())
    }
}
