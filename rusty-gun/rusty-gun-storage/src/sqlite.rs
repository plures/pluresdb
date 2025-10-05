//! SQLite storage engine implementation

use crate::{
    error::{Result, StorageError},
    traits::{StorageEngine, Transaction, QueryResult, StorageStats},
    StorageConfig,
};
use rusty_gun_core::{Node, NodeId, types::*};
use serde_json::Value;
use sqlx::{SqlitePool, Sqlite, Row, Transaction as SqliteTransaction};
use std::collections::HashMap;
use tracing::{debug, error, info};

/// SQLite storage engine
pub struct SqliteStorage {
    pool: SqlitePool,
    config: StorageConfig,
}

impl SqliteStorage {
    /// Create a new SQLite storage engine
    pub async fn new(config: StorageConfig) -> Result<Self> {
        let database_url = format!("sqlite:{}", config.path);
        
        let pool = SqlitePool::connect(&database_url)
            .await
            .map_err(|e| StorageError::ConnectionFailed(e.to_string()))?;

        let mut storage = Self { pool, config };
        storage.initialize().await?;
        Ok(storage)
    }

    /// Create tables if they don't exist
    async fn create_tables(&self) -> Result<()> {
        let create_nodes_table = r#"
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
            )
        "#;

        let create_relationships_table = r#"
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
            )
        "#;

        let create_node_tags_table = r#"
            CREATE TABLE IF NOT EXISTS node_tags (
                node_id TEXT NOT NULL,
                tag TEXT NOT NULL,
                PRIMARY KEY (node_id, tag),
                FOREIGN KEY (node_id) REFERENCES nodes (id)
            )
        "#;

        let create_indexes = r#"
            CREATE INDEX IF NOT EXISTS idx_nodes_type ON nodes (node_type);
            CREATE INDEX IF NOT EXISTS idx_nodes_created_at ON nodes (created_at);
            CREATE INDEX IF NOT EXISTS idx_nodes_updated_at ON nodes (updated_at);
            CREATE INDEX IF NOT EXISTS idx_relationships_from ON relationships (from_node);
            CREATE INDEX IF NOT EXISTS idx_relationships_to ON relationships (to_node);
            CREATE INDEX IF NOT EXISTS idx_relationships_type ON relationships (relation_type);
            CREATE INDEX IF NOT EXISTS idx_node_tags_tag ON node_tags (tag);
        "#;

        // Execute table creation
        sqlx::query(create_nodes_table)
            .execute(&self.pool)
            .await
            .map_err(|e| StorageError::QueryFailed(format!("Failed to create nodes table: {}", e)))?;

        sqlx::query(create_relationships_table)
            .execute(&self.pool)
            .await
            .map_err(|e| StorageError::QueryFailed(format!("Failed to create relationships table: {}", e)))?;

        sqlx::query(create_node_tags_table)
            .execute(&self.pool)
            .await
            .map_err(|e| StorageError::QueryFailed(format!("Failed to create node_tags table: {}", e)))?;

        // Execute index creation
        sqlx::query(create_indexes)
            .execute(&self.pool)
            .await
            .map_err(|e| StorageError::QueryFailed(format!("Failed to create indexes: {}", e)))?;

        info!("SQLite tables created successfully");
        Ok(())
    }

    /// Enable WAL mode and foreign keys
    async fn configure_database(&self) -> Result<()> {
        if self.config.enable_wal {
            sqlx::query("PRAGMA journal_mode=WAL")
                .execute(&self.pool)
                .await
                .map_err(|e| StorageError::QueryFailed(format!("Failed to enable WAL mode: {}", e)))?;
        }

        if self.config.enable_foreign_keys {
            sqlx::query("PRAGMA foreign_keys=ON")
                .execute(&self.pool)
                .await
                .map_err(|e| StorageError::QueryFailed(format!("Failed to enable foreign keys: {}", e)))?;
        }

        // Set other useful pragmas
        sqlx::query("PRAGMA synchronous=NORMAL")
            .execute(&self.pool)
            .await
            .map_err(|e| StorageError::QueryFailed(format!("Failed to set synchronous mode: {}", e)))?;

        sqlx::query("PRAGMA cache_size=10000")
            .execute(&self.pool)
            .await
            .map_err(|e| StorageError::QueryFailed(format!("Failed to set cache size: {}", e)))?;

        Ok(())
    }
}

