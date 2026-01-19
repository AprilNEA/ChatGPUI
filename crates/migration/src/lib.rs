// SPDX-FileCopyrightText: 2026 AprilNEA LLC
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

pub use sea_orm_migration::prelude::*;

mod m20260118_01_create_tables;
mod m20260119_01_create_attachments;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260118_01_create_tables::Migration),
            Box::new(m20260119_01_create_attachments::Migration),
        ]
    }
}
