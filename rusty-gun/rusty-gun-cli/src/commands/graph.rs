//! Graph operations commands

use anyhow::Result;
use clap::{Args, Subcommand};
use rusty_gun_core::{NodeId, Relationship};
use rusty_gun_storage::{StorageConfig, SqliteStorage};
use tracing::info;

#[derive(Args)]
pub struct GraphCommand {
    #[command(subcommand)]
    action: GraphAction,
}

#[derive(Subcommand)]
enum GraphAction {
    /// Create a relationship between nodes
    Connect(ConnectArgs),
    /// Remove a relationship between nodes
    Disconnect(DisconnectArgs),
    /// Find path between nodes
    Path(PathArgs),
    /// Show graph statistics
    Stats(StatsArgs),
    /// Export graph data
    Export(ExportArgs),
}

#[derive(Args)]
struct ConnectArgs {
    /// Source node ID
    #[arg(short, long)]
    from: String,
    
    /// Target node ID
    #[arg(short, long)]
    to: String,
    
    /// Relationship type
    #[arg(short, long)]
    relation_type: String,
    
    /// Relationship metadata (JSON)
    #[arg(long)]
    metadata: Option<String>,
    
    /// Database path
    #[arg(long, default_value = "./data/rusty-gun.db")]
    db_path: String,
}

#[derive(Args)]
struct DisconnectArgs {
    /// Source node ID
    #[arg(short, long)]
    from: String,
    
    /// Target node ID
    #[arg(short, long)]
    to: String,
    
    /// Relationship type
    #[arg(short, long)]
    relation_type: String,
    
    /// Database path
    #[arg(long, default_value = "./data/rusty-gun.db")]
    db_path: String,
}

#[derive(Args)]
struct PathArgs {
    /// Source node ID
    #[arg(short, long)]
    from: String,
    
    /// Target node ID
    #[arg(short, long)]
    to: String,
    
    /// Database path
    #[arg(long, default_value = "./data/rusty-gun.db")]
    db_path: String,
    
    /// Show as JSON
    #[arg(long)]
    json: bool,
}

#[derive(Args)]
struct StatsArgs {
    /// Database path
    #[arg(long, default_value = "./data/rusty-gun.db")]
    db_path: String,
    
    /// Show as JSON
    #[arg(long)]
    json: bool,
}

#[derive(Args)]
struct ExportArgs {
    /// Output file path
    #[arg(short, long, default_value = "graph_export.json")]
    output: String,
    
    /// Database path
    #[arg(long, default_value = "./data/rusty-gun.db")]
    db_path: String,
    
    /// Export format
    #[arg(long, default_value = "json")]
    format: String,
}

pub async fn handle_graph_command(cmd: GraphCommand) -> Result<()> {
    match cmd.action {
        GraphAction::Connect(args) => connect_command(args).await,
        GraphAction::Disconnect(args) => disconnect_command(args).await,
        GraphAction::Path(args) => path_command(args).await,
        GraphAction::Stats(args) => stats_command(args).await,
        GraphAction::Export(args) => export_command(args).await,
    }
}

async fn connect_command(args: ConnectArgs) -> Result<()> {
    info!("🔗 Creating relationship: {} --[{}]--> {}", 
        args.from, args.relation_type, args.to);
    
    // Parse metadata
    let metadata = if let Some(meta_str) = args.metadata {
        serde_json::from_str(&meta_str)?
    } else {
        serde_json::json!({})
    };
    
    // Create relationship
    let relationship = Relationship::new(
        NodeId::from(args.from.clone()),
        NodeId::from(args.to.clone()),
        args.relation_type.clone(),
        metadata,
    );
    
    // Initialize storage
    let storage_config = StorageConfig {
        backend: rusty_gun_storage::StorageBackend::Sqlite,
        path: args.db_path,
        max_connections: 10,
        enable_wal: true,
        enable_foreign_keys: true,
        vector_config: rusty_gun_storage::VectorConfig::default(),
    };
    
    let mut storage = SqliteStorage::new(storage_config)?;
    storage.initialize().await?;
    
    // Store relationship
    storage.store_relationship(&relationship).await?;
    
    println!("✅ Relationship created successfully:");
    println!("  From: {}", args.from);
    println!("  To: {}", args.to);
    println!("  Type: {}", args.relation_type);
    println!("  Metadata: {}", serde_json::to_string_pretty(relationship.metadata())?);
    
    Ok(())
}

async fn disconnect_command(args: DisconnectArgs) -> Result<()> {
    info!("🔗 Removing relationship: {} --[{}]--> {}", 
        args.from, args.relation_type, args.to);
    
    // Initialize storage
    let storage_config = StorageConfig {
        backend: rusty_gun_storage::StorageBackend::Sqlite,
        path: args.db_path,
        max_connections: 10,
        enable_wal: true,
        enable_foreign_keys: true,
        vector_config: rusty_gun_storage::VectorConfig::default(),
    };
    
    let mut storage = SqliteStorage::new(storage_config)?;
    storage.initialize().await?;
    
    // Delete relationship
    storage.delete_relationship(
        &NodeId::from(args.from.clone()),
        &NodeId::from(args.to.clone()),
        &args.relation_type,
    ).await?;
    
    println!("✅ Relationship removed successfully:");
    println!("  From: {}", args.from);
    println!("  To: {}", args.to);
    println!("  Type: {}", args.relation_type);
    
    Ok(())
}

