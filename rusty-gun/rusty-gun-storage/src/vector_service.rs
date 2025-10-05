//! Vector search service that combines embeddings and vector search

use crate::{
    error::{Result, StorageError},
    traits::VectorSearchEngine,
    vector::{HnswVectorEngine, InMemoryVectorEngine},
    embeddings::{EmbeddingGenerator, EmbeddingConfig, EmbeddingModel, EmbeddingResult, TextPreprocessor},
    VectorConfig,
};
use serde_json::Value;
use std::collections::HashMap;
use tracing::{debug, info, warn};

/// Vector search service that combines text embedding and vector search
pub struct VectorSearchService {
    vector_engine: Box<dyn VectorSearchEngine + Send + Sync>,
    embedding_generator: EmbeddingGenerator,
    text_to_vector: HashMap<String, String>, // Maps text hash to vector ID
    vector_to_text: HashMap<String, String>, // Maps vector ID to text hash
}

impl VectorSearchService {
    /// Create a new vector search service
    pub fn new(vector_config: VectorConfig, embedding_config: EmbeddingConfig) -> Self {
        let vector_engine: Box<dyn VectorSearchEngine + Send + Sync> = if vector_config.dimensions <= 1000 {
            Box::new(HnswVectorEngine::new(vector_config.clone()))
        } else {
            Box::new(InMemoryVectorEngine::new(vector_config.clone()))
        };

        Self {
            vector_engine,
            embedding_generator: EmbeddingGenerator::new(embedding_config),
            text_to_vector: HashMap::new(),
            vector_to_text: HashMap::new(),
        }
    }

    /// Initialize the service
    pub async fn initialize(&mut self) -> Result<()> {
        self.vector_engine.initialize().await?;
        info!("Vector search service initialized");
        Ok(())
    }

    /// Add text content and generate embedding
    pub async fn add_text(&mut self, id: &str, text: &str, metadata: &Value) -> Result<()> {
        // Preprocess text
        let processed_text = TextPreprocessor::preprocess_text(text);
        
        // Generate embedding
        let embedding_result = self.embedding_generator.generate_embedding(&processed_text).await?;
        
        // Add to vector engine
        self.vector_engine.add_vector(id, &embedding_result.vector, metadata).await?;
        
        // Track text-to-vector mapping
        let text_hash = self.hash_text(&processed_text);
        self.text_to_vector.insert(text_hash.clone(), id.to_string());
        self.vector_to_text.insert(id.to_string(), text_hash);
        
        debug!("Added text content: {} (vector dimensions: {})", id, embedding_result.dimensions);
        Ok(())
    }

    /// Add multiple text contents in batch
    pub async fn add_texts_batch(&mut self, items: Vec<(String, String, Value)>) -> Result<()> {
        let texts: Vec<String> = items.iter()
            .map(|(_, text, _)| TextPreprocessor::preprocess_text(text))
            .collect();
        
        // Generate embeddings in batch
        let embedding_results = self.embedding_generator.generate_embeddings_batch(&texts).await?;
        
        // Add to vector engine
        for (i, (id, _, metadata)) in items.iter().enumerate() {
            let embedding_result = &embedding_results[i];
            self.vector_engine.add_vector(id, &embedding_result.vector, metadata).await?;
            
            // Track mappings
            let text_hash = self.hash_text(&texts[i]);
            self.text_to_vector.insert(text_hash.clone(), id.clone());
            self.vector_to_text.insert(id.clone(), text_hash);
        }
        
        info!("Added {} text contents in batch", items.len());
        Ok(())
    }

    /// Search for similar text content
    pub async fn search_text(&mut self, query: &str, limit: usize) -> Result<Vec<TextSearchResult>> {
        // Preprocess query
        let processed_query = TextPreprocessor::preprocess_text(query);
        
        // Generate query embedding
        let query_embedding = self.embedding_generator.generate_embedding(&processed_query).await?;
        
        // Search vectors
        let vector_results = self.vector_engine.search_vectors(&query_embedding.vector, limit).await?;
        
        // Convert to text search results
        let mut text_results = Vec::new();
        for result in vector_results {
            if let Some(text_hash) = self.vector_to_text.get(&result.id) {
                text_results.push(TextSearchResult {
                    id: result.id,
                    score: result.score,
                    metadata: result.metadata,
                    text_hash: text_hash.clone(),
                });
            }
        }
        
        debug!("Text search completed: {} results for query: {}", text_results.len(), &query[..50.min(query.len())]);
        Ok(text_results)
    }

    /// Search for similar content by vector
    pub async fn search_by_vector(&self, query_vector: &[f32], limit: usize) -> Result<Vec<VectorSearchResult>> {
        self.vector_engine.search_vectors(query_vector, limit).await
    }