#[async_trait::async_trait]
impl StorageEngine for SqliteStorage {
    async fn initialize(&mut self) -> Result<()> {
        self.configure_database().await?;
        self.create_tables().await?;
        info!("SQLite storage engine initialized");
        Ok(())
    }

    async fn close(&mut self) -> Result<()> {
        self.pool.close().await;
        info!("SQLite storage engine closed");
        Ok(())
    }

    async fn store_node(&self, node: &Node) -> Result<()> {
        let mut tx = self.pool.begin().await
            .map_err(|e| StorageError::TransactionFailed(e.to_string()))?;

        // Serialize node data and metadata
        let data_json = serde_json::to_string(&node.data.data)
            .map_err(|e| StorageError::Serialization(e))?;
        let metadata_json = serde_json::to_string(&node.data.metadata)
            .map_err(|e| StorageError::Serialization(e))?;

        // Insert or update node
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO nodes 
            (id, data, metadata, created_at, updated_at, created_by, updated_by, node_type, deleted)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(node.id())
        .bind(&data_json)
        .bind(&metadata_json)
        .bind(node.data.metadata.created_at)
        .bind(node.data.metadata.updated_at)
        .bind(&node.data.metadata.created_by)
        .bind(&node.data.metadata.updated_by)
        .bind(&node.data.metadata.node_type)
        .bind(node.data.metadata.deleted)
        .execute(&mut *tx)
        .await
        .map_err(|e| StorageError::QueryFailed(format!("Failed to store node: {}", e)))?;

        // Update tags
        sqlx::query("DELETE FROM node_tags WHERE node_id = ?")
            .bind(node.id())
            .execute(&mut *tx)
            .await
            .map_err(|e| StorageError::QueryFailed(format!("Failed to delete old tags: {}", e)))?;

        for tag in &node.data.metadata.tags {
            sqlx::query("INSERT INTO node_tags (node_id, tag) VALUES (?, ?)")
                .bind(node.id())
                .bind(tag)
                .execute(&mut *tx)
                .await
                .map_err(|e| StorageError::QueryFailed(format!("Failed to insert tag: {}", e)))?;
        }

        tx.commit().await
            .map_err(|e| StorageError::TransactionFailed(e.to_string()))?;

        debug!("Stored node: {}", node.id());
        Ok(())
    }

    async fn load_node(&self, node_id: &NodeId) -> Result<Option<Node>> {
        let row = sqlx::query(
            "SELECT data, metadata FROM nodes WHERE id = ? AND deleted = FALSE"
        )
        .bind(node_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| StorageError::QueryFailed(format!("Failed to load node: {}", e)))?;

        if let Some(row) = row {
            let data_json: String = row.get("data");
            let metadata_json: String = row.get("metadata");

            let data: serde_json::Value = serde_json::from_str(&data_json)
                .map_err(|e| StorageError::Serialization(e))?;
            let metadata: NodeMetadata = serde_json::from_str(&metadata_json)
                .map_err(|e| StorageError::Serialization(e))?;

            let node = Node {
                data: NodeData { data, metadata },
            };

            debug!("Loaded node: {}", node_id);
            Ok(Some(node))
        } else {
            Ok(None)
        }
    }

    async fn delete_node(&self, node_id: &NodeId) -> Result<()> {
        sqlx::query("UPDATE nodes SET deleted = TRUE WHERE id = ?")
            .bind(node_id)
            .execute(&self.pool)
            .await
            .map_err(|e| StorageError::QueryFailed(format!("Failed to delete node: {}", e)))?;

        debug!("Deleted node: {}", node_id);
        Ok(())
    }

    async fn list_node_ids(&self) -> Result<Vec<NodeId>> {
        let rows = sqlx::query("SELECT id FROM nodes WHERE deleted = FALSE")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| StorageError::QueryFailed(format!("Failed to list node IDs: {}", e)))?;

        let ids: Vec<NodeId> = rows.into_iter()
            .map(|row| row.get("id"))
            .collect();

