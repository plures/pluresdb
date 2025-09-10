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
</script>

<h3>Vector search</h3>
<input placeholder="Query" bind:value={q} on:input={debounced} />
<div class="stack">
  {#each results as r}
    <button on:click={() => pick(r.id)} class="ghost">{r.id}</button>
  {/each}
</div>

<style>
  .stack { display:flex; flex-direction:column; gap: .25rem }
  .ghost { background: transparent; border: 1px solid var(--pico-muted-border-color) }
  .ghost:hover { background: var(--pico-muted-border-color) }
</style>