    /// Update text content
    pub async fn update_text(&mut self, id: &str, text: &str, metadata: &Value) -> Result<()> {
        // Remove old mapping
        if let Some(text_hash) = self.vector_to_text.remove(id) {
            self.text_to_vector.remove(&text_hash);
        }
        
        // Add updated content
        self.add_text(id, text, metadata).await?;
        
        debug!("Updated text content: {}", id);
        Ok(())
    }

    /// Remove text content
    pub async fn remove_text(&mut self, id: &str) -> Result<()> {
        // Remove from vector engine
        self.vector_engine.remove_vector(id).await?;
        
        // Remove mappings
        if let Some(text_hash) = self.vector_to_text.remove(id) {
            self.text_to_vector.remove(&text_hash);
        }
        
        debug!("Removed text content: {}", id);
        Ok(())
    }

    /// Get text content by ID
    pub async fn get_text(&self, id: &str) -> Result<Option<(Vec<f32>, Value)>> {
        self.vector_engine.get_vector(id).await
    }

    /// Get statistics
    pub async fn get_stats(&self) -> Result<VectorServiceStats> {
        let vector_stats = self.vector_engine.get_stats().await?;
        let cache_stats = self.embedding_generator.get_cache_stats();
        
        Ok(VectorServiceStats {
            vector_count: vector_stats.vector_count,
            dimensions: vector_stats.dimensions,
            index_size: vector_stats.index_size,
            last_updated: vector_stats.last_updated,
            cache_size: cache_stats.cache_size,
            cache_hits: cache_stats.cache_hits,
            cache_misses: cache_stats.cache_misses,
        })
    }

    /// Clear all data
    pub async fn clear_all(&mut self) -> Result<()> {
        // Clear vector engine (would need to be implemented)
        self.text_to_vector.clear();
        self.vector_to_text.clear();
        self.embedding_generator.clear_cache();
        
        info!("Vector search service cleared");
        Ok(())
    }

    /// Generate embedding for text without storing
    pub async fn generate_embedding(&mut self, text: &str) -> Result<EmbeddingResult> {
        let processed_text = TextPreprocessor::preprocess_text(text);
        self.embedding_generator.generate_embedding(&processed_text).await
    }

    /// Get embedding model information
    pub fn get_model_info(&self) -> ModelInfo {
        ModelInfo {
            model_name: self.embedding_generator.config.model.name().to_string(),
            dimensions: self.embedding_generator.config.model.dimensions(),
            max_text_length: self.embedding_generator.config.max_text_length,
            batch_size: self.embedding_generator.config.batch_size,
            cache_enabled: self.embedding_generator.config.enable_cache,
        }
    }

    /// Hash text for mapping
    fn hash_text(&self, text: &str) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        text.hash(&mut hasher);
        format!("{:x}", hasher.finish())
    }
}

/// Text search result
#[derive(Debug, Clone)]
pub struct TextSearchResult {
    pub id: String,
    pub score: f32,
    pub metadata: Value,
    pub text_hash: String,
}

/// Vector search result (re-exported from traits)
pub use crate::traits::VectorSearchResult;

/// Vector service statistics
#[derive(Debug, Clone)]
pub struct VectorServiceStats {
    pub vector_count: u64,
    pub dimensions: usize,
    pub index_size: u64,
    pub last_updated: chrono::DateTime<chrono::Utc>,
    pub cache_size: usize,
    pub cache_hits: u64,
    pub cache_misses: u64,
}

/// Model information
#[derive(Debug, Clone)]
pub struct ModelInfo {
    pub model_name: String,
    pub dimensions: usize,
    pub max_text_length: usize,
    pub batch_size: usize,
    pub cache_enabled: bool,
}

/// Semantic search query builder
pub struct SemanticSearchQuery {
    query: String,
    filters: Vec<SearchFilter>,
    limit: usize,
    threshold: f32,
}

/// Search filter
#[derive(Debug, Clone)]
pub struct SearchFilter {
    pub field: String,
    pub operator: FilterOperator,
    pub value: Value,
}

/// Filter operator
#[derive(Debug, Clone)]
pub enum FilterOperator {
    Equals,
    NotEquals,
    Contains,
    GreaterThan,
    LessThan,
    In,
    NotIn,
}

impl SemanticSearchQuery {
    /// Create a new semantic search query
    pub fn new(query: &str) -> Self {
        Self {
            query: query.to_string(),
            filters: Vec::new(),
            limit: 10,
            threshold: 0.0,
        }
    }

    /// Add a filter
    pub fn with_filter(mut self, field: &str, operator: FilterOperator, value: Value) -> Self {
        self.filters.push(SearchFilter {
            field: field.to_string(),
            operator,
            value,
        });
        self
    }

