<script lang="ts">
  import { selected } from '../lib/stores'
  import { push as toast } from '../lib/toasts'
  import JsonEditor from './JsonEditor.svelte'
  let text = ''
  let timer: any
  let dark = false
  $: text = JSON.stringify($selected?.data ?? {}, null, 2)
  $: dark = (document.documentElement.getAttribute('data-theme') === 'dark')
  function debounced(){ clearTimeout(timer); timer = setTimeout(save, 350) }
  function onEditorChange(v: string){ text = v; debounced() }
  async function save(){
    if(!$selected) return
    let data: any; try{ data = JSON.parse(text) }catch{ toast('Invalid JSON','error'); return }
    try{
      await fetch('/api/put', { method:'POST', headers:{'content-type':'application/json'}, body: JSON.stringify({ id: $selected.id, data }) })
      toast('Saved','success')
    }catch{ toast('Save failed','error') }
  }
  async function del(){
    if(!$selected) return
    try{
      await fetch('/api/delete?id='+encodeURIComponent($selected.id))
      toast('Deleted','success')
    }catch{ toast('Delete failed','error') }
  }
</script>

<h3>Details</h3>
{#if $selected}
  <label for="id">Id</label>
  <input id="id" value={$selected.id} disabled />
  <label for="json">JSON</label>
  <JsonEditor {dark} value={text} onChange={onEditorChange} />
  <div><button class="secondary" on:click={del}>Delete</button></div>
{:else}
  <p class="muted">Select a node</p>
{/if}

<style>
  .muted { color: var(--pico-muted-color) }
</style>

