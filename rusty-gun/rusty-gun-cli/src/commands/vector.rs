//! Vector search commands

use anyhow::Result;
use clap::{Args, Subcommand};
use rusty_gun_storage::{
    VectorSearchService, EmbeddingConfig, EmbeddingModel, VectorConfig,
    SqliteStorage, StorageConfig,
};
use serde_json::Value;
use tracing::{info, error};

#[derive(Args)]
pub struct VectorCommand {
    #[command(subcommand)]
    action: VectorAction,
}

#[derive(Subcommand)]
enum VectorAction {
    /// Add text content for vector search
    Add(AddTextArgs),
    /// Search for similar text
    Search(SearchTextArgs),
    /// Generate embedding for text
    Embed(EmbedTextArgs),
    /// List all vector content
    List(ListVectorArgs),
    /// Get vector statistics
    Stats(StatsArgs),
    /// Clear all vector data
    Clear(ClearArgs),
}

#[derive(Args)]
struct AddTextArgs {
    /// Content ID
    #[arg(short, long)]
    id: String,
    
    /// Text content
    #[arg(short, long)]
    text: String,
    
    /// Metadata (JSON)
    #[arg(long)]
    metadata: Option<String>,
    
    /// Database path
    #[arg(long, default_value = "./data/rusty-gun.db")]
    db_path: String,
    
    /// Vector dimensions
    #[arg(long, default_value = "384")]
    dimensions: usize,
    
    /// Embedding model
    #[arg(long, default_value = "sentence-transformers-minilm")]
    model: String,
    
    /// OpenAI API key (for OpenAI models)
    #[arg(long)]
    openai_api_key: Option<String>,
}

#[derive(Args)]
struct SearchTextArgs {
    /// Search query
    #[arg(short, long)]
    query: String,
    
    /// Database path
    #[arg(long, default_value = "./data/rusty-gun.db")]
    db_path: String,
    
    /// Number of results
    #[arg(short, long, default_value = "5")]
    limit: usize,
    
    /// Similarity threshold (0.0-1.0)
    #[arg(long, default_value = "0.3")]
    threshold: f32,
    
    /// Show as JSON
    #[arg(long)]
    json: bool,
    
    /// Vector dimensions
    #[arg(long, default_value = "384")]
    dimensions: usize,
    
    /// Embedding model
    #[arg(long, default_value = "sentence-transformers-minilm")]
    model: String,
    
    /// OpenAI API key (for OpenAI models)
    #[arg(long)]
    openai_api_key: Option<String>,
}

#[derive(Args)]
struct EmbedTextArgs {
    /// Text to embed
    #[arg(short, long)]
    text: String,
    
    /// Database path
    #[arg(long, default_value = "./data/rusty-gun.db")]
    db_path: String,
    
    /// Show as JSON
    #[arg(long)]
    json: bool,
    
    /// Vector dimensions
    #[arg(long, default_value = "384")]
    dimensions: usize,
    
    /// Embedding model
    #[arg(long, default_value = "sentence-transformers-minilm")]
    model: String,
    
    /// OpenAI API key (for OpenAI models)
    #[arg(long)]
    openai_api_key: Option<String>,
}

#[derive(Args)]
struct ListVectorArgs {
    /// Database path
    #[arg(long, default_value = "./data/rusty-gun.db")]
    db_path: String,
    
    /// Limit number of results
    #[arg(short, long, default_value = "100")]
    limit: usize,
    
    /// Show as JSON
    #[arg(long)]
    json: bool,
    
    /// Vector dimensions
    #[arg(long, default_value = "384")]
    dimensions: usize,
}

#[derive(Args)]
struct StatsArgs {
    /// Database path
    #[arg(long, default_value = "./data/rusty-gun.db")]
    db_path: String,
    
    /// Show as JSON
    #[arg(long)]
    json: bool,
    
    /// Vector dimensions
    #[arg(long, default_value = "384")]
    dimensions: usize,
}

#[derive(Args)]
struct ClearArgs {
    /// Database path
    #[arg(long, default_value = "./data/rusty-gun.db")]
    db_path: String,
    
    /// Force clear without confirmation
    #[arg(short, long)]
    force: bool,
    
    /// Vector dimensions
    #[arg(long, default_value = "384")]
    dimensions: usize,
}

