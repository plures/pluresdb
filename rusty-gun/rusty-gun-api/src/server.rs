//! Main API server implementation for Rusty Gun

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{Html, Json},
    routing::{get, post, put, delete},
    Router,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;
use tower::ServiceBuilder;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use tracing::{info, warn, error};

use rusty_gun_core::{Node, NodeId, types::*};
use rusty_gun_storage::{StorageEngine, VectorSearchService, StorageConfig, VectorConfig, EmbeddingConfig};
use rusty_gun_network::{NetworkEngine, NetworkConfig};

use crate::vector_api::{create_vector_router, VectorApiState};

/// Main API server state
#[derive(Clone)]
pub struct ApiState {
    pub storage: Arc<Mutex<Box<dyn StorageEngine + Send + Sync>>>,
    pub vector_service: Arc<Mutex<VectorSearchService>>,
    pub network_engine: Arc<Mutex<Box<dyn NetworkEngine + Send + Sync>>>,
    pub config: ServerConfig,
}

/// Server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub enable_cors: bool,
    pub enable_tracing: bool,
    pub max_request_size: usize,
    pub request_timeout: u64,
    pub enable_metrics: bool,
    pub enable_health_check: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 34569,
            enable_cors: true,
            enable_tracing: true,
            max_request_size: 10 * 1024 * 1024, // 10MB
            request_timeout: 30,
            enable_metrics: true,
            enable_health_check: true,
        }
    }
}

/// Health check response
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub services: HashMap<String, String>,
}

/// API response wrapper
#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            timestamp: chrono::Utc::now(),
        }
    }

    pub fn error(message: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message),
            timestamp: chrono::Utc::now(),
        }
    }
}

