//! Node management commands

use anyhow::Result;
use clap::{Args, Subcommand};
use rusty_gun_core::{Node, NodeId};
use rusty_gun_storage::{StorageConfig, SqliteStorage};
use serde_json::Value;
use tracing::{info, error};

#[derive(Args)]
pub struct NodeCommand {
    #[command(subcommand)]
    action: NodeAction,
}

#[derive(Subcommand)]
enum NodeAction {
    /// Create a new node
    Create(CreateNodeArgs),
    /// Get a node by ID
    Get(GetNodeArgs),
    /// Update a node
    Update(UpdateNodeArgs),
    /// Delete a node
    Delete(DeleteNodeArgs),
    /// List all nodes
    List(ListNodesArgs),
    /// Search nodes
    Search(SearchNodesArgs),
    /// Show node relationships
    Relationships(RelationshipsArgs),
}

#[derive(Args)]
struct CreateNodeArgs {
    /// Node ID (auto-generated if not provided)
    #[arg(short, long)]
    id: Option<String>,
    
    /// Node data (JSON)
    #[arg(short, long)]
    data: String,
    
    /// Node metadata (JSON)
    #[arg(long)]
    metadata: Option<String>,
    
    /// Node tags (comma-separated)
    #[arg(long)]
    tags: Option<String>,
    
    /// Database path
    #[arg(long, default_value = "./data/rusty-gun.db")]
    db_path: String,
}

#[derive(Args)]
struct GetNodeArgs {
    /// Node ID
    #[arg(short, long)]
    id: String,
    
    /// Database path
    #[arg(long, default_value = "./data/rusty-gun.db")]
    db_path: String,
    
    /// Show as JSON
    #[arg(long)]
    json: bool,
}

#[derive(Args)]
struct UpdateNodeArgs {
    /// Node ID
    #[arg(short, long)]
    id: String,
    
    /// New node data (JSON)
    #[arg(short, long)]
    data: Option<String>,
    
    /// New node metadata (JSON)
    #[arg(long)]
    metadata: Option<String>,
    
    /// New node tags (comma-separated)
    #[arg(long)]
    tags: Option<String>,
    
    /// Database path
    #[arg(long, default_value = "./data/rusty-gun.db")]
    db_path: String,
}

#[derive(Args)]
struct DeleteNodeArgs {
    /// Node ID
    #[arg(short, long)]
    id: String,
    
    /// Database path
    #[arg(long, default_value = "./data/rusty-gun.db")]
    db_path: String,
    
    /// Force deletion
    #[arg(short, long)]
    force: bool,
}

#[derive(Args)]
struct ListNodesArgs {
    /// Database path
    #[arg(long, default_value = "./data/rusty-gun.db")]
    db_path: String,
    
    /// Limit number of results
    #[arg(short, long, default_value = "100")]
    limit: usize,
    
    /// Offset for pagination
    #[arg(long, default_value = "0")]
    offset: usize,
    
    /// Show as JSON
    #[arg(long)]
    json: bool,
    
    /// Filter by type
    #[arg(long)]
    type_filter: Option<String>,
    
    /// Filter by tag
    #[arg(long)]
    tag_filter: Option<String>,
}

#[derive(Args)]
struct SearchNodesArgs {
    /// Search query
    #[arg(short, long)]
    query: String,
    
    /// Database path
    #[arg(long, default_value = "./data/rusty-gun.db")]
    db_path: String,
    
    /// Limit number of results
    #[arg(short, long, default_value = "10")]
    limit: usize,
    
    /// Show as JSON
    #[arg(long)]
    json: bool,
}

#[derive(Args)]
struct RelationshipsArgs {
    /// Node ID
    #[arg(short, long)]
    id: String,
    
    /// Database path
    #[arg(long, default_value = "./data/rusty-gun.db")]
    db_path: String,
    
    /// Show as JSON
    #[arg(long)]
    json: bool,
}

