<script lang="ts">
  import { selectedId, nodes } from '../lib/stores'
  import VirtualList from './VirtualList.svelte'
  let filter = ''
  let sortBy: 'id' | 'type' = 'id'
  let sortDesc = false
  function select(id: string){ selectedId.set(id) }
  
  $: items = Object.values($nodes)
    .filter(it => it.id.toLowerCase().includes(filter.toLowerCase()))
    .sort((a,b)=>{
      let aVal: any, bVal: any
      if(sortBy === 'id'){ aVal = a.id; bVal = b.id }
      else if(sortBy === 'type'){ aVal = a.data?.type ?? ''; bVal = b.data?.type ?? '' }
      const cmp = typeof aVal === 'string' ? aVal.localeCompare(bVal) : aVal - bVal
      return sortDesc ? -cmp : cmp
    })
  
  async function createNode(){
    const id = prompt('New node id:')
    if(!id) return
    await fetch('/api/put', { method:'POST', headers:{'content-type':'application/json'}, body: JSON.stringify({ id, data: {} }) })
    selectedId.set(id)
  }

  function handleKeydown(e: KeyboardEvent, id: string){
    if(e.key === 'Enter' || e.key === ' '){
      e.preventDefault()
      select(id)
    }
    else if(e.key === 'ArrowDown'){
      e.preventDefault()
      const idx = items.findIndex(it => it.id === id)
      if(idx < items.length - 1) select(items[idx+1].id)
    }
    else if(e.key === 'ArrowUp'){
      e.preventDefault()
      const idx = items.findIndex(it => it.id === id)
      if(idx > 0) select(items[idx-1].id)
    }
  }

  function toggleSort(field: typeof sortBy){
    if(sortBy === field) sortDesc = !sortDesc
    else{ sortBy = field; sortDesc = false }
  }
</script>

<section aria-labelledby="nodes-heading">
  <h3 id="nodes-heading">Nodes</h3>
  <label for="filter-input" class="sr-only">Filter nodes</label>
  <input id="filter-input" placeholder="Filter" bind:value={filter} aria-label="Filter nodes by ID" />
  
  <div role="toolbar" aria-label="Sort controls" class="sort-controls">
    <button 
      class="secondary outline compact" 
      on:click={() => toggleSort('id')}
      aria-label="Sort by ID {sortBy === 'id' ? (sortDesc ? 'descending' : 'ascending') : ''}"
      aria-pressed={sortBy === 'id'}
    >
      ID {sortBy === 'id' ? (sortDesc ? '↓' : '↑') : ''}
    </button>
    <button 
      class="secondary outline compact" 
      on:click={() => toggleSort('type')}
      aria-label="Sort by type {sortBy === 'type' ? (sortDesc ? 'descending' : 'ascending') : ''}"
      aria-pressed={sortBy === 'type'}
    >
      Type {sortBy === 'type' ? (sortDesc ? '↓' : '↑') : ''}
    </button>
  </div>

  <div role="listbox" aria-label="Available nodes" tabindex="0">
    <VirtualList {items} itemHeight={36} height={420}>
      <svelte:fragment let:visible let:startIndex>
        {#each visible as it, i}
          <button 
            role="option"
            aria-selected={$selectedId===it.id}
            class:selected={$selectedId===it.id} 
            on:click={() => select(it.id)}
            on:keydown={(e) => handleKeydown(e, it.id)}
            style="display:block; width:100%; text-align:left; padding:.3rem .5rem;"
          >
            {it.id}
          </button>
        {/each}
      </svelte:fragment>
    </VirtualList>
  </div>
  <button on:click={createNode} aria-label="Create new node">Create</button>
</section>

<style>
  button.selected { background: var(--pico-primary-background); color: var(--pico-primary-inverse) }
  .sr-only { position: absolute; width: 1px; height: 1px; padding: 0; margin: -1px; overflow: hidden; clip: rect(0,0,0,0); white-space: nowrap; border-width: 0; }
  .sort-controls { display: flex; gap: .5rem; margin: .5rem 0; }
  .compact { padding: .25rem .5rem; font-size: .85em; }
</style>

