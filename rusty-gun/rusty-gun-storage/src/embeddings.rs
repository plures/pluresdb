//! Text embedding generation for Rusty Gun

use crate::error::{Result, StorageError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, info, warn};

/// Embedding model types
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EmbeddingModel {
    /// OpenAI text-embedding-ada-002 (1536 dimensions)
    OpenAITextEmbeddingAda002,
    /// OpenAI text-embedding-3-small (1536 dimensions)
    OpenAITextEmbedding3Small,
    /// OpenAI text-embedding-3-large (3072 dimensions)
    OpenAITextEmbedding3Large,
    /// Sentence Transformers all-MiniLM-L6-v2 (384 dimensions)
    SentenceTransformersMiniLM,
    /// Sentence Transformers all-mpnet-base-v2 (768 dimensions)
    SentenceTransformersMPNet,
    /// Custom model
    Custom {
        name: String,
        dimensions: usize,
    },
}

impl EmbeddingModel {
    /// Get the default dimensions for this model
    pub fn dimensions(&self) -> usize {
        match self {
            EmbeddingModel::OpenAITextEmbeddingAda002 => 1536,
            EmbeddingModel::OpenAITextEmbedding3Small => 1536,
            EmbeddingModel::OpenAITextEmbedding3Large => 3072,
            EmbeddingModel::SentenceTransformersMiniLM => 384,
            EmbeddingModel::SentenceTransformersMPNet => 768,
            EmbeddingModel::Custom { dimensions, .. } => *dimensions,
        }
    }

    /// Get the model name
    pub fn name(&self) -> &str {
        match self {
            EmbeddingModel::OpenAITextEmbeddingAda002 => "text-embedding-ada-002",
            EmbeddingModel::OpenAITextEmbedding3Small => "text-embedding-3-small",
            EmbeddingModel::OpenAITextEmbedding3Large => "text-embedding-3-large",
            EmbeddingModel::SentenceTransformersMiniLM => "all-MiniLM-L6-v2",
            EmbeddingModel::SentenceTransformersMPNet => "all-mpnet-base-v2",
            EmbeddingModel::Custom { name, .. } => name,
        }
    }
}

/// Embedding configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingConfig {
    /// Model to use for embeddings
    pub model: EmbeddingModel,
    /// API key for external services (OpenAI, etc.)
    pub api_key: Option<String>,
    /// Base URL for API requests
    pub base_url: Option<String>,
    /// Maximum text length to embed
    pub max_text_length: usize,
    /// Batch size for embedding requests
    pub batch_size: usize,
    /// Cache embeddings locally
    pub enable_cache: bool,
    /// Cache directory
    pub cache_dir: Option<String>,
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            model: EmbeddingModel::SentenceTransformersMiniLM,
            api_key: None,
            base_url: None,
            max_text_length: 8192,
            batch_size: 100,
            enable_cache: true,
            cache_dir: Some("./cache/embeddings".to_string()),
        }
    }
}

/// Embedding result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingResult {
    /// The embedding vector
    pub vector: Vec<f32>,
    /// The original text
    pub text: String,
    /// Model used for embedding
    pub model: EmbeddingModel,
    /// Dimensions of the vector
    pub dimensions: usize,
    /// Timestamp when embedding was created
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Embedding cache entry
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry {
    vector: Vec<f32>,
    created_at: chrono::DateTime<chrono::Utc>,
    model: EmbeddingModel,
}

/// Text embedding generator
pub struct EmbeddingGenerator {
    config: EmbeddingConfig,
    cache: HashMap<String, CacheEntry>,
    client: Option<reqwest::Client>,
}

impl EmbeddingGenerator {
    /// Create a new embedding generator
    pub fn new(config: EmbeddingConfig) -> Self {
        let client = if matches!(
            config.model,
            EmbeddingModel::OpenAITextEmbeddingAda002
                | EmbeddingModel::OpenAITextEmbedding3Small
                | EmbeddingModel::OpenAITextEmbedding3Large
        ) {
            Some(reqwest::Client::new())
        } else {
            None
        };

        Self {
            config,
            cache: HashMap::new(),
            client,
        }
    }

