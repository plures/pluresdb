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
  <label for="kvPath">KV Path</label>
  <input id="kvPath" bind:value={$settings.kvPath} on:input={debounced} placeholder="/data/rg.sqlite" />
  <div class="grid">
    <div>
      <label for="port">Port</label>
      <input id="port" type="number" bind:value={$settings.port} on:input={debounced} />
    </div>
    <div>
      <label for="apiOffset">API Offset</label>
      <input id="apiOffset" type="number" bind:value={$settings.apiPortOffset} on:input={debounced} />
    </div>
  </div>
  <label for="peers">Peers (comma separated)</label>
  <input id="peers" bind:value={peersText} on:input={onPeers} />
</article>

