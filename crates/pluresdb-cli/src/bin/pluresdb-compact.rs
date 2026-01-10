//! WAL compaction command-line tool.

use anyhow::Result;
use clap::Parser;
use pluresdb_storage::WriteAheadLog;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "pluresdb-compact")]
#[command(about = "Compact PluresDB WAL by removing old entries")]
struct Args {
    /// Path to data directory containing WAL
    #[arg(short, long)]
    data_dir: PathBuf,
    
    /// Compact entries before this sequence number
    #[arg(short, long)]
    before_seq: Option<u64>,
    
    /// Compaction strategy (auto or aggressive)
    #[arg(short, long, default_value = "auto")]
    strategy: String,
    
    /// Dry run - show what would be compacted without doing it
    #[arg(long)]
    dry_run: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();
    
    let args = Args::parse();
    
    println!("PluresDB WAL Compaction Tool");
    println!("============================");
    println!("Data directory: {}", args.data_dir.display());
    println!("Strategy: {}", args.strategy);
    
    let wal = WriteAheadLog::open(&args.data_dir)?;
    
    // Read current state
    let entries = wal.read_all().await?;
    println!("\nCurrent state:");
    println!("  Total entries: {}", entries.len());
    
    if entries.is_empty() {
        println!("No entries to compact.");
        return Ok(());
    }
    
    let min_seq = entries.iter().map(|e| e.seq).min().unwrap_or(0);
    let max_seq = entries.iter().map(|e| e.seq).max().unwrap_or(0);
    println!("  Sequence range: {} - {}", min_seq, max_seq);
    
    // Determine checkpoint sequence
    let checkpoint_seq = if let Some(seq) = args.before_seq {
        seq
    } else {
        match args.strategy.as_str() {
            "aggressive" => {
                // Keep only last 1000 entries
                if entries.len() > 1000 {
                    entries[entries.len() - 1000].seq
                } else {
                    0
                }
            }
            _ => {
                // Auto: keep last 10000 entries or 50% of total, whichever is larger
                let keep = std::cmp::max(10000, entries.len() / 2);
                if entries.len() > keep {
                    entries[entries.len() - keep].seq
                } else {
                    0
                }
            }
        }
    };
    
    if checkpoint_seq == 0 {
        println!("\nNo compaction needed.");
        return Ok(());
    }
    
    let entries_to_remove = entries.iter().filter(|e| e.seq < checkpoint_seq).count();
    
    println!("\nCompaction plan:");
    println!("  Checkpoint sequence: {}", checkpoint_seq);
    println!("  Entries to remove: {}", entries_to_remove);
    println!("  Entries to keep: {}", entries.len() - entries_to_remove);
    
    if args.dry_run {
        println!("\n[DRY RUN] No changes made.");
    } else {
        println!("\nPerforming compaction...");
        wal.compact(checkpoint_seq).await?;
        println!("Compaction completed successfully.");
        
        // Verify
        let remaining = wal.read_all().await?;
        println!("\nPost-compaction state:");
        println!("  Remaining entries: {}", remaining.len());
    }
    
    Ok(())
}
