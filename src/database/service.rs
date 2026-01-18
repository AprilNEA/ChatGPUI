// SPDX-FileCopyrightText: 2026 AprilNEA LLC
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

use anyhow::Result;
use gpui::{App, Global};
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use entity::{conversation, message};
use super::manager::DatabaseManager;
use super::repository::{ConversationRepository, MessageRepository};

/// Global database service accessible from GPUI context
#[derive(Clone)]
pub struct DatabaseService {
    manager: Arc<RwLock<Option<DatabaseManager>>>,
    connection: Arc<RwLock<Option<DatabaseConnection>>>,
}

impl Global for DatabaseService {}

impl DatabaseService {
    pub fn new() -> Self {
        Self {
            manager: Arc::new(RwLock::new(None)),
            connection: Arc::new(RwLock::new(None)),
        }
    }

    /// Initialize the database (should be called once at startup)
    pub async fn initialize(&self) -> Result<()> {
        tracing::info!("Starting database initialization...");

        let mut manager = DatabaseManager::new().await?;
        manager.initialize().await?;

        let connection = manager
            .connection()
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Database connection not available"))?;

        *self.manager.write().await = Some(manager);
        *self.connection.write().await = Some(connection);

        tracing::info!("Database initialized successfully");
        Ok(())
    }

    /// Check if database is ready
    pub async fn is_ready(&self) -> bool {
        self.connection.read().await.is_some()
    }

    /// Get database connection
    async fn get_connection(&self) -> Result<DatabaseConnection> {
        self.connection
            .read()
            .await
            .clone()
            .ok_or_else(|| anyhow::anyhow!("Database not initialized"))
    }

    // ============ Conversation Methods ============

    /// Create a new conversation
    pub async fn create_conversation(
        &self,
        title: String,
        provider_id: String,
        model: String,
        system_prompt: Option<String>,
    ) -> Result<conversation::Model> {
        let conn = self.get_connection().await?;
        ConversationRepository::create(&conn, title, provider_id, model, system_prompt).await
    }

    /// List all conversations
    pub async fn list_conversations(&self) -> Result<Vec<conversation::Model>> {
        let conn = self.get_connection().await?;
        ConversationRepository::list(&conn).await
    }

    /// Get conversation by ID
    pub async fn get_conversation(&self, id: Uuid) -> Result<Option<conversation::Model>> {
        let conn = self.get_connection().await?;
        ConversationRepository::find_by_id(&conn, id).await
    }

    /// Update conversation title
    pub async fn update_conversation_title(
        &self,
        id: Uuid,
        title: String,
    ) -> Result<conversation::Model> {
        let conn = self.get_connection().await?;
        ConversationRepository::update_title(&conn, id, title).await
    }

    /// Delete conversation
    pub async fn delete_conversation(&self, id: Uuid) -> Result<()> {
        let conn = self.get_connection().await?;
        ConversationRepository::delete(&conn, id).await
    }

    // ============ Message Methods ============

    /// Create a new message
    pub async fn create_message(
        &self,
        conversation_id: Uuid,
        role: message::MessageRole,
        content: String,
        status: message::MessageStatus,
    ) -> Result<message::Model> {
        let conn = self.get_connection().await?;
        MessageRepository::create(&conn, conversation_id, role, content, status).await
    }

    /// List messages for a conversation
    pub async fn list_messages(&self, conversation_id: Uuid) -> Result<Vec<message::Model>> {
        let conn = self.get_connection().await?;
        MessageRepository::list_by_conversation(&conn, conversation_id).await
    }

    /// Update message content
    pub async fn update_message_content(
        &self,
        id: Uuid,
        content: String,
    ) -> Result<message::Model> {
        let conn = self.get_connection().await?;
        MessageRepository::update_content(&conn, id, content).await
    }

    /// Update message status
    pub async fn update_message_status(
        &self,
        id: Uuid,
        status: message::MessageStatus,
        error_message: Option<String>,
    ) -> Result<message::Model> {
        let conn = self.get_connection().await?;
        MessageRepository::update_status(&conn, id, status, error_message).await
    }

    /// Shutdown the database
    pub async fn shutdown(&self) -> Result<()> {
        // Drop connection first
        *self.connection.write().await = None;

        // Then shutdown manager
        if let Some(mut manager) = self.manager.write().await.take() {
            manager.shutdown().await?;
        }

        tracing::info!("Database shutdown complete");
        Ok(())
    }

    /// Synchronous shutdown - blocks current thread until complete
    /// Uses a new tokio runtime for the shutdown process
    pub fn shutdown_sync(&self) {
        let manager = self.manager.clone();
        let connection = self.connection.clone();

        // Create a new runtime just for shutdown
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build();

        if let Ok(rt) = rt {
            rt.block_on(async {
                // Drop connection first
                *connection.write().await = None;

                // Then shutdown manager
                if let Some(mut mgr) = manager.write().await.take() {
                    if let Err(e) = mgr.shutdown().await {
                        tracing::error!("Failed to shutdown database: {}", e);
                    }
                }
            });
            tracing::info!("Database shutdown complete (sync)");
        } else {
            tracing::error!("Failed to create tokio runtime for database shutdown");
        }
    }
}

/// Get database service from GPUI context
pub fn get_db(cx: &App) -> &DatabaseService {
    cx.global::<DatabaseService>()
}

/// Initialize database service in GPUI
pub fn init(cx: &mut App) {
    cx.set_global(DatabaseService::new());
}
