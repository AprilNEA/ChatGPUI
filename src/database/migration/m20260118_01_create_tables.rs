// SPDX-FileCopyrightText: 2026 AprilNEA LLC
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create conversations table
        manager
            .create_table(
                Table::create()
                    .table(Conversations::Table)
                    .if_not_exists()
                    .col(uuid(Conversations::Id).primary_key())
                    .col(string(Conversations::Title).not_null())
                    .col(string(Conversations::ProviderId).not_null())
                    .col(string(Conversations::Model).not_null())
                    .col(text_null(Conversations::SystemPrompt))
                    .col(
                        timestamp_with_time_zone(Conversations::CreatedAt)
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        timestamp_with_time_zone(Conversations::UpdatedAt)
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        // Create messages table
        manager
            .create_table(
                Table::create()
                    .table(Messages::Table)
                    .if_not_exists()
                    .col(uuid(Messages::Id).primary_key())
                    .col(uuid(Messages::ConversationId).not_null())
                    .col(string_len(Messages::Role, 16).not_null())
                    .col(text(Messages::Content).not_null())
                    .col(string_len(Messages::Status, 16).not_null())
                    .col(text_null(Messages::ErrorMessage))
                    .col(
                        timestamp_with_time_zone(Messages::CreatedAt)
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_message_conversation")
                            .from(Messages::Table, Messages::ConversationId)
                            .to(Conversations::Table, Conversations::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Create index on conversation_id for faster lookups
        manager
            .create_index(
                Index::create()
                    .name("idx_messages_conversation_id")
                    .table(Messages::Table)
                    .col(Messages::ConversationId)
                    .to_owned(),
            )
            .await?;

        // Create index on created_at for sorting
        manager
            .create_index(
                Index::create()
                    .name("idx_conversations_created_at")
                    .table(Conversations::Table)
                    .col(Conversations::CreatedAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Messages::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Conversations::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum Conversations {
    Table,
    Id,
    Title,
    ProviderId,
    Model,
    SystemPrompt,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Messages {
    Table,
    Id,
    ConversationId,
    Role,
    Content,
    Status,
    ErrorMessage,
    CreatedAt,
}
