//! # Rusty Gun API
//! 
//! HTTP/WebSocket API server for Rusty Gun with vector search capabilities.

pub mod vector_api;
pub mod server;
pub mod websocket;

// Re-export main API components
pub use vector_api::{
    create_vector_router, create_vector_api_state, initialize_vector_api_state,
    VectorApiState, TextSearchRequest, AddTextRequest, AddTextsBatchRequest,
    UpdateTextRequest, VectorSearchRequest, GenerateEmbeddingRequest,
    ApiResponse, TextSearchResponse, VectorSearchResponse, EmbeddingResponse,
    StatsResponse, SearchFilterRequest,
};

pub use server::{
    create_api_router, create_middleware_stack, create_api_state, start_server,
    ApiState, ServerConfig, HealthResponse,
};

pub use websocket::{
    WebSocketManager, WebSocketMessage, WebSocketConnection,
    websocket_handler, websocket_channel_handler,
    broadcast_node_update, broadcast_relationship_update,
    broadcast_vector_search_result, broadcast_graph_change,
};
