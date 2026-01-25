/**
 * PluresDB Plugin System
 * 
 * Provides extension points for customizing PluresDB behavior:
 * - Custom embedding providers
 * - Custom UI panels
 * - Query transformers
 * - Data validators
 */

export interface EmbeddingProvider {
  name: string;
  /**
   * Generate embeddings for text content
   * @param text - Text to embed
   * @returns Promise resolving to embedding vector
   */
  embed(text: string): Promise<number[]>;
  /**
   * Dimensionality of embeddings produced by this provider
   */
  dimensions: number;
}

export interface UIPanel {
  /**
   * Unique identifier for the panel
   */
  id: string;
  /**
   * Display name for the panel tab
   */
  name: string;
  /**
   * Icon name or emoji
   */
  icon?: string;
  /**
   * Svelte component to render
   */
  component: any;
  /**
   * Position in tab list (lower numbers appear first)
   */
  order?: number;
}

export interface QueryTransformer {
  /**
   * Unique identifier for the transformer
   */
  id: string;
  /**
   * Transform a query before execution
   * @param query - Original query
   * @returns Transformed query
   */
  transform(query: any): Promise<any>;
}

export interface DataValidator {
  /**
   * Unique identifier for the validator
   */
  id: string;
  /**
   * Validate data before storage
   * @param data - Data to validate
   * @returns Validation result with errors if any
   */
  validate(data: Record<string, unknown>): Promise<{
    valid: boolean;
    errors?: string[];
  }>;
}

export interface Plugin {
  /**
   * Unique identifier for the plugin
   */
  id: string;
  /**
   * Plugin name
   */
  name: string;
  /**
   * Plugin version
   */
  version: string;
  /**
   * Plugin description
   */
  description?: string;
  /**
   * Embedding providers contributed by this plugin
   */
  embeddingProviders?: EmbeddingProvider[];
  /**
   * UI panels contributed by this plugin
   */
  uiPanels?: UIPanel[];
  /**
   * Query transformers contributed by this plugin
   */
  queryTransformers?: QueryTransformer[];
  /**
   * Data validators contributed by this plugin
   */
  dataValidators?: DataValidator[];
  /**
   * Initialize the plugin
   */
  init?(): Promise<void>;
  /**
   * Cleanup the plugin
   */
  destroy?(): Promise<void>;
}

class PluginManager {
  private plugins: Map<string, Plugin> = new Map();
  private embeddingProviders: Map<string, EmbeddingProvider> = new Map();
  private uiPanels: Map<string, UIPanel> = new Map();
  private queryTransformers: Map<string, QueryTransformer> = new Map();
  private dataValidators: Map<string, DataValidator> = new Map();

  /**
   * Register a plugin
   */
  async register(plugin: Plugin): Promise<void> {
    if (this.plugins.has(plugin.id)) {
      throw new Error(`Plugin ${plugin.id} is already registered`);
    }

    // Register embedding providers
    if (plugin.embeddingProviders) {
      for (const provider of plugin.embeddingProviders) {
        this.embeddingProviders.set(provider.name, provider);
      }
    }

    // Register UI panels
    if (plugin.uiPanels) {
      for (const panel of plugin.uiPanels) {
        this.uiPanels.set(panel.id, panel);
      }
    }

    // Register query transformers
    if (plugin.queryTransformers) {
      for (const transformer of plugin.queryTransformers) {
        this.queryTransformers.set(transformer.id, transformer);
      }
    }

    // Register data validators
    if (plugin.dataValidators) {
      for (const validator of plugin.dataValidators) {
        this.dataValidators.set(validator.id, validator);
      }
    }

    // Initialize plugin
    if (plugin.init) {
      await plugin.init();
    }

    this.plugins.set(plugin.id, plugin);
  }

  /**
   * Unregister a plugin
   */
  async unregister(pluginId: string): Promise<void> {
    const plugin = this.plugins.get(pluginId);
    if (!plugin) {
      return;
    }

    // Cleanup plugin
    if (plugin.destroy) {
      await plugin.destroy();
    }

    // Remove embedding providers
    if (plugin.embeddingProviders) {
      for (const provider of plugin.embeddingProviders) {
        this.embeddingProviders.delete(provider.name);
      }
    }

    // Remove UI panels
    if (plugin.uiPanels) {
      for (const panel of plugin.uiPanels) {
        this.uiPanels.delete(panel.id);
      }
    }

    // Remove query transformers
    if (plugin.queryTransformers) {
      for (const transformer of plugin.queryTransformers) {
        this.queryTransformers.delete(transformer.id);
      }
    }

    // Remove data validators
    if (plugin.dataValidators) {
      for (const validator of plugin.dataValidators) {
        this.dataValidators.delete(validator.id);
      }
    }

    this.plugins.delete(pluginId);
  }

  /**
   * Get all registered plugins
   */
  getPlugins(): Plugin[] {
    return Array.from(this.plugins.values());
  }

  /**
   * Get embedding provider by name
   */
  getEmbeddingProvider(name: string): EmbeddingProvider | undefined {
    return this.embeddingProviders.get(name);
  }

  /**
   * Get all embedding providers
   */
  getEmbeddingProviders(): EmbeddingProvider[] {
    return Array.from(this.embeddingProviders.values());
  }

  /**
   * Get UI panel by ID
   */
  getUIPanel(id: string): UIPanel | undefined {
    return this.uiPanels.get(id);
  }

  /**
   * Get all UI panels sorted by order
   */
  getUIPanels(): UIPanel[] {
    return Array.from(this.uiPanels.values()).sort(
      (a, b) => (a.order ?? 999) - (b.order ?? 999)
    );
  }

  /**
   * Get query transformer by ID
   */
  getQueryTransformer(id: string): QueryTransformer | undefined {
    return this.queryTransformers.get(id);
  }

  /**
   * Get all query transformers
   */
  getQueryTransformers(): QueryTransformer[] {
    return Array.from(this.queryTransformers.values());
  }

  /**
   * Get data validator by ID
   */
  getDataValidator(id: string): DataValidator | undefined {
    return this.dataValidators.get(id);
  }

  /**
   * Get all data validators
   */
  getDataValidators(): DataValidator[] {
    return Array.from(this.dataValidators.values());
  }

  /**
   * Apply all query transformers to a query
   */
  async transformQuery(query: any): Promise<any> {
    let transformed = query;
    for (const transformer of this.queryTransformers.values()) {
      transformed = await transformer.transform(transformed);
    }
    return transformed;
  }

  /**
   * Validate data using all validators
   */
  async validateData(data: Record<string, unknown>): Promise<{
    valid: boolean;
    errors: string[];
  }> {
    const errors: string[] = [];
    for (const validator of this.dataValidators.values()) {
      const result = await validator.validate(data);
      if (!result.valid && result.errors) {
        errors.push(...result.errors);
      }
    }
    return {
      valid: errors.length === 0,
      errors,
    };
  }
}

// Global plugin manager instance
export const pluginManager = new PluginManager();

// Export types
export type { Plugin, EmbeddingProvider, UIPanel, QueryTransformer, DataValidator };
