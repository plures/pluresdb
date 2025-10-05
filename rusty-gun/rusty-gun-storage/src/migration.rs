//! Database migration system for Rusty Gun Storage

use crate::error::{Result, StorageError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{info, warn};

/// Migration version type
pub type MigrationVersion = u32;

/// Migration status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MigrationStatus {
    /// Migration is pending
    Pending,
    /// Migration is running
    Running,
    /// Migration completed successfully
    Completed,
    /// Migration failed
    Failed,
    /// Migration was rolled back
    RolledBack,
}

/// A database migration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Migration {
    /// Migration version number
    pub version: MigrationVersion,
    /// Migration name
    pub name: String,
    /// Migration description
    pub description: String,
    /// SQL up migration
    pub up_sql: String,
    /// SQL down migration
    pub down_sql: String,
    /// Migration status
    pub status: MigrationStatus,
    /// When the migration was created
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// When the migration was applied
    pub applied_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Migration manager
pub struct MigrationManager {
    migrations: HashMap<MigrationVersion, Migration>,
    current_version: MigrationVersion,
}

impl MigrationManager {
    /// Create a new migration manager
    pub fn new() -> Self {
        Self {
            migrations: HashMap::new(),
            current_version: 0,
        }
    }

    /// Add a migration
    pub fn add_migration(&mut self, migration: Migration) {
        self.migrations.insert(migration.version, migration);
    }

    /// Get all migrations sorted by version
    pub fn get_migrations(&self) -> Vec<&Migration> {
        let mut migrations: Vec<&Migration> = self.migrations.values().collect();
        migrations.sort_by_key(|m| m.version);
        migrations
    }

    /// Get pending migrations
    pub fn get_pending_migrations(&self) -> Vec<&Migration> {
        self.get_migrations()
            .into_iter()
            .filter(|m| m.status == MigrationStatus::Pending)
            .collect()
    }

    /// Get completed migrations
    pub fn get_completed_migrations(&self) -> Vec<&Migration> {
        self.get_migrations()
            .into_iter()
            .filter(|m| m.status == MigrationStatus::Completed)
            .collect()
    }

    /// Get current database version
    pub fn get_current_version(&self) -> MigrationVersion {
        self.current_version
    }

    /// Set current database version
    pub fn set_current_version(&mut self, version: MigrationVersion) {
        self.current_version = version;
    }

    /// Check if migrations are needed
    pub fn needs_migration(&self) -> bool {
        !self.get_pending_migrations().is_empty()
    }

    /// Get the latest migration version
    pub fn get_latest_version(&self) -> Option<MigrationVersion> {
        self.migrations.keys().max().copied()
    }
}

/// Built-in migrations for Rusty Gun
pub struct BuiltinMigrations;

