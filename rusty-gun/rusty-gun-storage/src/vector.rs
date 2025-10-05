//! Vector search engine implementation

use crate::{
    error::{Result, StorageError},
    traits::{VectorSearchEngine, VectorSearchResult, VectorStats},
    VectorConfig,
};
use hnsw_rs::prelude::*;
use serde_json::Value;
use std::collections::HashMap;
use tracing::{debug, info};

/// HNSW-based vector search engine
pub struct HnswVectorEngine {
    index: Option<Hnsw<f32, DistCosine>>,
    vectors: HashMap<String, (Vec<f32>, Value)>,
    config: VectorConfig,
}

impl HnswVectorEngine {
    /// Create a new HNSW vector search engine
    pub fn new(config: VectorConfig) -> Self {
        Self {
            index: None,
            vectors: HashMap::new(),
            config,
        }
    }

    /// Initialize the HNSW index
    fn initialize_index(&mut self) -> Result<()> {
        let hnsw_params = HnswParams::<DistCosine>::new(
            self.config.hnsw_m,
            self.config.hnsw_ef_construction,
            self.config.hnsw_ef,
            self.config.max_vectors,
        );

        let index = Hnsw::<f32, DistCosine>::new(
            self.config.dimensions,
            &hnsw_params,
        );

        self.index = Some(index);
        info!("HNSW vector index initialized with {} dimensions", self.config.dimensions);
        Ok(())
    }
}

#[async_trait::async_trait]
impl VectorSearchEngine for HnswVectorEngine {
    async fn initialize(&mut self) -> Result<()> {
        self.initialize_index()?;
        info!("Vector search engine initialized");
        Ok(())
    }

    async fn add_vector(
        &self,
        id: &str,
        vector: &[f32],
        metadata: &Value,
    ) -> Result<()> {
        if vector.len() != self.config.dimensions {
            return Err(StorageError::VectorSearchFailed(format!(
                "Vector dimension mismatch: expected {}, got {}",
                self.config.dimensions,
                vector.len()
            )));
        }

        if let Some(ref index) = self.index {
            index.insert(vector, id)?;
        }

        // Store in memory map
        self.vectors.insert(id.to_string(), (vector.to_vec(), metadata.clone()));

        debug!("Added vector: {} (dimensions: {})", id, vector.len());
        Ok(())
    }

    async fn search_vectors(
        &self,
        query_vector: &[f32],
        limit: usize,
    ) -> Result<Vec<VectorSearchResult>> {
        if query_vector.len() != self.config.dimensions {
            return Err(StorageError::VectorSearchFailed(format!(
                "Query vector dimension mismatch: expected {}, got {}",
                self.config.dimensions,
                query_vector.len()
            )));
        }

        if let Some(ref index) = self.index {
            let search_results = index.search(query_vector, limit, 0)?;
            
            let mut results = Vec::new();
            for (id, score) in search_results {
                if let Some((_, metadata)) = self.vectors.get(&id) {
                    results.push(VectorSearchResult {
                        id: id.clone(),
                        score: 1.0 - score, // Convert distance to similarity
                        metadata: metadata.clone(),
                    });
                }
            }

            // Sort by score (descending)
            results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

            debug!("Vector search completed: {} results", results.len());
            Ok(results)
        } else {
            Err(StorageError::VectorSearchFailed("Index not initialized".to_string()))
        }
    }

    async fn remove_vector(&self, id: &str) -> Result<()> {
        // Note: HNSW doesn't support removal, so we just remove from memory map
        self.vectors.remove(id);
        debug!("Removed vector: {}", id);
        Ok(())
    }

    async fn update_vector(
        &self,
        id: &str,
        vector: &[f32],
        metadata: &Value,
    ) -> Result<()> {
        if vector.len() != self.config.dimensions {
            return Err(StorageError::VectorSearchFailed(format!(
                "Vector dimension mismatch: expected {}, got {}",
                self.config.dimensions,
                vector.len()
            )));
        }

        // Remove old vector and add new one
        self.remove_vector(id).await?;
        self.add_vector(id, vector, metadata).await?;

        debug!("Updated vector: {}", id);
        Ok(())
    }

    async fn get_vector(&self, id: &str) -> Result<Option<(Vec<f32>, Value)>> {
        Ok(self.vectors.get(id).map(|(v, m)| (v.clone(), m.clone())))
    }

    async fn get_stats(&self) -> Result<VectorStats> {
        Ok(VectorStats {
            vector_count: self.vectors.len() as u64,
            dimensions: self.config.dimensions,
            index_size: self.vectors.len() as u64 * self.config.dimensions as u64 * 4, // Approximate
            last_updated: chrono::Utc::now(),
        })
    }
}

/// Cosine distance implementation
#[derive(Debug, Clone)]
pub struct DistCosine;

impl Distance<f32> for DistCosine {
    fn distance(&self, a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() {
            return f32::INFINITY;
        }

        let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_a == 0.0 || norm_b == 0.0 {
            return 1.0;
        }

        1.0 - (dot_product / (norm_a * norm_b))
    }
}

/// Simple in-memory vector search engine (fallback)
pub struct InMemoryVectorEngine {
    vectors: HashMap<String, (Vec<f32>, Value)>,
    config: VectorConfig,
}

impl InMemoryVectorEngine {
    /// Create a new in-memory vector search engine
    pub fn new(config: VectorConfig) -> Self {
        Self {
            vectors: HashMap::new(),
            config,
        }
    }