    /// Generate embedding for text
    pub async fn generate_embedding(&mut self, text: &str) -> Result<EmbeddingResult> {
        // Check cache first
        if self.config.enable_cache {
            let cache_key = self.get_cache_key(text);
            if let Some(entry) = self.cache.get(&cache_key) {
                if entry.model == self.config.model {
                    debug!("Using cached embedding for text: {}", &text[..50.min(text.len())]);
                    return Ok(EmbeddingResult {
                        vector: entry.vector.clone(),
                        text: text.to_string(),
                        model: self.config.model.clone(),
                        dimensions: entry.vector.len(),
                        created_at: entry.created_at,
                    });
                }
            }
        }

        // Truncate text if too long
        let text = if text.len() > self.config.max_text_length {
            &text[..self.config.max_text_length]
        } else {
            text
        };

        // Generate embedding based on model
        let vector = match &self.config.model {
            EmbeddingModel::OpenAITextEmbeddingAda002
            | EmbeddingModel::OpenAITextEmbedding3Small
            | EmbeddingModel::OpenAITextEmbedding3Large => {
                self.generate_openai_embedding(text).await?
            }
            EmbeddingModel::SentenceTransformersMiniLM
            | EmbeddingModel::SentenceTransformersMPNet => {
                self.generate_sentence_transformer_embedding(text).await?
            }
            EmbeddingModel::Custom { .. } => {
                self.generate_custom_embedding(text).await?
            }
        };

        let result = EmbeddingResult {
            vector: vector.clone(),
            text: text.to_string(),
            model: self.config.model.clone(),
            dimensions: vector.len(),
            created_at: chrono::Utc::now(),
        };

        // Cache the result
        if self.config.enable_cache {
            let cache_key = self.get_cache_key(text);
            self.cache.insert(cache_key, CacheEntry {
                vector,
                created_at: result.created_at,
                model: self.config.model.clone(),
            });
        }

        debug!("Generated embedding for text: {} (dimensions: {})", 
            &text[..50.min(text.len())], result.dimensions);
        Ok(result)
    }

    /// Generate embeddings for multiple texts
    pub async fn generate_embeddings_batch(&mut self, texts: &[String]) -> Result<Vec<EmbeddingResult>> {
        let mut results = Vec::new();
        
        for chunk in texts.chunks(self.config.batch_size) {
            let chunk_results = match &self.config.model {
                EmbeddingModel::OpenAITextEmbeddingAda002
                | EmbeddingModel::OpenAITextEmbedding3Small
                | EmbeddingModel::OpenAITextEmbedding3Large => {
                    self.generate_openai_embeddings_batch(chunk).await?
                }
                _ => {
                    // For other models, generate one by one
                    let mut chunk_results = Vec::new();
                    for text in chunk {
                        chunk_results.push(self.generate_embedding(text).await?);
                    }
                    chunk_results
                }
            };
            
            results.extend(chunk_results);
        }

        info!("Generated {} embeddings in batch", results.len());
        Ok(results)
    }

