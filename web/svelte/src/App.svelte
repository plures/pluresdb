<script lang="ts">
  import { onMount } from "svelte";
  import { nodes, selectedId, selected, settings, upsertNode, removeNode } from "./lib/stores";
  import NodeList from "./components/NodeList.svelte";
  import NodeDetail from "./components/NodeDetail.svelte";
  import SearchPanel from "./components/SearchPanel.svelte";
  import SettingsPanel from "./components/SettingsPanel.svelte";
  import TypeExplorer from "./components/TypeExplorer.svelte";
  import HistoryViewer from "./components/HistoryViewer.svelte";
  import CRDTInspector from "./components/CRDTInspector.svelte";
  import ImportExport from "./components/ImportExport.svelte";
  import GraphView from "./components/GraphView.svelte";
  import VectorExplorer from "./components/VectorExplorer.svelte";
  import FacetedSearch from "./components/FacetedSearch.svelte";
  import Notebooks from "./components/Notebooks.svelte";
  import QueryBuilder from "./components/QueryBuilder.svelte";
  import RulesBuilder from "./components/RulesBuilder.svelte";
  import TasksScheduler from "./components/TasksScheduler.svelte";
  import MeshPanel from "./components/MeshPanel.svelte";
  import StorageIndexes from "./components/StorageIndexes.svelte";
  import Profiling from "./components/Profiling.svelte";
  import SecurityPanel from "./components/SecurityPanel.svelte";
  import PackagingPanel from "./components/PackagingPanel.svelte";
  import BillingPanel from "./components/BillingPanel.svelte";
  import SQLiteCompatibility from "./components/SQLiteCompatibility.svelte";
  import P2PEcosystem from "./components/P2PEcosystem.svelte";
  import IdentityDiscovery from "./components/IdentityDiscovery.svelte";
  import EncryptedSharing from "./components/EncryptedSharing.svelte";
  import CrossDeviceSync from "./components/CrossDeviceSync.svelte";
  import Toasts from "./components/Toasts.svelte";
  import GuidedTour from "./components/GuidedTour.svelte";
  import ExampleDatasets from "./components/ExampleDatasets.svelte";
  let activeView:
    | "data"
    | "types"
    | "history"
    | "crdt"
    | "import"
    | "graph"
    | "vector"
    | "faceted"
    | "notebooks"
    | "queries"
    | "rules"
    | "tasks"
    | "mesh"
    | "storage"
    | "profiling"
    | "security"
    | "packaging"
    | "billing"
    | "sqlite"
    | "p2p"
    | "identity"
    | "sharing"
    | "sync"
    | "settings"
    | "examples" = "data";
  let dark = false;

  function handleViewChange(view: string) {
    activeView = view as typeof activeView;
  }

  async function loadConfig() {
    const res = await fetch("/api/config");
    const cfg = await res.json();
    settings.set(cfg);
    dark = (cfg as any).dark === true;
    applyTheme();
  }
  function applyTheme() {
    document.documentElement.setAttribute("data-theme", dark ? "dark" : "light");
  }
  async function toggleTheme() {
    dark = !dark;
    applyTheme();
    const res = await fetch("/api/config");
    const cfg: any = await res.json();
    cfg.dark = dark;
    await fetch("/api/config", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(cfg),
    });
  }

  onMount(async () => {
    await loadConfig();
    const es = new EventSource("/api/events");
    es.onmessage = (ev) => {
      const e = JSON.parse(ev.data);
      if (e.node) {
        upsertNode({ id: e.id, data: e.node.data });
      } else {
        removeNode(e.id);
        selectedId.update((id) => (id === e.id ? null : id));
      }
    };
  });
</script>

<svelte:head>
  <link rel="stylesheet" href="https://unpkg.com/@picocss/pico@2.0.6/css/pico.min.css" />
  <style>
    /* WCAG AA Compliant Color Overrides */
    :root {
      --pico-contrast: #0d1117;
      --pico-muted-color: #57606a;
      --pico-primary: #0969da;
      --pico-primary-hover: #0550ae;
      --success-color: #1a7f37;
      --error-color: #cf222e;
    }
    [data-theme="dark"] {
      --pico-contrast: #f6f8fa;
      --pico-muted-color: #8b949e;
      --pico-primary: #58a6ff;
      --pico-primary-hover: #79c0ff;
      --success-color: #3fb950;
      --error-color: #f85149;
    }
    button {
      font-weight: 500;
    }
    *:focus-visible {
      outline: 2px solid var(--pico-primary);
      outline-offset: 2px;
    }
  </style>
</svelte:head>

