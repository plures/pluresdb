use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use pluresdb_core::{CrdtOperation, CrdtStore};
use pluresdb_storage::{MemoryStorage, StorageEngine, StoredNode};
use tokio::runtime::Runtime;
use tracing_subscriber::{fmt, EnvFilter};

#[derive(Parser, Debug)]
#[command(
    name = "pluresdb",
    version,
    about = "Native PluresDB command-line interface",
    propagate_version = true
)]
struct Cli {
    /// Path to a data directory (optional; in-memory if omitted)
    #[arg(long)]
    data_dir: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Insert or update a node using JSON payload from STDIN or --file
    Put {
        /// Node identifier; generated if omitted
        #[arg(long)]
        id: Option<String>,
        /// Actor identifier for CRDT merge
        #[arg(long, default_value = "cli-actor")]
        actor: String,
        /// File containing JSON payload; if omitted reads from STDIN
        #[arg(long)]
        file: Option<PathBuf>,
    },
    /// Retrieve a node by identifier
    Get { id: String },
    /// List all nodes currently stored
    List,
}

fn init_runtime() -> Runtime {
    Runtime::new().expect("failed to initialise Tokio runtime")
}

fn init_logging() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    fmt().with_env_filter(filter).init();
}

fn load_payload(file: Option<PathBuf>) -> Result<serde_json::Value> {
    let input = if let Some(path) = file {
        fs::read_to_string(path).context("failed to read payload file")?
    } else {
        use std::io::Read;
        let mut buffer = String::new();
        std::io::stdin()
            .read_to_string(&mut buffer)
            .context("failed to read payload from STDIN")?;
        buffer
    };
    Ok(serde_json::from_str(&input).context("invalid JSON payload")?)
}

fn main() -> Result<()> {
    init_logging();
    let cli = Cli::parse();

    let rt = init_runtime();
    rt.block_on(async move {
        // For now we always use the in-memory engine; future work will feed the
        // sled-backed implementation based on CLI flags.
        let storage = MemoryStorage::default();
        let store = CrdtStore::default();

        match cli.command {
            Commands::Put { id, actor, file } => {
                let payload = load_payload(file)?;
                let node_id = if let Some(id) = id {
                    id
                } else {
                    let (generated_id, _op) = store.operation_for(&actor, payload.clone());
                    generated_id
                };
                let op = CrdtOperation::Put {
                    id: node_id.clone(),
                    actor: actor.clone(),
                    data: payload.clone(),
                };
                store.apply(op)?;
                storage
                    .put(StoredNode {
                        id: node_id.clone(),
                        payload,
                    })
                    .await?;
                println!("{{\"id\":\"{}\"}}", node_id);
            }
            Commands::Get { id } => match storage.get(&id).await? {
                Some(node) => println!("{}", serde_json::to_string_pretty(&node.payload)?),
                None => {
                    eprintln!("node not found: {}", id);
                    std::process::exit(1);
                }
            },
            Commands::List => {
                let nodes = storage.list().await?;
                println!("{}", serde_json::to_string_pretty(&nodes)?);
            }
        }
        Ok(())
    })
}
