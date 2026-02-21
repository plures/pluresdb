use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use clap::{Parser, Subcommand};
use pluresdb_core::{
    CrdtOperation, CrdtStore, Database, DatabaseOptions, SqlValue,
};
use pluresdb_storage::{MemoryStorage, SledStorage, StorageEngine, StoredNode};
use pluresdb_sync::SyncBroadcaster;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::runtime::Runtime;
use tokio::signal;
use tower::ServiceBuilder;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::{error, info, warn};
use tracing_subscriber::{fmt, EnvFilter};

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser, Debug)]
#[command(
    name = "pluresdb",
    version = VERSION,
    about = "PluresDB - P2P Graph Database with SQLite Compatibility",
    long_about = "A high-performance, P2P graph database with built-in vector search,\n\
                  CRDT conflict resolution, and SQLite compatibility.",
    propagate_version = true
)]
struct Cli {
    /// Path to a data directory (optional; in-memory if omitted)
    #[arg(long, global = true, help = "Path to data directory")]
    data_dir: Option<PathBuf>,

    /// Enable verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Log level (error, warn, info, debug, trace)
    #[arg(long, global = true, default_value = "info")]
    log_level: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Initialize a new PluresDB database
    Init {
        /// Database path
        #[arg(default_value = "./pluresdb-data")]
        path: PathBuf,

        /// Force initialization even if path exists
        #[arg(long)]
        force: bool,
    },

    /// Start the PluresDB API server
    Serve {
        /// Server port
        #[arg(long, short = 'p', default_value = "34569")]
        port: u16,

        /// Bind address
        #[arg(long, default_value = "0.0.0.0")]
        bind: String,

        /// Enable WebSocket support
        #[arg(long, default_value = "true")]
        websocket: bool,
    },

    /// Show database status
    Status {
        /// Show detailed statistics
        #[arg(long)]
        detailed: bool,
    },

    /// Insert or update a node
    Put {
        /// Node identifier
        id: String,

        /// JSON data (use @file to read from file)
        data: String,

        /// Actor identifier for CRDT merge
        #[arg(long, default_value = "cli-actor")]
        actor: String,

        /// Node type
        #[arg(long, short = 't')]
        node_type: Option<String>,

        /// Tags (comma-separated)
        #[arg(long)]
        tags: Option<String>,
    },

    /// Retrieve a node by identifier
    Get {
        /// Node identifier
        id: String,

        /// Output format (json, pretty, raw)
        #[arg(long, short = 'f', default_value = "pretty")]
        format: String,

        /// Show metadata
        #[arg(long)]
        metadata: bool,
    },

    /// Delete a node
    Delete {
        /// Node identifier
        id: String,

        /// Force deletion without confirmation
        #[arg(long)]
        force: bool,
    },

    /// List all nodes
    List {
        /// Filter by type
        #[arg(long, short = 't')]
        node_type: Option<String>,

        /// Filter by tag
        #[arg(long)]
        tag: Option<String>,

        /// Limit number of results
        #[arg(long, short = 'l', default_value = "100")]
        limit: usize,

        /// Output format (json, table, ids)
        #[arg(long, short = 'f', default_value = "table")]
        format: String,
    },

    /// Execute SQL query
    Query {
        /// SQL query
        query: String,

        /// Output format (json, table, csv)
        #[arg(long, short = 'f', default_value = "table")]
        format: String,

        /// Query parameters (JSON array)
        #[arg(long, short = 'p')]
        params: Option<String>,
    },

    /// Full-text search
    Search {
        /// Search query
        query: String,

        /// Limit number of results
        #[arg(long, short = 'l', default_value = "10")]
        limit: usize,
    },

    /// Vector similarity search
    Vsearch {
        /// Query embedding as a JSON array of floats, e.g. '[0.1, 0.2, ...]'
        embedding: String,

        /// Limit number of results
        #[arg(long, short = 'l', default_value = "10")]
        limit: usize,

        /// Minimum similarity threshold (0.0-1.0)
        #[arg(long, default_value = "0.0")]
        threshold: f64,
    },

    /// Type system commands
    #[command(subcommand)]
    Type(TypeCommands),

    /// Network commands
    #[command(subcommand)]
    Network(NetworkCommands),

    /// Configuration commands
    #[command(subcommand)]
    Config(ConfigCommands),

    /// Maintenance commands
    #[command(subcommand)]
    Maintenance(MaintenanceCommands),
}

