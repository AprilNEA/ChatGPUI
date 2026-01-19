// SPDX-FileCopyrightText: 2026 AprilNEA LLC
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Attachments::Table)
                    .if_not_exists()
                    .col(uuid(Attachments::Id).primary_key())
                    .col(uuid(Attachments::MessageId).not_null())
                    .col(string_len(Attachments::AttachmentType, 16).not_null())
                    .col(text(Attachments::Name).not_null())
                    .col(string_len(Attachments::MimeType, 128).not_null())
                    .col(text(Attachments::FilePath).not_null())
                    .col(big_unsigned(Attachments::FileSize).not_null())
                    .col(
                        timestamp_with_time_zone(Attachments::CreatedAt)
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_attachment_message")
                            .from(Attachments::Table, Attachments::MessageId)
                            .to(Messages::Table, Messages::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_attachments_message_id")
                    .table(Attachments::Table)
                    .col(Attachments::MessageId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Attachments::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum Attachments {
    Table,
    Id,
    MessageId,
    AttachmentType,
    Name,
    MimeType,
    FilePath,
    FileSize,
    CreatedAt,
}

#[derive(DeriveIden)]
enum Messages {
    Table,
    Id,
}