    /// Set result limit
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }

    /// Set similarity threshold
    pub fn with_threshold(mut self, threshold: f32) -> Self {
        self.threshold = threshold;
        self
    }

    /// Execute the search
    pub async fn execute(&mut self, service: &mut VectorSearchService) -> Result<Vec<TextSearchResult>> {
        let mut results = service.search_text(&self.query, self.limit).await?;
        
        // Apply filters
        if !self.filters.is_empty() {
            results.retain(|result| {
                self.filters.iter().all(|filter| {
                    self.apply_filter(&result.metadata, filter)
                })
            });
        }
        
        // Apply threshold
        results.retain(|result| result.score >= self.threshold);
        
        Ok(results)
    }

    /// Apply a filter to metadata
    fn apply_filter(&self, metadata: &Value, filter: &SearchFilter) -> bool {
        if let Some(Value::Object(obj)) = metadata.as_object() {
            if let Some(field_value) = obj.get(&filter.field) {
                match &filter.operator {
                    FilterOperator::Equals => field_value == &filter.value,
                    FilterOperator::NotEquals => field_value != &filter.value,
                    FilterOperator::Contains => {
                        if let (Some(field_str), Some(filter_str)) = (field_value.as_str(), filter.value.as_str()) {
                            field_str.contains(filter_str)
                        } else {
                            false
                        }
                    }
                    FilterOperator::GreaterThan => {
                        if let (Some(field_num), Some(filter_num)) = (field_value.as_f64(), filter.value.as_f64()) {
                            field_num > filter_num
                        } else {
                            false
                        }
                    }
                    FilterOperator::LessThan => {
                        if let (Some(field_num), Some(filter_num)) = (field_value.as_f64(), filter.value.as_f64()) {
                            field_num < filter_num
                        } else {
                            false
                        }
                    }
                    FilterOperator::In => {
                        if let Some(filter_array) = filter.value.as_array() {
                            filter_array.contains(field_value)
                        } else {
                            false
                        }
                    }
                    FilterOperator::NotIn => {
                        if let Some(filter_array) = filter.value.as_array() {
                            !filter_array.contains(field_value)
                        } else {
                            true
                        }
                    }
                }
            } else {
                false
            }
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_vector_search_service() {
        let vector_config = VectorConfig::default();
        let embedding_config = EmbeddingConfig::default();
        let mut service = VectorSearchService::new(vector_config, embedding_config);
        service.initialize().await.unwrap();

        // Add test content
        let metadata = serde_json::json!({"type": "document", "title": "Test Document"});
        service.add_text("doc1", "This is a test document about machine learning", &metadata).await.unwrap();
        service.add_text("doc2", "Another document about artificial intelligence", &metadata).await.unwrap();

        // Search for similar content
        let results = service.search_text("machine learning algorithms", 5).await.unwrap();
        assert!(!results.is_empty());
        assert!(results[0].score > 0.0);

        // Test statistics
        let stats = service.get_stats().await.unwrap();
        assert_eq!(stats.vector_count, 2);
    }

    #[tokio::test]
    async fn test_semantic_search_query() {
        let vector_config = VectorConfig::default();
        let embedding_config = EmbeddingConfig::default();
        let mut service = VectorSearchService::new(vector_config, embedding_config);
        service.initialize().await.unwrap();

        // Add test content
        let metadata1 = serde_json::json!({"type": "document", "category": "tech"});
        let metadata2 = serde_json::json!({"type": "document", "category": "science"});
        
        service.add_text("doc1", "Machine learning algorithms", &metadata1).await.unwrap();
        service.add_text("doc2", "Scientific research methods", &metadata2).await.unwrap();

        // Search with filter
        let query = SemanticSearchQuery::new("learning")
            .with_filter("category", FilterOperator::Equals, serde_json::json!("tech"))
            .with_limit(5);
        
        let results = query.execute(&mut service).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "doc1");
    }

    #[tokio::test]
    async fn test_batch_operations() {
        let vector_config = VectorConfig::default();
        let embedding_config = EmbeddingConfig::default();
        let mut service = VectorSearchService::new(vector_config, embedding_config);
        service.initialize().await.unwrap();

        // Add multiple texts in batch
        let items = vec![
            ("doc1".to_string(), "First document".to_string(), serde_json::json!({"id": 1})),
            ("doc2".to_string(), "Second document".to_string(), serde_json::json!({"id": 2})),
            ("doc3".to_string(), "Third document".to_string(), serde_json::json!({"id": 3})),
        ];
        
        service.add_texts_batch(items).await.unwrap();

        // Verify all were added
        let stats = service.get_stats().await.unwrap();
        assert_eq!(stats.vector_count, 3);
    }
}