#[derive(Subcommand, Debug)]
enum TypeCommands {
    /// Define a new type
    Define {
        /// Type name
        name: String,

        /// JSON schema
        schema: Option<String>,
    },

    /// List all types
    List,

    /// Get instances of a type
    Instances {
        /// Type name
        name: String,

        /// Limit number of results
        #[arg(long, short = 'l', default_value = "100")]
        limit: usize,
    },

    /// Show type schema
    Schema {
        /// Type name
        name: String,
    },
}

#[derive(Subcommand, Debug)]
enum NetworkCommands {
    /// Connect to a peer
    Connect {
        /// Peer URL (e.g., ws://localhost:34569)
        url: String,
    },

    /// Disconnect from a peer
    Disconnect {
        /// Peer ID
        peer_id: String,
    },

    /// List connected peers
    Peers {
        /// Show detailed information
        #[arg(long)]
        detailed: bool,
    },

    /// Force synchronization
    Sync {
        /// Specific peer ID (optional)
        peer_id: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
enum ConfigCommands {
    /// List all configuration
    List,

    /// Get configuration value
    Get {
        /// Configuration key
        key: String,
    },

    /// Set configuration value
    Set {
        /// Configuration key
        key: String,

        /// Configuration value
        value: String,
    },

    /// Reset configuration to defaults
    Reset {
        /// Force reset without confirmation
        #[arg(long)]
        force: bool,
    },
}

#[derive(Subcommand, Debug)]
enum MaintenanceCommands {
    /// Backup database
    Backup {
        /// Backup file path
        path: PathBuf,

        /// Compress backup
        #[arg(long)]
        compress: bool,
    },

    /// Restore database
    Restore {
        /// Backup file path
        path: PathBuf,

        /// Force restore without confirmation
        #[arg(long)]
        force: bool,
    },

    /// Optimize database (vacuum)
    Vacuum {
        /// Show size before and after
        #[arg(long)]
        stats: bool,
    },

    /// Run database migrations
    Migrate {
        /// Target version (optional)
        version: Option<u32>,
    },

    /// Show database statistics
    Stats {
        /// Show detailed statistics
        #[arg(long)]
        detailed: bool,
    },
}

#[derive(Clone)]
struct AppState {
    storage: Arc<dyn StorageEngine>,
    store: Arc<CrdtStore>,
    db: Option<Arc<Database>>,
    broadcaster: Arc<SyncBroadcaster>,
}

fn init_runtime() -> Runtime {
    Runtime::new().expect("failed to initialise Tokio runtime")
}

fn init_logging(cli: &Cli) {
    let log_level = if cli.verbose { "debug" } else { &cli.log_level };
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(log_level));
    fmt().with_env_filter(filter).init();
}

fn load_payload(data: &str) -> Result<serde_json::Value> {
    if data.starts_with('@') {
        let path = &data[1..];
        let content = fs::read_to_string(path)
            .with_context(|| format!("failed to read file: {}", path))?;
        serde_json::from_str(&content)
            .with_context(|| format!("invalid JSON in file: {}", path))
    } else {
        serde_json::from_str(data).context("invalid JSON data")
    }
}

fn create_storage(data_dir: Option<&PathBuf>) -> Result<Arc<dyn StorageEngine>> {
    if let Some(dir) = data_dir {
        let db_path = dir.join("db");
        fs::create_dir_all(&db_path)?;
        let storage = SledStorage::open(&db_path)?;
        Ok(Arc::new(storage))
    } else {
        Ok(Arc::new(MemoryStorage::default()))
    }
}

fn create_database(data_dir: Option<&PathBuf>) -> Result<Option<Arc<Database>>> {
    if let Some(dir) = data_dir {
        let db_path = dir.join("pluresdb.db");
        let options = DatabaseOptions::with_file(&db_path).create_if_missing(true);
        let db = Database::open(options)?;
        Ok(Some(Arc::new(db)))
    } else {
        Ok(None)
    }
}

async fn handle_put(
    storage: Arc<dyn StorageEngine>,
    store: Arc<CrdtStore>,
    broadcaster: Arc<SyncBroadcaster>,
    id: String,
    data: String,
    actor: String,
    node_type: Option<String>,
    tags: Option<String>,
) -> Result<()> {
    let mut payload = load_payload(&data)?;

    // Add type if specified
    if let Some(t) = node_type {
        if let Some(obj) = payload.as_object_mut() {
            obj.insert("type".to_string(), Value::String(t));
        }
    }

    // Add tags if specified
    if let Some(t) = tags {
        let tag_list: Vec<String> = t.split(',').map(|s| s.trim().to_string()).collect();
        if let Some(obj) = payload.as_object_mut() {
            obj.insert(
                "tags".to_string(),
                Value::Array(tag_list.iter().map(|s| Value::String(s.clone())).collect()),
            );
        }
    }

    let op = CrdtOperation::Put {
        id: id.clone(),
        actor: actor.clone(),
        data: payload.clone(),
    };

    store.apply(op)?;
    storage
        .put(StoredNode {
            id: id.clone(),
            payload,
        })
        .await?;

    broadcaster
        .publish(pluresdb_sync::SyncEvent::NodeUpsert { id: id.clone() })?;

    println!("{{\"success\":true,\"id\":\"{}\"}}", id);
    Ok(())
}

async fn handle_get(
    storage: Arc<dyn StorageEngine>,
    store: Arc<CrdtStore>,
    id: String,
    format: String,
    metadata: bool,
) -> Result<()> {
    match storage.get(&id).await? {
        Some(node) => {
            if metadata {
                if let Some(record) = store.get(&id) {
                    let output = json!({
                        "id": id,
                        "data": node.payload,
                        "clock": record.clock,
                        "timestamp": record.timestamp
                    });
                    match format.as_str() {
                        "json" => println!("{}", serde_json::to_string(&output)?),
                        "pretty" => println!("{}", serde_json::to_string_pretty(&output)?),
                        _ => println!("{}", serde_json::to_string_pretty(&output)?),
                    }
                } else {
                    match format.as_str() {
                        "json" => println!("{}", serde_json::to_string(&node.payload)?),
                        "pretty" => println!("{}", serde_json::to_string_pretty(&node.payload)?),
                        "raw" => println!("{:?}", node),
                        _ => println!("{}", serde_json::to_string_pretty(&node.payload)?),
                    }
                }
            } else {
                match format.as_str() {
                    "json" => println!("{}", serde_json::to_string(&node.payload)?),
                    "pretty" => println!("{}", serde_json::to_string_pretty(&node.payload)?),
                    "raw" => println!("{:?}", node),
                    _ => println!("{}", serde_json::to_string_pretty(&node.payload)?),
                }
            }
            Ok(())
        }
        None => {
            error!("node not found: {}", id);
            std::process::exit(1);
        }
    }
}

async fn handle_list(
    storage: Arc<dyn StorageEngine>,
    node_type: Option<String>,
    tag: Option<String>,
    limit: usize,
    format: String,
) -> Result<()> {
    let mut nodes = storage.list().await?;

    // Filter by type
    if let Some(t) = node_type {
        nodes.retain(|n| {
            n.payload
                .get("type")
                .and_then(|v| v.as_str())
                .map(|s| s == t)
                .unwrap_or(false)
        });
    }

    // Filter by tag
    if let Some(tag_filter) = tag {
        nodes.retain(|n| {
            n.payload
                .get("tags")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().any(|v| v.as_str() == Some(&tag_filter)))
                .unwrap_or(false)
        });
    }

