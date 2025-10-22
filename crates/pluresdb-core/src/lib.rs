//! Core data structures, CRDT logic, and domain models that power PluresDB.
//!
//! The goal of this crate is to offer a lightweight, dependency-free-on-FFI
//! foundation that can be reused across the native CLI, the Node addon, and
//! any future host integrations.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use parking_lot::Mutex;
use rusqlite::types::{Value as SqliteValue, ValueRef};
use rusqlite::{params_from_iter, Connection, OpenFlags, Transaction};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};
use thiserror::Error;
use tracing::debug;
use uuid::Uuid;

/// Unique identifier for a stored node.
pub type NodeId = String;

/// Logical actor identifier used when merging CRDT updates.
pub type ActorId = String;

/// A key-value map of logical clocks per actor.
pub type VectorClock = HashMap<ActorId, u64>;

/// Arbitrary JSON payload that callers persist inside PluresDB.
pub type NodeData = JsonValue;

/// Metadata associated with a persisted node in the CRDT store.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NodeRecord {
    pub id: NodeId,
    pub data: NodeData,
    pub clock: VectorClock,
    pub timestamp: DateTime<Utc>,
}

impl NodeRecord {
    /// Creates a new node record with a fresh logical clock entry for the actor.
    pub fn new(id: NodeId, actor: impl Into<ActorId>, data: NodeData) -> Self {
        let actor = actor.into();
        let mut clock = VectorClock::default();
        clock.insert(actor.clone(), 1);
        Self {
            id,
            data,
            clock,
            timestamp: Utc::now(),
        }
    }

    /// Increments the logical clock for the given actor and updates the payload.
    pub fn merge_update(&mut self, actor: impl Into<ActorId>, data: NodeData) {
        let actor = actor.into();
        let counter = self.clock.entry(actor).or_insert(0);
        *counter += 1;
        self.timestamp = Utc::now();
        self.data = data;
    }
}

/// Errors that can be produced by the CRDT store.
#[derive(Debug, Error)]
pub enum StoreError {
    #[error("node not found: {0}")]
    NotFound(NodeId),
}

/// CRDT operations that clients may apply to the store.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CrdtOperation {
    Put {
        id: NodeId,
        actor: ActorId,
        data: NodeData,
    },
    Delete {
        id: NodeId,
    },
}

/// A simple conflict-free replicated data store backed by a concurrent map.
#[derive(Debug, Default)]
pub struct CrdtStore {
    nodes: DashMap<NodeId, NodeRecord>,
}

impl CrdtStore {
    /// Inserts or updates a node using CRDT semantics.
    pub fn put(&self, id: impl Into<NodeId>, actor: impl Into<ActorId>, data: NodeData) -> NodeId {
        let id = id.into();
        let actor = actor.into();
        self.nodes
            .entry(id.clone())
            .and_modify(|record| record.merge_update(actor.clone(), data.clone()))
            .or_insert_with(|| NodeRecord::new(id.clone(), actor, data));
        id
    }

    /// Removes a node from the store.
    pub fn delete(&self, id: impl AsRef<str>) -> Result<(), StoreError> {
        self.nodes
            .remove(id.as_ref())
            .map(|_| ())
            .ok_or_else(|| StoreError::NotFound(id.as_ref().to_owned()))
    }

    /// Fetches a node by identifier.
    pub fn get(&self, id: impl AsRef<str>) -> Option<NodeRecord> {
        self.nodes
            .get(id.as_ref())
            .map(|entry| entry.value().clone())
    }

