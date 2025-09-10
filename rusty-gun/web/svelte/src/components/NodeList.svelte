<script lang="ts">
  import { selectedId, nodes } from '../lib/stores'
  import VirtualList from './VirtualList.svelte'
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
<VirtualList {items} itemHeight={36} height={420}>
  <svelte:fragment let:visible let:startIndex>
    {#each visible as it, i}
      <button class:selected={$selectedId===it.id} on:click={() => select(it.id)} style="display:block; width:100%; text-align:left; padding:.3rem .5rem;">
        {it.id}
      </button>
    {/each}
  </svelte:fragment>
</VirtualList>
<button on:click={createNode}>Create</button>

<style>
  button.selected { background: var(--pico-primary-background); color: var(--pico-primary-inverse) }
</style>

