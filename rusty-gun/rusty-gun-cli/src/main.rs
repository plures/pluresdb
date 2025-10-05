//! Rusty Gun CLI - Command-line interface for Rusty Gun

use anyhow::Result;
use clap::{Parser, Subcommand};
use rusty_gun_cli::commands::{
    server, node, graph, vector, sql, network, config, version,
};
use tracing::{info, error};

#[derive(Parser)]
#[command(
    name = "rusty-gun",
    about = "Rusty Gun - High-performance graph database with vector search",
    version = env!("CARGO_PKG_VERSION"),
    author = "Rusty Gun Team"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    
    /// Verbose output
    #[arg(short, long, global = true)]
    verbose: bool,
    
    /// Quiet output (minimal logging)
    #[arg(short, long, global = true)]
    quiet: bool,
    
    /// Configuration file path
    #[arg(short, long, global = true)]
    config: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the Rusty Gun server
    Server(server::ServerCommand),
    
    /// Node management operations
    Node(node::NodeCommand),
    
    /// Graph operations
    Graph(graph::GraphCommand),
    
    /// Vector search operations
    Vector(vector::VectorCommand),
    
    /// SQL query operations
    Sql(sql::SqlCommand),
    
    /// Network operations
    Network(network::NetworkCommand),
    
    /// Configuration management
    Config(config::ConfigCommand),
    
    /// Show version information
    Version(version::VersionCommand),
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    
    // Initialize logging
    init_logging(cli.verbose, cli.quiet)?;
    
    info!("🚀 Rusty Gun CLI v{}", env!("CARGO_PKG_VERSION"));
    
    // Execute command
    match cli.command {
        Commands::Server(cmd) => server::handle_server_command(cmd).await,
        Commands::Node(cmd) => node::handle_node_command(cmd).await,
        Commands::Graph(cmd) => graph::handle_graph_command(cmd).await,
        Commands::Vector(cmd) => vector::handle_vector_command(cmd).await,
        Commands::Sql(cmd) => sql::handle_sql_command(cmd).await,
        Commands::Network(cmd) => network::handle_network_command(cmd).await,
        Commands::Config(cmd) => config::handle_config_command(cmd).await,
        Commands::Version(cmd) => version::handle_version_command(cmd).await,
    }
}

/// Initialize logging based on verbosity flags
fn init_logging(verbose: bool, quiet: bool) -> Result<()> {
    let filter = if quiet {
        "warn"
    } else if verbose {
        "debug"
    } else {
        "info"
    };
    
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env()
            .add_directive(filter.parse()?))
        .init();
    
    Ok(())
}