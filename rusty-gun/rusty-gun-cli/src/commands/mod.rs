//! CLI command modules

pub mod server;
pub mod node;
pub mod graph;
pub mod vector;
pub mod sql;
pub mod network;
pub mod config;
pub mod version;

// Re-export command handlers
pub use server::handle_server_command;
pub use node::handle_node_command;
pub use graph::handle_graph_command;
pub use vector::handle_vector_command;
pub use sql::handle_sql_command;
pub use network::handle_network_command;
pub use config::handle_config_command;
pub use version::handle_version_command;