async fn path_command(args: PathArgs) -> Result<()> {
    info!("🛤️ Finding path from {} to {}", args.from, args.to);
    
    // Initialize storage
    let storage_config = StorageConfig {
        backend: rusty_gun_storage::StorageBackend::Sqlite,
        path: args.db_path,
        max_connections: 10,
        enable_wal: true,
        enable_foreign_keys: true,
        vector_config: rusty_gun_storage::VectorConfig::default(),
    };
    
    let mut storage = SqliteStorage::new(storage_config)?;
    storage.initialize().await?;
    
    // For now, implement a simple path finding algorithm
    // In a real implementation, this would use a proper graph traversal algorithm
    let from_id = NodeId::from(args.from.clone());
    let to_id = NodeId::from(args.to.clone());
    
    // Check if nodes exist
    let from_node = storage.load_node(&from_id).await?;
    let to_node = storage.load_node(&to_id).await?;
    
    if from_node.is_none() {
        println!("❌ Source node not found: {}", args.from);
        return Ok(());
    }
    
    if to_node.is_none() {
        println!("❌ Target node not found: {}", args.to);
        return Ok(());
    }
    
    // Simple path finding (direct connection check)
    let relationships = storage.load_relationships(&from_id).await?;
    let mut path = vec![args.from.clone()];
    
    for rel in relationships {
        if rel.to() == to_id {
            path.push(args.to.clone());
            break;
        }
    }
    
    if args.json {
        let path_json = serde_json::json!({
            "from": args.from,
            "to": args.to,
            "path": path,
            "length": path.len() - 1
        });
        println!("{}", serde_json::to_string_pretty(&path_json)?);
    } else {
        if path.len() > 1 {
            println!("🛤️ Path found (length: {}):", path.len() - 1);
            for (i, node_id) in path.iter().enumerate() {
                if i == path.len() - 1 {
                    println!("  {} ──> {}", node_id, args.to);
                } else {
                    print!("  {} ──> ", node_id);
                }
            }
        } else {
            println!("❌ No path found from {} to {}", args.from, args.to);
        }
    }
    
    Ok(())
}

async fn stats_command(args: StatsArgs) -> Result<()> {
    info!("📊 Getting graph statistics...");
    
    // Initialize storage
    let storage_config = StorageConfig {
        backend: rusty_gun_storage::StorageBackend::Sqlite,
        path: args.db_path,
        max_connections: 10,
        enable_wal: true,
        enable_foreign_keys: true,
        vector_config: rusty_gun_storage::VectorConfig::default(),
    };
    
    let mut storage = SqliteStorage::new(storage_config)?;
    storage.initialize().await?;
    
    // Get statistics
    let stats = storage.get_stats().await?;
    
    if args.json {
        let stats_json = serde_json::json!({
            "node_count": stats.node_count,
            "relationship_count": stats.relationship_count,
            "storage_size": stats.storage_size,
            "index_count": stats.index_count,
            "last_updated": stats.last_updated
        });
        println!("{}", serde_json::to_string_pretty(&stats_json)?);
    } else {
        println!("📊 Graph Statistics:");
        println!("  Nodes: {}", stats.node_count);
        println!("  Relationships: {}", stats.relationship_count);
        println!("  Storage size: {:.2} MB", stats.storage_size as f64 / 1024.0 / 1024.0);
        println!("  Indexes: {}", stats.index_count);
        println!("  Last updated: {}", stats.last_updated);
    }
    
    Ok(())
}

async fn export_command(args: ExportArgs) -> Result<()> {
    info!("📤 Exporting graph data to {}", args.output);
    
    // Initialize storage
    let storage_config = StorageConfig {
        backend: rusty_gun_storage::StorageBackend::Sqlite,
        path: args.db_path,
        max_connections: 10,
        enable_wal: true,
        enable_foreign_keys: true,
        vector_config: rusty_gun_storage::VectorConfig::default(),
    };
    
    let mut storage = SqliteStorage::new(storage_config)?;
    storage.initialize().await?;
    
    // Get all nodes
    let node_ids = storage.list_node_ids().await?;
    let mut nodes = Vec::new();
    
    for node_id in node_ids {
        if let Some(node) = storage.load_node(&node_id).await? {
            nodes.push(serde_json::json!({
                "id": node.id(),
                "data": node.data(),
                "metadata": node.metadata(),
                "tags": node.tags(),
                "created_at": node.created_at(),
                "updated_at": node.updated_at()
            }));
        }
    }
    
    // Get all relationships
    let mut relationships = Vec::new();
    for node_id in &node_ids {
        let rels = storage.load_relationships(node_id).await?;
        for rel in rels {
            relationships.push(serde_json::json!({
                "from": rel.from(),
                "to": rel.to(),
                "relation_type": rel.relation_type(),
                "metadata": rel.metadata(),
                "created_at": rel.created_at()
            }));
        }
    }
    
    // Create export data
    let export_data = serde_json::json!({
        "exported_at": chrono::Utc::now(),
        "format": args.format,
        "statistics": {
            "node_count": nodes.len(),
            "relationship_count": relationships.len()
        },
        "nodes": nodes,
        "relationships": relationships
    });
    
    // Write to file
    let content = serde_json::to_string_pretty(&export_data)?;
    std::fs::write(&args.output, content)?;
    
    println!("✅ Graph exported successfully:");
    println!("  File: {}", args.output);
    println!("  Nodes: {}", nodes.len());
    println!("  Relationships: {}", relationships.len());
    println!("  Format: {}", args.format);
    
    Ok(())
}


