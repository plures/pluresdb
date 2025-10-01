<script lang="ts">
  import { selected } from '../lib/stores'
  import { push as toast } from '../lib/toasts'
  import JsonEditor from './JsonEditor.svelte'
  import Ajv from 'ajv'
  let text = ''
  let originalText = ''
  let timer: any
  let dark = false
  let schemaText = ''
  const ajv = new Ajv({ allErrors: true, strict: false })

  $: {
    const newText = JSON.stringify($selected?.data ?? {}, null, 2)
    if(text === '' || text === originalText) {
      text = newText
      originalText = newText
    } else if($selected?.id !== previousId) {
      text = newText
      originalText = newText
    }
  }
  let previousId: string | undefined
  $: previousId = $selected?.id

  $: dark = (document.documentElement.getAttribute('data-theme') === 'dark')
  $: hasChanges = text !== originalText
  
  function debounced(){ clearTimeout(timer); timer = setTimeout(save, 350) }
  function onEditorChange(v: string){ text = v; debounced() }
  function pretty(){ try{ text = JSON.stringify(JSON.parse(text), null, 2) }catch{ toast('Invalid JSON','error') } }
  function compact(){ try{ text = JSON.stringify(JSON.parse(text)) }catch{ toast('Invalid JSON','error') } }
  function revert(){ 
    text = originalText
    toast('Changes reverted','info') 
  }
  function copyAsCurl(){
    if(!$selected) return
    let data: any
    try{ data = JSON.parse(text) }catch{ toast('Invalid JSON','error'); return }
    const host = window.location.origin
    const curl = `curl -X POST ${host}/api/put -H "Content-Type: application/json" -d '${JSON.stringify({ id: $selected.id, data })}'`
    navigator.clipboard.writeText(curl)
    toast('cURL copied to clipboard','success')
  }
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
      originalText = text
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

<section aria-labelledby="detail-heading">
  <h3 id="detail-heading">Details</h3>
  {#if $selected}
    <label for="id">Id</label>
    <input id="id" value={$selected.id} disabled aria-label="Node ID (read-only)" />
    <div role="toolbar" aria-label="Editor actions" class="row">
      <button class="secondary" on:click={pretty} aria-label="Format JSON with indentation" title="Format JSON with indentation">Pretty</button>
      <button class="secondary" on:click={compact} aria-label="Format JSON compactly" title="Format JSON compactly">Compact</button>
      <button class="secondary" on:click={copyAsCurl} aria-label="Copy as cURL command" title="Copy as cURL command">Copy cURL</button>
      <button class="secondary" on:click={revert} disabled={!hasChanges} aria-label="Revert unsaved changes" title="Revert unsaved changes">Revert</button>
    </div>
    <div role="toolbar" aria-label="Node actions" class="row">
      <button class="secondary" on:click={validate} aria-label="Validate JSON against schema" title="Validate against schema">Validate</button>
      <button class="secondary outline" on:click={del} aria-label="Delete this node" title="Delete this node">Delete</button>
    </div>
    <label for="json">JSON</label>
    <div role="region" aria-label="JSON editor">
      <JsonEditor {dark} value={text} schema={schemaText} onChange={onEditorChange} />
    </div>
    <label for="schema">Schema (optional)</label>
    <textarea 
      id="schema" 
      rows="6" 
      bind:value={schemaText} 
      placeholder='&#123;"type":"object","properties":&#123;&#125;&#125;'
      aria-label="JSON Schema for validation"
      aria-describedby="schema-help"
    ></textarea>
    <span id="schema-help" class="sr-only">Enter a JSON Schema to validate the node data against</span>
  {:else}
    <p class="muted" role="status">Select a node</p>
  {/if}
</section>

<style>
  .muted { color: var(--pico-muted-color) }
  .row { display:flex; gap: .5rem; margin:.5rem 0 }
  .sr-only { position: absolute; width: 1px; height: 1px; padding: 0; margin: -1px; overflow: hidden; clip: rect(0,0,0,0); white-space: nowrap; border-width: 0; }
</style>

