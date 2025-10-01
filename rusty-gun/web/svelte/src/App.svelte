<script lang="ts">
  import { onMount } from 'svelte'
  import { nodes, selectedId, selected, settings, upsertNode, removeNode } from './lib/stores'
  import NodeList from './components/NodeList.svelte'
  import NodeDetail from './components/NodeDetail.svelte'
  import SearchPanel from './components/SearchPanel.svelte'
  import SettingsPanel from './components/SettingsPanel.svelte'
  import TypeExplorer from './components/TypeExplorer.svelte'
  import HistoryViewer from './components/HistoryViewer.svelte'
  import CRDTInspector from './components/CRDTInspector.svelte'
  import ImportExport from './components/ImportExport.svelte'
  import Toasts from './components/Toasts.svelte'
  let activeView: 'data' | 'types' | 'history' | 'crdt' | 'import' | 'settings' = 'data'
  let dark = false

  async function loadConfig(){
    const res = await fetch('/api/config')
    const cfg = await res.json()
    settings.set(cfg)
    dark = (cfg as any).dark === true
    applyTheme()
  }
  function applyTheme(){
    document.documentElement.setAttribute('data-theme', dark ? 'dark' : 'light')
  }
  async function toggleTheme(){
    dark = !dark
    applyTheme()
    const res = await fetch('/api/config')
    const cfg: any = await res.json()
    cfg.dark = dark
    await fetch('/api/config', { method:'POST', headers:{'content-type':'application/json'}, body: JSON.stringify(cfg) })
  }

  onMount(async () => {
    await loadConfig()
    const es = new EventSource('/api/events')
    es.onmessage = (ev) => {
      const e = JSON.parse(ev.data)
      if(e.node){
        upsertNode({ id: e.id, data: e.node.data })
      } else {
        removeNode(e.id)
        selectedId.update(id => id===e.id ? null : id)
      }
    }
  })
</script>

<svelte:head>
  <link rel="stylesheet" href="https://unpkg.com/@picocss/pico@2.0.6/css/pico.min.css" />
  <style>
    {`
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
    button { font-weight: 500; }
    *:focus-visible {
      outline: 2px solid var(--pico-primary);
      outline-offset: 2px;
    }
    `}
  </style>
</svelte:head>

<main class="container">
  <nav aria-label="Main navigation">
    <ul>
      <li><strong>Rusty Gun</strong></li>
    </ul>
    <ul role="menubar" aria-label="View selection">
      <li role="none">
        <button 
          role="menuitem"
          class:secondary={activeView !== 'data'} 
          on:click={() => activeView = 'data'}
          aria-current={activeView === 'data' ? 'page' : undefined}
        >
          Data
        </button>
      </li>
      <li role="none">
        <button 
          role="menuitem"
          class:secondary={activeView !== 'types'} 
          on:click={() => activeView = 'types'}
          aria-current={activeView === 'types' ? 'page' : undefined}
        >
          Types
        </button>
      </li>
      <li role="none">
        <button 
          role="menuitem"
          class:secondary={activeView !== 'history'} 
          on:click={() => activeView = 'history'}
          aria-current={activeView === 'history' ? 'page' : undefined}
        >
          History
        </button>
      </li>
      <li role="none">
        <button 
          role="menuitem"
          class:secondary={activeView !== 'crdt'} 
          on:click={() => activeView = 'crdt'}
          aria-current={activeView === 'crdt' ? 'page' : undefined}
        >
          CRDT
        </button>
      </li>
      <li role="none">
        <button 
          role="menuitem"
          class:secondary={activeView !== 'import'} 
          on:click={() => activeView = 'import'}
          aria-current={activeView === 'import' ? 'page' : undefined}
        >
          Import/Export
        </button>
      </li>
      <li role="none">
        <button 
          role="menuitem"
          class:secondary={activeView !== 'settings'} 
          on:click={() => activeView = 'settings'}
          aria-current={activeView === 'settings' ? 'page' : undefined}
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

  {#if activeView === 'data'}
    <div class="grid" role="main">
      <section aria-label="Node list and filters">
        <NodeList />
      </section>
      <section aria-label="Node details and search">
        <NodeDetail />
        <SearchPanel />
      </section>
    </div>
  {:else if activeView === 'types'}
    <div role="main">
      <TypeExplorer />
    </div>
  {:else if activeView === 'history'}
    <div role="main">
      <HistoryViewer />
    </div>
  {:else if activeView === 'crdt'}
    <div role="main">
      <CRDTInspector />
    </div>
  {:else if activeView === 'import'}
    <div role="main">
      <ImportExport />
    </div>
  {:else if activeView === 'settings'}
    <div role="main">
      <SettingsPanel />
    </div>
  {/if}

  <Toasts />
</main>

<style>
  main { margin-top: 1rem; }
  nav { margin-bottom: 0.5rem; }
  section { padding: 0.75rem; }
</style>






