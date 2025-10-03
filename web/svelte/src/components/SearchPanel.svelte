<script lang="ts">
  import { selectedId } from '../lib/stores'
  let q = ''
  let timer: any
  let results: Array<{ id: string; data: Record<string,unknown> }> = []
  function debounced(){ clearTimeout(timer); timer = setTimeout(search, 250) }
  async function search(){
    const res = await fetch('/api/search?q='+encodeURIComponent(q)+'&k=10')
    results = await res.json()
  }
  function pick(id: string){ selectedId.set(id) }
  function handleResultKeydown(e: KeyboardEvent, id: string){
    if(e.key === 'Enter' || e.key === ' '){
      e.preventDefault()
      pick(id)
    }
  }
</script>

<section aria-labelledby="search-heading">
  <h3 id="search-heading">Vector search</h3>
  <label for="search-query" class="sr-only">Search query</label>
  <input 
    id="search-query" 
    placeholder="Query" 
    bind:value={q} 
    on:input={debounced}
    aria-label="Enter search query for vector search"
    aria-describedby="search-results-count"
  />
  <div 
    id="search-results-count" 
    class="sr-only" 
    role="status" 
    aria-live="polite"
  >
    {results.length} results found
  </div>
  <div class="stack" aria-label="Search results">
    {#each results as r}
      <button 
        on:click={() => pick(r.id)} 
        on:keydown={(e) => handleResultKeydown(e, r.id)}
        class="ghost"
        aria-label="Select node {r.id}"
      >
        {r.id}
      </button>
    {/each}
  </div>
</section>

<style>
  .stack { display:flex; flex-direction:column; gap: .25rem }
  .ghost { background: transparent; border: 1px solid var(--pico-muted-border-color) }
  .ghost:hover { background: var(--pico-muted-border-color) }
  .sr-only { position: absolute; width: 1px; height: 1px; padding: 0; margin: -1px; overflow: hidden; clip: rect(0,0,0,0); white-space: nowrap; border-width: 0; }
</style>

