//! WAL replay command-line tool.

use anyhow::Result;
use clap::Parser;
use pluresdb_storage::{rebuild_from_wal, replay_wal};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "pluresdb-replay")]
#[command(about = "Replay PluresDB WAL operations for debugging and recovery")]
struct Args {
    /// Path to WAL directory
    #[arg(short, long)]
    wal_dir: PathBuf,
    
    /// Filter operations by actor
    #[arg(short, long)]
    actor: Option<String>,
    
    /// Validate checksums during replay
    #[arg(short, long, default_value_t = true)]
    validate: bool,
    
    /// Output format (json or summary)
    #[arg(short, long, default_value = "summary")]
    output: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();
    
    let args = Args::parse();
    
    println!("PluresDB WAL Replay Tool");
    println!("========================");
    println!("WAL directory: {}", args.wal_dir.display());
    
    if args.validate {
        println!("Validating checksums...");
    }
    
    // Perform replay
    let (state, stats) = if args.validate {
        rebuild_from_wal(&args.wal_dir, true).await?
    } else {
        replay_wal(&args.wal_dir, args.actor.as_deref()).await?
    };
    
    // Output results
    match args.output.as_str() {
        "json" => {
            let output = serde_json::json!({
                "stats": {
                    "total_entries": stats.total_entries,
                    "puts": stats.puts,
                    "deletes": stats.deletes,
                    "checkpoints": stats.checkpoints,
                    "compacts": stats.compacts,
                    "errors": stats.errors,
                    "final_node_count": stats.final_node_count,
                    "success_rate": stats.success_rate(),
                },
                "state": state,
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        _ => {
            println!("\nReplay Statistics:");
            println!("  Total entries: {}", stats.total_entries);
            println!("  Put operations: {}", stats.puts);
            println!("  Delete operations: {}", stats.deletes);
            println!("  Checkpoints: {}", stats.checkpoints);
            println!("  Compacts: {}", stats.compacts);
            println!("  Errors: {}", stats.errors);
            println!("  Final node count: {}", stats.final_node_count);
            println!("  Success rate: {:.2}%", stats.success_rate() * 100.0);
            
            println!("\nFinal State:");
            for (id, data) in state.iter().take(10) {
                println!("  {} -> {}", id, data);
            }
            if state.len() > 10 {
                println!("  ... and {} more nodes", state.len() - 10);
            }
        }
    }
    
    Ok(())
}
