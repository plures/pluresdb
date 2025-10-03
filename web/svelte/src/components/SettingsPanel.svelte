<script lang="ts">
  import { settings } from "../lib/stores";
  let peersText = "";
  let saveStatus = "";
  $: peersText = ($settings.peers && $settings.peers.join(",")) || "";
  let timer: any;
  function debounced() {
    clearTimeout(timer);
    timer = setTimeout(save, 250);
  }
  async function save() {
    saveStatus = "saving";
    await fetch("/api/config", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify($settings),
    });
    saveStatus = "saved";
    setTimeout(() => (saveStatus = ""), 2000);
  }
  function onPeers(e: Event) {
    const t = e.target as HTMLInputElement;
    peersText = t.value;
    settings.update((s) => ({
      ...s,
      peers: peersText
        .split(",")
        .map((x) => x.trim())
        .filter(Boolean),
    }));
    debounced();
  }
</script>

<article>
  <section aria-labelledby="settings-heading">
    <h3 id="settings-heading">Settings</h3>
    <div role="status" aria-live="polite" aria-atomic="true" class="sr-only">
      {saveStatus === "saving" ? "Saving settings" : saveStatus === "saved" ? "Settings saved" : ""}
    </div>
    <label for="kvPath">KV Path</label>
    <input
      id="kvPath"
      bind:value={$settings.kvPath}
      on:input={debounced}
      placeholder="/data/rg.sqlite"
      aria-describedby="kvPath-help"
    />
    <span id="kvPath-help" class="sr-only">File path for the Deno KV database</span>
    <div class="grid">
      <div>
        <label for="port">Port</label>
        <input
          id="port"
          type="number"
          bind:value={$settings.port}
          on:input={debounced}
          aria-label="WebSocket server port"
        />
      </div>
      <div>
        <label for="apiOffset">API Offset</label>
        <input
          id="apiOffset"
          type="number"
          bind:value={$settings.apiPortOffset}
          on:input={debounced}
          aria-label="HTTP API port offset from WebSocket port"
        />
      </div>
    </div>
    <label for="peers">Peers (comma separated)</label>
    <input
      id="peers"
      bind:value={peersText}
      on:input={onPeers}
      aria-describedby="peers-help"
      placeholder="ws://localhost:8080, ws://localhost:8081"
    />
    <span id="peers-help" class="sr-only">List of peer WebSocket URLs separated by commas</span>
  </section>
</article>

<style>
  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    padding: 0;
    margin: -1px;
    overflow: hidden;
    clip: rect(0, 0, 0, 0);
    white-space: nowrap;
    border-width: 0;
  }
</style>
