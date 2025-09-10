<script lang="ts">
  import { get } from 'svelte/store'
  import { nodes, selectedId } from '../lib/stores'
  let filter = ''
  function select(id: string){ selectedId.set(id) }
  $: items = Object.values($nodes).filter(it => it.id.toLowerCase().includes(filter.toLowerCase())).sort((a,b)=>a.id.localeCompare(b.id))
  async function createNode(){
    const id = prompt('New node id:')
    if(!id) return
    await fetch('/api/put', { method:'POST', headers:{'content-type':'application/json'}, body: JSON.stringify({ id, data: {} }) })
    selectedId.set(id)
  }
</script>

<h3>Nodes</h3>
<input placeholder="Filter" bind:value={filter} />
<ul>
  {#each items as it}
    <li class:selected={$selectedId===it.id} on:click={() => select(it.id)}>{it.id}</li>
  {/each}
  {#if items.length === 0}
    <li class="muted">No nodes</li>
  {/if}
  </ul>
<button on:click={createNode}>Create</button>

<style>
  ul { list-style: none; padding-left: 0; max-height: 60vh; overflow:auto }
  li { padding: .25rem .5rem; cursor:pointer; border-radius: .25rem }
  li.selected { background: var(--pico-primary-background); color: var(--pico-primary-inverse) }
  .muted { color: var(--pico-muted-color) }
</style>