    /// Generate OpenAI embedding
    async fn generate_openai_embedding(&self, text: &str) -> Result<Vec<f32>> {
        let client = self.client.as_ref()
            .ok_or_else(|| StorageError::VectorSearchFailed("HTTP client not initialized".to_string()))?;

        let api_key = self.config.api_key.as_ref()
            .ok_or_else(|| StorageError::VectorSearchFailed("OpenAI API key not provided".to_string()))?;

        let base_url = self.config.base_url.as_deref()
            .unwrap_or("https://api.openai.com/v1");

        let request_body = serde_json::json!({
            "input": text,
            "model": self.config.model.name()
        });

        let response = client
            .post(&format!("{}/embeddings", base_url))
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| StorageError::VectorSearchFailed(format!("OpenAI API request failed: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(StorageError::VectorSearchFailed(format!(
                "OpenAI API error: {} - {}", response.status(), error_text
            )));
        }

        let response_json: serde_json::Value = response.json().await
            .map_err(|e| StorageError::VectorSearchFailed(format!("Failed to parse OpenAI response: {}", e)))?;

        let embedding = response_json["data"][0]["embedding"]
            .as_array()
            .ok_or_else(|| StorageError::VectorSearchFailed("Invalid OpenAI response format".to_string()))?
            .iter()
            .map(|v| v.as_f64().unwrap_or(0.0) as f32)
            .collect();

        Ok(embedding)
    }

    /// Generate OpenAI embeddings in batch
    async fn generate_openai_embeddings_batch(&self, texts: &[String]) -> Result<Vec<EmbeddingResult>> {
        let client = self.client.as_ref()
            .ok_or_else(|| StorageError::VectorSearchFailed("HTTP client not initialized".to_string()))?;

        let api_key = self.config.api_key.as_ref()
            .ok_or_else(|| StorageError::VectorSearchFailed("OpenAI API key not provided".to_string()))?;

        let base_url = self.config.base_url.as_deref()
            .unwrap_or("https://api.openai.com/v1");

        let request_body = serde_json::json!({
            "input": texts,
            "model": self.config.model.name()
        });

        let response = client
            .post(&format!("{}/embeddings", base_url))
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| StorageError::VectorSearchFailed(format!("OpenAI API request failed: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(StorageError::VectorSearchFailed(format!(
                "OpenAI API error: {} - {}", response.status(), error_text
            )));
        }

        let response_json: serde_json::Value = response.json().await
            .map_err(|e| StorageError::VectorSearchFailed(format!("Failed to parse OpenAI response: {}", e)))?;

        let mut results = Vec::new();
        let embeddings = response_json["data"]
            .as_array()
            .ok_or_else(|| StorageError::VectorSearchFailed("Invalid OpenAI response format".to_string()))?;

        for (i, embedding_data) in embeddings.iter().enumerate() {
            let embedding = embedding_data["embedding"]
                .as_array()
                .ok_or_else(|| StorageError::VectorSearchFailed("Invalid embedding format".to_string()))?
                .iter()
                .map(|v| v.as_f64().unwrap_or(0.0) as f32)
                .collect();

            results.push(EmbeddingResult {
                vector: embedding,
                text: texts[i].clone(),
                model: self.config.model.clone(),
                dimensions: self.config.model.dimensions(),
                created_at: chrono::Utc::now(),
            });
        }

        Ok(results)
    }

    /// Generate Sentence Transformer embedding (mock implementation)
    async fn generate_sentence_transformer_embedding(&self, text: &str) -> Result<Vec<f32>> {
        // In a real implementation, this would use a local Sentence Transformer model
        // For now, we'll generate a mock embedding based on text hash
        let hash = self.simple_hash(text);
        let mut vector = vec![0.0; self.config.model.dimensions()];
        
        // Generate deterministic "embedding" based on text hash
        for i in 0..vector.len() {
            vector[i] = ((hash + i as u64) % 1000) as f32 / 1000.0 - 0.5;
        }

        // Normalize the vector
        let norm: f32 = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for v in &mut vector {
                *v /= norm;
            }
        }

        Ok(vector)
    }

    /// Generate custom embedding (mock implementation)
    async fn generate_custom_embedding(&self, text: &str) -> Result<Vec<f32>> {
        // Mock implementation for custom models
        let hash = self.simple_hash(text);
        let mut vector = vec![0.0; self.config.model.dimensions()];
        
        for i in 0..vector.len() {
            vector[i] = ((hash + i as u64) % 1000) as f32 / 1000.0 - 0.5;
        }

        let norm: f32 = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for v in &mut vector {
                *v /= norm;
            }
        }

        Ok(vector)
    }

    /// Get cache key for text
    fn get_cache_key(&self, text: &str) -> String {
        format!("{}:{}", self.config.model.name(), self.simple_hash(text))
    }

    /// Simple hash function for cache keys
    fn simple_hash(&self, text: &str) -> u64 {
        let mut hash = 0u64;
        for byte in text.bytes() {
            hash = hash.wrapping_mul(31).wrapping_add(byte as u64);
        }
        hash
    }

    /// Clear the embedding cache
    pub fn clear_cache(&mut self) {
        self.cache.clear();
        info!("Embedding cache cleared");
    }

    /// Get cache statistics
    pub fn get_cache_stats(&self) -> CacheStats {
        CacheStats {
            cache_size: self.cache.len(),
            cache_hits: 0, // Would need to track this
            cache_misses: 0, // Would need to track this
        }
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub cache_size: usize,
    pub cache_hits: u64,
    pub cache_misses: u64,
}

