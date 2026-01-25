// @ts-nocheck
import {
  assertEquals,
  assertExists,
  assertRejects,
  assert,
} from "jsr:@std/assert@1.0.14";
import {
  pluginManager,
  type Plugin,
  type EmbeddingProvider,
  type UIPanel,
  type QueryTransformer,
  type DataValidator,
} from "../../plugins/plugin-system.ts";

// Test helpers
class TestEmbeddingProvider implements EmbeddingProvider {
  name = "test-embeddings";
  dimensions = 128;

  async embed(text: string): Promise<number[]> {
    return new Array(this.dimensions).fill(0.1);
  }
}

class TestQueryTransformer implements QueryTransformer {
  id = "test-transformer";

  async transform(query: any): Promise<any> {
    return { ...query, transformed: true };
  }
}

class TestDataValidator implements DataValidator {
  id = "test-validator";

  async validate(data: Record<string, unknown>): Promise<{
    valid: boolean;
    errors?: string[];
  }> {
    if (!data.required) {
      return { valid: false, errors: ["Missing required field"] };
    }
    return { valid: true };
  }
}

Deno.test("Plugin System - Plugin Registration", async () => {
  const plugin: Plugin = {
    id: "test-plugin-1",
    name: "Test Plugin",
    version: "1.0.0",
    description: "A test plugin",
  };

  await pluginManager.register(plugin);

  const plugins = pluginManager.getPlugins();
  const registered = plugins.find((p) => p.id === "test-plugin-1");

  assertExists(registered);
  assertEquals(registered.name, "Test Plugin");
  assertEquals(registered.version, "1.0.0");

  // Cleanup
  await pluginManager.unregister("test-plugin-1");
});

Deno.test("Plugin System - Prevent Duplicate Plugin IDs", async () => {
  const plugin1: Plugin = {
    id: "duplicate-test",
    name: "First Plugin",
    version: "1.0.0",
  };

  const plugin2: Plugin = {
    id: "duplicate-test",
    name: "Second Plugin",
    version: "2.0.0",
  };

  await pluginManager.register(plugin1);

  // Attempting to register a plugin with duplicate ID should throw
  await assertRejects(
    async () => {
      await pluginManager.register(plugin2);
    },
    Error,
    "already registered",
  );

  // Cleanup
  await pluginManager.unregister("duplicate-test");
});

Deno.test("Plugin System - Embedding Provider Registration", async () => {
  const provider = new TestEmbeddingProvider();
  const plugin: Plugin = {
    id: "embedding-test",
    name: "Embedding Test",
    version: "1.0.0",
    embeddingProviders: [provider],
  };

  await pluginManager.register(plugin);

  const registeredProvider = pluginManager.getEmbeddingProvider(
    "test-embeddings",
  );
  assertExists(registeredProvider);
  assertEquals(registeredProvider.dimensions, 128);

  const embedding = await registeredProvider.embed("test");
  assertEquals(embedding.length, 128);
  assertEquals(embedding[0], 0.1);

  // Cleanup
  await pluginManager.unregister("embedding-test");
});

Deno.test("Plugin System - Query Transformer Registration", async () => {
  const transformer = new TestQueryTransformer();
  const plugin: Plugin = {
    id: "transformer-test",
    name: "Transformer Test",
    version: "1.0.0",
    queryTransformers: [transformer],
  };

  await pluginManager.register(plugin);

  const registeredTransformer = pluginManager.getQueryTransformer(
    "test-transformer",
  );
  assertExists(registeredTransformer);

  const query = { field: "value" };
  const transformed = await registeredTransformer.transform(query);
  assertEquals(transformed.field, "value");
  assertEquals(transformed.transformed, true);

  // Cleanup
  await pluginManager.unregister("transformer-test");
});

Deno.test("Plugin System - Data Validator Registration", async () => {
  const validator = new TestDataValidator();
  const plugin: Plugin = {
    id: "validator-test",
    name: "Validator Test",
    version: "1.0.0",
    dataValidators: [validator],
  };

  await pluginManager.register(plugin);

  const registeredValidator = pluginManager.getDataValidator("test-validator");
  assertExists(registeredValidator);

  // Test valid data
  const validResult = await registeredValidator.validate({ required: true });
  assertEquals(validResult.valid, true);

  // Test invalid data
  const invalidResult = await registeredValidator.validate({});
  assertEquals(invalidResult.valid, false);
  assertExists(invalidResult.errors);
  assertEquals(invalidResult.errors.length, 1);

  // Cleanup
  await pluginManager.unregister("validator-test");
});

Deno.test("Plugin System - Plugin Lifecycle (init/destroy)", async () => {
  let initCalled = false;
  let destroyCalled = false;

  const plugin: Plugin = {
    id: "lifecycle-test",
    name: "Lifecycle Test",
    version: "1.0.0",
    async init() {
      initCalled = true;
    },
    async destroy() {
      destroyCalled = true;
    },
  };

  await pluginManager.register(plugin);
  assertEquals(initCalled, true);

  await pluginManager.unregister("lifecycle-test");
  assertEquals(destroyCalled, true);
});

