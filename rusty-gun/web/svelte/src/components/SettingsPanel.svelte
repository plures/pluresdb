<script lang="ts">
  import { settings } from '../lib/stores'
  let peersText = ''
  $: peersText = ($settings.peers && $settings.peers.join(',')) || ''
  let timer: any
  function debounced(){ clearTimeout(timer); timer = setTimeout(save, 250) }
  async function save(){
    await fetch('/api/config', { method:'POST', headers:{'content-type':'application/json'}, body: JSON.stringify($settings) })
  }
  function onPeers(e: Event){
    const t = e.target as HTMLInputElement
    peersText = t.value
    settings.update(s => ({ ...s, peers: peersText.split(',').map(x=>x.trim()).filter(Boolean) }))
    debounced()
  }
</script>

<article>
  <h3>Settings</h3>
  <label>KV Path</label>
  <input bind:value={$settings.kvPath} on:input={debounced} placeholder="/data/rg.sqlite" />
  <div class="grid">
    <div>
      <label>Port</label>
      <input type="number" bind:value={$settings.port} on:input={debounced} />
    </div>
    <div>
      <label>API Offset</label>
      <input type="number" bind:value={$settings.apiPortOffset} on:input={debounced} />
    </div>
  </div>
  <label>Peers (comma separated)</label>
  <input bind:value={peersText} on:input={onPeers} />
</article>

