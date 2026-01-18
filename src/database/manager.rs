// SPDX-FileCopyrightText: 2026 AprilNEA LLC
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

use anyhow::Result;
use postgresql_embedded::{PostgreSQL, Settings};
use rand::Rng;
use rand::distr::Alphanumeric;
use sea_orm::{Database, DatabaseConnection};
use sea_orm_migration::MigratorTrait;
use std::path::PathBuf;

use migration::Migrator;

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

        // Check for existing PostgreSQL process and handle it
        let pid_file = pg_data_dir.join("postmaster.pid");
        if pid_file.exists() {
            #[cfg(unix)]
            {
                if let Some(pid) = Self::read_pid_from_file(&pid_file) {
                    let process_running = unsafe { libc::kill(pid, 0) } == 0;
                    if process_running {
                        tracing::info!(
                            "PostgreSQL process {} is still running, stopping it...",
                            pid
                        );
                        // Send SIGTERM to gracefully stop PostgreSQL
                        unsafe { libc::kill(pid, libc::SIGTERM) };
                        // Wait for process to exit (up to 10 seconds)
                        for _ in 0..100 {
                            std::thread::sleep(std::time::Duration::from_millis(100));
                            if unsafe { libc::kill(pid, 0) } != 0 {
                                tracing::info!("PostgreSQL process {} stopped", pid);
                                break;
                            }
                        }
                        // Clean up shared memory after process stops
                        Self::cleanup_shared_memory(&pid_file);
                    } else {
                        tracing::warn!("Found stale postmaster.pid, cleaning up...");
                        Self::cleanup_shared_memory(&pid_file);
                    }
                }
            }
            let _ = std::fs::remove_file(&pid_file);
        }

        // Get or create password based on storage mode setting
        let password = Self::get_or_create_password(&data_dir)?;
        tracing::debug!("Password obtained, length: {}", password.len());

        let settings = Settings {
            installation_dir: data_dir.join("postgresql"),
            password_file: data_dir.join(".pgpass"),
            data_dir: pg_data_dir,
            temporary: false,
            username: "postgres".to_string(),
            password,
            ..Default::default()
        };

        tracing::info!("Initializing embedded PostgreSQL...");
        let mut postgres = PostgreSQL::new(settings);
        postgres.setup().await?;
        postgres.start().await?;

        tracing::info!("PostgreSQL started on port {}", postgres.settings().port);
        tracing::debug!(
            "PostgreSQL settings - username: {}, password length: {}",
            postgres.settings().username,
            postgres.settings().password.len()
        );

        Ok(Self {
            postgres,
            connection: None,
        })
    }

    /// Get or create password from .pgpass file
    fn get_or_create_password(data_dir: &PathBuf) -> Result<String> {
        let pgpass_file = data_dir.join(".pgpass");

        // Try to read existing password
        if pgpass_file.exists() {
            if let Ok(content) = std::fs::read_to_string(&pgpass_file) {
                let password = content.trim().to_string();
                if !password.is_empty() {
                    tracing::info!("Retrieved database password from .pgpass file");
                    return Ok(password);
                }
            }
        }

        // Generate new password (postgresql_embedded will write it to .pgpass)
        let password = Self::generate_password();
        tracing::info!("Generated new database password");
        Ok(password)
    }

    /// Generate a secure random password
    fn generate_password() -> String {
        rand::rng()
            .sample_iter(&Alphanumeric)
            .take(32)
            .map(char::from)
            .collect()
    }

    /// Read PID from postmaster.pid file
    #[cfg(unix)]
    fn read_pid_from_file(pid_file: &std::path::Path) -> Option<i32> {
        std::fs::read_to_string(pid_file)
            .ok()
            .and_then(|contents| contents.lines().next().map(|s| s.to_string()))
            .and_then(|pid_str| pid_str.trim().parse::<i32>().ok())
    }

    /// Clean up stale shared memory segments from a crashed PostgreSQL
    #[cfg(unix)]
    fn cleanup_shared_memory(_pid_file: &std::path::Path) {
        tracing::info!("Cleaning up shared memory segments...");
        // Use ipcrm to clean up shared memory segments owned by current user
        // This is a best-effort cleanup
        let _ = std::process::Command::new("ipcrm").args(["-a"]).output();
    }

    /// Get the data directory for the database
    fn get_data_dir() -> Result<PathBuf> {
        let data_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(env!("APP_IDENTIFIER"))
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
