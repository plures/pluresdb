use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use pluresdb_core::{CrdtOperation, CrdtStore};
use pluresdb_storage::{MemoryStorage, StorageEngine, StoredNode};
use serde_json::Value;
use tokio::runtime::Runtime;
use tracing::{error, info};
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
        /// Search query (text or vector)
        query: String,

        /// Limit number of results
        #[arg(long, short = 'l', default_value = "10")]
        limit: usize,

        /// Similarity threshold (0.0-1.0)
        #[arg(long, default_value = "0.7")]
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
        serde_json::from_str(data)
            .context("invalid JSON data")
    }
}

async fn handle_put(
    storage: &MemoryStorage,
    store: &CrdtStore,
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
            obj.insert("tags".to_string(), Value::Array(
                tag_list.iter().map(|s| Value::String(s.clone())).collect()
            ));
        }
    }
    
    let op = CrdtOperation::Put {
        id: id.clone(),
        actor: actor.clone(),
        data: payload.clone(),
    };
    
    store.apply(op)?;
    storage.put(StoredNode {
        id: id.clone(),
        payload,
    }).await?;
    
    println!("{{\"success\":true,\"id\":\"{}\"}}", id);
    Ok(())
}

async fn handle_get(
    storage: &MemoryStorage,
    id: String,
    format: String,
    metadata: bool,
) -> Result<()> {
    match storage.get(&id).await? {
        Some(node) => {
            match format.as_str() {
                "json" => println!("{}", serde_json::to_string(&node.payload)?),
                "pretty" => println!("{}", serde_json::to_string_pretty(&node.payload)?),
                "raw" => println!("{:?}", node),
                _ => println!("{}", serde_json::to_string_pretty(&node.payload)?),
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
    storage: &MemoryStorage,
    node_type: Option<String>,
    tag: Option<String>,
    limit: usize,
    format: String,
) -> Result<()> {
    let mut nodes = storage.list().await?;
    
    // Filter by type
    if let Some(t) = node_type {
        nodes.retain(|n| {
            n.payload.get("type")
                .and_then(|v| v.as_str())
                .map(|s| s == t)
                .unwrap_or(false)
        });
    }
    
    // Filter by tag
    if let Some(tag_filter) = tag {
        nodes.retain(|n| {
            n.payload.get("tags")
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
                let node_type = node.payload.get("type")
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

fn main() -> Result<()> {
    let cli = Cli::parse();
    init_logging(&cli);
    
    info!("PluresDB CLI v{}", VERSION);

    let rt = init_runtime();
    rt.block_on(async move {
        // For now we always use the in-memory engine
        // Future: Use data_dir to initialize persistent storage
        let storage = MemoryStorage::default();
        let store = CrdtStore::default();

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
            
            Commands::Serve { port, bind, websocket } => {
                info!("Starting PluresDB server on {}:{}", bind, port);
                info!("WebSocket support: {}", websocket);
                println!("Server starting on http://{}:{}", bind, port);
                println!("Press Ctrl+C to stop");
                // TODO: Start actual server
                Ok(())
            }
            
            Commands::Status { detailed } => {
                println!("PluresDB Status");
                println!("Version: {}", VERSION);
                println!("Status: Running");
                if detailed {
                    println!("Storage: In-memory");
                    println!("Nodes: {}", storage.list().await?.len());
                }
                Ok(())
            }
            
            Commands::Put { id, data, actor, node_type, tags } => {
                handle_put(&storage, &store, id, data, actor, node_type, tags).await
            }
            
            Commands::Get { id, format, metadata } => {
                handle_get(&storage, id, format, metadata).await
            }
            
            Commands::Delete { id, force } => {
                if !force {
                    println!("Delete node '{}'? [y/N]: ", id);
                    let mut input = String::new();
                    std::io::stdin().read_line(&mut input)?;
                    if !input.trim().eq_ignore_ascii_case("y") {
                        println!("Cancelled");
                        return Ok(());
                    }
                }
                
                storage.delete(&id).await?;
                println!("{{\"success\":true,\"id\":\"{}\"}}", id);
                Ok(())
            }
            
            Commands::List { node_type, tag, limit, format } => {
                handle_list(&storage, node_type, tag, limit, format).await
            }
            
            Commands::Query { query, format, params } => {
                info!("Executing query: {}", query);
                println!("Query execution not yet implemented");
                println!("Query: {}", query);
                if let Some(p) = params {
                    println!("Params: {}", p);
                }
                Ok(())
            }
            
            Commands::Search { query, limit } => {
                info!("Searching for: {}", query);
                println!("Full-text search not yet implemented");
                println!("Query: {}", query);
                println!("Limit: {}", limit);
                Ok(())
            }
            
            Commands::Vsearch { query, limit, threshold } => {
                info!("Vector search for: {}", query);
                println!("Vector search not yet implemented");
                println!("Query: {}", query);
                println!("Limit: {}", limit);
                println!("Threshold: {}", threshold);
                Ok(())
            }
            
            Commands::Type(cmd) => {
                match cmd {
                    TypeCommands::Define { name, schema } => {
                        println!("Type definition not yet implemented");
                        println!("Type: {}", name);
                        if let Some(s) = schema {
                            println!("Schema: {}", s);
                        }
                    }
                    TypeCommands::List => {
                        println!("Type listing not yet implemented");
                    }
                    TypeCommands::Instances { name, limit } => {
                        println!("Instance listing not yet implemented");
                        println!("Type: {}", name);
                        println!("Limit: {}", limit);
                    }
                    TypeCommands::Schema { name } => {
                        println!("Schema display not yet implemented");
                        println!("Type: {}", name);
                    }
                }
                Ok(())
            }
            
            Commands::Network(cmd) => {
                match cmd {
                    NetworkCommands::Connect { url } => {
                        println!("Network connection not yet implemented");
                        println!("URL: {}", url);
                    }
                    NetworkCommands::Disconnect { peer_id } => {
                        println!("Network disconnection not yet implemented");
                        println!("Peer: {}", peer_id);
                    }
                    NetworkCommands::Peers { detailed } => {
                        println!("Peer listing not yet implemented");
                        println!("Detailed: {}", detailed);
                    }
                    NetworkCommands::Sync { peer_id } => {
                        println!("Sync not yet implemented");
                        if let Some(p) = peer_id {
                            println!("Peer: {}", p);
                        }
                    }
                }
                Ok(())
            }
            
            Commands::Config(cmd) => {
                match cmd {
                    ConfigCommands::List => {
                        println!("Configuration:");
                        println!("  data_dir: {:?}", cli.data_dir);
                        println!("  log_level: {}", cli.log_level);
                    }
                    ConfigCommands::Get { key } => {
                        println!("Config get not yet implemented");
                        println!("Key: {}", key);
                    }
                    ConfigCommands::Set { key, value } => {
                        println!("Config set not yet implemented");
                        println!("Key: {}", key);
                        println!("Value: {}", value);
                    }
                    ConfigCommands::Reset { force } => {
                        println!("Config reset not yet implemented");
                        println!("Force: {}", force);
                    }
                }
                Ok(())
            }
            
            Commands::Maintenance(cmd) => {
                match cmd {
                    MaintenanceCommands::Backup { path, compress } => {
                        println!("Backup not yet implemented");
                        println!("Path: {:?}", path);
                        println!("Compress: {}", compress);
                    }
                    MaintenanceCommands::Restore { path, force } => {
                        println!("Restore not yet implemented");
                        println!("Path: {:?}", path);
                        println!("Force: {}", force);
                    }
                    MaintenanceCommands::Vacuum { stats } => {
                        println!("Vacuum not yet implemented");
                        println!("Stats: {}", stats);
                    }
                    MaintenanceCommands::Migrate { version } => {
                        println!("Migration not yet implemented");
                        if let Some(v) = version {
                            println!("Target version: {}", v);
                        }
                    }
                    MaintenanceCommands::Stats { detailed } => {
                let nodes = storage.list().await?;
                        println!("Database Statistics:");
                        println!("  Total nodes: {}", nodes.len());
                        if detailed {
                            // TODO: More detailed stats
                        }
                    }
                }
                Ok(())
            }
        }
    })
}
