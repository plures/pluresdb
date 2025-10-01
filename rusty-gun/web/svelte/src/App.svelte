<script lang="ts">
  import { onMount } from 'svelte'
  import { nodes, selectedId, selected, settings, upsertNode, removeNode } from './lib/stores'
  import NodeList from './components/NodeList.svelte'
  import NodeDetail from './components/NodeDetail.svelte'
  import SearchPanel from './components/SearchPanel.svelte'
  import SettingsPanel from './components/SettingsPanel.svelte'
  import Toasts from './components/Toasts.svelte'
  let showSettings = false
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
          class:secondary={showSettings} 
          on:click={() => showSettings=false}
          aria-current={!showSettings ? 'page' : undefined}
        >
          Data
        </button>
      </li>
      <li role="none">
        <button 
          role="menuitem"
          class:secondary={!showSettings} 
          on:click={() => showSettings=true}
          aria-current={showSettings ? 'page' : undefined}
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

  {#if !showSettings}
    <div class="grid" role="main">
      <section aria-label="Node list and filters">
        <NodeList />
      </section>
      <section aria-label="Node details and search">
        <NodeDetail />
        <SearchPanel />
      </section>
    </div>
  {:else}
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






