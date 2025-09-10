<script lang="ts">
  import { selected } from '../lib/stores'
  let text = ''
  let timer: any
  $: text = JSON.stringify($selected?.data ?? {}, null, 2)
  function debounced(){ clearTimeout(timer); timer = setTimeout(save, 350) }
  async function save(){
    if(!$selected) return
    let data: any; try{ data = JSON.parse(text) }catch{return}
    await fetch('/api/put', { method:'POST', headers:{'content-type':'application/json'}, body: JSON.stringify({ id: $selected.id, data }) })
  }
  async function del(){
    if(!$selected) return
    await fetch('/api/delete?id='+encodeURIComponent($selected.id))
  }
</script>

<h3>Details</h3>
{#if $selected}
  <label>Id</label>
  <input value={$selected.id} disabled />
  <label>JSON</label>
  <textarea rows="18" bind:value={text} on:input={debounced}></textarea>
  <div><button class="secondary" on:click={del}>Delete</button></div>
{:else}
  <p class="muted">Select a node</p>
{/if}

<style>
  .muted { color: var(--pico-muted-color) }
</style>

