<script lang="ts">
  import { selected } from '../lib/stores'
  import { push as toast } from '../lib/toasts'
  import JsonEditor from './JsonEditor.svelte'
  import Ajv from 'ajv'
  let text = ''
  let timer: any
  let dark = false
  let schemaText = ''
  const ajv = new Ajv({ allErrors: true, strict: false })

  $: text = JSON.stringify($selected?.data ?? {}, null, 2)
  $: dark = (document.documentElement.getAttribute('data-theme') === 'dark')
  function debounced(){ clearTimeout(timer); timer = setTimeout(save, 350) }
  function onEditorChange(v: string){ text = v; debounced() }
  function pretty(){ try{ text = JSON.stringify(JSON.parse(text), null, 2) }catch{ toast('Invalid JSON','error') } }
  function compact(){ try{ text = JSON.stringify(JSON.parse(text)) }catch{ toast('Invalid JSON','error') } }
  function validate(){
    let data: any; let schema: any
    try{ data = JSON.parse(text) }catch{ toast('Invalid JSON','error'); return }
    if(!schemaText.trim()){ toast('No schema provided','info'); return }
    try{ schema = JSON.parse(schemaText) }catch{ toast('Invalid schema JSON','error'); return }
    const validate = ajv.compile(schema)
    const ok = validate(data)
    if(ok) toast('Valid against schema','success')
    else toast('Schema validation failed: '+ajv.errorsText(validate.errors),'error')
  }
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
  <div class="row">
    <button class="secondary" on:click={pretty}>Pretty</button>
    <button class="secondary" on:click={compact}>Compact</button>
    <button class="secondary" on:click={validate}>Validate</button>
    <button class="secondary" on:click={del}>Delete</button>
  </div>
  <label for="json">JSON</label>
  <JsonEditor {dark} value={text} onChange={onEditorChange} />
  <label for="schema">Schema (optional)</label>
  <textarea id="schema" rows="6" bind:value={schemaText} placeholder='&#123;"type":"object","properties":&#123;&#125;&#125;'></textarea>
{:else}
  <p class="muted">Select a node</p>
{/if}

<style>
  .muted { color: var(--pico-muted-color) }
  .row { display:flex; gap: .5rem; margin:.5rem 0 }
</style>

