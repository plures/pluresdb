//! SQL query commands

use anyhow::Result;
use clap::{Args, Subcommand};
use rusty_gun_storage::{StorageConfig, SqliteStorage};
use tracing::info;

#[derive(Args)]
pub struct SqlCommand {
    #[command(subcommand)]
    action: SqlAction,
}

#[derive(Subcommand)]
enum SqlAction {
    /// Execute SQL query
    Query(QueryArgs),
    /// Explain SQL query
    Explain(ExplainArgs),
}

#[derive(Args)]
struct QueryArgs {
    /// SQL query
    #[arg(short, long)]
    query: String,
    
    /// Query parameters (JSON array)
    #[arg(long)]
    params: Option<String>,
    
    /// Database path
    #[arg(long, default_value = "./data/rusty-gun.db")]
    db_path: String,
    
    /// Show as JSON
    #[arg(long)]
    json: bool,
}

#[derive(Args)]
struct ExplainArgs {
    /// SQL query
    #[arg(short, long)]
    query: String,
    
    /// Database path
    #[arg(long, default_value = "./data/rusty-gun.db")]
    db_path: String,
}

pub async fn handle_sql_command(cmd: SqlCommand) -> Result<()> {
    match cmd.action {
        SqlAction::Query(args) => query_command(args).await,
        SqlAction::Explain(args) => explain_command(args).await,
    }
}

async fn query_command(args: QueryArgs) -> Result<()> {
    info!("🔍 Executing SQL query: {}", args.query);
    
    // Parse parameters
    let params = if let Some(params_str) = args.params {
        serde_json::from_str::<Vec<serde_json::Value>>(&params_str)?
    } else {
        Vec::new()
    };
    
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
    
    // Execute query
    let result = storage.execute_query(&args.query, &params).await?;
    
    if args.json {
        let result_json = serde_json::json!({
            "rows": result.rows,
            "columns": result.columns,
            "changes": result.changes,
            "last_insert_row_id": result.last_insert_row_id
        });
        println!("{}", serde_json::to_string_pretty(&result_json)?);
    } else {
        println!("📊 Query Results:");
        println!("  Rows: {}", result.rows.len());
        println!("  Columns: {:?}", result.columns);
        println!("  Changes: {}", result.changes);
        println!("  Last insert row ID: {}", result.last_insert_row_id);
        
        if !result.rows.is_empty() {
            println!("\n📋 Data:");
            for (i, row) in result.rows.iter().enumerate() {
                println!("  Row {}: {:?}", i + 1, row);
            }
        }
    }
    
    Ok(())
}

async fn explain_command(args: ExplainArgs) -> Result<()> {
    info!("🔍 Explaining SQL query: {}", args.query);
    
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
    
    // Explain query
    let explain_query = format!("EXPLAIN QUERY PLAN {}", args.query);
    let result = storage.execute_query(&explain_query, &[]).await?;
    
    println!("🔍 Query Explanation:");
    for (i, row) in result.rows.iter().enumerate() {
        println!("  {}: {:?}", i + 1, row);
    }
    
    Ok(())
}


