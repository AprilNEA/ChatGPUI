// SPDX-FileCopyrightText: 2026 AprilNEA LLC
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

use anyhow::Result;
use keyring::Entry;
use postgresql_embedded::{PostgreSQL, Settings};
use rand::Rng;
use rand::distr::Alphanumeric;
use sea_orm::{Database, DatabaseConnection};
use sea_orm_migration::MigratorTrait;
use std::path::PathBuf;

use migration::Migrator;

const KEYRING_SERVICE: &str = "com.aprilnea.chatgpui";
const KEYRING_USER: &str = "postgres";

/// Database manager that handles embedded PostgreSQL and SeaORM connection
pub struct DatabaseManager {
    postgres: PostgreSQL,
    connection: Option<DatabaseConnection>,
}

impl DatabaseManager {
    /// Create a new database manager with embedded PostgreSQL
    pub async fn new() -> Result<Self> {
        let data_dir = Self::get_data_dir()?;
        let pg_data_dir = data_dir.join("data");

        // Clean up stale lock file if no postgres process is running
        let pid_file = pg_data_dir.join("postmaster.pid");
        if pid_file.exists() {
            tracing::warn!("Found stale postmaster.pid, cleaning up...");
            let _ = std::fs::remove_file(&pid_file);
        }

        // Get or create password from system keychain
        let password = Self::get_or_create_password()?;

        let settings = Settings {
            installation_dir: data_dir.join("postgresql"),
            password_file: data_dir.join(".pgpass"),
            data_dir: pg_data_dir,
            temporary: false,
            username: KEYRING_USER.to_string(),
            password,
            ..Default::default()
        };

        tracing::info!("Initializing embedded PostgreSQL...");
        let mut postgres = PostgreSQL::new(settings);
        postgres.setup().await?;
        postgres.start().await?;

        tracing::info!("PostgreSQL started on port {}", postgres.settings().port);

        Ok(Self {
            postgres,
            connection: None,
        })
    }

    /// Get password from keychain or create a new one
    fn get_or_create_password() -> Result<String> {
        let entry = Entry::new(KEYRING_SERVICE, KEYRING_USER)
            .map_err(|e| anyhow::anyhow!("Failed to create keyring entry: {}", e))?;

        // Try to get existing password
        match entry.get_password() {
            Ok(password) => {
                tracing::info!("Retrieved database password from system keychain");
                Ok(password)
            }
            Err(keyring::Error::NoEntry) => {
                // Generate a new secure password
                let password: String = rand::thread_rng()
                    .sample_iter(&Alphanumeric)
                    .take(32)
                    .map(char::from)
                    .collect();

                // Store in keychain
                entry
                    .set_password(&password)
                    .map_err(|e| anyhow::anyhow!("Failed to store password in keychain: {}", e))?;

                tracing::info!("Generated and stored new database password in system keychain");
                Ok(password)
            }
            Err(e) => Err(anyhow::anyhow!("Failed to access keychain: {}", e)),
        }
    }

    /// Get the data directory for the database
    fn get_data_dir() -> Result<PathBuf> {
        let data_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("chatgpui")
            .join("db");

        std::fs::create_dir_all(&data_dir)?;
        Ok(data_dir)
    }

    /// Initialize the database connection and run migrations
    pub async fn initialize(&mut self) -> Result<()> {
        let database_name = "chatgpui";

        // Create database if not exists
        self.create_database_if_not_exists(database_name).await?;

        // Connect to the database
        let connection_url = self.get_connection_url(database_name);
        tracing::info!("Connecting to database...");

        let connection = Database::connect(&connection_url).await?;

        // Run migrations
        tracing::info!("Running database migrations...");
        Migrator::up(&connection, None).await?;
        tracing::info!("Migrations completed successfully");

        self.connection = Some(connection);
        Ok(())
    }

    /// Create the database if it doesn't exist
    async fn create_database_if_not_exists(&self, database_name: &str) -> Result<()> {
        let postgres_url = self.get_connection_url("postgres");
        let conn = Database::connect(&postgres_url).await?;

        use sea_orm::ConnectionTrait;
        let result = conn
            .execute_unprepared(&format!(
                "SELECT 1 FROM pg_database WHERE datname = '{}'",
                database_name
            ))
            .await?;

        if result.rows_affected() == 0 {
            tracing::info!("Creating database '{}'...", database_name);
            conn.execute_unprepared(&format!("CREATE DATABASE {}", database_name))
                .await?;
        }

        Ok(())
    }

    /// Get the connection URL for a database
    fn get_connection_url(&self, database_name: &str) -> String {
        let settings = self.postgres.settings();
        format!(
            "postgres://{}:{}@{}:{}/{}",
            settings.username, settings.password, settings.host, settings.port, database_name
        )
    }

    /// Get the database connection
    pub fn connection(&self) -> Option<&DatabaseConnection> {
        self.connection.as_ref()
    }

    /// Shutdown the database
    pub async fn shutdown(&mut self) -> Result<()> {
        if let Some(conn) = self.connection.take() {
            drop(conn);
        }

        tracing::info!("Stopping PostgreSQL...");
        self.postgres.stop().await?;
        Ok(())
    }
}

impl Drop for DatabaseManager {
    fn drop(&mut self) {
        // Note: async shutdown should be called explicitly before drop
        tracing::debug!("DatabaseManager dropped");
    }
}
