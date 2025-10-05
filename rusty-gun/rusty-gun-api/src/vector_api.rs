//! Vector search API endpoints for Rusty Gun

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post, delete, put},
    Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, info, warn};

use rusty_gun_storage::{
    VectorSearchService, SemanticSearchQuery, TextSearchResult, VectorServiceStats, 
    ModelInfo, SearchFilter, FilterOperator, EmbeddingConfig, EmbeddingModel, VectorConfig
};

/// Vector search API state
#[derive(Clone)]
pub struct VectorApiState {
    pub vector_service: std::sync::Arc<tokio::sync::Mutex<VectorSearchService>>,
}

/// Text search request
#[derive(Debug, Deserialize)]
pub struct TextSearchRequest {
    pub query: String,
    pub limit: Option<usize>,
    pub threshold: Option<f32>,
    pub filters: Option<Vec<SearchFilterRequest>>,
}

/// Search filter request
#[derive(Debug, Deserialize)]
pub struct SearchFilterRequest {
    pub field: String,
    pub operator: String,
    pub value: serde_json::Value,
}

/// Add text request
#[derive(Debug, Deserialize)]
pub struct AddTextRequest {
    pub id: String,
    pub text: String,
    pub metadata: Option<serde_json::Value>,
}

/// Add texts batch request
#[derive(Debug, Deserialize)]
pub struct AddTextsBatchRequest {
    pub items: Vec<AddTextRequest>,
}

/// Update text request
#[derive(Debug, Deserialize)]
pub struct UpdateTextRequest {
    pub text: String,
    pub metadata: Option<serde_json::Value>,
}

/// Vector search request
#[derive(Debug, Deserialize)]
pub struct VectorSearchRequest {
    pub vector: Vec<f32>,
    pub limit: Option<usize>,
}

/// Generate embedding request
#[derive(Debug, Deserialize)]
pub struct GenerateEmbeddingRequest {
    pub text: String,
}

/// API response wrapper
#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn error(message: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message),
        }
    }
}

/// Text search result response
#[derive(Debug, Serialize)]
pub struct TextSearchResponse {
    pub results: Vec<TextSearchResult>,
    pub total: usize,
    pub query: String,
    pub model_info: ModelInfo,
}

/// Vector search result response
#[derive(Debug, Serialize)]
pub struct VectorSearchResponse {
    pub results: Vec<rusty_gun_storage::VectorSearchResult>,
    pub total: usize,
}

/// Embedding response
#[derive(Debug, Serialize)]
pub struct EmbeddingResponse {
    pub vector: Vec<f32>,
    pub text: String,
    pub dimensions: usize,
    pub model: String,
}

/// Statistics response
#[derive(Debug, Serialize)]
pub struct StatsResponse {
    pub stats: VectorServiceStats,
    pub model_info: ModelInfo,
}

/// Create vector search API router
pub fn create_vector_router() -> Router<VectorApiState> {
    Router::new()
        .route("/search/text", post(search_text))
        .route("/search/vector", post(search_vector))
        .route("/text", post(add_text))
        .route("/text/batch", post(add_texts_batch))
        .route("/text/:id", get(get_text))
        .route("/text/:id", put(update_text))
        .route("/text/:id", delete(remove_text))
        .route("/embedding", post(generate_embedding))
        .route("/stats", get(get_stats))
        .route("/model", get(get_model_info))
        .route("/clear", post(clear_all))
}

