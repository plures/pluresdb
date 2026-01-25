<script lang="ts">
  import { createEventDispatcher } from "svelte";
  import { push as toast } from "../lib/toasts";
  
  const dispatch = createEventDispatcher();
  
  let loadingDatasets = new Set<string>();
  let clearingData = false;
  let loadedDataset: string | null = null;
  
  const datasets = [
    {
      id: "users",
      name: "User Profiles",
      description: "Sample user data with profiles, preferences, and relationships",
      count: 50
    },
    {
      id: "products",
      name: "E-Commerce Products",
      description: "Product catalog with categories, prices, and reviews",
      count: 100
    },
    {
      id: "social",
      name: "Social Graph",
      description: "Social network with users, posts, likes, and follows",
      count: 150
    },
    {
      id: "documents",
      name: "Document Collection",
      description: "Text documents with vector embeddings for similarity search",
      count: 75
    }
  ];
  
  async function loadDataset(datasetId: string) {
    loadingDatasets.add(datasetId);
    loadingDatasets = loadingDatasets; // trigger reactivity
    try {
      const response = await fetch(`/api/examples/${datasetId}`, {
        method: "POST"
      });
      
      if (!response.ok) {
        throw new Error(`Failed to load dataset: ${response.statusText}`);
      }
      
      const result = await response.json();
      loadedDataset = datasetId;
      toast(`Successfully loaded ${result.count} nodes from ${datasetId} dataset`, "success");
      dispatch("loaded", { dataset: datasetId, count: result.count });
      
      // Refresh the page data
      window.location.reload();
    } catch (error: any) {
      console.error("Error loading dataset:", error);
      toast(`Failed to load dataset: ${error.message}`, "error");
    } finally {
      loadingDatasets.delete(datasetId);
      loadingDatasets = loadingDatasets; // trigger reactivity
    }
  }
  
  async function clearAllData() {
    if (!confirm("Are you sure you want to delete all nodes? This cannot be undone.")) {
      return;
    }
    
    clearingData = true;
    try {
      const response = await fetch("/api/data/clear", {
        method: "POST"
      });
      
      if (!response.ok) {
        throw new Error(`Failed to clear data: ${response.statusText}`);
      }
      
      loadedDataset = null;
      toast("All data cleared successfully", "success");
      window.location.reload();
    } catch (error: any) {
      console.error("Error clearing data:", error);
      toast(`Failed to clear data: ${error.message}`, "error");
    } finally {
      clearingData = false;
    }
  }
</script>

<div class="example-datasets" role="region" aria-labelledby="datasets-heading">
  <h3 id="datasets-heading">Example Datasets</h3>
  <p>Load pre-built datasets to explore PluresDB features</p>
  
  <div class="datasets-grid">
    {#each datasets as dataset}
      <div class="dataset-card">
        <h4>{dataset.name}</h4>
        <p>{dataset.description}</p>
        <p class="dataset-count">{dataset.count} nodes</p>
        <button
          on:click={() => loadDataset(dataset.id)}
          disabled={loadingDatasets.has(dataset.id) || clearingData}
          aria-label={`Load ${dataset.name} dataset`}
        >
          {loadingDatasets.has(dataset.id) ? "Loading..." : "Load Dataset"}
        </button>
      </div>
    {/each}
  </div>
  
  <div class="actions">
    <button
      class="secondary"
      on:click={clearAllData}
      disabled={loadingDatasets.size > 0 || clearingData}
      aria-label="Clear all data"
    >
      {clearingData ? "Clearing..." : "Clear All Data"}
    </button>
  </div>
</div>

<style>
  .example-datasets {
    padding: 1rem;
  }
  
  .datasets-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(250px, 1fr));
    gap: 1rem;
    margin: 1.5rem 0;
  }
  
  .dataset-card {
    border: 1px solid var(--pico-muted-border-color);
    border-radius: 8px;
    padding: 1rem;
    background: var(--pico-card-background-color);
  }
  
  .dataset-card h4 {
    margin-top: 0;
    color: var(--pico-primary);
  }
  
  .dataset-card p {
    color: var(--pico-muted-color);
    font-size: 0.9rem;
    margin-bottom: 0.5rem;
  }
  
  .dataset-count {
    font-weight: 600;
    color: var(--pico-contrast);
  }
  
  .dataset-card button {
    width: 100%;
    margin-top: 0.5rem;
  }
  
  .actions {
    margin-top: 2rem;
    text-align: center;
  }
</style>
