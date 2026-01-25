# PluresDB Plugin System

The PluresDB plugin system allows you to extend the database with custom functionality.

## Plugin Types

### 1. Embedding Providers

Add custom embedding providers for vector search:

```typescript
import { Plugin, EmbeddingProvider } from "@plures/pluresdb/plugins";

class MyEmbeddingProvider implements EmbeddingProvider {
  name = "my-embeddings";
  dimensions = 384;
  
  async embed(text: string): Promise<number[]> {
    // Your embedding logic here
    return new Array(this.dimensions).fill(0);
  }
}

const plugin: Plugin = {
  id: "my-plugin",
  name: "My Plugin",
  version: "1.0.0",
  embeddingProviders: [new MyEmbeddingProvider()],
};
```

### 2. UI Panels

Add custom UI panels to the web interface:

```typescript
import MyPanel from "./MyPanel.svelte";

const plugin: Plugin = {
  id: "my-ui-plugin",
  name: "My UI Plugin",
  version: "1.0.0",
  uiPanels: [{
    id: "my-panel",
    name: "My Panel",
    icon: "🎨",
    component: MyPanel,
    order: 100,
  }],
};
```

### 3. Query Transformers

Transform queries before execution:

```typescript
const plugin: Plugin = {
  id: "query-plugin",
  name: "Query Plugin",
  version: "1.0.0",
  queryTransformers: [{
    id: "my-transformer",
    async transform(query: any) {
      // Modify query here
      return { ...query, enhanced: true };
    },
  }],
};
```

### 4. Data Validators

Validate data before storage:

```typescript
const plugin: Plugin = {
  id: "validator-plugin",
  name: "Validator Plugin",
  version: "1.0.0",
  dataValidators: [{
    id: "email-validator",
    async validate(data: Record<string, unknown>) {
      if (data.email && typeof data.email === "string") {
        const isValid = /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(data.email);
        if (!isValid) {
          return {
            valid: false,
            errors: ["Invalid email format"],
          };
        }
      }
      return { valid: true };
    },
  }],
};
```

## Registering Plugins

```typescript
import { pluginManager } from "@plures/pluresdb/plugins";
import { myPlugin } from "./my-plugin";

// Register plugin
await pluginManager.register(myPlugin);

// Use custom embedding provider
const provider = pluginManager.getEmbeddingProvider("my-embeddings");
if (provider) {
  const embedding = await provider.embed("Hello, world!");
}

// Unregister plugin
await pluginManager.unregister("my-plugin");
```

## Complete Example

See `example-embedding-plugin.ts` for a complete example plugin.

## API Reference

### Plugin Interface

```typescript
interface Plugin {
  id: string;
  name: string;
  version: string;
  description?: string;
  embeddingProviders?: EmbeddingProvider[];
  uiPanels?: UIPanel[];
  queryTransformers?: QueryTransformer[];
  dataValidators?: DataValidator[];
  init?(): Promise<void>;
  destroy?(): Promise<void>;
}
```

### PluginManager Methods

- `register(plugin: Plugin): Promise<void>` - Register a plugin
- `unregister(pluginId: string): Promise<void>` - Unregister a plugin
- `getPlugins(): Plugin[]` - Get all registered plugins
- `getEmbeddingProvider(name: string): EmbeddingProvider | undefined`
- `getEmbeddingProviders(): EmbeddingProvider[]`
- `getUIPanel(id: string): UIPanel | undefined`
- `getUIPanels(): UIPanel[]`
- `getQueryTransformer(id: string): QueryTransformer | undefined`
- `getQueryTransformers(): QueryTransformer[]`
- `getDataValidator(id: string): DataValidator | undefined`
- `getDataValidators(): DataValidator[]`
- `transformQuery(query: any): Promise<any>` - Apply all transformers
- `validateData(data: Record<string, unknown>): Promise<{valid: boolean; errors: string[]}>` - Validate with all validators

## Best Practices

1. **Unique IDs**: Use unique, descriptive IDs for plugins and their components
2. **Error Handling**: Handle errors gracefully in your plugin code
3. **Cleanup**: Implement `destroy()` to clean up resources
4. **Documentation**: Document your plugin's functionality and configuration
5. **Testing**: Test your plugin thoroughly before deployment
6. **Versioning**: Use semantic versioning for your plugins

## Security Considerations

- Validate all input data in your plugins
- Avoid storing sensitive data (API keys, etc.) in plugin code
- Use environment variables for configuration
- Review third-party dependencies carefully
- Test plugins in a safe environment before production use