impl BuiltinMigrations {
    /// Get all built-in migrations
    pub fn get_migrations() -> Vec<Migration> {
        vec![
            Migration {
                version: 1,
                name: "create_initial_tables".to_string(),
                description: "Create initial database tables".to_string(),
                up_sql: r#"
                    CREATE TABLE IF NOT EXISTS nodes (
                        id TEXT PRIMARY KEY,
                        data TEXT NOT NULL,
                        metadata TEXT NOT NULL,
                        created_at DATETIME NOT NULL,
                        updated_at DATETIME NOT NULL,
                        created_by TEXT NOT NULL,
                        updated_by TEXT NOT NULL,
                        node_type TEXT,
                        deleted BOOLEAN NOT NULL DEFAULT FALSE
                    );

                    CREATE TABLE IF NOT EXISTS relationships (
                        id TEXT PRIMARY KEY,
                        from_node TEXT NOT NULL,
                        to_node TEXT NOT NULL,
                        relation_type TEXT NOT NULL,
                        data TEXT,
                        created_at DATETIME NOT NULL,
                        created_by TEXT NOT NULL,
                        FOREIGN KEY (from_node) REFERENCES nodes (id),
                        FOREIGN KEY (to_node) REFERENCES nodes (id)
                    );

                    CREATE TABLE IF NOT EXISTS node_tags (
                        node_id TEXT NOT NULL,
                        tag TEXT NOT NULL,
                        PRIMARY KEY (node_id, tag),
                        FOREIGN KEY (node_id) REFERENCES nodes (id)
                    );

                    CREATE TABLE IF NOT EXISTS migrations (
                        version INTEGER PRIMARY KEY,
                        name TEXT NOT NULL,
                        description TEXT NOT NULL,
                        applied_at DATETIME NOT NULL
                    );
                "#.to_string(),
                down_sql: r#"
                    DROP TABLE IF EXISTS node_tags;
                    DROP TABLE IF EXISTS relationships;
                    DROP TABLE IF EXISTS nodes;
                    DROP TABLE IF EXISTS migrations;
                "#.to_string(),
                status: MigrationStatus::Pending,
                created_at: chrono::Utc::now(),
                applied_at: None,
            },
            Migration {
                version: 2,
                name: "create_indexes".to_string(),
                description: "Create database indexes for performance".to_string(),
                up_sql: r#"
                    CREATE INDEX IF NOT EXISTS idx_nodes_type ON nodes (node_type);
                    CREATE INDEX IF NOT EXISTS idx_nodes_created_at ON nodes (created_at);
                    CREATE INDEX IF NOT EXISTS idx_nodes_updated_at ON nodes (updated_at);
                    CREATE INDEX IF NOT EXISTS idx_relationships_from ON relationships (from_node);
                    CREATE INDEX IF NOT EXISTS idx_relationships_to ON relationships (to_node);
                    CREATE INDEX IF NOT EXISTS idx_relationships_type ON relationships (relation_type);
                    CREATE INDEX IF NOT EXISTS idx_node_tags_tag ON node_tags (tag);
                "#.to_string(),
                down_sql: r#"
                    DROP INDEX IF EXISTS idx_node_tags_tag;
                    DROP INDEX IF EXISTS idx_relationships_type;
                    DROP INDEX IF EXISTS idx_relationships_to;
                    DROP INDEX IF EXISTS idx_relationships_from;
                    DROP INDEX IF EXISTS idx_nodes_updated_at;
                    DROP INDEX IF EXISTS idx_nodes_created_at;
                    DROP INDEX IF EXISTS idx_nodes_type;
                "#.to_string(),
                status: MigrationStatus::Pending,
                created_at: chrono::Utc::now(),
                applied_at: None,
            },
            Migration {
                version: 3,
                name: "add_vector_search_tables".to_string(),
                description: "Add tables for vector search functionality".to_string(),
                up_sql: r#"
                    CREATE TABLE IF NOT EXISTS vectors (
                        id TEXT PRIMARY KEY,
                        vector_data BLOB NOT NULL,
                        metadata TEXT NOT NULL,
                        created_at DATETIME NOT NULL,
                        updated_at DATETIME NOT NULL
                    );

                    CREATE INDEX IF NOT EXISTS idx_vectors_created_at ON vectors (created_at);
                "#.to_string(),
                down_sql: r#"
                    DROP INDEX IF EXISTS idx_vectors_created_at;
                    DROP TABLE IF EXISTS vectors;
                "#.to_string(),
                status: MigrationStatus::Pending,
                created_at: chrono::Utc::now(),
                applied_at: None,
            },
        ]
    }
}

/// Migration runner trait
#[async_trait::async_trait]
pub trait MigrationRunner {
    /// Run all pending migrations
    async fn run_migrations(&mut self) -> Result<()>;

    /// Rollback to a specific version
    async fn rollback_to(&mut self, version: MigrationVersion) -> Result<()>;

    /// Get current migration status
    async fn get_migration_status(&self) -> Result<Vec<Migration>>;

    /// Mark a migration as completed
    async fn mark_migration_completed(&mut self, version: MigrationVersion) -> Result<()>;

    /// Mark a migration as failed
    async fn mark_migration_failed(&mut self, version: MigrationVersion, error: &str) -> Result<()>;
}

/// SQLite migration runner
pub struct SqliteMigrationRunner {
    pool: sqlx::SqlitePool,
    manager: MigrationManager,
}

impl SqliteMigrationRunner {
    /// Create a new SQLite migration runner
    pub fn new(pool: sqlx::SqlitePool) -> Self {
        let mut manager = MigrationManager::new();
        
        // Add built-in migrations
        for migration in BuiltinMigrations::get_migrations() {
            manager.add_migration(migration);
        }

        Self { pool, manager }
    }

    /// Load migration status from database
    async fn load_migration_status(&mut self) -> Result<()> {
        // Try to create migrations table if it doesn't exist
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS migrations (
                version INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT NOT NULL,
                applied_at DATETIME NOT NULL
            )
            "#
        )
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::MigrationFailed(format!("Failed to create migrations table: {}", e)))?;

        // Load applied migrations
        let rows = sqlx::query("SELECT version FROM migrations ORDER BY version")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| StorageError::MigrationFailed(format!("Failed to load migration status: {}", e)))?;

        let mut current_version = 0;
        for row in rows {
            let version: i64 = row.get("version");
            current_version = current_version.max(version as u32);
        }

        self.manager.set_current_version(current_version);
        info!("Loaded migration status: current version = {}", current_version);
        Ok(())
    }
}

