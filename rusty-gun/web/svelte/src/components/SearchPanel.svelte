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
<ul>
  {#each results as r}
    <li on:click={() => pick(r.id)}>{r.id}</li>
  {/each}
</ul>

<style>
  ul { list-style: none; padding-left: 0; }
  li { padding: .25rem .5rem; cursor:pointer }
  li:hover { text-decoration: underline }
</style>

