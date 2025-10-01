<script lang="ts">
  import { onMount } from 'svelte'
  import { push as toast } from '../lib/toasts'
  import JsonEditor from './JsonEditor.svelte'
  
  let nodeId = ''
  let history: Array<{
    id: string
    data: Record<string, unknown>
    timestamp: number
    vectorClock: Record<string, number>
    state?: Record<string, number>
  }> = []
  let selectedVersion: number | null = null
  let loading = false
  let dark = false
  let showDiff = false

  $: dark = (document.documentElement.getAttribute('data-theme') === 'dark')

  async function loadHistory() {
    if (!nodeId.trim()) return
    
    loading = true
    try {
      const res = await fetch(`/api/history?id=${encodeURIComponent(nodeId)}`)
      if (!res.ok) throw new Error('Failed to load history')
      history = await res.json()
      
      if (history.length > 0) {
        selectedVersion = history[0].timestamp // Most recent
      }
    } catch (error) {
      toast('Failed to load history', 'error')
      console.error('Error loading history:', error)
    } finally {
      loading = false
    }
  }

  async function restoreVersion(timestamp: number) {
    if (!nodeId || !confirm('Are you sure you want to restore this version? This will overwrite the current data.')) return
    
    try {
      const res = await fetch(`/api/restore?id=${encodeURIComponent(nodeId)}&timestamp=${timestamp}`, {
        method: 'POST'
      })
      
      if (!res.ok) throw new Error('Failed to restore version')
      
      toast('Version restored successfully', 'success')
      await loadHistory() // Reload to show updated history
    } catch (error) {
      toast('Failed to restore version', 'error')
      console.error('Error restoring version:', error)
    }
  }

  function formatTimestamp(timestamp: number): string {
    return new Date(timestamp).toLocaleString()
  }

  function getVersionData(timestamp: number) {
    return history.find(h => h.timestamp === timestamp)?.data || {}
  }

  function getVersionDiff(timestamp: number) {
    const current = history[0]?.data || {}
    const version = getVersionData(timestamp)
    
    const changes: Array<{ field: string; old: any; new: any }> = []
    
    // Check for changes
    const allKeys = new Set([...Object.keys(current), ...Object.keys(version)])
    for (const key of allKeys) {
      const oldVal = current[key]
      const newVal = version[key]
      
      if (JSON.stringify(oldVal) !== JSON.stringify(newVal)) {
        changes.push({ field: key, old: oldVal, new: newVal })
      }
    }
    
    return changes
  }

  function selectVersion(timestamp: number) {
    selectedVersion = timestamp
  }
</script>

