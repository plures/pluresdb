import type { Plugin, EmbeddingProvider } from "./plugin-system";

/**
 * Example Custom Embedding Provider Plugin
 * 
 * This plugin demonstrates how to add a custom embedding provider to PluresDB.
 * It provides a simple character-based embedding for demonstration purposes.
 */

class SimpleEmbeddingProvider implements EmbeddingProvider {
  name = "simple-char-embeddings";
  dimensions = 256;

  /**
   * Generate simple character-based embeddings
   * Maps characters to vector positions
   */
  async embed(text: string): Promise<number[]> {
    const vector = new Array(this.dimensions).fill(0);
    
    // Simple algorithm: increment vector position for each character
    for (let i = 0; i < text.length && i < this.dimensions; i++) {
      const charCode = text.charCodeAt(i);
      vector[charCode % this.dimensions] += 1;
    }
    
    // Normalize
    const magnitude = Math.sqrt(vector.reduce((sum, val) => sum + val * val, 0));
    if (magnitude > 0) {
      for (let i = 0; i < vector.length; i++) {
        vector[i] /= magnitude;
      }
    }
    
    return vector;
  }
}

/**
 * OpenAI Embedding Provider
 * (Stub - would need actual API key and implementation)
 */
class OpenAIEmbeddingProvider implements EmbeddingProvider {
  name = "openai-ada-002";
  dimensions = 1536;
  
  constructor(private apiKey: string) {}

  async embed(text: string): Promise<number[]> {
    // Stub implementation - in production, would call OpenAI API
    console.warn("OpenAI embeddings not implemented - using random vector");
    return Array.from({ length: this.dimensions }, () => Math.random() - 0.5);
  }
}

export const customEmbeddingsPlugin: Plugin = {
  id: "custom-embeddings",
  name: "Custom Embeddings Provider",
  version: "1.0.0",
  description: "Provides custom embedding providers for PluresDB",
  
  embeddingProviders: [
    new SimpleEmbeddingProvider(),
    // Uncomment and configure with API key to use:
    // new OpenAIEmbeddingProvider(process.env.OPENAI_API_KEY || ""),
  ],
  
  async init() {
    console.log("Custom embeddings plugin initialized");
  },
  
  async destroy() {
    console.log("Custom embeddings plugin destroyed");
  },
};
