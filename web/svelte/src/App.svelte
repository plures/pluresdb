<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { db } from "./lib/state.svelte.ts";
  import { Sidebar, TitleBar, StatusBar, StatusBarItem, StatusBarSpacer, Tabs } from "@plures/design-dojo/layout";
  import "@plures/design-dojo/tokens.css";
  import "./styles/a11y.css";
  import type { Tab } from "@plures/design-dojo/layout";
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
  const tabs = [
    { key: "data",      label: "🗄 Data" },
    { key: "graph",     label: "🕸 Graph" },
    { key: "vector",    label: "🔍 Vector" },
    { key: "faceted",   label: "🔎 Search" },
    { key: "queries",   label: "📝 Queries" },
    { key: "notebooks", label: "📓 Notebooks" },
    { key: "types",     label: "🏷 Types" },
    { key: "rules",     label: "📋 Rules" },
    { key: "tasks",     label: "⏱ Tasks" },
    { key: "history",   label: "🕐 History" },
    { key: "crdt",      label: "🔀 CRDT" },
    { key: "import",    label: "📦 Import/Export" },
    { key: "mesh",      label: "🌐 Mesh" },
    { key: "storage",   label: "💾 Storage" },
    { key: "profiling", label: "📊 Profiling" },
    { key: "p2p",       label: "🔗 P2P" },
    { key: "identity",  label: "🪪 Identity" },
    { key: "sharing",   label: "🔒 Sharing" },
    { key: "sync",      label: "🔄 Sync" },
    { key: "security",  label: "🛡 Security" },
    { key: "packaging", label: "📫 Packaging" },
    { key: "billing",   label: "💳 Billing" },
    { key: "sqlite",    label: "🗃 SQLite" },
    { key: "settings",  label: "⚙ Settings" },
  ] as const satisfies Tab[];

  type ViewKey = (typeof tabs)[number]["key"];

  let activeView = $state<ViewKey>("data");
  let dark = $state(false);
  let sidebarCollapsed = $state(false);
  const nodeCount = $derived(Object.keys(db.nodes).length);

  function handleViewChange(view: string) {
    activeView = view as ViewKey;
  }

  async function loadConfig() {
    const res = await fetch("/api/config");
    const cfg = await res.json();
    db.settings = cfg;
    dark = cfg.dark === true;
    db.settings.dark = dark;
    applyTheme();
  }

  function applyTheme() {
    document.documentElement.setAttribute("data-theme", dark ? "dark" : "light");
  }

  async function toggleTheme() {
    dark = !dark;
    db.settings.dark = dark;
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

  let eventSource: EventSource | null = null;

  onMount(async () => {
    await loadConfig();

    // Close any existing connection before creating a new one (defensive for HMR/remounts)
    if (eventSource) {
      eventSource.close();
      eventSource = null;
    }

    eventSource = new EventSource("/api/events");
    eventSource.onmessage = (ev) => {
      const e = JSON.parse(ev.data);
      if (e.node) {
        db.upsertNode({ id: e.id, data: e.node.data });
      } else {
        db.removeNode(e.id);
        if (db.selectedId === e.id) db.selectedId = null;
      }
    };
    eventSource.onerror = (err) => {
      console.error("EventSource connection error", err);
    };
  });

  onDestroy(() => {
    if (eventSource) {
      eventSource.close();
      eventSource = null;
    }
  });
</script>

<TitleBar title="PluresDB">
  {#snippet children()}
    <button
      onclick={toggleTheme}
      aria-label="Toggle dark mode"
      title={dark ? "Switch to light mode" : "Switch to dark mode"}
      class="theme-toggle"
    >
      {dark ? "☀" : "🌙"}
    </button>
  {/snippet}
</TitleBar>

<Tabs {tabs} activeTab={activeView} ontabchange={handleViewChange}>
  {#snippet children({ activeTab })}
    {#if activeTab === "data"}
      <Sidebar
        side="left"
        width={260}
        bind:collapsed={sidebarCollapsed}
        ontoggle={(c) => (sidebarCollapsed = c)}
      >
        {#snippet children()}
          <NodeList />
        {/snippet}
        {#snippet main()}
          <div class="panel" role="main">
            <NodeDetail />
            <SearchPanel />
          </div>
        {/snippet}
      </Sidebar>
    {:else}
      <div class="panel" role="main">
        {#if activeTab === "types"}
          <TypeExplorer />
        {:else if activeTab === "history"}
          <HistoryViewer />
        {:else if activeTab === "crdt"}
          <CRDTInspector />
        {:else if activeTab === "import"}
          <ImportExport />
        {:else if activeTab === "graph"}
          <GraphView />
        {:else if activeTab === "vector"}
          <VectorExplorer />
        {:else if activeTab === "faceted"}
          <FacetedSearch />
        {:else if activeTab === "notebooks"}
          <Notebooks />
        {:else if activeTab === "queries"}
          <QueryBuilder />
        {:else if activeTab === "rules"}
          <RulesBuilder />
        {:else if activeTab === "tasks"}
          <TasksScheduler />
        {:else if activeTab === "mesh"}
          <MeshPanel />
        {:else if activeTab === "storage"}
          <StorageIndexes />
        {:else if activeTab === "profiling"}
          <Profiling />
        {:else if activeTab === "security"}
          <SecurityPanel />
        {:else if activeTab === "packaging"}
          <PackagingPanel />
        {:else if activeTab === "billing"}
          <BillingPanel />
        {:else if activeTab === "sqlite"}
          <SQLiteCompatibility />
        {:else if activeTab === "p2p"}
          <P2PEcosystem />
        {:else if activeTab === "identity"}
          <IdentityDiscovery />
        {:else if activeTab === "sharing"}
          <EncryptedSharing />
        {:else if activeTab === "sync"}
          <CrossDeviceSync />
        {:else if activeTab === "settings"}
          <SettingsPanel />
        {/if}
      </div>
    {/if}
  {/snippet}
</Tabs>

<StatusBar>
  {#snippet children()}
    <StatusBarItem label="nodes" value={String(nodeCount)} color="accent" />
    {#if db.selectedId}
      <StatusBarItem label="selected" value={db.selectedId} />
    {/if}
    <StatusBarSpacer />
    <StatusBarItem value={dark ? "dark" : "light"} />
  {/snippet}
</StatusBar>

<GuidedTour onViewChange={handleViewChange} />
<Toasts />

<style>
  :global(body) {
    margin: 0;
    padding: 0;
    font-family: var(--font-sans, system-ui, sans-serif);
    background: var(--color-bg, #fff);
    color: var(--color-fg, #111);
  }

  .panel {
    padding: 1rem;
    overflow: auto;
  }

  .theme-toggle {
    background: transparent;
    border: none;
    cursor: pointer;
    font-size: 1.1rem;
    padding: 0.25rem 0.5rem;
    border-radius: var(--radius-sm, 4px);
    color: inherit;
  }

  .theme-toggle:hover {
    background: var(--color-surface-hover, rgba(0, 0, 0, 0.05));
  }
</style>
