<script lang="ts">
  import { onMount } from "svelte";
  import { push as toast } from "../lib/toasts";
  import JsonEditor from "./JsonEditor.svelte";
  import Ajv from "ajv";

  let types: Array<{ name: string; count: number; schema?: any }> = [];
  let selectedType: string | null = null;
  let typeInstances: Array<{ id: string; data: Record<string, unknown> }> = [];
  let schemaText = "";
  let schemaValid = true;
  let schemaError = "";
  let dark = false;
  let loading = false;
  const ajv = new Ajv({ allErrors: true, strict: false });

  $: dark = document.documentElement.getAttribute("data-theme") === "dark";

  onMount(() => {
    loadTypes();
  });

  async function loadTypes() {
    loading = true;
    try {
      // Get all nodes and group by type
      const res = await fetch("/api/list");
      const allNodes = await res.json();

      const typeMap = new Map<string, number>();
      const typeSchemas = new Map<string, any>();

      for (const node of allNodes) {
        const type = node.data?.type || "Untyped";
        typeMap.set(type, (typeMap.get(type) || 0) + 1);

        // Check if node has a schema
        if (node.data?.schema) {
          typeSchemas.set(type, node.data.schema);
        }
      }

      types = Array.from(typeMap.entries())
        .map(([name, count]) => ({
          name,
          count,
          schema: typeSchemas.get(name),
        }))
        .sort((a, b) => a.name.localeCompare(b.name));
    } catch (error) {
      toast("Failed to load types", "error");
      console.error("Error loading types:", error);
    } finally {
      loading = false;
    }
  }

  async function selectType(typeName: string) {
    selectedType = typeName;
    loading = true;
    try {
      const res = await fetch(`/api/instances?type=${encodeURIComponent(typeName)}`);
      typeInstances = await res.json();

      // Load schema if it exists
      const schemaNode = typeInstances.find((n) => n.data?.schema);
      if (schemaNode?.data?.schema) {
        schemaText = JSON.stringify(schemaNode.data.schema, null, 2);
        validateSchema();
      } else {
        schemaText = "";
        schemaValid = true;
        schemaError = "";
      }
    } catch (error) {
      toast("Failed to load type instances", "error");
      console.error("Error loading instances:", error);
    } finally {
      loading = false;
    }
  }

  function validateSchema() {
    if (!schemaText.trim()) {
      schemaValid = true;
      schemaError = "";
      return;
    }

    try {
      const schema = JSON.parse(schemaText);
      const validate = ajv.compile(schema);
      schemaValid = true;
      schemaError = "";
    } catch (error: any) {
      schemaValid = false;
      schemaError = error.message;
    }
  }

  async function saveSchema() {
    if (!selectedType || !schemaValid) return;

    try {
      const schema = JSON.parse(schemaText);
      const schemaId = `schema:${selectedType}`;

      await fetch("/api/put", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          id: schemaId,
          data: {
            type: "Schema",
            targetType: selectedType,
            schema: schema,
            createdAt: new Date().toISOString(),
          },
        }),
      });

      toast("Schema saved successfully", "success");
      await loadTypes(); // Refresh to show schema indicator
    } catch (error) {
      toast("Failed to save schema", "error");
      console.error("Error saving schema:", error);
    }
  }

  async function deleteSchema() {
    if (!selectedType) return;

    try {
      const schemaId = `schema:${selectedType}`;
      await fetch(`/api/delete?id=${encodeURIComponent(schemaId)}`);

      schemaText = "";
      schemaValid = true;
      schemaError = "";
      toast("Schema deleted", "success");
      await loadTypes(); // Refresh to remove schema indicator
    } catch (error) {
      toast("Failed to delete schema", "error");
      console.error("Error deleting schema:", error);
    }
  }

  function handleSchemaChange(value: string) {
    schemaText = value;
    validateSchema();
  }

  function createType() {
    const name = prompt("New type name:");
    if (!name) return;

    // Create a sample instance
    const id = `${name.toLowerCase()}:sample-${Date.now()}`;
    fetch("/api/put", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        id,
        data: { type: name, name: "Sample Instance" },
      }),
    })
      .then(() => {
        toast("Type created with sample instance", "success");
        loadTypes();
      })
      .catch(() => {
        toast("Failed to create type", "error");
      });
  }
</script>

