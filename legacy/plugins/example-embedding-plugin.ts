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
 * Example PluresDB plugin that registers a simple character-based embedding provider.
 *
 * Demonstrates the minimum required shape for a {@link Plugin} that supplies a
 * custom {@link EmbeddingProvider}.  The provider maps character codes to a
 * 256-dimensional normalised vector — useful for testing and prototyping but
 * **not** suitable for production semantic search.
 *
 * @example
 * ```typescript
 * import { PluresDB } from "pluresdb";
 * import { customEmbeddingsPlugin } from "./plugins/example-embedding-plugin";
 *
 * const db = new PluresDB({ plugins: [customEmbeddingsPlugin] });
 * await db.ready();
 * ```
 */
export const customEmbeddingsPlugin: Plugin = {
  id: "custom-embeddings",
  name: "Custom Embeddings Provider",
  version: "1.0.0",
  description: "Provides a simple character-based embedding provider for PluresDB",
  
  embeddingProviders: [
    new SimpleEmbeddingProvider(),
  ],
  
  async init() {
    console.log("Custom embeddings plugin initialized");
  },
  
  async destroy() {
    console.log("Custom embeddings plugin destroyed");
  },
};
