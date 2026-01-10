//! WAL inspection and validation tool.

use anyhow::Result;
use clap::Parser;
use pluresdb_storage::WriteAheadLog;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "pluresdb-inspect")]
#[command(about = "Inspect PluresDB WAL and show storage breakdown")]
struct Args {
    /// Path to data directory containing WAL
    #[arg(short, long)]
    data_dir: PathBuf,
    
    /// Show detailed breakdown
    #[arg(short, long)]
    detailed: bool,
    
    /// Check integrity (validate checksums)
    #[arg(short, long)]
    check_integrity: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();
    
    let args = Args::parse();
    
    println!("PluresDB WAL Inspection Tool");
    println!("============================");
    println!("Data directory: {}", args.data_dir.display());
    
    let wal = WriteAheadLog::open(&args.data_dir)?;
    
    // Read entries
    println!("\nReading WAL entries...");
    let entries = wal.read_all().await?;
    
    println!("\nBasic Statistics:");
    println!("  Total entries: {}", entries.len());
    
    if entries.is_empty() {
        println!("  WAL is empty.");
        return Ok(());
    }
    
    let min_seq = entries.iter().map(|e| e.seq).min().unwrap_or(0);
    let max_seq = entries.iter().map(|e| e.seq).max().unwrap_or(0);
    
    println!("  Sequence range: {} - {}", min_seq, max_seq);
    println!("  Sequence gaps: {}", (max_seq - min_seq + 1) - entries.len() as u64);
    
    // Count operation types
    let mut put_count = 0;
    let mut delete_count = 0;
    let mut checkpoint_count = 0;
    let mut compact_count = 0;
    
    for entry in &entries {
        match &entry.operation {
            pluresdb_storage::WalOperation::Put { .. } => put_count += 1,
            pluresdb_storage::WalOperation::Delete { .. } => delete_count += 1,
            pluresdb_storage::WalOperation::Checkpoint { .. } => checkpoint_count += 1,
            pluresdb_storage::WalOperation::Compact { .. } => compact_count += 1,
        }
    }
    
    println!("\nOperation Breakdown:");
    println!("  Put operations: {}", put_count);
    println!("  Delete operations: {}", delete_count);
    println!("  Checkpoints: {}", checkpoint_count);
    println!("  Compacts: {}", compact_count);
    
    // Actor breakdown
    if args.detailed {
        use std::collections::HashMap;
        let mut actor_counts: HashMap<String, usize> = HashMap::new();
        
        for entry in &entries {
            *actor_counts.entry(entry.actor.clone()).or_insert(0) += 1;
        }
        
        println!("\nActor Breakdown:");
        let mut actors: Vec<_> = actor_counts.iter().collect();
        actors.sort_by(|a, b| b.1.cmp(a.1));
        
        for (actor, count) in actors.iter().take(10) {
            println!("  {}: {} operations ({:.1}%)", 
                actor, count, (**count as f64 / entries.len() as f64) * 100.0);
        }
        
        if actors.len() > 10 {
            println!("  ... and {} more actors", actors.len() - 10);
        }
    }
    
    // Integrity check
    if args.check_integrity {
        println!("\nValidating integrity...");
        let validation = wal.validate().await?;
        
        println!("  Total segments: {}", validation.total_segments);
        println!("  Total entries: {}", validation.total_entries);
        println!("  Valid entries: {}", validation.valid_entries);
        println!("  Corrupted entries: {}", validation.corrupted_entries);
        println!("  Corrupted segments: {}", validation.corrupted_segments);
        println!("  Corruption rate: {:.2}%", validation.corruption_rate() * 100.0);
        
        if validation.is_healthy() {
            println!("\n✓ WAL is healthy!");
        } else {
            println!("\n✗ WAL has integrity issues!");
            return Err(anyhow::anyhow!("WAL validation failed"));
        }
    }
    
    Ok(())
}