Deno.test("Plugin System - Transform Query with Multiple Transformers", async () => {
  const transformer1: QueryTransformer = {
    id: "transformer-1",
    async transform(query: any) {
      return { ...query, step1: true };
    },
  };

  const transformer2: QueryTransformer = {
    id: "transformer-2",
    async transform(query: any) {
      return { ...query, step2: true };
    },
  };

  const plugin: Plugin = {
    id: "multi-transformer-test",
    name: "Multi Transformer Test",
    version: "1.0.0",
    queryTransformers: [transformer1, transformer2],
  };

  await pluginManager.register(plugin);

  const query = { original: true };
  const transformed = await pluginManager.transformQuery(query);

  assertEquals(transformed.original, true);
  assertEquals(transformed.step1, true);
  assertEquals(transformed.step2, true);

  // Cleanup
  await pluginManager.unregister("multi-transformer-test");
});

Deno.test("Plugin System - Validate Data with Multiple Validators", async () => {
  const validator1: DataValidator = {
    id: "validator-1",
    async validate(data: Record<string, unknown>) {
      if (!data.field1) {
        return { valid: false, errors: ["Missing field1"] };
      }
      return { valid: true };
    },
  };

  const validator2: DataValidator = {
    id: "validator-2",
    async validate(data: Record<string, unknown>) {
      if (!data.field2) {
        return { valid: false, errors: ["Missing field2"] };
      }
      return { valid: true };
    },
  };

  const plugin: Plugin = {
    id: "multi-validator-test",
    name: "Multi Validator Test",
    version: "1.0.0",
    dataValidators: [validator1, validator2],
  };

  await pluginManager.register(plugin);

  // Test with all fields valid
  const validResult = await pluginManager.validateData({
    field1: true,
    field2: true,
  });
  assertEquals(validResult.valid, true);
  assertEquals(validResult.errors.length, 0);

  // Test with missing fields
  const invalidResult = await pluginManager.validateData({});
  assertEquals(invalidResult.valid, false);
  assertEquals(invalidResult.errors.length, 2);
  assert(invalidResult.errors.includes("Missing field1"));
  assert(invalidResult.errors.includes("Missing field2"));

  // Cleanup
  await pluginManager.unregister("multi-validator-test");
});

Deno.test("Plugin System - Unregister Non-Existent Plugin", async () => {
  // Should not throw error when unregistering non-existent plugin
  await pluginManager.unregister("non-existent-plugin");
  // If we get here without error, test passes
  assert(true);
});

Deno.test("Plugin System - Get All Providers After Registration", async () => {
  const provider1 = new TestEmbeddingProvider();
  provider1.name = "provider-1";

  const provider2 = new TestEmbeddingProvider();
  provider2.name = "provider-2";

  const plugin: Plugin = {
    id: "multi-provider-test",
    name: "Multi Provider Test",
    version: "1.0.0",
    embeddingProviders: [provider1, provider2],
  };

  await pluginManager.register(plugin);

  const providers = pluginManager.getEmbeddingProviders();
  const testProviders = providers.filter((p) =>
    p.name === "provider-1" || p.name === "provider-2"
  );

  assertEquals(testProviders.length, 2);

  // Cleanup
  await pluginManager.unregister("multi-provider-test");
});

Deno.test("Plugin System - UI Panel Registration", async () => {
  const panel: UIPanel = {
    id: "test-panel",
    name: "Test Panel",
    icon: "🧪",
    component: {} as any, // Mock component
    order: 100,
  };

  const plugin: Plugin = {
    id: "panel-test",
    name: "Panel Test",
    version: "1.0.0",
    uiPanels: [panel],
  };

  await pluginManager.register(plugin);

  const registeredPanel = pluginManager.getUIPanel("test-panel");
  assertExists(registeredPanel);
  assertEquals(registeredPanel.name, "Test Panel");
  assertEquals(registeredPanel.icon, "🧪");
  assertEquals(registeredPanel.order, 100);

  // Cleanup
  await pluginManager.unregister("panel-test");
});

Deno.test("Plugin System - UI Panels Sorted by Order", async () => {
  const panel1: UIPanel = {
    id: "panel-1",
    name: "Panel 1",
    component: {} as any,
    order: 30,
  };

  const panel2: UIPanel = {
    id: "panel-2",
    name: "Panel 2",
    component: {} as any,
    order: 10,
  };

  const panel3: UIPanel = {
    id: "panel-3",
    name: "Panel 3",
    component: {} as any,
    order: 20,
  };

  const plugin: Plugin = {
    id: "sorted-panels-test",
    name: "Sorted Panels Test",
    version: "1.0.0",
    uiPanels: [panel1, panel2, panel3],
  };

  await pluginManager.register(plugin);

  const panels = pluginManager.getUIPanels();
  const testPanels = panels.filter((p) =>
    ["panel-1", "panel-2", "panel-3"].includes(p.id)
  );

  // Should be sorted by order: panel-2 (10), panel-3 (20), panel-1 (30)
  assertEquals(testPanels[0].id, "panel-2");
  assertEquals(testPanels[1].id, "panel-3");
  assertEquals(testPanels[2].id, "panel-1");

  // Cleanup
  await pluginManager.unregister("sorted-panels-test");
});