pub async fn handle_vector_command(cmd: VectorCommand) -> Result<()> {
    match cmd.action {
        VectorAction::Add(args) => add_text_command(args).await,
        VectorAction::Search(args) => search_text_command(args).await,
        VectorAction::Embed(args) => embed_text_command(args).await,
        VectorAction::List(args) => list_vector_command(args).await,
        VectorAction::Stats(args) => stats_command(args).await,
        VectorAction::Clear(args) => clear_command(args).await,
    }
}

async fn add_text_command(args: AddTextArgs) -> Result<()> {
    info!("➕ Adding text content for vector search...");
    
    // Parse metadata
    let metadata = if let Some(meta_str) = args.metadata {
        serde_json::from_str(&meta_str)?
    } else {
        serde_json::json!({})
    };
    
    // Create vector search service
    let vector_config = VectorConfig {
        dimensions: args.dimensions,
        max_vectors: 1_000_000,
        hnsw_m: 16,
        hnsw_ef_construction: 200,
        hnsw_ef: 50,
    };
    
    let embedding_model = match args.model.as_str() {
        "openai-ada-002" => EmbeddingModel::OpenAITextEmbeddingAda002,
        "openai-3-small" => EmbeddingModel::OpenAITextEmbedding3Small,
        "openai-3-large" => EmbeddingModel::OpenAITextEmbedding3Large,
        "sentence-transformers-mpnet" => EmbeddingModel::SentenceTransformersMPNet,
        _ => EmbeddingModel::SentenceTransformersMiniLM,
    };
    
    let embedding_config = EmbeddingConfig {
        model: embedding_model,
        api_key: args.openai_api_key,
        base_url: None,
        max_text_length: 8192,
        batch_size: 100,
        enable_cache: true,
        cache_dir: Some("./cache/embeddings".to_string()),
    };
    
    let mut vector_service = VectorSearchService::new(vector_config, embedding_config);
    vector_service.initialize().await?;
    
    // Add text content
    vector_service.add_text(&args.id, &args.text, &metadata).await?;
    
    println!("✅ Text content added successfully:");
    println!("  ID: {}", args.id);
    println!("  Text: {}", args.text);
    println!("  Metadata: {}", serde_json::to_string_pretty(&metadata)?);
    println!("  Dimensions: {}", args.dimensions);
    println!("  Model: {}", args.model);
    
    Ok(())
}

async fn search_text_command(args: SearchTextArgs) -> Result<()> {
    info!("🔍 Searching for similar text: {}", args.query);
    
    // Create vector search service
    let vector_config = VectorConfig {
        dimensions: args.dimensions,
        max_vectors: 1_000_000,
        hnsw_m: 16,
        hnsw_ef_construction: 200,
        hnsw_ef: 50,
    };
    
    let embedding_model = match args.model.as_str() {
        "openai-ada-002" => EmbeddingModel::OpenAITextEmbeddingAda002,
        "openai-3-small" => EmbeddingModel::OpenAITextEmbedding3Small,
        "openai-3-large" => EmbeddingModel::OpenAITextEmbedding3Large,
        "sentence-transformers-mpnet" => EmbeddingModel::SentenceTransformersMPNet,
        _ => EmbeddingModel::SentenceTransformersMiniLM,
    };
    
    let embedding_config = EmbeddingConfig {
        model: embedding_model,
        api_key: args.openai_api_key,
        base_url: None,
        max_text_length: 8192,
        batch_size: 100,
        enable_cache: true,
        cache_dir: Some("./cache/embeddings".to_string()),
    };
    
    let mut vector_service = VectorSearchService::new(vector_config, embedding_config);
    vector_service.initialize().await?;
    
    // Search for similar text
    let results = vector_service.search_text(&args.query, args.limit).await?;
    
    if args.json {
        let mut json_results = Vec::new();
        for result in &results {
            json_results.push(serde_json::json!({
                "id": result.id,
                "score": result.score,
                "metadata": result.metadata,
                "text_hash": result.text_hash
            }));
        }
        println!("{}", serde_json::to_string_pretty(&json_results)?);
    } else {
        println!("🔍 Search Results for '{}' (threshold: {:.2}):", 
            args.query, args.threshold);
        
        if results.is_empty() {
            println!("  No similar content found");
        } else {
            for (i, result) in results.iter().enumerate() {
                println!("  {}. {} (similarity: {:.1}%)", 
                    i + 1, 
                    result.id,
                    result.score * 100.0
                );
                if let Some(title) = result.metadata.get("title").and_then(|v| v.as_str()) {
                    println!("     Title: {}", title);
                }
                if let Some(category) = result.metadata.get("category").and_then(|v| v.as_str()) {
                    println!("     Category: {}", category);
                }
            }
        }
    }
    
    Ok(())
}

