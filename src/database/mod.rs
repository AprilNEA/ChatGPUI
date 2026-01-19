// SPDX-FileCopyrightText: 2026 AprilNEA LLC
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

pub mod manager;
pub mod repository;
pub mod service;

pub use entity::{attachment, conversation, message};
#[allow(unused_imports)]
pub use service::{DatabaseService, get_db, init};