        Ok(ids)
    }

    async fn list_nodes_by_type(&self, node_type: &str) -> Result<Vec<Node>> {
        let rows = sqlx::query(
            "SELECT data, metadata FROM nodes WHERE node_type = ? AND deleted = FALSE"
        )
        .bind(node_type)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StorageError::QueryFailed(format!("Failed to list nodes by type: {}", e)))?;

        let mut nodes = Vec::new();
        for row in rows {
            let data_json: String = row.get("data");
            let metadata_json: String = row.get("metadata");

            let data: serde_json::Value = serde_json::from_str(&data_json)
                .map_err(|e| StorageError::Serialization(e))?;
            let metadata: NodeMetadata = serde_json::from_str(&metadata_json)
                .map_err(|e| StorageError::Serialization(e))?;

            nodes.push(Node {
                data: NodeData { data, metadata },
            });
        }

        Ok(nodes)
    }

    async fn list_nodes_by_tag(&self, tag: &str) -> Result<Vec<Node>> {
        let rows = sqlx::query(
            r#"
            SELECT n.data, n.metadata 
            FROM nodes n 
            JOIN node_tags nt ON n.id = nt.node_id 
            WHERE nt.tag = ? AND n.deleted = FALSE
            "#
        )
        .bind(tag)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StorageError::QueryFailed(format!("Failed to list nodes by tag: {}", e)))?;

        let mut nodes = Vec::new();
        for row in rows {
            let data_json: String = row.get("data");
            let metadata_json: String = row.get("metadata");

            let data: serde_json::Value = serde_json::from_str(&data_json)
                .map_err(|e| StorageError::Serialization(e))?;
            let metadata: NodeMetadata = serde_json::from_str(&metadata_json)
                .map_err(|e| StorageError::Serialization(e))?;

            nodes.push(Node {
                data: NodeData { data, metadata },
            });
        }

        Ok(nodes)
    }

    async fn search_nodes(&self, query: &str) -> Result<Vec<Node>> {
        let search_query = format!("%{}%", query);
        let rows = sqlx::query(
            "SELECT data, metadata FROM nodes WHERE data LIKE ? AND deleted = FALSE"
        )
        .bind(&search_query)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StorageError::QueryFailed(format!("Failed to search nodes: {}", e)))?;

        let mut nodes = Vec::new();
        for row in rows {
            let data_json: String = row.get("data");
            let metadata_json: String = row.get("metadata");

            let data: serde_json::Value = serde_json::from_str(&data_json)
                .map_err(|e| StorageError::Serialization(e))?;
            let metadata: NodeMetadata = serde_json::from_str(&metadata_json)
                .map_err(|e| StorageError::Serialization(e))?;

            nodes.push(Node {
                data: NodeData { data, metadata },
            });
        }

        Ok(nodes)
    }

    async fn store_relationship(&self, relationship: &Relationship) -> Result<()> {
        let relationship_id = format!("{}:{}:{}", relationship.from, relationship.to, relationship.relation_type);
        let data_json = relationship.data.as_ref()
            .map(|d| serde_json::to_string(d).unwrap_or_default())
            .unwrap_or_default();

        sqlx::query(
            r#"
            INSERT OR REPLACE INTO relationships 
            (id, from_node, to_node, relation_type, data, created_at, created_by)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(&relationship_id)
        .bind(&relationship.from)
        .bind(&relationship.to)
        .bind(&relationship.relation_type)
        .bind(&data_json)
        .bind(relationship.created_at)
        .bind(&relationship.created_by)
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::QueryFailed(format!("Failed to store relationship: {}", e)))?;

        debug!("Stored relationship: {} -> {}", relationship.from, relationship.to);
        Ok(())
    }

    async fn load_relationships(&self, node_id: &NodeId) -> Result<Vec<Relationship>> {
        let rows = sqlx::query(
            "SELECT from_node, to_node, relation_type, data, created_at, created_by FROM relationships WHERE from_node = ? OR to_node = ?"
        )
        .bind(node_id)
        .bind(node_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StorageError::QueryFailed(format!("Failed to load relationships: {}", e)))?;

        let mut relationships = Vec::new();
        for row in rows {
            let data_json: String = row.get("data");
            let data = if data_json.is_empty() {
                None
            } else {
                Some(serde_json::from_str(&data_json)
                    .map_err(|e| StorageError::Serialization(e))?)
            };

            relationships.push(Relationship {
                from: row.get("from_node"),
                to: row.get("to_node"),
                relation_type: row.get("relation_type"),
                data,
                created_at: row.get("created_at"),
                created_by: row.get("created_by"),
            });
        }

        Ok(relationships)
    }

    async fn delete_relationship(
        &self,
        from: &NodeId,
        to: &NodeId,
        relation_type: &str,
    ) -> Result<()> {
        let relationship_id = format!("{}:{}:{}", from, to, relation_type);
        
        sqlx::query("DELETE FROM relationships WHERE id = ?")
            .bind(&relationship_id)
            .execute(&self.pool)
            .await
            .map_err(|e| StorageError::QueryFailed(format!("Failed to delete relationship: {}", e)))?;

        debug!("Deleted relationship: {} -> {} ({})", from, to, relation_type);
        Ok(())
    }

    async fn execute_query(&self, query: &str, params: &[Value]) -> Result<QueryResult> {
        let mut sqlx_query = sqlx::query(query);
        
        // Bind parameters
        for param in params {
            match param {
                Value::String(s) => sqlx_query = sqlx_query.bind(s),
                Value::Number(n) => {
                    if let Some(i) = n.as_i64() {
                        sqlx_query = sqlx_query.bind(i);
                    } else if let Some(f) = n.as_f64() {
                        sqlx_query = sqlx_query.bind(f);
                    } else {
                        sqlx_query = sqlx_query.bind(n.to_string());
                    }
                }
                Value::Bool(b) => sqlx_query = sqlx_query.bind(b),
                Value::Null => sqlx_query = sqlx_query.bind(Option::<String>::None),
                _ => sqlx_query = sqlx_query.bind(param.to_string()),
            }
        }

        let rows = sqlx_query
            .fetch_all(&self.pool)
            .await
            .map_err(|e| StorageError::QueryFailed(format!("Failed to execute query: {}", e)))?;

        let mut result_rows = Vec::new();
        let mut columns = Vec::new();

        if let Some(first_row) = rows.first() {
            columns = first_row.columns().iter().map(|c| c.name().to_string()).collect();
            
            for row in rows {
                let mut row_map = HashMap::new();
                for (i, column) in columns.iter().enumerate() {
                    let value: Value = match row.try_get::<String, _>(i) {
                        Ok(s) => Value::String(s),
                        Err(_) => match row.try_get::<i64, _>(i) {
                            Ok(n) => Value::Number(serde_json::Number::from(n)),
                            Err(_) => match row.try_get::<f64, _>(i) {
                                Ok(f) => Value::Number(serde_json::Number::from_f64(f).unwrap_or(serde_json::Number::from(0))),
                                Err(_) => match row.try_get::<bool, _>(i) {
                                    Ok(b) => Value::Bool(b),
                                    Err(_) => Value::Null,
                                }
                            }
                        }
                    };
                    row_map.insert(column.clone(), value);
                }
                result_rows.push(row_map);
            }
        }

        Ok(QueryResult {
            rows: result_rows,
            columns,
            changes: 0, // SQLite doesn't provide this easily
            last_insert_row_id: 0, // SQLite doesn't provide this easily
        })
    }

    async fn begin_transaction(&self) -> Result<Box<dyn Transaction>> {
        let tx = self.pool.begin().await
            .map_err(|e| StorageError::TransactionFailed(e.to_string()))?;
        
        Ok(Box::new(SqliteTransactionWrapper { tx }))
    }

    async fn get_stats(&self) -> Result<StorageStats> {
        let node_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM nodes WHERE deleted = FALSE")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| StorageError::QueryFailed(format!("Failed to get node count: {}", e)))?;

        let relationship_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM relationships")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| StorageError::QueryFailed(format!("Failed to get relationship count: {}", e)))?;

        let index_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name NOT LIKE 'sqlite_%'"
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| StorageError::QueryFailed(format!("Failed to get index count: {}", e)))?;

        // Get database file size
        let storage_size = std::fs::metadata(&self.config.path)
            .map(|m| m.len())
            .unwrap_or(0);

        Ok(StorageStats {
            node_count: node_count as u64,
            relationship_count: relationship_count as u64,
            storage_size,
            index_count: index_count as u64,
            last_updated: chrono::Utc::now(),
        })
    }
}