    /// Calculate cosine similarity between two vectors
    fn cosine_similarity(&self, a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() {
            return 0.0;
        }

        let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_a == 0.0 || norm_b == 0.0 {
            0.0
        } else {
            dot_product / (norm_a * norm_b)
        }
    }
}

#[async_trait::async_trait]
impl VectorSearchEngine for InMemoryVectorEngine {
    async fn initialize(&mut self) -> Result<()> {
        info!("In-memory vector search engine initialized");
        Ok(())
    }

    async fn add_vector(
        &self,
        id: &str,
        vector: &[f32],
        metadata: &Value,
    ) -> Result<()> {
        if vector.len() != self.config.dimensions {
            return Err(StorageError::VectorSearchFailed(format!(
                "Vector dimension mismatch: expected {}, got {}",
                self.config.dimensions,
                vector.len()
            )));
        }

        self.vectors.insert(id.to_string(), (vector.to_vec(), metadata.clone()));
        debug!("Added vector: {} (dimensions: {})", id, vector.len());
        Ok(())
    }

    async fn search_vectors(
        &self,
        query_vector: &[f32],
        limit: usize,
    ) -> Result<Vec<VectorSearchResult>> {
        if query_vector.len() != self.config.dimensions {
            return Err(StorageError::VectorSearchFailed(format!(
                "Query vector dimension mismatch: expected {}, got {}",
                self.config.dimensions,
                query_vector.len()
            )));
        }

        let mut results: Vec<VectorSearchResult> = self.vectors
            .iter()
            .map(|(id, (vector, metadata))| {
                let score = self.cosine_similarity(query_vector, vector);
                VectorSearchResult {
                    id: id.clone(),
                    score,
                    metadata: metadata.clone(),
                }
            })
            .collect();

        // Sort by score (descending)
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

        // Limit results
        results.truncate(limit);

        debug!("Vector search completed: {} results", results.len());
        Ok(results)
    }

    async fn remove_vector(&self, id: &str) -> Result<()> {
        self.vectors.remove(id);
        debug!("Removed vector: {}", id);
        Ok(())
    }

    async fn update_vector(
        &self,
        id: &str,
        vector: &[f32],
        metadata: &Value,
    ) -> Result<()> {
        if vector.len() != self.config.dimensions {
            return Err(StorageError::VectorSearchFailed(format!(
                "Vector dimension mismatch: expected {}, got {}",
                self.config.dimensions,
                vector.len()
            )));
        }

        self.vectors.insert(id.to_string(), (vector.to_vec(), metadata.clone()));
        debug!("Updated vector: {}", id);
        Ok(())
    }

    async fn get_vector(&self, id: &str) -> Result<Option<(Vec<f32>, Value)>> {
        Ok(self.vectors.get(id).map(|(v, m)| (v.clone(), m.clone())))
    }

    async fn get_stats(&self) -> Result<VectorStats> {
        Ok(VectorStats {
            vector_count: self.vectors.len() as u64,
            dimensions: self.config.dimensions,
            index_size: self.vectors.len() as u64 * self.config.dimensions as u64 * 4, // Approximate
            last_updated: chrono::Utc::now(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_in_memory_vector_engine() {
        let config = VectorConfig::default();
        let mut engine = InMemoryVectorEngine::new(config);
        engine.initialize().await.unwrap();

        // Add test vectors
        let vector1 = vec![1.0, 0.0, 0.0];
        let vector2 = vec![0.0, 1.0, 0.0];
        let vector3 = vec![0.0, 0.0, 1.0];

        engine.add_vector("1", &vector1, &serde_json::json!({"name": "vector1"})).await.unwrap();
        engine.add_vector("2", &vector2, &serde_json::json!({"name": "vector2"})).await.unwrap();
        engine.add_vector("3", &vector3, &serde_json::json!({"name": "vector3"})).await.unwrap();

        // Search for similar vectors
        let query = vec![1.0, 0.0, 0.0];
        let results = engine.search_vectors(&query, 2).await.unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].id, "1");
        assert!((results[0].score - 1.0).abs() < 0.001); // Should be very similar

        // Test stats
        let stats = engine.get_stats().await.unwrap();
        assert_eq!(stats.vector_count, 3);
        assert_eq!(stats.dimensions, 3);
    }

    #[tokio::test]
    async fn test_vector_operations() {
        let config = VectorConfig::default();
        let mut engine = InMemoryVectorEngine::new(config);
        engine.initialize().await.unwrap();

        let vector = vec![1.0, 2.0, 3.0];
        let metadata = serde_json::json!({"test": "data"});

        // Add vector
        engine.add_vector("test", &vector, &metadata).await.unwrap();

        // Get vector
        let retrieved = engine.get_vector("test").await.unwrap();
        assert!(retrieved.is_some());
        let (retrieved_vector, retrieved_metadata) = retrieved.unwrap();
        assert_eq!(retrieved_vector, vector);
        assert_eq!(retrieved_metadata, metadata);

        // Update vector
        let new_vector = vec![4.0, 5.0, 6.0];
        let new_metadata = serde_json::json!({"test": "updated"});
        engine.update_vector("test", &new_vector, &new_metadata).await.unwrap();

        // Verify update
        let retrieved = engine.get_vector("test").await.unwrap();
        assert!(retrieved.is_some());
        let (retrieved_vector, retrieved_metadata) = retrieved.unwrap();
        assert_eq!(retrieved_vector, new_vector);
        assert_eq!(retrieved_metadata, new_metadata);

        // Remove vector
        engine.remove_vector("test").await.unwrap();
        let retrieved = engine.get_vector("test").await.unwrap();
        assert!(retrieved.is_none());
    }
}


