<script lang="ts">
  import { onMount } from "svelte";
  import { push as toast } from "../lib/toasts";
  import JsonEditor from "./JsonEditor.svelte";

  let nodeId = "";
  let nodeData: any = null;
  let loading = false;
  let dark = false;
  let showFieldStates = true;
  let showVectorClock = true;

  $: dark = document.documentElement.getAttribute("data-theme") === "dark";

  async function loadNode() {
    if (!nodeId.trim()) return;

    loading = true;
    try {
      const res = await fetch(`/api/get?id=${encodeURIComponent(nodeId)}`);
      if (!res.ok) throw new Error("Failed to load node");
      nodeData = await res.json();
    } catch (error) {
      toast("Failed to load node", "error");
      console.error("Error loading node:", error);
    } finally {
      loading = false;
    }
  }

  function formatTimestamp(timestamp: number): string {
    return new Date(timestamp).toLocaleString();
  }

  function getFieldStates() {
    if (!nodeData?.state) return [];

    return Object.entries(nodeData.state)
      .map(([field, timestamp]) => ({
        field,
        timestamp: timestamp as number,
        age: Date.now() - (timestamp as number),
        formatted: formatTimestamp(timestamp as number),
      }))
      .sort((a, b) => b.timestamp - a.timestamp);
  }

  function getVectorClockEntries() {
    if (!nodeData?.vectorClock) return [];

    return Object.entries(nodeData.vectorClock)
      .map(([peerId, clock]) => ({
        peerId,
        clock: clock as number,
      }))
      .sort((a, b) => b.clock - a.clock);
  }

  function getConflicts() {
    if (!nodeData?.state) return [];

    const conflicts = [];
    const fieldStates = getFieldStates();
    const now = Date.now();

    // Find fields that might have conflicts (same timestamp from different peers)
    const fieldGroups = new Map<string, typeof fieldStates>();
    for (const field of fieldStates) {
      if (!fieldGroups.has(field.field)) {
        fieldGroups.set(field.field, []);
      }
      fieldGroups.get(field.field)!.push(field);
    }

    for (const [field, states] of fieldGroups) {
      if (states.length > 1) {
        // Check if there are multiple states with the same timestamp
        const timestampGroups = new Map<number, typeof states>();
        for (const state of states) {
          if (!timestampGroups.has(state.timestamp)) {
            timestampGroups.set(state.timestamp, []);
          }
          timestampGroups.get(state.timestamp)!.push(state);
        }

        for (const [timestamp, sameTimeStates] of timestampGroups) {
          if (sameTimeStates.length > 1) {
            conflicts.push({
              field,
              timestamp,
              count: sameTimeStates.length,
              states: sameTimeStates,
            });
          }
        }
      }
    }

    return conflicts;
  }

  function getMergeInfo() {
    if (!nodeData) return null;

    return {
      lastMerge: nodeData.timestamp,
      totalFields: Object.keys(nodeData.data || {}).length,
      fieldStates: Object.keys(nodeData.state || {}).length,
      peers: Object.keys(nodeData.vectorClock || {}).length,
      hasVector: !!nodeData.vector,
      vectorLength: nodeData.vector?.length || 0,
    };
  }

  function forceResolveConflict(field: string, resolution: "keep" | "delete") {
    if (!nodeData) return;

    const newData = { ...nodeData.data };
    if (resolution === "delete") {
      delete newData[field];
    }

    // This would need a new API endpoint to force resolve conflicts
    toast("Conflict resolution not yet implemented", "info");
  }
</script>

