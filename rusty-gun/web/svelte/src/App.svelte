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
</svelte:head>

<main class="container">
  <nav>
    <ul>
      <li><strong>Rusty Gun</strong></li>
    </ul>
    <ul>
      <li><a role="button" class:secondary={showSettings} on:click={() => showSettings=false}>Data</a></li>
      <li><a role="button" class:secondary={!showSettings} on:click={() => showSettings=true}>Settings</a></li>
      <li><label><input type="checkbox" role="switch" bind:checked={dark} on:change={toggleTheme} /> Dark</label></li>
    </ul>
  </nav>

  {#if !showSettings}
    <div class="grid">
      <section>
        <NodeList />
      </section>
      <section>
        <NodeDetail />
        <SearchPanel />
      </section>
    </div>
  {:else}
    <SettingsPanel />
  {/if}

  <Toasts />
</main>

<style>
  main { margin-top: 1rem; }
  nav { margin-bottom: 0.5rem; }
  section { padding: 0.75rem; }
</style>