/// SQLite transaction wrapper
struct SqliteTransactionWrapper {
    tx: SqliteTransaction<'static, Sqlite>,
}

#[async_trait::async_trait]
impl Transaction for SqliteTransactionWrapper {
    async fn commit(self: Box<Self>) -> Result<()> {
        self.tx.commit().await
            .map_err(|e| StorageError::TransactionFailed(e.to_string()))?;
        Ok(())
    }

    async fn rollback(self: Box<Self>) -> Result<()> {
        self.tx.rollback().await
            .map_err(|e| StorageError::TransactionFailed(e.to_string()))?;
        Ok(())
    }

    async fn store_node(&mut self, node: &Node) -> Result<()> {
        let data_json = serde_json::to_string(&node.data.data)
            .map_err(|e| StorageError::Serialization(e))?;
        let metadata_json = serde_json::to_string(&node.data.metadata)
            .map_err(|e| StorageError::Serialization(e))?;

        sqlx::query(
            r#"
            INSERT OR REPLACE INTO nodes 
            (id, data, metadata, created_at, updated_at, created_by, updated_by, node_type, deleted)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(node.id())
        .bind(&data_json)
        .bind(&metadata_json)
        .bind(node.data.metadata.created_at)
        .bind(node.data.metadata.updated_at)
        .bind(&node.data.metadata.created_by)
        .bind(&node.data.metadata.updated_by)
        .bind(&node.data.metadata.node_type)
        .bind(node.data.metadata.deleted)
        .execute(&mut **self.tx)
        .await
        .map_err(|e| StorageError::QueryFailed(format!("Failed to store node in transaction: {}", e)))?;

        Ok(())
    }

    async fn load_node(&self, node_id: &NodeId) -> Result<Option<Node>> {
        let row = sqlx::query(
            "SELECT data, metadata FROM nodes WHERE id = ? AND deleted = FALSE"
        )
        .bind(node_id)
        .fetch_optional(&mut **self.tx)
        .await
        .map_err(|e| StorageError::QueryFailed(format!("Failed to load node in transaction: {}", e)))?;

        if let Some(row) = row {
            let data_json: String = row.get("data");
            let metadata_json: String = row.get("metadata");

            let data: serde_json::Value = serde_json::from_str(&data_json)
                .map_err(|e| StorageError::Serialization(e))?;
            let metadata: NodeMetadata = serde_json::from_str(&metadata_json)
                .map_err(|e| StorageError::Serialization(e))?;

            Ok(Some(Node {
                data: NodeData { data, metadata },
            }))
        } else {
            Ok(None)
        }
    }

    async fn delete_node(&mut self, node_id: &NodeId) -> Result<()> {
        sqlx::query("UPDATE nodes SET deleted = TRUE WHERE id = ?")
            .bind(node_id)
            .execute(&mut **self.tx)
            .await
            .map_err(|e| StorageError::QueryFailed(format!("Failed to delete node in transaction: {}", e)))?;

        Ok(())
    }

    async fn execute_query(&self, query: &str, params: &[Value]) -> Result<QueryResult> {
        let mut sqlx_query = sqlx::query(query);
        
        for param in params {
            match param {
                Value::String(s) => sqlx_query = sqlx_query.bind(s),
                Value::Number(n) => {
                    if let Some(i) = n.as_i64() {
                        sqlx_query = sqlx_query.bind(i);
                    } else if let Some(f) = n.as_f64() {
                        sqlx_query = sqlx_query.bind(f);
                    } else {
                        sqlx_query = sqlx_query.bind(n.to_string());
                    }
                }
                Value::Bool(b) => sqlx_query = sqlx_query.bind(b),
                Value::Null => sqlx_query = sqlx_query.bind(Option::<String>::None),
                _ => sqlx_query = sqlx_query.bind(param.to_string()),
            }
        }

        let rows = sqlx_query
            .fetch_all(&mut **self.tx)
            .await
            .map_err(|e| StorageError::QueryFailed(format!("Failed to execute query in transaction: {}", e)))?;

        let mut result_rows = Vec::new();
        let mut columns = Vec::new();

        if let Some(first_row) = rows.first() {
            columns = first_row.columns().iter().map(|c| c.name().to_string()).collect();
            
            for row in rows {
                let mut row_map = HashMap::new();
                for (i, column) in columns.iter().enumerate() {
                    let value: Value = match row.try_get::<String, _>(i) {
                        Ok(s) => Value::String(s),
                        Err(_) => match row.try_get::<i64, _>(i) {
                            Ok(n) => Value::Number(serde_json::Number::from(n)),
                            Err(_) => match row.try_get::<f64, _>(i) {
                                Ok(f) => Value::Number(serde_json::Number::from_f64(f).unwrap_or(serde_json::Number::from(0))),
                                Err(_) => match row.try_get::<bool, _>(i) {
                                    Ok(b) => Value::Bool(b),
                                    Err(_) => Value::Null,
                                }
                            }
                        }
                    };
                    row_map.insert(column.clone(), value);
                }
                result_rows.push(row_map);
            }
        }

        Ok(QueryResult {
            rows: result_rows,
            columns,
            changes: 0,
            last_insert_row_id: 0,
        })
    }
}