<section aria-labelledby="crdt-heading">
  <h3 id="crdt-heading">CRDT Inspector</h3>

  <div class="crdt-controls">
    <div class="input-group">
      <label for="node-id-input">Node ID</label>
      <input
        id="node-id-input"
        type="text"
        bind:value={nodeId}
        placeholder="Enter node ID to inspect CRDT state"
        on:keydown={(e) => e.key === "Enter" && loadNode()}
      />
      <button on:click={loadNode} disabled={loading || !nodeId.trim()}>
        {loading ? "Loading..." : "Inspect Node"}
      </button>
    </div>

    {#if nodeData}
      <div class="view-controls">
        <label>
          <input type="checkbox" bind:checked={showFieldStates} />
          Field States
        </label>
        <label>
          <input type="checkbox" bind:checked={showVectorClock} />
          Vector Clock
        </label>
      </div>
    {/if}
  </div>

  {#if nodeData}
    <div class="crdt-grid">
      <!-- Merge Information -->
      <div class="merge-info">
        <h4>Merge Information</h4>
        <div class="info-grid">
          {#if getMergeInfo()}
            {@const info = getMergeInfo()}
            <div class="info-item">
              <span class="label">Last Merge:</span>
              <span class="value">{formatTimestamp(info.lastMerge)}</span>
            </div>
            <div class="info-item">
              <span class="label">Total Fields:</span>
              <span class="value">{info.totalFields}</span>
            </div>
            <div class="info-item">
              <span class="label">Field States:</span>
              <span class="value">{info.fieldStates}</span>
            </div>
            <div class="info-item">
              <span class="label">Peers:</span>
              <span class="value">{info.peers}</span>
            </div>
            <div class="info-item">
              <span class="label">Vector Length:</span>
              <span class="value">{info.vectorLength}</span>
            </div>
          {/if}
        </div>
      </div>

      <!-- Conflicts -->
      {#if getConflicts().length > 0}
        <div class="conflicts-section">
          <h4>Conflicts ({getConflicts().length})</h4>
          <div class="conflict-list">
            {#each getConflicts() as conflict}
              <div class="conflict-item">
                <div class="conflict-header">
                  <code class="field-name">{conflict.field}</code>
                  <span class="conflict-count"
                    >{conflict.count} states at {formatTimestamp(conflict.timestamp)}</span
                  >
                </div>
                <div class="conflict-actions">
                  <button
                    on:click={() => forceResolveConflict(conflict.field, "keep")}
                    class="keep-button"
                    aria-label="Keep field {conflict.field}"
                  >
                    Keep
                  </button>
                  <button
                    on:click={() => forceResolveConflict(conflict.field, "delete")}
                    class="delete-button"
                    aria-label="Delete field {conflict.field}"
                  >
                    Delete
                  </button>
                </div>
              </div>
            {/each}
          </div>
        </div>
      {/if}

      <!-- Field States -->
      {#if showFieldStates}
        <div class="field-states">
          <h4>Field States</h4>
          <div class="field-list">
            {#each getFieldStates() as field}
              <div class="field-item">
                <code class="field-name">{field.field}</code>
                <span class="field-timestamp">{field.formatted}</span>
                <span class="field-age">{Math.round(field.age / 1000)}s ago</span>
              </div>
            {/each}
          </div>
        </div>
      {/if}

      <!-- Vector Clock -->
      {#if showVectorClock}
        <div class="vector-clock">
          <h4>Vector Clock</h4>
          <div class="clock-list">
            {#each getVectorClockEntries() as entry}
              <div class="clock-item">
                <code class="peer-id">{entry.peerId}</code>
                <span class="clock-value">{entry.clock}</span>
              </div>
            {/each}
          </div>
        </div>
      {/if}

      <!-- Raw Data -->
      <div class="raw-data">
        <h4>Raw Node Data</h4>
        <div role="region" aria-label="Raw node data">
          <JsonEditor {dark} value={JSON.stringify(nodeData, null, 2)} onChange={() => {}} />
        </div>
      </div>
    </div>
  {:else if nodeId && !loading}
    <p class="muted">Node not found</p>
  {/if}
</section>

<style>
  .crdt-controls {
    display: flex;
    gap: 1rem;
    margin-bottom: 1rem;
    align-items: end;
  }

  .input-group {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    flex: 1;
  }

  .view-controls {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .crdt-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
    gap: 1rem;
  }

  .merge-info,
  .conflicts-section,
  .field-states,
  .vector-clock,
  .raw-data {
    border: 1px solid var(--pico-muted-border-color);
    border-radius: 8px;
    padding: 1rem;
  }

  .info-grid {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .info-item {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .info-item .label {
    font-weight: 500;
    color: var(--pico-muted-color);
  }

  .info-item .value {
    font-family: monospace;
    font-size: 0.875rem;
  }

  .conflict-list {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .conflict-item {
    padding: 0.75rem;
    border: 1px solid var(--error-color);
    border-radius: 4px;
    background: rgba(207, 34, 46, 0.05);
  }

  .conflict-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 0.5rem;
  }

  .field-name {
    font-weight: 600;
    color: var(--pico-primary);
  }

  .conflict-count {
    font-size: 0.875rem;
    color: var(--error-color);
  }

  .conflict-actions {
    display: flex;
    gap: 0.5rem;
  }

  .keep-button {
    background: var(--success-color);
    color: white;
    border: none;
    padding: 0.25rem 0.75rem;
    border-radius: 3px;
    font-size: 0.875rem;
  }

  .delete-button {
    background: var(--error-color);
    color: white;
    border: none;
    padding: 0.25rem 0.75rem;
    border-radius: 3px;
    font-size: 0.875rem;
  }

  .field-list,
  .clock-list {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    max-height: 200px;
    overflow-y: auto;
  }

  .field-item,
  .clock-item {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.5rem;
    border: 1px solid var(--pico-muted-border-color);
    border-radius: 4px;
    background: rgba(0, 0, 0, 0.02);
  }

  .field-timestamp,
  .field-age,
  .clock-value {
    font-size: 0.875rem;
    color: var(--pico-muted-color);
  }

  .peer-id {
    font-family: monospace;
    font-size: 0.875rem;
  }

  .raw-data {
    grid-column: 1 / -1;
  }

  .muted {
    color: var(--pico-muted-color);
    font-style: italic;
  }

  @media (max-width: 768px) {
    .crdt-controls {
      flex-direction: column;
      align-items: stretch;
    }

    .crdt-grid {
      grid-template-columns: 1fr;
    }
  }
</style>
