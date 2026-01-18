// SPDX-FileCopyrightText: 2026 AprilNEA LLC
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

pub mod entity;
pub mod manager;
pub mod migration;
pub mod repository;
pub mod service;

pub use entity::{conversation, message};
pub use service::{get_db, init, DatabaseService};
