<script lang="ts">
  import { db } from "../lib/state.svelte.ts";
  let peersText = $state((db.settings.peers ?? []).join(","));
  let saveStatus = $state("");
  let timer: ReturnType<typeof setTimeout> | undefined = undefined;

  $effect(() => {
    peersText = (db.settings.peers ?? []).join(",");
  });

  function debounced() {
    clearTimeout(timer);
    timer = setTimeout(save, 250);
  }
  async function save() {
    saveStatus = "saving";
    try {
      const res = await fetch("/api/config", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify(db.settings),
      });
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      saveStatus = "saved";
    } catch {
      saveStatus = "error";
    }
    setTimeout(() => (saveStatus = ""), 2000);
  }
  function onPeers(e: Event) {
    const t = e.target as HTMLInputElement;
    peersText = t.value;
    db.settings = {
      ...db.settings,
      peers: peersText
        .split(",")
        .map((x) => x.trim())
        .filter(Boolean),
    };
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
      bind:value={db.settings.kvPath}
      oninput={debounced}
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
          bind:value={db.settings.port}
          oninput={debounced}
          aria-label="WebSocket server port"
        />
      </div>
      <div>
        <label for="apiOffset">API Offset</label>
        <input
          id="apiOffset"
          type="number"
          bind:value={db.settings.apiPortOffset}
          oninput={debounced}
          aria-label="HTTP API port offset from WebSocket port"
        />
      </div>
    </div>
    <label for="peers">Peers (comma separated)</label>
    <input
      id="peers"
      bind:value={peersText}
      oninput={onPeers}
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