pub async fn handle_node_command(cmd: NodeCommand) -> Result<()> {
    match cmd.action {
        NodeAction::Create(args) => create_node_command(args).await,
        NodeAction::Get(args) => get_node_command(args).await,
        NodeAction::Update(args) => update_node_command(args).await,
        NodeAction::Delete(args) => delete_node_command(args).await,
        NodeAction::List(args) => list_nodes_command(args).await,
        NodeAction::Search(args) => search_nodes_command(args).await,
        NodeAction::Relationships(args) => relationships_command(args).await,
    }
}

async fn create_node_command(args: CreateNodeArgs) -> Result<()> {
    info!("➕ Creating new node...");
    
    // Parse data
    let data: Value = serde_json::from_str(&args.data)?;
    
    // Parse metadata
    let metadata = if let Some(meta_str) = args.metadata {
        serde_json::from_str(&meta_str)?
    } else {
        serde_json::json!({})
    };
    
    // Parse tags
    let tags = if let Some(tags_str) = args.tags {
        tags_str.split(',').map(|s| s.trim().to_string()).collect()
    } else {
        Vec::new()
    };
    
    // Create node ID
    let node_id = if let Some(id) = args.id {
        NodeId::from(id)
    } else {
        NodeId::from(uuid::Uuid::new_v4().to_string())
    };
    
    // Create node
    let node = Node::new(node_id.clone(), data, metadata, tags);
    
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
    
    // Store node
    storage.store_node(&node).await?;
    
    println!("✅ Node created successfully:");
    println!("  ID: {}", node.id());
    println!("  Data: {}", serde_json::to_string_pretty(node.data())?);
    println!("  Metadata: {}", serde_json::to_string_pretty(node.metadata())?);
    println!("  Tags: {:?}", node.tags());
    
    Ok(())
}

async fn get_node_command(args: GetNodeArgs) -> Result<()> {
    info!("🔍 Getting node: {}", args.id);
    
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
    
    // Get node
    let node_id = NodeId::from(args.id);
    match storage.load_node(&node_id).await? {
        Some(node) => {
            if args.json {
                let node_json = serde_json::json!({
                    "id": node.id(),
                    "data": node.data(),
                    "metadata": node.metadata(),
                    "tags": node.tags(),
                    "created_at": node.created_at(),
                    "updated_at": node.updated_at()
                });
                println!("{}", serde_json::to_string_pretty(&node_json)?);
            } else {
                println!("📄 Node Details:");
                println!("  ID: {}", node.id());
                println!("  Data: {}", serde_json::to_string_pretty(node.data())?);
                println!("  Metadata: {}", serde_json::to_string_pretty(node.metadata())?);
                println!("  Tags: {:?}", node.tags());
                println!("  Created: {}", node.created_at());
                println!("  Updated: {}", node.updated_at());
            }
        }
        None => {
            println!("❌ Node not found: {}", node_id);
        }
    }
    
    Ok(())
}

async fn update_node_command(args: UpdateNodeArgs) -> Result<()> {
    info!("✏️ Updating node: {}", args.id);
    
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
    
    // Get existing node
    let node_id = NodeId::from(args.id.clone());
    let mut node = storage.load_node(&node_id).await?
        .ok_or_else(|| anyhow::anyhow!("Node not found: {}", args.id))?;
    
    // Update fields
    if let Some(data_str) = args.data {
        let data: Value = serde_json::from_str(&data_str)?;
        node.set_data(data);
    }
    
    if let Some(metadata_str) = args.metadata {
        let metadata: Value = serde_json::from_str(&metadata_str)?;
        node.set_metadata(metadata);
    }
    
    if let Some(tags_str) = args.tags {
        let tags: Vec<String> = tags_str.split(',').map(|s| s.trim().to_string()).collect();
        node.set_tags(tags);
    }
    
    // Store updated node
    storage.store_node(&node).await?;
    
    println!("✅ Node updated successfully:");
    println!("  ID: {}", node.id());
    println!("  Data: {}", serde_json::to_string_pretty(node.data())?);
    println!("  Metadata: {}", serde_json::to_string_pretty(node.metadata())?);
    println!("  Tags: {:?}", node.tags());
    
    Ok(())
}