/// Search for similar text content
async fn search_text(
    State(state): State<VectorApiState>,
    Json(request): Json<TextSearchRequest>,
) -> Result<Json<ApiResponse<TextSearchResponse>>, StatusCode> {
    let mut service = state.vector_service.lock().await;
    
    let mut query = SemanticSearchQuery::new(&request.query)
        .with_limit(request.limit.unwrap_or(10))
        .with_threshold(request.threshold.unwrap_or(0.0));

    // Add filters
    if let Some(filters) = request.filters {
        for filter_req in filters {
            let operator = match filter_req.operator.as_str() {
                "equals" => FilterOperator::Equals,
                "not_equals" => FilterOperator::NotEquals,
                "contains" => FilterOperator::Contains,
                "greater_than" => FilterOperator::GreaterThan,
                "less_than" => FilterOperator::LessThan,
                "in" => FilterOperator::In,
                "not_in" => FilterOperator::NotIn,
                _ => {
                    return Ok(Json(ApiResponse::error(format!(
                        "Invalid filter operator: {}", filter_req.operator
                    ))));
                }
            };
            
            query = query.with_filter(&filter_req.field, operator, filter_req.value);
        }
    }

    match query.execute(&mut service).await {
        Ok(results) => {
            let model_info = service.get_model_info();
            let response = TextSearchResponse {
                results,
                total: results.len(),
                query: request.query,
                model_info,
            };
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            warn!("Text search failed: {}", e);
            Ok(Json(ApiResponse::error(format!("Search failed: {}", e))))
        }
    }
}

/// Search for similar vectors
async fn search_vector(
    State(state): State<VectorApiState>,
    Json(request): Json<VectorSearchRequest>,
) -> Result<Json<ApiResponse<VectorSearchResponse>>, StatusCode> {
    let service = state.vector_service.lock().await;
    
    match service.search_by_vector(&request.vector, request.limit.unwrap_or(10)).await {
        Ok(results) => {
            let response = VectorSearchResponse {
                results,
                total: results.len(),
            };
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            warn!("Vector search failed: {}", e);
            Ok(Json(ApiResponse::error(format!("Search failed: {}", e))))
        }
    }
}

/// Add text content
async fn add_text(
    State(state): State<VectorApiState>,
    Json(request): Json<AddTextRequest>,
) -> Result<Json<ApiResponse<String>>, StatusCode> {
    let mut service = state.vector_service.lock().await;
    
    let metadata = request.metadata.unwrap_or(serde_json::json!({}));
    
    match service.add_text(&request.id, &request.text, &metadata).await {
        Ok(_) => {
            info!("Added text content: {}", request.id);
            Ok(Json(ApiResponse::success("Text added successfully".to_string())))
        }
        Err(e) => {
            warn!("Failed to add text: {}", e);
            Ok(Json(ApiResponse::error(format!("Failed to add text: {}", e))))
        }
    }
}

/// Add multiple text contents in batch
async fn add_texts_batch(
    State(state): State<VectorApiState>,
    Json(request): Json<AddTextsBatchRequest>,
) -> Result<Json<ApiResponse<String>>, StatusCode> {
    let mut service = state.vector_service.lock().await;
    
    let items: Vec<(String, String, serde_json::Value)> = request.items
        .into_iter()
        .map(|item| (item.id, item.text, item.metadata.unwrap_or(serde_json::json!({}))))
        .collect();
    
    match service.add_texts_batch(items).await {
        Ok(_) => {
            info!("Added {} text contents in batch", request.items.len());
            Ok(Json(ApiResponse::success("Texts added successfully".to_string())))
        }
        Err(e) => {
            warn!("Failed to add texts batch: {}", e);
            Ok(Json(ApiResponse::error(format!("Failed to add texts: {}", e))))
        }
    }
}