<main class="container">
  <nav aria-label="Main navigation">
    <ul>
      <li><strong>PluresDB</strong></li>
    </ul>
    <ul role="menubar" aria-label="View selection">
      <li role="none">
        <button
          role="menuitem"
          class:secondary={activeView !== "data"}
          on:click={() => (activeView = "data")}
          aria-current={activeView === "data" ? "page" : undefined}
        >
          Data
        </button>
      </li>
      <li role="none">
        <button
          role="menuitem"
          class:secondary={activeView !== "types"}
          on:click={() => (activeView = "types")}
          aria-current={activeView === "types" ? "page" : undefined}
        >
          Types
        </button>
      </li>
      <li role="none">
        <button
          role="menuitem"
          class:secondary={activeView !== "history"}
          on:click={() => (activeView = "history")}
          aria-current={activeView === "history" ? "page" : undefined}
        >
          History
        </button>
      </li>
      <li role="none">
        <button
          role="menuitem"
          class:secondary={activeView !== "crdt"}
          on:click={() => (activeView = "crdt")}
          aria-current={activeView === "crdt" ? "page" : undefined}
        >
          CRDT
        </button>
      </li>
      <li role="none">
        <button
          role="menuitem"
          class:secondary={activeView !== "import"}
          on:click={() => (activeView = "import")}
          aria-current={activeView === "import" ? "page" : undefined}
        >
          Import/Export
        </button>
      </li>
      <li role="none">
        <button
          role="menuitem"
          class:secondary={activeView !== "graph"}
          on:click={() => (activeView = "graph")}
          aria-current={activeView === "graph" ? "page" : undefined}
        >
          Graph
        </button>
      </li>
      <li role="none">
        <button
          role="menuitem"
          class:secondary={activeView !== "vector"}
          on:click={() => (activeView = "vector")}
          aria-current={activeView === "vector" ? "page" : undefined}
        >
          Vector
        </button>
      </li>
      <li role="none">
        <button
          role="menuitem"
          class:secondary={activeView !== "faceted"}
          on:click={() => (activeView = "faceted")}
          aria-current={activeView === "faceted" ? "page" : undefined}
        >
          Search
        </button>
      </li>
      <li role="none">
        <button
          role="menuitem"
          class:secondary={activeView !== "notebooks"}
          on:click={() => (activeView = "notebooks")}
          aria-current={activeView === "notebooks" ? "page" : undefined}
        >
          Notebooks
        </button>
      </li>
      <li role="none">
        <button
          role="menuitem"
          class:secondary={activeView !== "queries"}
          on:click={() => (activeView = "queries")}
          aria-current={activeView === "queries" ? "page" : undefined}
        >
          Queries
        </button>
      </li>
      <li role="none">
        <button
          role="menuitem"
          class:secondary={activeView !== "rules"}
          on:click={() => (activeView = "rules")}
          aria-current={activeView === "rules" ? "page" : undefined}
        >
          Rules
        </button>
      </li>
      <li role="none">
        <button
          role="menuitem"
          class:secondary={activeView !== "tasks"}
          on:click={() => (activeView = "tasks")}
          aria-current={activeView === "tasks" ? "page" : undefined}
        >
          Tasks
        </button>
      </li>
      <li role="none">
        <button
          role="menuitem"
          class:secondary={activeView !== "mesh"}
          on:click={() => (activeView = "mesh")}
          aria-current={activeView === "mesh" ? "page" : undefined}
        >
          Mesh
        </button>
      </li>
      <li role="none">
        <button
          role="menuitem"
          class:secondary={activeView !== "storage"}
          on:click={() => (activeView = "storage")}
          aria-current={activeView === "storage" ? "page" : undefined}
        >
          Storage
        </button>
      </li>
      <li role="none">
        <button
          role="menuitem"
          class:secondary={activeView !== "profiling"}
          on:click={() => (activeView = "profiling")}
          aria-current={activeView === "profiling" ? "page" : undefined}
        >
          Profiling
        </button>
      </li>
      <li role="none">
        <button
          role="menuitem"
          class:secondary={activeView !== "security"}
          on:click={() => (activeView = "security")}
          aria-current={activeView === "security" ? "page" : undefined}
        >
          Security
        </button>
      </li>
      <li role="none">
        <button
          role="menuitem"
          class:secondary={activeView !== "packaging"}
          on:click={() => (activeView = "packaging")}
          aria-current={activeView === "packaging" ? "page" : undefined}
        >
          Packaging
        </button>
      </li>
      <li role="none">
        <button
          role="menuitem"
          class:secondary={activeView !== "billing"}
          on:click={() => (activeView = "billing")}
          aria-current={activeView === "billing" ? "page" : undefined}
        >
          Billing
        </button>
      </li>
      <li role="none">
        <button
          role="menuitem"
          class:secondary={activeView !== "sqlite"}
          on:click={() => (activeView = "sqlite")}
          aria-current={activeView === "sqlite" ? "page" : undefined}
        >
          SQLite
        </button>
      </li>
      <li role="none">
        <button
          role="menuitem"
          class:secondary={activeView !== "p2p"}
          on:click={() => (activeView = "p2p")}
          aria-current={activeView === "p2p" ? "page" : undefined}
        >
          P2P
        </button>
      </li>
      <li role="none">
        <button
          role="menuitem"
          class:secondary={activeView !== "identity"}
          on:click={() => (activeView = "identity")}
          aria-current={activeView === "identity" ? "page" : undefined}
        >
          Identity
        </button>
      </li>
      <li role="none">
        <button
          role="menuitem"
          class:secondary={activeView !== "sharing"}
          on:click={() => (activeView = "sharing")}
          aria-current={activeView === "sharing" ? "page" : undefined}
        >
          Sharing
        </button>
      </li>
      <li role="none">
        <button
          role="menuitem"
          class:secondary={activeView !== "sync"}
          on:click={() => (activeView = "sync")}
          aria-current={activeView === "sync" ? "page" : undefined}
        >
          Sync
        </button>
      </li>
      <li role="none">
        <button
          role="menuitem"
          class:secondary={activeView !== "settings"}
          on:click={() => (activeView = "settings")}
          aria-current={activeView === "settings" ? "page" : undefined}
        >
          Settings
        </button>
      </li>
      <li role="none">
        <label>
          <input
            type="checkbox"
            role="switch"
            bind:checked={dark}
            on:change={toggleTheme}
            aria-label="Toggle dark mode"
          />
          Dark
        </label>
      </li>
    </ul>
  </nav>

  {#if activeView === "data"}
    <div class="grid" role="main">
      <section aria-label="Node list and filters">
        <NodeList />
      </section>
      <section aria-label="Node details and search">
        <NodeDetail />
        <SearchPanel />
      </section>
    </div>
  {:else if activeView === "types"}
    <div role="main">
      <TypeExplorer />
    </div>
  {:else if activeView === "history"}
    <div role="main">
      <HistoryViewer />
    </div>
  {:else if activeView === "crdt"}
    <div role="main">
      <CRDTInspector />
    </div>
  {:else if activeView === "import"}
    <div role="main">
      <ImportExport />
    </div>
  {:else if activeView === "graph"}
    <div role="main">
      <GraphView />
    </div>
  {:else if activeView === "vector"}
    <div role="main">
      <VectorExplorer />
    </div>
  {:else if activeView === "faceted"}
    <div role="main">
      <FacetedSearch />
    </div>
  {:else if activeView === "notebooks"}
    <div role="main">
      <Notebooks />
    </div>
  {:else if activeView === "queries"}
    <div role="main">
      <QueryBuilder />
    </div>
  {:else if activeView === "rules"}
    <div role="main">
      <RulesBuilder />
    </div>
  {:else if activeView === "tasks"}
    <div role="main">
      <TasksScheduler />
    </div>
  {:else if activeView === "mesh"}
    <div role="main">
      <MeshPanel />
    </div>
  {:else if activeView === "storage"}
    <div role="main">
      <StorageIndexes />
    </div>
  {:else if activeView === "profiling"}
    <div role="main">
      <Profiling />
    </div>
  {:else if activeView === "security"}
    <div role="main">
      <SecurityPanel />
    </div>
  {:else if activeView === "packaging"}
    <div role="main">
      <PackagingPanel />
    </div>
  {:else if activeView === "billing"}
    <div role="main">
      <BillingPanel />
    </div>
  {:else if activeView === "sqlite"}
    <div role="main">
      <SQLiteCompatibility />
    </div>
  {:else if activeView === "p2p"}
    <div role="main">
      <P2PEcosystem />
    </div>
  {:else if activeView === "identity"}
    <div role="main">
      <IdentityDiscovery />
    </div>
  {:else if activeView === "sharing"}
    <div role="main">
      <EncryptedSharing />
    </div>
  {:else if activeView === "sync"}
    <div role="main">
      <CrossDeviceSync />
    </div>
  {:else if activeView === "settings"}
    <div role="main">
      <SettingsPanel />
    </div>
  {:else if activeView === "examples"}
    <div role="main">
      <ExampleDatasets />
    </div>
  {/if}

  <GuidedTour onViewChange={handleViewChange} />
  <Toasts />
</main>

<style>
  main {
    margin-top: 1rem;
  }
  nav {
    margin-bottom: 0.5rem;
  }
  section {
    padding: 0.75rem;
  }
</style>