async fn delete_node_command(args: DeleteNodeArgs) -> Result<()> {
    info!("🗑️ Deleting node: {}", args.id);
    
    if !args.force {
        print!("Are you sure you want to delete node '{}'? (y/N): ", args.id);
        use std::io::{self, Write};
        io::stdout().flush()?;
        
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        
        if !input.trim().to_lowercase().starts_with('y') {
            println!("❌ Deletion cancelled");
            return Ok(());
        }
    }
    
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
    
    // Delete node
    let node_id = NodeId::from(args.id);
    storage.delete_node(&node_id).await?;
    
    println!("✅ Node deleted successfully: {}", node_id);
    
    Ok(())
}

async fn list_nodes_command(args: ListNodesArgs) -> Result<()> {
    info!("📋 Listing nodes...");
    
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
    
    // Get node IDs
    let node_ids = storage.list_node_ids().await?;
    let total = node_ids.len();
    
    // Apply pagination
    let start = args.offset;
    let end = (args.offset + args.limit).min(total);
    let paginated_ids = &node_ids[start..end];
    
    if args.json {
        let mut nodes = Vec::new();
        for node_id in paginated_ids {
            if let Some(node) = storage.load_node(node_id).await? {
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
        println!("{}", serde_json::to_string_pretty(&nodes)?);
    } else {
        println!("📋 Nodes (showing {}-{} of {}):", start + 1, end, total);
        
        for (i, node_id) in paginated_ids.iter().enumerate() {
            if let Some(node) = storage.load_node(node_id).await? {
                println!("  {}. {} - {}", 
                    start + i + 1, 
                    node.id(),
                    serde_json::to_string(node.data())?
                );
            }
        }
        
        if total > args.limit {
            println!("  ... and {} more nodes", total - args.limit);
        }
    }
    
    Ok(())
}

async fn search_nodes_command(args: SearchNodesArgs) -> Result<()> {
    info!("🔍 Searching nodes: {}", args.query);
    
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
    
    // Search nodes
    let nodes = storage.search_nodes(&args.query).await?;
    let total = nodes.len();
    
    // Apply limit
    let limited_nodes = if nodes.len() > args.limit {
        &nodes[..args.limit]
    } else {
        &nodes
    };
    
    if args.json {
        let mut results = Vec::new();
        for node in limited_nodes {
            results.push(serde_json::json!({
                "id": node.id(),
                "data": node.data(),
                "metadata": node.metadata(),
                "tags": node.tags(),
                "created_at": node.created_at(),
                "updated_at": node.updated_at()
            }));
        }
        println!("{}", serde_json::to_string_pretty(&results)?);
    } else {
        println!("🔍 Search Results for '{}' (showing {} of {}):", 
            args.query, limited_nodes.len(), total);
        
        for (i, node) in limited_nodes.iter().enumerate() {
            println!("  {}. {} - {}", 
                i + 1, 
                node.id(),
                serde_json::to_string(node.data())?
            );
        }
        
        if total > args.limit {
            println!("  ... and {} more results", total - args.limit);
        }
    }
    
    Ok(())
}

async fn relationships_command(args: RelationshipsArgs) -> Result<()> {
    info!("🔗 Getting relationships for node: {}", args.id);
    
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
    
    // Get relationships
    let node_id = NodeId::from(args.id);
    let relationships = storage.load_relationships(&node_id).await?;
    
    if args.json {
        let mut results = Vec::new();
        for rel in &relationships {
            results.push(serde_json::json!({
                "from": rel.from(),
                "to": rel.to(),
                "relation_type": rel.relation_type(),
                "metadata": rel.metadata(),
                "created_at": rel.created_at()
            }));
        }
        println!("{}", serde_json::to_string_pretty(&results)?);
    } else {
        println!("🔗 Relationships for node '{}':", node_id);
        
        if relationships.is_empty() {
            println!("  No relationships found");
        } else {
            for (i, rel) in relationships.iter().enumerate() {
                println!("  {}. {} --[{}]--> {}", 
                    i + 1,
                    rel.from(),
                    rel.relation_type(),
                    rel.to()
                );
            }
        }
    }
    
    Ok(())
}