<section aria-labelledby="types-heading">
  <h3 id="types-heading">Type Explorer</h3>

  <div class="type-controls">
    <button on:click={loadTypes} disabled={loading} aria-label="Refresh types">
      {loading ? "Loading..." : "Refresh"}
    </button>
    <button on:click={createType} class="secondary" aria-label="Create new type">
      Create Type
    </button>
  </div>

  <div class="grid">
    <!-- Type List -->
    <div class="type-list">
      <h4>Types ({types.length})</h4>
      <div aria-label="Available types">
        {#each types as type}
          <button
            class="type-item"
            class:selected={selectedType === type.name}
            on:click={() => selectType(type.name)}
            on:keydown={(e) => e.key === "Enter" && selectType(type.name)}
            aria-label="Select type {type.name} with {type.count} instances"
          >
            <span class="type-name">{type.name}</span>
            <span class="type-count">{type.count}</span>
            {#if type.schema}
              <span class="schema-indicator" title="Has schema">📋</span>
            {/if}
          </button>
        {/each}
      </div>
    </div>

    <!-- Type Details -->
    <div class="type-details">
      {#if selectedType}
        <h4>Type: {selectedType}</h4>

        <!-- Schema Editor -->
        <div class="schema-section">
          <div class="schema-header">
            <label for="schema-editor">JSON Schema</label>
            <div class="schema-actions">
              <button
                on:click={saveSchema}
                disabled={!schemaValid || !schemaText.trim()}
                class="secondary"
                aria-label="Save schema"
              >
                Save Schema
              </button>
              {#if schemaText.trim()}
                <button
                  on:click={deleteSchema}
                  class="secondary outline"
                  aria-label="Delete schema"
                >
                  Delete
                </button>
              {/if}
            </div>
          </div>

          {#if !schemaValid}
            <div class="validation-error" role="alert">
              Invalid JSON: {schemaError}
            </div>
          {/if}

          <div role="region" aria-label="Schema editor">
            <JsonEditor {dark} value={schemaText} onChange={handleSchemaChange} />
          </div>
        </div>

        <!-- Instances List -->
        <div class="instances-section">
          <h5>Instances ({typeInstances.length})</h5>
          <div class="instances-list">
            {#each typeInstances as instance}
              <div class="instance-item">
                <code class="instance-id">{instance.id}</code>
                <span class="instance-preview">
                  {Object.keys(instance.data).length} properties
                </span>
              </div>
            {/each}
          </div>
        </div>
      {:else}
        <p class="muted">Select a type to view details</p>
      {/if}
    </div>
  </div>
</section>

<style>
  .type-controls {
    display: flex;
    gap: 0.5rem;
    margin-bottom: 1rem;
  }

  .grid {
    display: grid;
    grid-template-columns: 1fr 2fr;
    gap: 1rem;
    min-height: 400px;
  }

  .type-list {
    border: 1px solid var(--pico-muted-border-color);
    border-radius: 8px;
    padding: 0.5rem;
    overflow-y: auto;
    max-height: 400px;
  }

  .type-item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    width: 100%;
    padding: 0.5rem;
    margin-bottom: 0.25rem;
    text-align: left;
    border: 1px solid transparent;
    border-radius: 4px;
    background: transparent;
  }

  .type-item:hover {
    background: var(--pico-muted-border-color);
  }

  .type-item.selected {
    background: var(--pico-primary);
    color: var(--pico-primary-inverse);
  }

  .type-name {
    font-weight: 500;
  }

  .type-count {
    font-size: 0.875rem;
    opacity: 0.7;
  }

  .schema-indicator {
    font-size: 0.875rem;
    margin-left: 0.5rem;
  }

  .type-details {
    border: 1px solid var(--pico-muted-border-color);
    border-radius: 8px;
    padding: 1rem;
    overflow-y: auto;
    max-height: 400px;
  }

  .schema-section {
    margin-bottom: 1.5rem;
  }

  .schema-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 0.5rem;
  }

  .schema-actions {
    display: flex;
    gap: 0.5rem;
  }

  .validation-error {
    color: var(--error-color);
    font-size: 0.875rem;
    margin-bottom: 0.5rem;
    padding: 0.5rem;
    background: rgba(207, 34, 46, 0.1);
    border-radius: 4px;
  }

  .instances-section h5 {
    margin: 0 0 0.5rem 0;
    font-size: 1rem;
  }

  .instances-list {
    max-height: 200px;
    overflow-y: auto;
  }

  .instance-item {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.25rem 0;
    border-bottom: 1px solid var(--pico-muted-border-color);
  }

  .instance-item:last-child {
    border-bottom: none;
  }

  .instance-id {
    font-family: monospace;
    font-size: 0.875rem;
    color: var(--pico-primary);
  }

  .instance-preview {
    font-size: 0.75rem;
    color: var(--pico-muted-color);
  }

  .muted {
    color: var(--pico-muted-color);
    font-style: italic;
  }

  @media (max-width: 768px) {
    .grid {
      grid-template-columns: 1fr;
    }
  }
</style>
