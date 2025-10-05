//! Version information commands

use anyhow::Result;
use clap::Args;
use tracing::info;

#[derive(Args)]
pub struct VersionCommand {
    /// Show detailed version information
    #[arg(short, long)]
    detailed: bool,
    
    /// Show as JSON
    #[arg(long)]
    json: bool,
}

pub async fn handle_version_command(cmd: VersionCommand) -> Result<()> {
    info!("ℹ️ Showing version information...");
    
    let version = env!("CARGO_PKG_VERSION");
    let name = env!("CARGO_PKG_NAME");
    let description = env!("CARGO_PKG_DESCRIPTION");
    let authors = env!("CARGO_PKG_AUTHORS");
    let repository = env!("CARGO_PKG_REPOSITORY");
    let license = env!("CARGO_PKG_LICENSE");
    
    if cmd.json {
        let version_info = serde_json::json!({
            "name": name,
            "version": version,
            "description": description,
            "authors": authors.split(':').collect::<Vec<_>>(),
            "repository": repository,
            "license": license,
            "rust_version": env!("RUSTC_SEMVER"),
            "build_date": env!("VERGEN_BUILD_DATE"),
            "git_commit": env!("VERGEN_GIT_SHA_SHORT"),
            "target": env!("TARGET"),
            "features": get_available_features()
        });
        println!("{}", serde_json::to_string_pretty(&version_info)?);
    } else {
        println!("🚀 {} v{}", name, version);
        println!("  {}", description);
        println!("  Authors: {}", authors);
        println!("  Repository: {}", repository);
        println!("  License: {}", license);
        
        if cmd.detailed {
            println!("\n📊 Detailed Information:");
            println!("  Rust Version: {}", env!("RUSTC_SEMVER"));
            println!("  Build Date: {}", env!("VERGEN_BUILD_DATE"));
            println!("  Git Commit: {}", env!("VERGEN_GIT_SHA_SHORT"));
            println!("  Target: {}", env!("TARGET"));
            
            println!("\n🔧 Available Features:");
            let features = get_available_features();
            for feature in features {
                println!("  - {}", feature);
            }
            
            println!("\n📦 Dependencies:");
            println!("  - rusty-gun-core: {}", env!("CARGO_PKG_VERSION"));
            println!("  - rusty-gun-storage: {}", env!("CARGO_PKG_VERSION"));
            println!("  - rusty-gun-network: {}", env!("CARGO_PKG_VERSION"));
            println!("  - rusty-gun-api: {}", env!("CARGO_PKG_VERSION"));
        }
    }
    
    Ok(())
}

fn get_available_features() -> Vec<&'static str> {
    vec![
        "vector-search",
        "hnsw-index",
        "embeddings",
        "sqlite-compatibility",
        "p2p-networking",
        "quic-protocol",
        "webrtc-support",
        "libp2p-support",
        "encryption",
        "websocket-api",
        "rest-api",
        "cli-interface",
        "graph-operations",
        "conflict-resolution",
        "offline-first",
        "real-time-sync",
        "performance-optimized",
        "memory-safe",
        "cross-platform",
        "production-ready"
    ]
}