/// Get text content by ID
async fn get_text(
    State(state): State<VectorApiState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<serde_json::Value>>, StatusCode> {
    let service = state.vector_service.lock().await;
    
    match service.get_text(&id).await {
        Ok(Some((vector, metadata))) => {
            let response = serde_json::json!({
                "id": id,
                "vector": vector,
                "metadata": metadata
            });
            Ok(Json(ApiResponse::success(response)))
        }
        Ok(None) => {
            Ok(Json(ApiResponse::error("Text not found".to_string())))
        }
        Err(e) => {
            warn!("Failed to get text: {}", e);
            Ok(Json(ApiResponse::error(format!("Failed to get text: {}", e))))
        }
    }
}

/// Update text content
async fn update_text(
    State(state): State<VectorApiState>,
    Path(id): Path<String>,
    Json(request): Json<UpdateTextRequest>,
) -> Result<Json<ApiResponse<String>>, StatusCode> {
    let mut service = state.vector_service.lock().await;
    
    let metadata = request.metadata.unwrap_or(serde_json::json!({}));
    
    match service.update_text(&id, &request.text, &metadata).await {
        Ok(_) => {
            info!("Updated text content: {}", id);
            Ok(Json(ApiResponse::success("Text updated successfully".to_string())))
        }
        Err(e) => {
            warn!("Failed to update text: {}", e);
            Ok(Json(ApiResponse::error(format!("Failed to update text: {}", e))))
        }
    }
}

/// Remove text content
async fn remove_text(
    State(state): State<VectorApiState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<String>>, StatusCode> {
    let mut service = state.vector_service.lock().await;
    
    match service.remove_text(&id).await {
        Ok(_) => {
            info!("Removed text content: {}", id);
            Ok(Json(ApiResponse::success("Text removed successfully".to_string())))
        }
        Err(e) => {
            warn!("Failed to remove text: {}", e);
            Ok(Json(ApiResponse::error(format!("Failed to remove text: {}", e))))
        }
    }
}

/// Generate embedding for text
async fn generate_embedding(
    State(state): State<VectorApiState>,
    Json(request): Json<GenerateEmbeddingRequest>,
) -> Result<Json<ApiResponse<EmbeddingResponse>>, StatusCode> {
    let mut service = state.vector_service.lock().await;
    
    match service.generate_embedding(&request.text).await {
        Ok(result) => {
            let response = EmbeddingResponse {
                vector: result.vector,
                text: result.text,
                dimensions: result.dimensions,
                model: result.model.name().to_string(),
            };
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            warn!("Failed to generate embedding: {}", e);
            Ok(Json(ApiResponse::error(format!("Failed to generate embedding: {}", e))))
        }
    }
}

/// Get vector search statistics
async fn get_stats(
    State(state): State<VectorApiState>,
) -> Result<Json<ApiResponse<StatsResponse>>, StatusCode> {
    let service = state.vector_service.lock().await;
    
    match service.get_stats().await {
        Ok(stats) => {
            let model_info = service.get_model_info();
            let response = StatsResponse { stats, model_info };
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            warn!("Failed to get stats: {}", e);
            Ok(Json(ApiResponse::error(format!("Failed to get stats: {}", e))))
        }
    }
}

/// Get model information
async fn get_model_info(
    State(state): State<VectorApiState>,
) -> Result<Json<ApiResponse<ModelInfo>>, StatusCode> {
    let service = state.vector_service.lock().await;
    let model_info = service.get_model_info();
    Ok(Json(ApiResponse::success(model_info)))
}

/// Clear all vector data
async fn clear_all(
    State(state): State<VectorApiState>,
) -> Result<Json<ApiResponse<String>>, StatusCode> {
    let mut service = state.vector_service.lock().await;
    
    match service.clear_all().await {
        Ok(_) => {
            info!("Cleared all vector data");
            Ok(Json(ApiResponse::success("All data cleared successfully".to_string())))
        }
        Err(e) => {
            warn!("Failed to clear data: {}", e);
            Ok(Json(ApiResponse::error(format!("Failed to clear data: {}", e))))
        }
    }
}

/// Create vector API state
pub fn create_vector_api_state(
    vector_config: VectorConfig,
    embedding_config: EmbeddingConfig,
) -> VectorApiState {
    let vector_service = VectorSearchService::new(vector_config, embedding_config);
    VectorApiState {
        vector_service: std::sync::Arc::new(tokio::sync::Mutex::new(vector_service)),
    }
}

/// Initialize vector API state
pub async fn initialize_vector_api_state(state: &VectorApiState) -> Result<(), Box<dyn std::error::Error>> {
    let mut service = state.vector_service.lock().await;
    service.initialize().await?;
    info!("Vector API state initialized");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
        Router,
    };
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_vector_api_routes() {
        let vector_config = VectorConfig::default();
        let embedding_config = EmbeddingConfig::default();
        let state = create_vector_api_state(vector_config, embedding_config);
        
        let app = Router::new()
            .merge(create_vector_router())
            .with_state(state);

        // Test that routes are properly configured
        let response = app
            .oneshot(Request::builder().uri("/stats").method("GET").body(Body::empty()).unwrap())
            .await
            .unwrap();

        // Should return 200 (even if service not initialized)
        assert_eq!(response.status(), StatusCode::OK);
    }
}