#[async_trait::async_trait]
impl MigrationRunner for SqliteMigrationRunner {
    async fn run_migrations(&mut self) -> Result<()> {
        self.load_migration_status().await?;

        let pending_migrations = self.manager.get_pending_migrations();
        if pending_migrations.is_empty() {
            info!("No pending migrations");
            return Ok(());
        }

        info!("Running {} pending migrations", pending_migrations.len());

        for migration in pending_migrations {
            info!("Running migration {}: {}", migration.version, migration.name);
            
            // Mark migration as running
            if let Some(m) = self.manager.migrations.get_mut(&migration.version) {
                m.status = MigrationStatus::Running;
            }

            // Execute migration
            match sqlx::query(&migration.up_sql).execute(&self.pool).await {
                Ok(_) => {
                    // Mark as completed
                    if let Some(m) = self.manager.migrations.get_mut(&migration.version) {
                        m.status = MigrationStatus::Completed;
                        m.applied_at = Some(chrono::Utc::now());
                    }

                    // Record in migrations table
                    sqlx::query(
                        "INSERT OR REPLACE INTO migrations (version, name, description, applied_at) VALUES (?, ?, ?, ?)"
                    )
                    .bind(migration.version as i64)
                    .bind(&migration.name)
                    .bind(&migration.description)
                    .bind(chrono::Utc::now())
                    .execute(&self.pool)
                    .await
                    .map_err(|e| StorageError::MigrationFailed(format!("Failed to record migration: {}", e)))?;

                    info!("Migration {} completed successfully", migration.version);
                }
                Err(e) => {
                    // Mark as failed
                    if let Some(m) = self.manager.migrations.get_mut(&migration.version) {
                        m.status = MigrationStatus::Failed;
                    }

                    warn!("Migration {} failed: {}", migration.version, e);
                    return Err(StorageError::MigrationFailed(format!(
                        "Migration {} failed: {}",
                        migration.version, e
                    )));
                }
            }
        }

        info!("All migrations completed successfully");
        Ok(())
    }

    async fn rollback_to(&mut self, version: MigrationVersion) -> Result<()> {
        self.load_migration_status().await?;

        let completed_migrations = self.manager.get_completed_migrations();
        let migrations_to_rollback: Vec<&Migration> = completed_migrations
            .into_iter()
            .filter(|m| m.version > version)
            .rev() // Rollback in reverse order
            .collect();

        if migrations_to_rollback.is_empty() {
            info!("No migrations to rollback");
            return Ok(());
        }

        info!("Rolling back {} migrations to version {}", migrations_to_rollback.len(), version);

        for migration in migrations_to_rollback {
            info!("Rolling back migration {}: {}", migration.version, migration.name);

            // Execute rollback
            match sqlx::query(&migration.down_sql).execute(&self.pool).await {
                Ok(_) => {
                    // Remove from migrations table
                    sqlx::query("DELETE FROM migrations WHERE version = ?")
                        .bind(migration.version as i64)
                        .execute(&self.pool)
                        .await
                        .map_err(|e| StorageError::MigrationFailed(format!("Failed to remove migration record: {}", e)))?;

                    // Mark as rolled back
                    if let Some(m) = self.manager.migrations.get_mut(&migration.version) {
                        m.status = MigrationStatus::RolledBack;
                        m.applied_at = None;
                    }

                    info!("Migration {} rolled back successfully", migration.version);
                }
                Err(e) => {
                    warn!("Failed to rollback migration {}: {}", migration.version, e);
                    return Err(StorageError::MigrationFailed(format!(
                        "Failed to rollback migration {}: {}",
                        migration.version, e
                    )));
                }
            }
        }

        info!("Rollback completed successfully");
        Ok(())
    }

    async fn get_migration_status(&self) -> Result<Vec<Migration>> {
        Ok(self.manager.get_migrations().into_iter().cloned().collect())
    }

    async fn mark_migration_completed(&mut self, version: MigrationVersion) -> Result<()> {
        if let Some(migration) = self.manager.migrations.get_mut(&version) {
            migration.status = MigrationStatus::Completed;
            migration.applied_at = Some(chrono::Utc::now());
        }
        Ok(())
    }

    async fn mark_migration_failed(&mut self, version: MigrationVersion, error: &str) -> Result<()> {
        if let Some(migration) = self.manager.migrations.get_mut(&version) {
            migration.status = MigrationStatus::Failed;
        }
        warn!("Migration {} failed: {}", version, error);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_migration_manager() {
        let mut manager = MigrationManager::new();
        
        let migration = Migration {
            version: 1,
            name: "test_migration".to_string(),
            description: "Test migration".to_string(),
            up_sql: "CREATE TABLE test (id INTEGER)".to_string(),
            down_sql: "DROP TABLE test".to_string(),
            status: MigrationStatus::Pending,
            created_at: chrono::Utc::now(),
            applied_at: None,
        };

        manager.add_migration(migration);
        assert_eq!(manager.get_migrations().len(), 1);
        assert_eq!(manager.get_pending_migrations().len(), 1);
        assert!(manager.needs_migration());
    }

    #[test]
    fn test_builtin_migrations() {
        let migrations = BuiltinMigrations::get_migrations();
        assert!(!migrations.is_empty());
        
        // Check that versions are sequential
        for (i, migration) in migrations.iter().enumerate() {
            assert_eq!(migration.version, (i + 1) as u32);
        }
    }
}