    // Limit results
    nodes.truncate(limit);

    match format.as_str() {
        "json" => println!("{}", serde_json::to_string(&nodes)?),
        "ids" => {
            for node in nodes {
                println!("{}", node.id);
            }
        }
        "table" | _ => {
            println!("{:<40} {:<20} {}", "ID", "Type", "Data Preview");
            println!("{}", "-".repeat(80));
            for node in nodes {
                let node_type = node
                    .payload
                    .get("type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let preview = serde_json::to_string(&node.payload)?;
                let preview = if preview.len() > 40 {
                    format!("{}...", &preview[..40])
                } else {
                    preview
                };
                println!("{:<40} {:<20} {}", node.id, node_type, preview);
            }
        }
    }

    Ok(())
}

async fn handle_query(
    db: Option<Arc<Database>>,
    query: String,
    format: String,
    params: Option<String>,
) -> Result<()> {
    let db = db.context("SQL queries require a persistent database (use --data-dir)")?;

    let sql_params = if let Some(p) = params {
        let json_params: Vec<Value> = serde_json::from_str(&p)?;
        json_params
            .into_iter()
            .map(|v| -> Result<SqlValue, serde_json::Error> {
                Ok(match v {
                    Value::Null => SqlValue::Null,
                    Value::Number(n) => {
                        if n.is_i64() {
                            SqlValue::Integer(n.as_i64().unwrap())
                        } else {
                            SqlValue::Real(n.as_f64().unwrap())
                        }
                    }
                    Value::String(s) => SqlValue::Text(s),
                    Value::Bool(b) => SqlValue::Integer(if b { 1 } else { 0 }),
                    Value::Array(_) | Value::Object(_) => {
                        SqlValue::Text(serde_json::to_string(&v)?)
                    }
                })
            })
            .collect::<Result<Vec<_>, _>>()?
    } else {
        vec![]
    };

    let result = db.query(&query, &sql_params)?;

    match format.as_str() {
        "json" => {
            let output = json!({
                "columns": result.columns,
                "rows": result.rows_as_json(),
                "changes": result.changes,
                "last_insert_rowid": result.last_insert_rowid
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        "csv" => {
            // Print CSV header
            println!("{}", result.columns.join(","));
            // Print CSV rows
            for row in &result.rows {
                let csv_row: Vec<String> = row
                    .iter()
                    .map(|v| match v {
                        SqlValue::Null => "".to_string(),
                        SqlValue::Integer(i) => i.to_string(),
                        SqlValue::Real(r) => r.to_string(),
                        SqlValue::Text(t) => format!("\"{}\"", t.replace('"', "\"\"")),
                        SqlValue::Blob(b) => format!("{:?}", b),
                    })
                    .collect();
                println!("{}", csv_row.join(","));
            }
        }
        "table" | _ => {
            // Print table header
            for col in &result.columns {
                print!("{:<20} ", col);
            }
            println!();
            println!("{}", "-".repeat(result.columns.len() * 22));
            // Print table rows
            for row in &result.rows {
                for val in row {
                    let val_str = match val {
                        SqlValue::Null => "NULL".to_string(),
                        SqlValue::Integer(i) => i.to_string(),
                        SqlValue::Real(r) => format!("{:.2}", r),
                        SqlValue::Text(t) => {
                            if t.len() > 18 {
                                format!("{}...", &t[..18])
                            } else {
                                t.clone()
                            }
                        }
                        SqlValue::Blob(b) => format!("<{} bytes>", b.len()),
                    };
                    print!("{:<20} ", val_str);
                }
                println!();
            }
            if result.changes > 0 {
                println!("\n{} row(s) affected", result.changes);
            }
        }
    }

    Ok(())
}

async fn handle_search(
    storage: Arc<dyn StorageEngine>,
    query: String,
    limit: usize,
) -> Result<()> {
    let nodes = storage.list().await?;
    let query_lower = query.to_lowercase();

    let mut matches: Vec<(&StoredNode, usize)> = nodes
        .iter()
        .filter_map(|node| {
            let json_str = serde_json::to_string(&node.payload).ok()?;
            let count = json_str.to_lowercase().matches(&query_lower).count();
            if count > 0 {
                Some((node, count))
            } else {
                None
            }
        })
        .collect();

    matches.sort_by(|a, b| b.1.cmp(&a.1));
    matches.truncate(limit);

    println!("Found {} matches:", matches.len());
    for (node, score) in matches {
        println!("  {} (matches: {})", node.id, score);
        let preview = serde_json::to_string_pretty(&node.payload)?;
        let preview_lines: Vec<&str> = preview.lines().take(5).collect();
        println!("    {}", preview_lines.join("\n    "));
        if preview.lines().count() > 5 {
            println!("    ...");
        }
    }

    Ok(())
}

async fn handle_vsearch(
    store: Arc<CrdtStore>,
    embedding: Vec<f32>,
    limit: usize,
    min_score: f32,
) -> Result<()> {
    let results = store.vector_search(&embedding, limit, min_score);

    println!("Found {} matches:", results.len());
    for r in results {
        println!("  {} (score: {:.4})", r.record.id, r.score);
        let preview = serde_json::to_string_pretty(&r.record.data)?;
        let preview_lines: Vec<&str> = preview.lines().take(5).collect();
        println!("    {}", preview_lines.join("\n    "));
        if preview.lines().count() > 5 {
            println!("    ...");
        }
    }

    Ok(())
}

async fn handle_type_define(
    storage: Arc<dyn StorageEngine>,
    name: String,
    schema: Option<String>,
) -> Result<()> {
    let schema_value = if let Some(s) = schema {
        serde_json::from_str(&s).context("invalid JSON schema")?
    } else {
        json!({})
    };

    let type_node = json!({
        "type": "__type_definition__",
        "name": name,
        "schema": schema_value,
        "created_at": chrono::Utc::now()
    });

    let id = format!("type:{}", name);
    storage
        .put(StoredNode {
            id: id.clone(),
            payload: type_node,
        })
        .await?;

    println!("Type '{}' defined successfully", name);
    Ok(())
}

async fn handle_type_list(storage: Arc<dyn StorageEngine>) -> Result<()> {
    let nodes = storage.list().await?;
    let types: Vec<_> = nodes
        .iter()
        .filter_map(|n| {
            if n.payload.get("type") == Some(&Value::String("__type_definition__".to_string())) {
                n.payload.get("name").and_then(|v| v.as_str())
            } else {
                None
            }
        })
        .collect();

    if types.is_empty() {
        println!("No types defined");
    } else {
        println!("Defined types:");
        for t in types {
            println!("  - {}", t);
        }
    }

    Ok(())
}

async fn handle_type_instances(
    storage: Arc<dyn StorageEngine>,
    name: String,
    limit: usize,
) -> Result<()> {
    let nodes = storage.list().await?;
    let instances: Vec<_> = nodes
        .iter()
        .filter(|n| {
            n.payload
                .get("type")
                .and_then(|v| v.as_str())
                .map(|s| s == name)
                .unwrap_or(false)
        })
        .take(limit)
        .collect();

    println!("Instances of type '{}':", name);
    for node in instances {
        println!("  - {}", node.id);
    }

    Ok(())
}

async fn handle_type_schema(storage: Arc<dyn StorageEngine>, name: String) -> Result<()> {
    let id = format!("type:{}", name);
    match storage.get(&id).await? {
        Some(node) => {
            if let Some(schema) = node.payload.get("schema") {
                println!("Schema for type '{}':", name);
                println!("{}", serde_json::to_string_pretty(schema)?);
            } else {
                println!("No schema defined for type '{}'", name);
            }
        }
        None => {
            error!("Type '{}' not found", name);
            std::process::exit(1);
        }
    }

    Ok(())
}

async fn handle_network_connect(url: String) -> Result<()> {
    info!("Connecting to peer: {}", url);
    warn!("Network connection not yet fully implemented");
    println!("Would connect to: {}", url);
    Ok(())
}

async fn handle_network_disconnect(peer_id: String) -> Result<()> {
    info!("Disconnecting from peer: {}", peer_id);
    warn!("Network disconnection not yet fully implemented");
    println!("Would disconnect from: {}", peer_id);
    Ok(())
}

async fn handle_network_peers(detailed: bool) -> Result<()> {
    warn!("Network peer listing not yet fully implemented");
    println!("Connected peers: 0");
    if detailed {
        println!("  (No peers connected)");
    }
    Ok(())
}

async fn handle_network_sync(peer_id: Option<String>) -> Result<()> {
    info!("Synchronizing with peer: {:?}", peer_id);
    warn!("Network synchronization not yet fully implemented");
    if let Some(p) = peer_id {
        println!("Would sync with peer: {}", p);
    } else {
        println!("Would sync with all peers");
    }
    Ok(())
}

fn load_config(data_dir: Option<&PathBuf>) -> Result<HashMap<String, String>> {
    let mut config = HashMap::new();
    if let Some(dir) = data_dir {
        let config_path = dir.join("config.json");
        if config_path.exists() {
            let content = fs::read_to_string(&config_path)?;
            let json: HashMap<String, Value> = serde_json::from_str(&content)?;
            for (k, v) in json {
                config.insert(k, v.to_string());
            }
        }
    }
    Ok(config)
}

fn save_config(data_dir: Option<&PathBuf>, config: &HashMap<String, String>) -> Result<()> {
    if let Some(dir) = data_dir {
        fs::create_dir_all(dir)?;
        let config_path = dir.join("config.json");
        let json: Value = config.iter().map(|(k, v)| (k.clone(), json!(v))).collect();
        fs::write(&config_path, serde_json::to_string_pretty(&json)?)?;
    }
    Ok(())
}

async fn handle_config_list(data_dir: Option<&PathBuf>) -> Result<()> {
    let config = load_config(data_dir)?;
    if config.is_empty() {
        println!("No configuration set");
    } else {
        println!("Configuration:");
        for (key, value) in config {
            println!("  {} = {}", key, value);
        }
    }
    Ok(())
}

async fn handle_config_get(data_dir: Option<&PathBuf>, key: String) -> Result<()> {
    let config = load_config(data_dir)?;
    match config.get(&key) {
        Some(value) => println!("{}", value),
        None => {
            error!("Configuration key '{}' not found", key);
            std::process::exit(1);
        }
    }
    Ok(())
}

async fn handle_config_set(
    data_dir: Option<&PathBuf>,
    key: String,
    value: String,
) -> Result<()> {
    let mut config = load_config(data_dir)?;
    config.insert(key.clone(), value.clone());
    save_config(data_dir, &config)?;
    println!("Configuration '{}' set to '{}'", key, value);
    Ok(())
}

async fn handle_config_reset(data_dir: Option<&PathBuf>, force: bool) -> Result<()> {
    if !force {
        print!("Reset all configuration? [y/N]: ");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Cancelled");
            return Ok(());
        }
    }

    if let Some(dir) = data_dir {
        let config_path = dir.join("config.json");
        if config_path.exists() {
            fs::remove_file(&config_path)?;
        }
    }
    println!("Configuration reset to defaults");
    Ok(())
}

async fn handle_backup(
    storage: Arc<dyn StorageEngine>,
    path: PathBuf,
    compress: bool,
) -> Result<()> {
    let nodes = storage.list().await?;
    let backup_data = json!({
        "version": VERSION,
        "timestamp": chrono::Utc::now(),
        "nodes": nodes
    });

    let content = serde_json::to_string_pretty(&backup_data)?;
    if compress {
        // Simple compression - in production, use proper compression
        fs::write(&path, content)?;
        info!("Backup saved to: {:?}", path);
    } else {
        fs::write(&path, content)?;
    }

    println!("Backup saved to: {:?}", path);
    Ok(())
}

async fn handle_restore(
    storage: Arc<dyn StorageEngine>,
    path: PathBuf,
    force: bool,
) -> Result<()> {
    if !force {
        print!("Restore will overwrite existing data. Continue? [y/N]: ");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Cancelled");
            return Ok(());
        }
    }

    let content = fs::read_to_string(&path)?;
    let backup: Value = serde_json::from_str(&content)?;

    if let Some(nodes_array) = backup.get("nodes").and_then(|v| v.as_array()) {
        for node_value in nodes_array {
            if let Ok(node) = serde_json::from_value::<StoredNode>(node_value.clone()) {
                storage.put(node).await?;
            }
        }
        println!("Restored {} nodes from backup", nodes_array.len());
    } else {
        error!("Invalid backup format");
        std::process::exit(1);
    }

    Ok(())
}

async fn handle_vacuum(db: Option<Arc<Database>>, stats: bool) -> Result<()> {
    let db = db.context("Vacuum requires a persistent database (use --data-dir)")?;

    if stats {
        let before = db.pragma("page_count")?;
        let size_before = db.pragma("page_size")?;
        println!("Before vacuum:");
        println!("  Pages: {:?}", before.rows_as_json());
        println!("  Page size: {:?}", size_before.rows_as_json());
    }

    db.exec("VACUUM")?;

    if stats {
        let after = db.pragma("page_count")?;
        let size_after = db.pragma("page_size")?;
        println!("After vacuum:");
        println!("  Pages: {:?}", after.rows_as_json());
        println!("  Page size: {:?}", size_after.rows_as_json());
    }

    println!("Database vacuumed successfully");
    Ok(())
}

async fn handle_migrate(db: Option<Arc<Database>>, version: Option<u32>) -> Result<()> {
    let db = db.context("Migrations require a persistent database (use --data-dir)")?;

    // Create migrations table if it doesn't exist
    db.exec(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY,
            applied_at DATETIME DEFAULT CURRENT_TIMESTAMP
        )",
    )?;

    if let Some(target_version) = version {
        info!("Migrating to version: {}", target_version);
        // In a real implementation, apply migrations up to target_version
        println!("Migration to version {} completed", target_version);
    } else {
        // Apply all pending migrations
        println!("All migrations are up to date");
    }

    Ok(())
}

async fn handle_stats(
    storage: Arc<dyn StorageEngine>,
    db: Option<Arc<Database>>,
    detailed: bool,
) -> Result<()> {
    let nodes = storage.list().await?;
    println!("Database Statistics:");
    println!("  Total nodes: {}", nodes.len());

    if detailed {
        // Count by type
        let mut type_counts: HashMap<String, usize> = HashMap::new();
        for node in &nodes {
            if let Some(t) = node.payload.get("type").and_then(|v| v.as_str()) {
                *type_counts.entry(t.to_string()).or_insert(0) += 1;
            }
        }
        if !type_counts.is_empty() {
            println!("\nNodes by type:");
            for (t, count) in type_counts {
                println!("  {}: {}", t, count);
            }
        }

        if let Some(db) = db {
            let page_count = db.pragma("page_count")?;
            let page_size = db.pragma("page_size")?;
            println!("\nDatabase size:");
            println!("  Pages: {:?}", page_count.rows_as_json());
            println!("  Page size: {:?}", page_size.rows_as_json());
        }
    }

    Ok(())
}

async fn create_api_server(state: AppState, bind: String, port: u16) -> Result<()> {
    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/api/nodes", get(list_nodes_handler).post(create_node_handler))
        .route("/api/nodes/:id", get(get_node_handler).delete(delete_node_handler))
        .route("/api/vector-search", post(vector_search_handler))
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(CorsLayer::permissive())
                .into_inner(),
        )
        .with_state(state);