async fn embed_text_command(args: EmbedTextArgs) -> Result<()> {
    info!("🧠 Generating embedding for text...");
    
    // Create vector search service
    let vector_config = VectorConfig {
        dimensions: args.dimensions,
        max_vectors: 1_000_000,
        hnsw_m: 16,
        hnsw_ef_construction: 200,
        hnsw_ef: 50,
    };
    
    let embedding_model = match args.model.as_str() {
        "openai-ada-002" => EmbeddingModel::OpenAITextEmbeddingAda002,
        "openai-3-small" => EmbeddingModel::OpenAITextEmbedding3Small,
        "openai-3-large" => EmbeddingModel::OpenAITextEmbedding3Large,
        "sentence-transformers-mpnet" => EmbeddingModel::SentenceTransformersMPNet,
        _ => EmbeddingModel::SentenceTransformersMiniLM,
    };
    
    let embedding_config = EmbeddingConfig {
        model: embedding_model,
        api_key: args.openai_api_key,
        base_url: None,
        max_text_length: 8192,
        batch_size: 100,
        enable_cache: true,
        cache_dir: Some("./cache/embeddings".to_string()),
    };
    
    let mut vector_service = VectorSearchService::new(vector_config, embedding_config);
    vector_service.initialize().await?;
    
    // Generate embedding
    let result = vector_service.generate_embedding(&args.text).await?;
    
    if args.json {
        let embedding_json = serde_json::json!({
            "text": result.text,
            "vector": result.vector,
            "dimensions": result.dimensions,
            "model": result.model.name(),
            "created_at": result.created_at
        });
        println!("{}", serde_json::to_string_pretty(&embedding_json)?);
    } else {
        println!("🧠 Embedding generated:");
        println!("  Text: {}", result.text);
        println!("  Dimensions: {}", result.dimensions);
        println!("  Model: {}", result.model.name());
        println!("  Vector (first 10 values): [{:.4}, {:.4}, {:.4}, {:.4}, {:.4}, {:.4}, {:.4}, {:.4}, {:.4}, {:.4}, ...]",
            result.vector[0], result.vector[1], result.vector[2], result.vector[3], result.vector[4],
            result.vector[5], result.vector[6], result.vector[7], result.vector[8], result.vector[9]
        );
    }
    
    Ok(())
}

async fn list_vector_command(args: ListVectorArgs) -> Result<()> {
    info!("📋 Listing vector content...");
    
    // Create vector search service
    let vector_config = VectorConfig {
        dimensions: args.dimensions,
        max_vectors: 1_000_000,
        hnsw_m: 16,
        hnsw_ef_construction: 200,
        hnsw_ef: 50,
    };
    
    let embedding_config = EmbeddingConfig {
        model: EmbeddingModel::SentenceTransformersMiniLM,
        api_key: None,
        base_url: None,
        max_text_length: 8192,
        batch_size: 100,
        enable_cache: true,
        cache_dir: Some("./cache/embeddings".to_string()),
    };
    
    let mut vector_service = VectorSearchService::new(vector_config, embedding_config);
    vector_service.initialize().await?;
    
    // Get statistics
    let stats = vector_service.get_stats().await?;
    
    if args.json {
        let stats_json = serde_json::json!({
            "vector_count": stats.vector_count,
            "dimensions": stats.dimensions,
            "index_size": stats.index_size,
            "last_updated": stats.last_updated,
            "cache_size": stats.cache_size,
            "cache_hits": stats.cache_hits,
            "cache_misses": stats.cache_misses
        });
        println!("{}", serde_json::to_string_pretty(&stats_json)?);
    } else {
        println!("📋 Vector Content Statistics:");
        println!("  Total vectors: {}", stats.vector_count);
        println!("  Dimensions: {}", stats.dimensions);
        println!("  Index size: {:.2} MB", stats.index_size as f64 / 1024.0 / 1024.0);
        println!("  Cache size: {} entries", stats.cache_size);
        println!("  Cache hits: {}", stats.cache_hits);
        println!("  Cache misses: {}", stats.cache_misses);
        println!("  Last updated: {}", stats.last_updated);
    }
    
    Ok(())
}