    /// Lists all nodes currently stored.
    pub fn list(&self) -> Vec<NodeRecord> {
        self.nodes
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// Applies a CRDT operation, returning the resulting node identifier when relevant.
    pub fn apply(&self, op: CrdtOperation) -> Result<Option<NodeId>, StoreError> {
        match op {
            CrdtOperation::Put { id, actor, data } => Ok(Some(self.put(id, actor, data))),
            CrdtOperation::Delete { id } => {
                self.delete(&id)?;
                Ok(None)
            }
        }
    }

    /// Generates a CRDT operation representing the provided node data.
    pub fn operation_for(
        &self,
        actor: impl Into<ActorId>,
        data: NodeData,
    ) -> (NodeId, CrdtOperation) {
        let id = Uuid::new_v4().to_string();
        let op = CrdtOperation::Put {
            id: id.clone(),
            actor: actor.into(),
            data,
        };
        (id, op)
    }
}

/// Primitive SQLite values returned by the native engine.
#[derive(Debug, Clone, PartialEq)]
pub enum SqlValue {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}

impl SqlValue {
    pub fn as_i64(&self) -> Option<i64> {
        if let Self::Integer(value) = self {
            Some(*value)
        } else {
            None
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        if let Self::Real(value) = self {
            Some(*value)
        } else {
            None
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        if let Self::Text(value) = self {
            Some(value.as_str())
        } else {
            None
        }
    }

    pub fn as_blob(&self) -> Option<&[u8]> {
        if let Self::Blob(value) = self {
            Some(value.as_slice())
        } else {
            None
        }
    }

    pub fn to_json(&self) -> JsonValue {
        match self {
            SqlValue::Null => JsonValue::Null,
            SqlValue::Integer(value) => json!(value),
            SqlValue::Real(value) => json!(value),
            SqlValue::Text(value) => json!(value),
            SqlValue::Blob(bytes) => json!(bytes),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<SqlValue>>,
    pub changes: u64,
    pub last_insert_rowid: i64,
}

impl QueryResult {
    pub fn rows_as_maps(&self) -> Vec<HashMap<String, SqlValue>> {
        self.rows
            .iter()
            .map(|row| {
                let mut map = HashMap::new();
                for (index, value) in row.iter().cloned().enumerate() {
                    if let Some(column) = self.columns.get(index) {
                        map.insert(column.clone(), value);
                    }
                }
                map
            })
            .collect()
    }

    pub fn rows_as_json(&self) -> Vec<JsonValue> {
        self.rows_as_maps()
            .into_iter()
            .map(|row| {
                let json_object: HashMap<String, JsonValue> = row
                    .into_iter()
                    .map(|(key, value)| (key, value.to_json()))
                    .collect();
                json!(json_object)
            })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExecutionResult {
    pub changes: u64,
    pub last_insert_rowid: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DatabasePath {
    InMemory,
    File(PathBuf),
}

#[derive(Debug, Clone)]
pub struct DatabaseOptions {
    pub path: DatabasePath,
    pub read_only: bool,
    pub create_if_missing: bool,
    pub apply_default_pragmas: bool,
    pub custom_pragmas: Vec<(String, String)>,
    pub busy_timeout: Option<Duration>,
}

impl Default for DatabaseOptions {
    fn default() -> Self {
        Self {
            path: DatabasePath::InMemory,
            read_only: false,
            create_if_missing: true,
            apply_default_pragmas: true,
            custom_pragmas: Vec::new(),
            busy_timeout: Some(Duration::from_millis(5_000)),
        }
    }
}

impl DatabaseOptions {
    pub fn in_memory() -> Self {
        Self::default()
    }

    pub fn with_file(path: impl Into<PathBuf>) -> Self {
        Self {
            path: DatabasePath::File(path.into()),
            ..Default::default()
        }
    }

    pub fn read_only(mut self, flag: bool) -> Self {
        self.read_only = flag;
        self
    }

    pub fn create_if_missing(mut self, flag: bool) -> Self {
        self.create_if_missing = flag;
        self
    }

    pub fn apply_default_pragmas(mut self, flag: bool) -> Self {
        self.apply_default_pragmas = flag;
        self
    }

    pub fn add_pragma(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.custom_pragmas.push((name.into(), value.into()));
        self
    }

    pub fn busy_timeout(mut self, timeout: Option<Duration>) -> Self {
        self.busy_timeout = timeout;
        self
    }
}

#[derive(Debug, Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
    path: DatabasePath,
}

#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
}

pub type DbResult<T> = Result<T, DatabaseError>;

const DEFAULT_PRAGMAS: &[(&str, &str)] = &[
    ("journal_mode", "WAL"),
    ("synchronous", "NORMAL"),
    ("temp_store", "MEMORY"),
    ("mmap_size", "30000000000"),
    ("page_size", "4096"),
    ("cache_size", "-64000"),
];

impl Database {
    pub fn open(options: DatabaseOptions) -> DbResult<Self> {
        let connection = match &options.path {
            DatabasePath::InMemory => Connection::open_in_memory()?,
            DatabasePath::File(path) => {
                Connection::open_with_flags(path, build_open_flags(&options))?
            }
        };

        if let Some(timeout) = options.busy_timeout {
            connection.busy_timeout(timeout)?;
        }

        if options.apply_default_pragmas {
            apply_pragmas(&connection, DEFAULT_PRAGMAS);
        }

        if !options.custom_pragmas.is_empty() {
            let custom: Vec<(&str, &str)> = options
                .custom_pragmas
                .iter()
                .map(|(name, value)| (name.as_str(), value.as_str()))
                .collect();
            apply_pragmas(&connection, &custom);
        }

        Ok(Self {
            conn: Arc::new(Mutex::new(connection)),
            path: options.path,
        })
    }

    pub fn path(&self) -> &DatabasePath {
        &self.path
    }

    pub fn prepare(&self, sql: impl Into<String>) -> DbResult<Statement> {
        Ok(Statement {
            database: self.clone(),
            sql: sql.into(),
        })
    }

    pub fn exec(&self, sql: &str) -> DbResult<ExecutionResult> {
        self.with_connection(|conn| {
            conn.execute_batch(sql)?;
            Ok(ExecutionResult {
                changes: conn.changes() as u64,
                last_insert_rowid: conn.last_insert_rowid(),
            })
        })
    }

    pub fn query(&self, sql: &str, params: &[SqlValue]) -> DbResult<QueryResult> {
        Statement {
            database: self.clone(),
            sql: sql.to_owned(),
        }
        .query_internal(params)
    }

    pub fn pragma(&self, pragma: &str) -> DbResult<QueryResult> {
        let normalized = if pragma.trim_start().to_lowercase().starts_with("pragma") {
            pragma.trim().to_owned()
        } else {
            format!("PRAGMA {}", pragma)
        };
        self.query(&normalized, &[])
    }

    pub fn transaction<F, T>(&self, f: F) -> DbResult<T>
    where
        F: FnOnce(&Transaction<'_>) -> DbResult<T>,
    {
        self.with_connection(|conn| {
            let tx = conn.transaction()?;
            let result = f(&tx)?;
            tx.commit()?;
            Ok(result)
        })
    }

    fn with_connection<T, F>(&self, f: F) -> DbResult<T>
    where
        F: FnOnce(&mut Connection) -> DbResult<T>,
    {
        let mut guard = self.conn.lock();
        f(&mut guard)
    }
}

#[derive(Debug, Clone)]
pub struct Statement {
    database: Database,
    sql: String,
}

impl Statement {
    pub fn sql(&self) -> &str {
        &self.sql
    }

    pub fn run(&self, params: &[SqlValue]) -> DbResult<ExecutionResult> {
        self.database.with_connection(|conn| {
            let mut stmt = conn.prepare(&self.sql)?;
            let values = params_to_values(params);
            let changes = stmt.execute(params_from_iter(values.iter()))? as u64;
            Ok(ExecutionResult {
                changes,
                last_insert_rowid: conn.last_insert_rowid(),
            })
        })
    }

    pub fn all(&self, params: &[SqlValue]) -> DbResult<QueryResult> {
        self.query_internal(params)
    }

    pub fn get(&self, params: &[SqlValue]) -> DbResult<Option<HashMap<String, SqlValue>>> {
        let result = self.query_internal(params)?;
        Ok(result.rows_as_maps().into_iter().next())
    }

    pub fn columns(&self) -> DbResult<Vec<String>> {
        self.database.with_connection(|conn| {
            let stmt = conn.prepare(&self.sql)?;
            Ok(stmt
                .column_names()
                .iter()
                .map(|name| name.to_string())
                .collect())
        })
    }

    fn query_internal(&self, params: &[SqlValue]) -> DbResult<QueryResult> {
        self.database.with_connection(|conn| {
            let mut stmt = conn.prepare(&self.sql)?;
            let columns = stmt
                .column_names()
                .iter()
                .map(|name| name.to_string())
                .collect::<Vec<_>>();
            let values = params_to_values(params);
            let column_count = columns.len();
            let mut rows_iter = stmt.query(params_from_iter(values.iter()))?;
            let mut rows = Vec::new();
            while let Some(row) = rows_iter.next()? {
                rows.push(read_row(&row, column_count)?);
            }
            Ok(QueryResult {
                columns,
                rows,
                changes: conn.changes() as u64,
                last_insert_rowid: conn.last_insert_rowid(),
            })
        })
    }
}

fn build_open_flags(options: &DatabaseOptions) -> OpenFlags {
    let mut flags = OpenFlags::SQLITE_OPEN_URI | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    if options.read_only {
        flags |= OpenFlags::SQLITE_OPEN_READ_ONLY;
    } else {
        flags |= OpenFlags::SQLITE_OPEN_READ_WRITE;
        if options.create_if_missing {
            flags |= OpenFlags::SQLITE_OPEN_CREATE;
        }
    }
    flags
}

fn apply_pragmas(connection: &Connection, pragmas: &[(&str, &str)]) {
    for (name, value) in pragmas {
        if let Err(error) = connection.pragma_update(None, name, value) {
            debug!(pragma = %name, "failed to apply pragma: {error}");
        }
    }
}

fn params_to_values(params: &[SqlValue]) -> Vec<SqliteValue> {
    params
        .iter()
        .map(|value| match value {
            SqlValue::Null => SqliteValue::Null,
            SqlValue::Integer(v) => SqliteValue::Integer(*v),
            SqlValue::Real(v) => SqliteValue::Real(*v),
            SqlValue::Text(v) => SqliteValue::Text(v.clone()),
            SqlValue::Blob(v) => SqliteValue::Blob(v.clone()),
        })
        .collect()
}

fn read_row(row: &rusqlite::Row<'_>, column_count: usize) -> Result<Vec<SqlValue>, rusqlite::Error> {
    let mut values = Vec::with_capacity(column_count);
    for index in 0..column_count {
        let value = match row.get_ref(index)? {
            ValueRef::Null => SqlValue::Null,
            ValueRef::Integer(v) => SqlValue::Integer(v),
            ValueRef::Real(v) => SqlValue::Real(v),
            ValueRef::Text(v) => SqlValue::Text(String::from_utf8_lossy(v).into_owned()),
            ValueRef::Blob(v) => SqlValue::Blob(v.to_vec()),
        };
        values.push(value);
    }
    Ok(values)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::ErrorCode;

    #[test]
    fn put_and_get_round_trip() {
        let store = CrdtStore::default();
        let id = store.put("node-1", "actor-a", serde_json::json!({"hello": "world"}));
        let record = store.get(&id).expect("record should exist");
        assert_eq!(record.data["hello"], "world");
        assert_eq!(record.clock.get("actor-a"), Some(&1));
    }

    #[test]
    fn delete_removes_node() {
        let store = CrdtStore::default();
        let id = store.put("node-2", "actor-a", serde_json::json!({"name": "plures"}));
        store.delete(&id).expect("delete succeeds");
        assert!(store.get(&id).is_none());
    }

    #[test]
    fn apply_operations() {
        let store = CrdtStore::default();
        let op = CrdtOperation::Put {
            id: "node-3".to_string(),
            actor: "actor-a".to_string(),
            data: serde_json::json!({"count": 1}),
        };
        let result = store.apply(op).expect("apply succeeds");
        assert_eq!(result, Some("node-3".to_string()));

        let delete = CrdtOperation::Delete {
            id: "node-3".to_string(),
        };
        let result = store.apply(delete).expect("delete succeeds");
        assert_eq!(result, None);
        assert!(store.get("node-3").is_none());
    }

    #[test]
    fn database_exec_and_query() {
        let db = Database::open(DatabaseOptions::default()).expect("open database");
        db.exec("CREATE TABLE users (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL)")
            .expect("create table");

        let insert = db
            .prepare("INSERT INTO users (name) VALUES (?1)")
            .expect("prepare insert");
        insert
            .run(&[SqlValue::Text("Alice".to_string())])
            .expect("insert row");

        let query = db
            .prepare("SELECT id, name FROM users ORDER BY id")
            .expect("prepare select");
        let result = query.all(&[]).expect("query rows");
        assert_eq!(result.columns, vec!["id".to_string(), "name".to_string()]);
        assert_eq!(result.rows.len(), 1);
        match &result.rows[0][1] {
            SqlValue::Text(value) => assert_eq!(value, "Alice"),
            other => panic!("unexpected value: {:?}", other),
        }
    }

    #[test]
    fn database_default_pragmas_applied() {
        let temp = tempfile::NamedTempFile::new().expect("create temp file");
        let db = Database::open(DatabaseOptions::with_file(temp.path()))
            .expect("open database");
        let result = db.pragma("journal_mode").expect("run pragma");
        assert!(!result.rows.is_empty());
        match &result.rows[0][0] {
            SqlValue::Text(mode) => assert_eq!(mode.to_lowercase(), "wal"),
            other => panic!("unexpected pragma value: {:?}", other),
        }
    }

    #[test]
    fn statement_get_returns_none_when_no_rows() {
        let db = Database::open(DatabaseOptions::default()).expect("open database");
        db.exec("CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT)")
            .expect("create table");

        let select = db
            .prepare("SELECT name FROM items WHERE id = ?1")
            .expect("prepare select");
        let result = select
            .get(&[SqlValue::Integer(42)])
            .expect("query should succeed");
        assert!(result.is_none());
    }

    #[test]
    fn statement_run_propagates_sql_errors() {
        let db = Database::open(DatabaseOptions::default()).expect("open database");
        db.exec("CREATE TABLE users (id INTEGER PRIMARY KEY, email TEXT UNIQUE NOT NULL)")
            .expect("create table");

        let insert = db
            .prepare("INSERT INTO users (email) VALUES (?1)")
            .expect("prepare insert");
        insert
            .run(&[SqlValue::Text("alice@example.com".into())])
            .expect("first insert succeeds");

        let err = insert
            .run(&[SqlValue::Text("alice@example.com".into())])
            .expect_err("second insert should fail");
        match err {
            DatabaseError::Sqlite(inner) => {
                assert_eq!(inner.sqlite_error_code(), Some(ErrorCode::ConstraintViolation));
            }
        }
    }

    #[test]
    fn statement_handles_blob_parameters_and_columns() {
        let db = Database::open(DatabaseOptions::default()).expect("open database");
        db.exec("CREATE TABLE files (id INTEGER PRIMARY KEY, data BLOB NOT NULL)")
            .expect("create table");

        let blob = vec![0_u8, 1, 2, 3];
        let insert = db
            .prepare("INSERT INTO files (id, data) VALUES (?1, ?2)")
            .expect("prepare insert");
        insert
            .run(&[SqlValue::Integer(1), SqlValue::Blob(blob.clone())])
            .expect("insert blob row");

        let select = db
            .prepare("SELECT id, data FROM files WHERE id = ?1")
            .expect("prepare select");
        let columns = select.columns().expect("inspect columns");
        assert_eq!(columns, vec!["id".to_string(), "data".to_string()]);

        let result = select
            .all(&[SqlValue::Integer(1)])
            .expect("query single row");
        assert_eq!(result.rows.len(), 1);
        match &result.rows[0][1] {
            SqlValue::Blob(value) => assert_eq!(value, &blob),
            other => panic!("unexpected value: {:?}", other),
        }
    }
}