<section aria-labelledby="history-heading">
  <h3 id="history-heading">History & Time Travel</h3>
  
  <div class="history-controls">
    <div class="input-group">
      <label for="node-id-input">Node ID</label>
      <input 
        id="node-id-input"
        type="text" 
        bind:value={nodeId} 
        placeholder="Enter node ID to view history"
        on:keydown={(e) => e.key === 'Enter' && loadHistory()}
      />
      <button on:click={loadHistory} disabled={loading || !nodeId.trim()}>
        {loading ? 'Loading...' : 'Load History'}
      </button>
    </div>
    
    {#if history.length > 0}
      <div class="view-controls">
        <label>
          <input type="checkbox" bind:checked={showDiff} />
          Show Diff
        </label>
      </div>
    {/if}
  </div>

  {#if history.length > 0}
    <div class="history-grid">
      <!-- Version List -->
      <div class="version-list">
        <h4>Versions ({history.length})</h4>
        <div class="version-items">
          {#each history as version, i}
            <button
              class="version-item"
              class:selected={selectedVersion === version.timestamp}
              on:click={() => selectVersion(version.timestamp)}
              on:keydown={(e) => e.key === 'Enter' && selectVersion(version.timestamp)}
              aria-label="Select version from {formatTimestamp(version.timestamp)}"
            >
              <div class="version-header">
                <span class="version-number">#{history.length - i}</span>
                <span class="version-time">{formatTimestamp(version.timestamp)}</span>
              </div>
              <div class="version-meta">
                <span class="peer-count">{Object.keys(version.vectorClock).length} peers</span>
                <span class="field-count">{Object.keys(version.data).length} fields</span>
              </div>
              {#if i === 0}
                <span class="current-badge">Current</span>
              {/if}
            </button>
          {/each}
        </div>
      </div>

      <!-- Version Details -->
      <div class="version-details">
        {#if selectedVersion !== null}
          <div class="version-header">
            <h4>Version Details</h4>
            <div class="version-actions">
              {#if selectedVersion !== history[0]?.timestamp}
                <button 
                  on:click={() => restoreVersion(selectedVersion)}
                  class="restore-button"
                  aria-label="Restore this version"
                >
                  Restore Version
                </button>
              {/if}
            </div>
          </div>
          
          <div class="version-content">
            {#if showDiff && selectedVersion !== history[0]?.timestamp}
              <div class="diff-section">
                <h5>Changes from Current</h5>
                <div class="diff-list">
                  {#each getVersionDiff(selectedVersion) as change}
                    <div class="diff-item">
                      <code class="field-name">{change.field}</code>
                      <div class="change-values">
                        <div class="old-value">
                          <span class="label">Old:</span>
                          <code>{JSON.stringify(change.old)}</code>
                        </div>
                        <div class="new-value">
                          <span class="label">New:</span>
                          <code>{JSON.stringify(change.new)}</code>
                        </div>
                      </div>
                    </div>
                  {/each}
                </div>
              </div>
            {/if}
            
            <div class="data-section">
              <h5>Data</h5>
              <div role="region" aria-label="Version data editor">
                <JsonEditor 
                  {dark} 
                  value={JSON.stringify(getVersionData(selectedVersion), null, 2)}
                  onChange={() => {}} 
                />
              </div>
            </div>
            
            <div class="metadata-section">
              <h5>Metadata</h5>
              <div class="metadata-grid">
                <div class="metadata-item">
                  <span class="label">Timestamp:</span>
                  <span class="value">{formatTimestamp(selectedVersion)}</span>
                </div>
                <div class="metadata-item">
                  <span class="label">Vector Clock:</span>
                  <code class="value">{JSON.stringify(history.find(h => h.timestamp === selectedVersion)?.vectorClock || {})}</code>
                </div>
                <div class="metadata-item">
                  <span class="label">Field States:</span>
                  <code class="value">{JSON.stringify(history.find(h => h.timestamp === selectedVersion)?.state || {})}</code>
                </div>
              </div>
            </div>
          </div>
        {:else}
          <p class="muted">Select a version to view details</p>
        {/if}
      </div>
    </div>
  {:else if nodeId && !loading}
    <p class="muted">No history found for this node</p>
  {/if}
</section>

<style>
  .history-controls {
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

  .input-group input {
    flex: 1;
  }

  .view-controls {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .history-grid {
    display: grid;
    grid-template-columns: 1fr 2fr;
    gap: 1rem;
    min-height: 400px;
  }

  .version-list {
    border: 1px solid var(--pico-muted-border-color);
    border-radius: 8px;
    padding: 0.5rem;
    overflow-y: auto;
    max-height: 500px;
  }

  .version-items {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }

  .version-item {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    width: 100%;
    padding: 0.75rem;
    text-align: left;
    border: 1px solid transparent;
    border-radius: 4px;
    background: transparent;
    position: relative;
  }

  .version-item:hover {
    background: var(--pico-muted-border-color);
  }

  .version-item.selected {
    background: var(--pico-primary);
    color: var(--pico-primary-inverse);
  }

  .version-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    width: 100%;
    margin-bottom: 0.25rem;
  }

  .version-number {
    font-weight: 600;
    font-size: 0.875rem;
  }

  .version-time {
    font-size: 0.75rem;
    opacity: 0.8;
  }

  .version-meta {
    display: flex;
    gap: 0.5rem;
    font-size: 0.75rem;
    opacity: 0.7;
  }

  .current-badge {
    position: absolute;
    top: 0.25rem;
    right: 0.25rem;
    background: var(--success-color);
    color: white;
    padding: 0.125rem 0.375rem;
    border-radius: 3px;
    font-size: 0.625rem;
    font-weight: 500;
  }

  .version-details {
    border: 1px solid var(--pico-muted-border-color);
    border-radius: 8px;
    padding: 1rem;
    overflow-y: auto;
    max-height: 500px;
  }

  .version-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 1rem;
  }

  .version-actions {
    display: flex;
    gap: 0.5rem;
  }

  .restore-button {
    background: var(--warning-color);
    color: white;
    border: none;
    padding: 0.5rem 1rem;
    border-radius: 4px;
    font-size: 0.875rem;
  }

  .restore-button:hover {
    opacity: 0.9;
  }

  .diff-section {
    margin-bottom: 1.5rem;
    padding: 1rem;
    background: rgba(0, 0, 0, 0.05);
    border-radius: 4px;
  }

  .diff-list {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .diff-item {
    padding: 0.75rem;
    border: 1px solid var(--pico-muted-border-color);
    border-radius: 4px;
    background: white;
  }

  .field-name {
    display: block;
    font-weight: 600;
    margin-bottom: 0.5rem;
    color: var(--pico-primary);
  }

  .change-values {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }

  .old-value, .new-value {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .old-value .label {
    color: var(--error-color);
    font-weight: 500;
    min-width: 3rem;
  }

  .new-value .label {
    color: var(--success-color);
    font-weight: 500;
    min-width: 3rem;
  }

  .old-value code, .new-value code {
    background: rgba(0, 0, 0, 0.05);
    padding: 0.25rem 0.5rem;
    border-radius: 3px;
    font-size: 0.875rem;
  }

  .data-section, .metadata-section {
    margin-bottom: 1.5rem;
  }

  .metadata-grid {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .metadata-item {
    display: flex;
    align-items: flex-start;
    gap: 0.5rem;
  }

  .metadata-item .label {
    font-weight: 500;
    min-width: 6rem;
    color: var(--pico-muted-color);
  }

  .metadata-item .value {
    font-family: monospace;
    font-size: 0.875rem;
    word-break: break-all;
  }

  .muted {
    color: var(--pico-muted-color);
    font-style: italic;
  }

  @media (max-width: 768px) {
    .history-grid {
      grid-template-columns: 1fr;
    }
    
    .history-controls {
      flex-direction: column;
      align-items: stretch;
    }
  }
</style>
