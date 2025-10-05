//! # Rusty Gun CLI
//! 
//! Command-line interface for Rusty Gun with comprehensive management capabilities.

pub mod commands;

// Re-export command handlers
pub use commands::{
    handle_server_command, handle_node_command, handle_graph_command,
    handle_vector_command, handle_sql_command, handle_network_command,
    handle_config_command, handle_version_command,
};