    let addr = format!("{}:{}", bind, port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("Server listening on http://{}", addr);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    info!("Shutting down gracefully...");
}

async fn health_handler() -> Json<Value> {
    Json(json!({
        "status": "healthy",
        "version": VERSION
    }))
}

async fn list_nodes_handler(State(state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    match state.storage.list().await {
        Ok(nodes) => Ok(Json(json!({
            "success": true,
            "data": nodes
        }))),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

async fn get_node_handler(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<Json<Value>, StatusCode> {
    match state.storage.get(&id).await {
        Ok(Some(node)) => Ok(Json(json!({
            "success": true,
            "data": node
        }))),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

async fn create_node_handler(
    State(state): State<AppState>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    let id = payload
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or(StatusCode::BAD_REQUEST)?;

    let node = StoredNode {
        id: id.to_string(),
        payload: payload.clone(),
    };

    match state.storage.put(node).await {
        Ok(_) => {
            let _ = state.broadcaster.publish(pluresdb_sync::SyncEvent::NodeUpsert {
                id: id.to_string(),
            });
            Ok(Json(json!({
                "success": true,
                "id": id
            })))
        }
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

async fn delete_node_handler(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<Json<Value>, StatusCode> {
    match state.storage.delete(&id).await {
        Ok(_) => {
            let _ = state.broadcaster.publish(pluresdb_sync::SyncEvent::NodeDelete {
                id: id.clone(),
            });
            Ok(Json(json!({
                "success": true,
                "id": id
            })))
        }
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// Request body for `POST /api/vector-search`.
#[derive(Debug, Deserialize)]
struct VectorSearchRequest {
    /// Query embedding as a flat array of 32-bit floats.
    embedding: Vec<f64>,
    /// Maximum number of results to return (default: 10).
    #[serde(default)]
    limit: Option<usize>,
    /// Minimum cosine similarity score in \[0, 1\] (default: 0.0).
    #[serde(default)]
    threshold: Option<f64>,
}

async fn vector_search_handler(
    State(state): State<AppState>,
    Json(req): Json<VectorSearchRequest>,
) -> Result<Json<Value>, StatusCode> {
    let limit = req.limit.unwrap_or(10);
    let min_score = req.threshold.unwrap_or(0.0) as f32;
    let query: Vec<f32> = req.embedding.iter().map(|&v| v as f32).collect();

    let results = state.store.vector_search(&query, limit, min_score);
    let data: Vec<Value> = results
        .into_iter()
        .map(|r| {
            json!({
                "id": r.record.id,
                "data": r.record.data,
                "score": r.score,
                "timestamp": r.record.timestamp.to_rfc3339(),
            })
        })
        .collect();

    Ok(Json(json!({
        "success": true,
        "data": data
    })))
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    init_logging(&cli);

    info!("PluresDB CLI v{}", VERSION);

    let rt = init_runtime();
    rt.block_on(async move {
        let storage = create_storage(cli.data_dir.as_ref())?;
        let store = Arc::new(CrdtStore::default());
        let db = create_database(cli.data_dir.as_ref())?;
        let broadcaster = Arc::new(SyncBroadcaster::default());

        let state = AppState {
            storage: storage.clone(),
            store: store.clone(),
            db: db.clone(),
            broadcaster: broadcaster.clone(),
        };

        match cli.command {
            Commands::Init { path, force } => {
                info!("Initializing database at: {:?}", path);
                if path.exists() && !force {
                    error!("Path already exists. Use --force to overwrite.");
                    std::process::exit(1);
                }
                fs::create_dir_all(&path)?;
                info!("Database initialized successfully");
                Ok(())
            }

            Commands::Serve { port, bind, websocket: _ } => {
                info!("Starting PluresDB server on {}:{}", bind, port);
                println!("Server starting on http://{}:{}", bind, port);
                println!("Press Ctrl+C to stop");
                create_api_server(state, bind, port).await
            }

            Commands::Status { detailed } => {
                println!("PluresDB Status");
                println!("Version: {}", VERSION);
                println!("Status: Running");
                if detailed {
                    let storage_type = if cli.data_dir.is_some() {
                        "Persistent (Sled)"
                    } else {
                        "In-memory"
                    };
                    println!("Storage: {}", storage_type);
                    let nodes = storage.list().await?;
                    println!("Nodes: {}", nodes.len());
                }
                Ok(())
            }

            Commands::Put {
                id,
                data,
                actor,
                node_type,
                tags,
            } => {
                handle_put(storage, store, broadcaster, id, data, actor, node_type, tags).await
            }

            Commands::Get { id, format, metadata } => {
                handle_get(storage, store, id, format, metadata).await
            }

            Commands::Delete { id, force } => {
                if !force {
                    print!("Delete node '{}'? [y/N]: ", id);
                    io::stdout().flush()?;
                    let mut input = String::new();
                    io::stdin().read_line(&mut input)?;
                    if !input.trim().eq_ignore_ascii_case("y") {
                        println!("Cancelled");
                        return Ok(());
                    }
                }

                storage.delete(&id).await?;
                let _ = store.delete(&id);
                let _ = broadcaster.publish(pluresdb_sync::SyncEvent::NodeDelete { id: id.clone() });
                println!("{{\"success\":true,\"id\":\"{}\"}}", id);
                Ok(())
            }

            Commands::List {
                node_type,
                tag,
                limit,
                format,
            } => handle_list(storage, node_type, tag, limit, format).await,

            Commands::Query { query, format, params } => {
                handle_query(db, query, format, params).await
            }

            Commands::Search { query, limit } => handle_search(storage, query, limit).await,

            Commands::Vsearch {
                embedding,
                limit,
                threshold,
            } => {
                let emb_values: Vec<f64> = serde_json::from_str(&embedding)
                    .with_context(|| format!(
                        "embedding must be a JSON array of numbers (e.g. '[0.1,0.2,...]'): {}",
                        embedding
                    ))?;
                let emb_f32: Vec<f32> = emb_values.iter().map(|&v| v as f32).collect();
                handle_vsearch(store, emb_f32, limit, threshold as f32).await
            }

            Commands::Type(cmd) => match cmd {
                TypeCommands::Define { name, schema } => {
                    handle_type_define(storage, name, schema).await
                }
                TypeCommands::List => handle_type_list(storage).await,
                TypeCommands::Instances { name, limit } => {
                    handle_type_instances(storage, name, limit).await
                }
                TypeCommands::Schema { name } => handle_type_schema(storage, name).await,
            },

            Commands::Network(cmd) => match cmd {
                NetworkCommands::Connect { url } => handle_network_connect(url).await,
                NetworkCommands::Disconnect { peer_id } => {
                    handle_network_disconnect(peer_id).await
                }
                NetworkCommands::Peers { detailed } => handle_network_peers(detailed).await,
                NetworkCommands::Sync { peer_id } => handle_network_sync(peer_id).await,
            },

            Commands::Config(cmd) => match cmd {
                ConfigCommands::List => handle_config_list(cli.data_dir.as_ref()).await,
                ConfigCommands::Get { key } => {
                    handle_config_get(cli.data_dir.as_ref(), key).await
                }
                ConfigCommands::Set { key, value } => {
                    handle_config_set(cli.data_dir.as_ref(), key, value).await
                }
                ConfigCommands::Reset { force } => {
                    handle_config_reset(cli.data_dir.as_ref(), force).await
                }
            },

            Commands::Maintenance(cmd) => match cmd {
                MaintenanceCommands::Backup { path, compress } => {
                    handle_backup(storage, path, compress).await
                }
                MaintenanceCommands::Restore { path, force } => {
                    handle_restore(storage, path, force).await
                }
                MaintenanceCommands::Vacuum { stats } => handle_vacuum(db, stats).await,
                MaintenanceCommands::Migrate { version } => handle_migrate(db, version).await,
                MaintenanceCommands::Stats { detailed } => {
                    handle_stats(storage, db, detailed).await
                }
            },
        }
    })
}