/// Node request/response types
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateNodeRequest {
    pub id: Option<String>,
    pub data: serde_json::Value,
    pub metadata: Option<serde_json::Value>,
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateNodeRequest {
    pub data: Option<serde_json::Value>,
    pub metadata: Option<serde_json::Value>,
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NodeResponse {
    pub id: String,
    pub data: serde_json::Value,
    pub metadata: serde_json::Value,
    pub tags: Vec<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchRequest {
    pub query: String,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    pub filters: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchResponse {
    pub nodes: Vec<NodeResponse>,
    pub total: usize,
    pub query: String,
    pub took_ms: u64,
}

/// Create the main API router
pub fn create_api_router() -> Router<ApiState> {
    Router::new()
        // Health and status endpoints
        .route("/health", get(health_check))
        .route("/status", get(status))
        .route("/metrics", get(metrics))
        
        // Node management endpoints
        .route("/nodes", get(list_nodes))
        .route("/nodes", post(create_node))
        .route("/nodes/:id", get(get_node))
        .route("/nodes/:id", put(update_node))
        .route("/nodes/:id", delete(delete_node))
        .route("/nodes/search", post(search_nodes))
        .route("/nodes/:id/relationships", get(get_relationships))
        
        // Relationship management endpoints
        .route("/relationships", post(create_relationship))
        .route("/relationships/:from/:to/:type", delete(delete_relationship))
        
        // Graph operations
        .route("/graph/path/:from/:to", get(find_path))
        .route("/graph/stats", get(graph_stats))
        .route("/graph/export", get(export_graph))
        
        // SQL query endpoints
        .route("/sql/query", post(execute_sql))
        .route("/sql/explain", post(explain_sql))
        
        // Vector search endpoints (merged from vector_api)
        .merge(create_vector_router())
        
        // WebSocket endpoints
        .route("/ws", get(websocket_handler))
        .route("/ws/:channel", get(websocket_channel_handler))
        
        // Static file serving
        .route("/", get(index_handler))
        .route("/demo", get(demo_handler))
}

/// Create middleware stack
pub fn create_middleware_stack() -> ServiceBuilder<
    tower::layer::util::Stack<
        tower_http::trace::TraceLayer<tower_http::classify::SharedClassifier<tower_http::classify::ServerErrorsAsFailures>>,
        tower_http::cors::CorsLayer,
    >,
> {
    ServiceBuilder::new()
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any))
}

/// Health check endpoint
async fn health_check(State(state): State<ApiState>) -> Result<Json<ApiResponse<HealthResponse>>, StatusCode> {
    let mut services = HashMap::new();
    
    // Check storage
    match state.storage.lock().await.get_stats().await {
        Ok(_) => services.insert("storage".to_string(), "healthy".to_string()),
        Err(e) => services.insert("storage".to_string(), format!("unhealthy: {}", e)),
    };
    
    // Check vector service
    match state.vector_service.lock().await.get_stats().await {
        Ok(_) => services.insert("vector_search".to_string(), "healthy".to_string()),
        Err(e) => services.insert("vector_search".to_string(), format!("unhealthy: {}", e)),
    };
    
    // Check network
    services.insert("network".to_string(), "healthy".to_string()); // Simplified for now
    
    let response = HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        timestamp: chrono::Utc::now(),
        services,
    };
    
    Ok(Json(ApiResponse::success(response)))
}

/// Status endpoint
async fn status(State(state): State<ApiState>) -> Result<Json<ApiResponse<serde_json::Value>>, StatusCode> {
    let storage_stats = state.storage.lock().await.get_stats().await.unwrap_or_default();
    let vector_stats = state.vector_service.lock().await.get_stats().await.unwrap_or_default();
    
    let status = serde_json::json!({
        "server": {
            "version": env!("CARGO_PKG_VERSION"),
            "uptime": "0s", // Would need to track actual uptime
            "config": state.config
        },
        "storage": {
            "node_count": storage_stats.node_count,
            "relationship_count": storage_stats.relationship_count,
            "storage_size": storage_stats.storage_size,
            "index_count": storage_stats.index_count
        },
        "vector_search": {
            "vector_count": vector_stats.vector_count,
            "dimensions": vector_stats.dimensions,
            "index_size": vector_stats.index_size,
            "cache_size": vector_stats.cache_size
        }
    });
    
    Ok(Json(ApiResponse::success(status)))
}

/// Metrics endpoint
async fn metrics(State(_state): State<ApiState>) -> Result<String, StatusCode> {
    // In a real implementation, this would return Prometheus metrics
    let metrics = "# HELP rusty_gun_requests_total Total number of requests
# TYPE rusty_gun_requests_total counter
rusty_gun_requests_total{method=\"GET\",endpoint=\"/health\"} 1
rusty_gun_requests_total{method=\"GET\",endpoint=\"/status\"} 1

# HELP rusty_gun_response_time_seconds Response time in seconds
# TYPE rusty_gun_response_time_seconds histogram
rusty_gun_response_time_seconds_bucket{le=\"0.1\"} 1
rusty_gun_response_time_seconds_bucket{le=\"0.5\"} 1
rusty_gun_response_time_seconds_bucket{le=\"1.0\"} 1
rusty_gun_response_time_seconds_bucket{le=\"+Inf\"} 1
rusty_gun_response_time_seconds_sum 0.05
rusty_gun_response_time_seconds_count 1
";
    
    Ok(metrics)
}

/// List all nodes
async fn list_nodes(
    State(state): State<ApiState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<ApiResponse<Vec<NodeResponse>>>, StatusCode> {
    let limit = params.get("limit").and_then(|s| s.parse().ok()).unwrap_or(100);
    let offset = params.get("offset").and_then(|s| s.parse().ok()).unwrap_or(0);
    
    match state.storage.lock().await.list_node_ids().await {
        Ok(node_ids) => {
            let mut nodes = Vec::new();
            let storage = state.storage.lock().await;
            
            for node_id in node_ids.into_iter().skip(offset).take(limit) {
                if let Ok(Some(node)) = storage.load_node(&node_id).await {
                    nodes.push(NodeResponse {
                        id: node.id().to_string(),
                        data: node.data().clone(),
                        metadata: node.metadata().clone(),
                        tags: node.tags().clone(),
                        created_at: node.created_at(),
                        updated_at: node.updated_at(),
                    });
                }
            }
            
            Ok(Json(ApiResponse::success(nodes)))
        }
        Err(e) => {
            error!("Failed to list nodes: {}", e);
            Ok(Json(ApiResponse::error(format!("Failed to list nodes: {}", e))))
        }
    }
}

/// Create a new node
async fn create_node(
    State(state): State<ApiState>,
    Json(request): Json<CreateNodeRequest>,
) -> Result<Json<ApiResponse<NodeResponse>>, StatusCode> {
    let node_id = request.id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let metadata = request.metadata.unwrap_or(serde_json::json!({}));
    let tags = request.tags.unwrap_or_default();
    
    let node = Node::new(
        NodeId::from(node_id.clone()),
        request.data,
        metadata,
        tags,
    );
    
    match state.storage.lock().await.store_node(&node).await {
        Ok(_) => {
            let response = NodeResponse {
                id: node.id().to_string(),
                data: node.data().clone(),
                metadata: node.metadata().clone(),
                tags: node.tags().clone(),
                created_at: node.created_at(),
                updated_at: node.updated_at(),
            };
            
            info!("Created node: {}", node_id);
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("Failed to create node: {}", e);
            Ok(Json(ApiResponse::error(format!("Failed to create node: {}", e))))
        }
    }
}

/// Get a specific node
async fn get_node(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<NodeResponse>>, StatusCode> {
    let node_id = NodeId::from(id.clone());
    
    match state.storage.lock().await.load_node(&node_id).await {
        Ok(Some(node)) => {
            let response = NodeResponse {
                id: node.id().to_string(),
                data: node.data().clone(),
                metadata: node.metadata().clone(),
                tags: node.tags().clone(),
                created_at: node.created_at(),
                updated_at: node.updated_at(),
            };
            
            Ok(Json(ApiResponse::success(response)))
        }
        Ok(None) => {
            Ok(Json(ApiResponse::error("Node not found".to_string())))
        }
        Err(e) => {
            error!("Failed to get node {}: {}", id, e);
            Ok(Json(ApiResponse::error(format!("Failed to get node: {}", e))))
        }
    }
}

/// Update a node
async fn update_node(
    State(state): State<ApiState>,
    Path(id): Path<String>,
    Json(request): Json<UpdateNodeRequest>,
) -> Result<Json<ApiResponse<NodeResponse>>, StatusCode> {
    let node_id = NodeId::from(id.clone());
    
    match state.storage.lock().await.load_node(&node_id).await {
        Ok(Some(mut node)) => {
            if let Some(data) = request.data {
                node.set_data(data);
            }
            if let Some(metadata) = request.metadata {
                node.set_metadata(metadata);
            }
            if let Some(tags) = request.tags {
                node.set_tags(tags);
            }
            
            match state.storage.lock().await.store_node(&node).await {
                Ok(_) => {
                    let response = NodeResponse {
                        id: node.id().to_string(),
                        data: node.data().clone(),
                        metadata: node.metadata().clone(),
                        tags: node.tags().clone(),
                        created_at: node.created_at(),
                        updated_at: node.updated_at(),
                    };
                    
                    info!("Updated node: {}", id);
                    Ok(Json(ApiResponse::success(response)))
                }
                Err(e) => {
                    error!("Failed to update node {}: {}", id, e);
                    Ok(Json(ApiResponse::error(format!("Failed to update node: {}", e))))
                }
            }
        }
        Ok(None) => {
            Ok(Json(ApiResponse::error("Node not found".to_string())))
        }
        Err(e) => {
            error!("Failed to get node {} for update: {}", id, e);
            Ok(Json(ApiResponse::error(format!("Failed to get node: {}", e))))
        }
    }
}

/// Delete a node
async fn delete_node(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<String>>, StatusCode> {
    let node_id = NodeId::from(id.clone());
    
    match state.storage.lock().await.delete_node(&node_id).await {
        Ok(_) => {
            info!("Deleted node: {}", id);
            Ok(Json(ApiResponse::success("Node deleted successfully".to_string())))
        }
        Err(e) => {
            error!("Failed to delete node {}: {}", id, e);
            Ok(Json(ApiResponse::error(format!("Failed to delete node: {}", e))))
        }
    }
}

/// Search nodes
async fn search_nodes(
    State(state): State<ApiState>,
    Json(request): Json<SearchRequest>,
) -> Result<Json<ApiResponse<SearchResponse>>, StatusCode> {
    let start_time = std::time::Instant::now();
    
    match state.storage.lock().await.search_nodes(&request.query).await {
        Ok(nodes) => {
            let total = nodes.len();
            let limit = request.limit.unwrap_or(100);
            let offset = request.offset.unwrap_or(0);
            
            let paginated_nodes: Vec<NodeResponse> = nodes
                .into_iter()
                .skip(offset)
                .take(limit)
                .map(|node| NodeResponse {
                    id: node.id().to_string(),
                    data: node.data().clone(),
                    metadata: node.metadata().clone(),
                    tags: node.tags().clone(),
                    created_at: node.created_at(),
                    updated_at: node.updated_at(),
                })
                .collect();
            
            let took_ms = start_time.elapsed().as_millis() as u64;
            
            let response = SearchResponse {
                nodes: paginated_nodes,
                total,
                query: request.query,
                took_ms,
            };
            
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("Failed to search nodes: {}", e);
            Ok(Json(ApiResponse::error(format!("Failed to search nodes: {}", e))))
        }
    }
}

/// Get relationships for a node
async fn get_relationships(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<Vec<Relationship>>>, StatusCode> {
    let node_id = NodeId::from(id.clone());
    
    match state.storage.lock().await.load_relationships(&node_id).await {
        Ok(relationships) => {
            Ok(Json(ApiResponse::success(relationships)))
        }
        Err(e) => {
            error!("Failed to get relationships for node {}: {}", id, e);
            Ok(Json(ApiResponse::error(format!("Failed to get relationships: {}", e))))
        }
    }
}

/// Create a relationship
async fn create_relationship(
    State(state): State<ApiState>,
    Json(relationship): Json<Relationship>,
) -> Result<Json<ApiResponse<String>>, StatusCode> {
    match state.storage.lock().await.store_relationship(&relationship).await {
        Ok(_) => {
            info!("Created relationship: {} -> {} ({})", 
                relationship.from(), relationship.to(), relationship.relation_type());
            Ok(Json(ApiResponse::success("Relationship created successfully".to_string())))
        }
        Err(e) => {
            error!("Failed to create relationship: {}", e);
            Ok(Json(ApiResponse::error(format!("Failed to create relationship: {}", e))))
        }
    }
}

/// Delete a relationship
async fn delete_relationship(
    State(state): State<ApiState>,
    Path((from, to, relation_type)): Path<(String, String, String)>,
) -> Result<Json<ApiResponse<String>>, StatusCode> {
    let from_id = NodeId::from(from);
    let to_id = NodeId::from(to);
    
    match state.storage.lock().await.delete_relationship(&from_id, &to_id, &relation_type).await {
        Ok(_) => {
            info!("Deleted relationship: {} -> {} ({})", from, to, relation_type);
            Ok(Json(ApiResponse::success("Relationship deleted successfully".to_string())))
        }
        Err(e) => {
            error!("Failed to delete relationship: {}", e);
            Ok(Json(ApiResponse::error(format!("Failed to delete relationship: {}", e))))
        }
    }
}

/// Find path between nodes
async fn find_path(
    State(state): State<ApiState>,
    Path((from, to)): Path<(String, String)>,
) -> Result<Json<ApiResponse<Vec<String>>>, StatusCode> {
    let from_id = NodeId::from(from);
    let to_id = NodeId::from(to);
    
    // This would need to be implemented in the storage layer
    // For now, return a simple path
    let path = vec![from_id.to_string(), to_id.to_string()];
    
    Ok(Json(ApiResponse::success(path)))
}

/// Get graph statistics
async fn graph_stats(
    State(state): State<ApiState>,
) -> Result<Json<ApiResponse<serde_json::Value>>, StatusCode> {
    match state.storage.lock().await.get_stats().await {
        Ok(stats) => {
            let graph_stats = serde_json::json!({
                "node_count": stats.node_count,
                "relationship_count": stats.relationship_count,
                "storage_size": stats.storage_size,
                "index_count": stats.index_count,
                "last_updated": stats.last_updated
            });
            
            Ok(Json(ApiResponse::success(graph_stats)))
        }
        Err(e) => {
            error!("Failed to get graph stats: {}", e);
            Ok(Json(ApiResponse::error(format!("Failed to get graph stats: {}", e))))
        }
    }
}

/// Export graph data
async fn export_graph(
    State(state): State<ApiState>,
) -> Result<Json<ApiResponse<serde_json::Value>>, StatusCode> {
    // This would export the entire graph as JSON
    // For now, return a placeholder
    let export_data = serde_json::json!({
        "nodes": [],
        "relationships": [],
        "exported_at": chrono::Utc::now()
    });
    
    Ok(Json(ApiResponse::success(export_data)))
}

/// Execute SQL query
async fn execute_sql(
    State(state): State<ApiState>,
    Json(request): Json<serde_json::Value>,
) -> Result<Json<ApiResponse<serde_json::Value>>, StatusCode> {
    let query = request.get("query")
        .and_then(|v| v.as_str())
        .ok_or_else(|| StatusCode::BAD_REQUEST)?;
    
    let params = request.get("params")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().map(|v| v.clone()).collect::<Vec<_>>())
        .unwrap_or_default();
    
    match state.storage.lock().await.execute_query(query, &params).await {
        Ok(result) => {
            let response = serde_json::json!({
                "rows": result.rows,
                "columns": result.columns,
                "changes": result.changes,
                "last_insert_row_id": result.last_insert_row_id
            });
            
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("Failed to execute SQL query: {}", e);
            Ok(Json(ApiResponse::error(format!("Failed to execute SQL query: {}", e))))
        }
    }
}

/// Explain SQL query
async fn explain_sql(
    State(state): State<ApiState>,
    Json(request): Json<serde_json::Value>,
) -> Result<Json<ApiResponse<serde_json::Value>>, StatusCode> {
    let query = request.get("query")
        .and_then(|v| v.as_str())
        .ok_or_else(|| StatusCode::BAD_REQUEST)?;
    
    // This would explain the query execution plan
    let explanation = serde_json::json!({
        "query": query,
        "explanation": "Query explanation would be implemented here",
        "estimated_cost": 1.0,
        "estimated_rows": 100
    });
    
    Ok(Json(ApiResponse::success(explanation)))
}

/// WebSocket handler
async fn websocket_handler(
    State(_state): State<ApiState>,
    ws: axum::extract::ws::WebSocketUpgrade,
) -> axum::response::Response {
    ws.on_upgrade(handle_websocket)
}

/// WebSocket channel handler
async fn websocket_channel_handler(
    State(_state): State<ApiState>,
    Path(channel): Path<String>,
    ws: axum::extract::ws::WebSocketUpgrade,
) -> axum::response::Response {
    ws.on_upgrade(move |socket| handle_websocket_channel(socket, channel))
}

/// Handle WebSocket connection
async fn handle_websocket(socket: axum::extract::ws::WebSocket) {
    // WebSocket handling would be implemented here
    info!("WebSocket connection established");
}

/// Handle WebSocket channel connection
async fn handle_websocket_channel(socket: axum::extract::ws::WebSocket, channel: String) {
    // WebSocket channel handling would be implemented here
    info!("WebSocket connection established for channel: {}", channel);
}

/// Index page handler
async fn index_handler() -> Html<&'static str> {
    Html(include_str!("../static/index.html"))
}

/// Demo page handler
async fn demo_handler() -> Html<&'static str> {
    Html(include_str!("../static/demo.html"))
}

/// Create API state
pub fn create_api_state(
    storage: Box<dyn StorageEngine + Send + Sync>,
    vector_service: VectorSearchService,
    network_engine: Box<dyn NetworkEngine + Send + Sync>,
    config: ServerConfig,
) -> ApiState {
    ApiState {
        storage: Arc::new(Mutex::new(storage)),
        vector_service: Arc::new(Mutex::new(vector_service)),
        network_engine: Arc::new(Mutex::new(network_engine)),
        config,
    }
}

/// Start the API server
pub async fn start_server(state: ApiState) -> Result<(), Box<dyn std::error::Error>> {
    let app = create_api_router()
        .layer(create_middleware_stack())
        .with_state(state.clone());
    
    let listener = tokio::net::TcpListener::bind(format!("{}:{}", state.config.host, state.config.port)).await?;
    
    info!("🚀 Rusty Gun API server starting on {}:{}", state.config.host, state.config.port);
    info!("📊 Health check: http://{}:{}/health", state.config.host, state.config.port);
    info!("📈 Metrics: http://{}:{}/metrics", state.config.host, state.config.port);
    info!("🔍 Vector search: http://{}:{}/api/vector", state.config.host, state.config.port);
    
    axum::serve(listener, app).await?;
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_api_response() {
        let success_response = ApiResponse::success("test data");
        assert!(success_response.success);
        assert_eq!(success_response.data, Some("test data"));
        assert!(success_response.error.is_none());

        let error_response = ApiResponse::<String>::error("test error".to_string());
        assert!(!error_response.success);
        assert!(error_response.data.is_none());
        assert_eq!(error_response.error, Some("test error".to_string()));
    }

    #[test]
    fn test_server_config_default() {
        let config = ServerConfig::default();
        assert_eq!(config.host, "0.0.0.0");
        assert_eq!(config.port, 34569);
        assert!(config.enable_cors);
        assert!(config.enable_tracing);
    }
}