async fn stats_command(args: StatsArgs) -> Result<()> {
    info!("📊 Getting vector search statistics...");
    
    // Create vector search service
    let vector_config = VectorConfig {
        dimensions: args.dimensions,
        max_vectors: 1_000_000,
        hnsw_m: 16,
        hnsw_ef_construction: 200,
        hnsw_ef: 50,
    };
    
    let embedding_config = EmbeddingConfig {
        model: EmbeddingModel::SentenceTransformersMiniLM,
        api_key: None,
        base_url: None,
        max_text_length: 8192,
        batch_size: 100,
        enable_cache: true,
        cache_dir: Some("./cache/embeddings".to_string()),
    };
    
    let mut vector_service = VectorSearchService::new(vector_config, embedding_config);
    vector_service.initialize().await?;
    
    // Get statistics
    let stats = vector_service.get_stats().await?;
    let model_info = vector_service.get_model_info();
    
    if args.json {
        let stats_json = serde_json::json!({
            "vector_count": stats.vector_count,
            "dimensions": stats.dimensions,
            "index_size": stats.index_size,
            "last_updated": stats.last_updated,
            "cache_size": stats.cache_size,
            "cache_hits": stats.cache_hits,
            "cache_misses": stats.cache_misses,
            "model_info": {
                "model_name": model_info.model_name,
                "dimensions": model_info.dimensions,
                "max_text_length": model_info.max_text_length,
                "batch_size": model_info.batch_size,
                "cache_enabled": model_info.cache_enabled
            }
        });
        println!("{}", serde_json::to_string_pretty(&stats_json)?);
    } else {
        println!("📊 Vector Search Statistics:");
        println!("  Vector count: {}", stats.vector_count);
        println!("  Dimensions: {}", stats.dimensions);
        println!("  Index size: {:.2} MB", stats.index_size as f64 / 1024.0 / 1024.0);
        println!("  Cache size: {} entries", stats.cache_size);
        println!("  Cache hits: {}", stats.cache_hits);
        println!("  Cache misses: {}", stats.cache_misses);
        println!("  Last updated: {}", stats.last_updated);
        
        println!("\n🤖 Model Information:");
        println!("  Model: {}", model_info.model_name);
        println!("  Dimensions: {}", model_info.dimensions);
        println!("  Max text length: {}", model_info.max_text_length);
        println!("  Batch size: {}", model_info.batch_size);
        println!("  Cache enabled: {}", model_info.cache_enabled);
    }
    
    Ok(())
}

async fn clear_command(args: ClearArgs) -> Result<()> {
    info!("🗑️ Clearing vector data...");
    
    if !args.force {
        print!("Are you sure you want to clear all vector data? (y/N): ");
        use std::io::{self, Write};
        io::stdout().flush()?;
        
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        
        if !input.trim().to_lowercase().starts_with('y') {
            println!("❌ Clear operation cancelled");
            return Ok(());
        }
    }
    
    // Create vector search service
    let vector_config = VectorConfig {
        dimensions: args.dimensions,
        max_vectors: 1_000_000,
        hnsw_m: 16,
        hnsw_ef_construction: 200,
        hnsw_ef: 50,
    };
    
    let embedding_config = EmbeddingConfig {
        model: EmbeddingModel::SentenceTransformersMiniLM,
        api_key: None,
        base_url: None,
        max_text_length: 8192,
        batch_size: 100,
        enable_cache: true,
        cache_dir: Some("./cache/embeddings".to_string()),
    };
    
    let mut vector_service = VectorSearchService::new(vector_config, embedding_config);
    vector_service.initialize().await?;
    
    // Clear all data
    vector_service.clear_all().await?;
    
    println!("✅ Vector data cleared successfully");
    
    Ok(())
}