/// Text preprocessing utilities
pub struct TextPreprocessor;

impl TextPreprocessor {
    /// Clean and normalize text for embedding
    pub fn preprocess_text(text: &str) -> String {
        text.trim()
            .to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric() || c.is_whitespace())
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<&str>>()
            .join(" ")
    }

    /// Extract text from JSON value
    pub fn extract_text_from_json(value: &serde_json::Value) -> String {
        match value {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Object(obj) => {
                // Try to find common text fields
                let text_fields = ["text", "content", "description", "title", "name", "body"];
                for field in &text_fields {
                    if let Some(serde_json::Value::String(text)) = obj.get(field) {
                        return text.clone();
                    }
                }
                // Fallback to string representation
                value.to_string()
            }
            _ => value.to_string(),
        }
    }

    /// Split text into chunks for embedding
    pub fn split_text_into_chunks(text: &str, max_chunk_size: usize) -> Vec<String> {
        let words: Vec<&str> = text.split_whitespace().collect();
        let mut chunks = Vec::new();
        let mut current_chunk = String::new();

        for word in words {
            if current_chunk.len() + word.len() + 1 > max_chunk_size {
                if !current_chunk.is_empty() {
                    chunks.push(current_chunk.trim().to_string());
                    current_chunk = String::new();
                }
            }
            if !current_chunk.is_empty() {
                current_chunk.push(' ');
            }
            current_chunk.push_str(word);
        }

        if !current_chunk.is_empty() {
            chunks.push(current_chunk.trim().to_string());
        }

        chunks
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_embedding_generation() {
        let config = EmbeddingConfig::default();
        let mut generator = EmbeddingGenerator::new(config);

        let text = "This is a test text for embedding generation";
        let result = generator.generate_embedding(text).await.unwrap();

        assert_eq!(result.text, text);
        assert_eq!(result.dimensions, 384); // SentenceTransformersMiniLM default
        assert_eq!(result.vector.len(), 384);
        assert!(!result.vector.iter().all(|&x| x == 0.0)); // Should not be all zeros
    }

    #[tokio::test]
    async fn test_embedding_caching() {
        let config = EmbeddingConfig {
            enable_cache: true,
            ..Default::default()
        };
        let mut generator = EmbeddingGenerator::new(config);

        let text = "Test text for caching";
        
        // First generation
        let result1 = generator.generate_embedding(text).await.unwrap();
        
        // Second generation (should use cache)
        let result2 = generator.generate_embedding(text).await.unwrap();

        assert_eq!(result1.vector, result2.vector);
        assert_eq!(result1.created_at, result2.created_at);
    }

    #[test]
    fn test_text_preprocessing() {
        let text = "  Hello, World! This is a test.  ";
        let processed = TextPreprocessor::preprocess_text(text);
        assert_eq!(processed, "hello world this is a test");
    }

    #[test]
    fn test_text_chunking() {
        let text = "This is a very long text that should be split into multiple chunks for better embedding generation";
        let chunks = TextPreprocessor::split_text_into_chunks(text, 20);
        
        assert!(chunks.len() > 1);
        for chunk in &chunks {
            assert!(chunk.len() <= 20);
        }
    }

    #[test]
    fn test_json_text_extraction() {
        let json = serde_json::json!({
            "title": "Test Title",
            "content": "Test content here",
            "id": 123
        });
        
        let text = TextPreprocessor::extract_text_from_json(&json);
        assert_eq!(text, "Test Title"); // Should extract the first text field
    }
}


