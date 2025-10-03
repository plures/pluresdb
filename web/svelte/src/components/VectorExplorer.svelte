<script lang="ts">
  import { onMount } from 'svelte'
  import { push as toast } from '../lib/toasts'
  import JsonEditor from './JsonEditor.svelte'
  
  let searchQuery = ''
  let searchResults: Array<{
    id: string
    data: Record<string, unknown>
    score: number
    vector?: number[]
  }> = []
  let selectedNode: any = null
  let nearestNeighbors: Array<{
    id: string
    data: Record<string, unknown>
    score: number
    distance: number
  }> = []
  let loading = false
  let dark = false
  let showRawVector = false
  let vectorDimensions = 0
  let indexType = 'brute-force'
  let searchLimit = 10
  
  $: dark = (document.documentElement.getAttribute('data-theme') === 'dark')
  
  onMount(() => {
    loadVectorStats()
  })
  
  async function loadVectorStats() {
    try {
      const res = await fetch('/api/list')
      const nodes = await res.json()
      
      // Find a node with vector data to get dimensions
      const nodeWithVector = nodes.find((n: any) => n.data.vector && Array.isArray(n.data.vector))
      if (nodeWithVector) {
        vectorDimensions = nodeWithVector.data.vector.length
      }
    } catch (error) {
      console.error('Error loading vector stats:', error)
    }
  }
  
  async function performVectorSearch() {
    if (!searchQuery.trim()) return
    
    loading = true
    try {
      const res = await fetch('/api/vsearch', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ 
          query: searchQuery, 
          limit: searchLimit 
        })
      })
      
      if (!res.ok) throw new Error('Vector search failed')
      
      searchResults = await res.json()
      
      if (searchResults.length > 0) {
        toast(`Found ${searchResults.length} similar nodes`, 'success')
        // Load nearest neighbors for the first result
        await loadNearestNeighbors(searchResults[0].id)
      } else {
        toast('No similar nodes found', 'info')
      }
    } catch (error) {
      toast('Vector search failed', 'error')
      console.error('Error performing vector search:', error)
    } finally {
      loading = false
    }
  }
  
  async function loadNearestNeighbors(nodeId: string) {
    try {
      const res = await fetch(`/api/vsearch`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ 
          query: nodeId, 
          limit: 5,
          excludeSelf: true
        })
      })
      
      if (res.ok) {
        nearestNeighbors = await res.json()
      }
    } catch (error) {
      console.error('Error loading nearest neighbors:', error)
    }
  }
  
  async function selectNode(node: any) {
    selectedNode = node
    await loadNearestNeighbors(node.id)
  }
  
  function formatVector(vector: number[]): string {
    if (!vector || vector.length === 0) return 'No vector data'
    
    const preview = vector.slice(0, 10)
    const remaining = vector.length - 10
    
    let result = `[${preview.join(', ')}`
    if (remaining > 0) {
      result += `, ... (+${remaining} more)`
    }
    result += ']'
    
    return result
  }
  
  function calculateVectorStats(vector: number[]): {
    magnitude: number
    mean: number
    min: number
    max: number
  } {
    if (!vector || vector.length === 0) {
      return { magnitude: 0, mean: 0, min: 0, max: 0 }
    }
    
    const magnitude = Math.sqrt(vector.reduce((sum, val) => sum + val * val, 0))
    const mean = vector.reduce((sum, val) => sum + val, 0) / vector.length
    const min = Math.min(...vector)
    const max = Math.max(...vector)
    
    return { magnitude, mean, min, max }
  }
  
  function exportVectorData() {
    if (!selectedNode) return
    
    const data = {
      nodeId: selectedNode.id,
      vector: selectedNode.vector,
      stats: calculateVectorStats(selectedNode.vector || []),
      nearestNeighbors: nearestNeighbors.map(n => ({
        id: n.id,
        score: n.score,
        distance: n.distance
      }))
    }
    
    const blob = new Blob([JSON.stringify(data, null, 2)], { type: 'application/json' })
    const url = URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.href = url
    a.download = `vector-data-${selectedNode.id}-${Date.now()}.json`
    document.body.appendChild(a)
    a.click()
    document.body.removeChild(a)
    URL.revokeObjectURL(url)
    
    toast('Vector data exported', 'success')
  }
  
  function visualizeVector(vector: number[]): string {
    if (!vector || vector.length === 0) return ''
    
    // Create a simple ASCII visualization of the vector
    const maxVal = Math.max(...vector.map(Math.abs))
    const normalized = vector.map(v => v / maxVal)
    
    return normalized.map(v => {
      const bar = '█'.repeat(Math.floor((v + 1) * 10))
      const spaces = ' '.repeat(20 - bar.length)
      return `${bar}${spaces}${v.toFixed(3)}`
    }).join('\n')
  }
</script>

<section aria-labelledby="vector-heading">
  <h3 id="vector-heading">Vector Explorer</h3>
  
  <div class="vector-controls">
    <div class="search-section">
      <div class="input-group">
        <label for="vector-search">Vector Search</label>
        <input 
          id="vector-search"
          type="text" 
          bind:value={searchQuery} 
          placeholder="Enter text or node ID to find similar nodes..."
          on:keydown={(e) => e.key === 'Enter' && performVectorSearch()}
        />
        <button 
          on:click={performVectorSearch} 
          disabled={loading || !searchQuery.trim()}
        >
          {loading ? 'Searching...' : 'Search'}
        </button>
      </div>
      
      <div class="search-options">
        <div class="option-group">
          <label for="search-limit">Results Limit</label>
          <select id="search-limit" bind:value={searchLimit}>
            <option value={5}>5</option>
            <option value={10}>10</option>
            <option value={20}>20</option>
            <option value={50}>50</option>
          </select>
        </div>
        
        <div class="option-group">
          <label for="index-type">Index Type</label>
          <select id="index-type" bind:value={indexType}>
            <option value="brute-force">Brute Force</option>
            <option value="hnsw">HNSW (Future)</option>
          </select>
        </div>
      </div>
    </div>
  </div>
  
  <div class="vector-results">
    {#if searchResults.length > 0}
      <div class="search-results">
        <h4>Search Results ({searchResults.length})</h4>
        <div class="result-list">
          {#each searchResults as result, index}
            <button
              class="result-item"
              class:selected={selectedNode?.id === result.id}
              on:click={() => selectNode(result)}
              on:keydown={(e) => e.key === 'Enter' && selectNode(result)}
            >
              <div class="result-header">
                <span class="result-id">{result.id}</span>
                <span class="result-score">Score: {result.score.toFixed(3)}</span>
              </div>
              <div class="result-preview">
                {JSON.stringify(result.data).substring(0, 100)}...
              </div>
            </button>
          {/each}
        </div>
      </div>
    {/if}
    
    {#if selectedNode}
      <div class="node-details">
        <div class="node-header">
          <h4>Node Details: {selectedNode.id}</h4>
          <div class="node-actions">
            <button on:click={exportVectorData} class="secondary">
              Export Vector Data
            </button>
            <label>
              <input type="checkbox" bind:checked={showRawVector} />
              Show Raw Vector
            </label>
          </div>
        </div>
        
        <div class="node-content">
          <div class="vector-section">
            <h5>Vector Information</h5>
            {#if selectedNode.vector}
              {@const stats = calculateVectorStats(selectedNode.vector)}
              <div class="vector-stats">
                <div class="stat-item">
                  <span class="stat-label">Dimensions:</span>
                  <span class="stat-value">{selectedNode.vector.length}</span>
                </div>
                <div class="stat-item">
                  <span class="stat-label">Magnitude:</span>
                  <span class="stat-value">{stats.magnitude.toFixed(3)}</span>
                </div>
                <div class="stat-item">
                  <span class="stat-label">Mean:</span>
                  <span class="stat-value">{stats.mean.toFixed(3)}</span>
                </div>
                <div class="stat-item">
                  <span class="stat-label">Range:</span>
                  <span class="stat-value">{stats.min.toFixed(3)} to {stats.max.toFixed(3)}</span>
                </div>
              </div>
              
              {#if showRawVector}
                <div class="vector-raw">
                  <h6>Raw Vector Data</h6>
                  <pre class="vector-preview">{formatVector(selectedNode.vector)}</pre>
                </div>
              {/if}
            {:else}
              <p class="no-vector">No vector data available for this node</p>
            {/if}
          </div>
          
          <div class="neighbors-section">
            <h5>Nearest Neighbors ({nearestNeighbors.length})</h5>
            {#if nearestNeighbors.length > 0}
              <div class="neighbors-list">
                {#each nearestNeighbors as neighbor}
                  <div class="neighbor-item">
                    <div class="neighbor-header">
                      <span class="neighbor-id">{neighbor.id}</span>
                      <span class="neighbor-score">Score: {neighbor.score.toFixed(3)}</span>
                    </div>
                    <div class="neighbor-preview">
                      {JSON.stringify(neighbor.data).substring(0, 80)}...
                    </div>
                  </div>
                {/each}
              </div>
            {:else}
              <p class="no-neighbors">No nearest neighbors found</p>
            {/if}
          </div>
          
          <div class="data-section">
            <h5>Node Data</h5>
            <div role="region" aria-label="Node data editor">
              <JsonEditor 
                {dark} 
                value={JSON.stringify(selectedNode.data, null, 2)}
                onChange={() => {}} 
              />
            </div>
          </div>
        </div>
      </div>
    {/if}
  </div>
  
  <div class="vector-info">
    <p>
      <strong>Vector Dimensions:</strong> {vectorDimensions} | 
      <strong>Index Type:</strong> {indexType} | 
      <strong>Search Results:</strong> {searchResults.length}
    </p>
  </div>
</section>

<style>
  .vector-controls {
    margin-bottom: 1.5rem;
  }
  
  .search-section {
    display: flex;
    flex-direction: column;
    gap: 1rem;
    padding: 1rem;
    background: var(--pico-muted-border-color);
    border-radius: 8px;
  }
  
  .input-group {
    display: flex;
    gap: 0.5rem;
    align-items: end;
  }
  
  .input-group input {
    flex: 1;
  }
  
  .search-options {
    display: flex;
    gap: 1rem;
    flex-wrap: wrap;
  }
  
  .option-group {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    min-width: 120px;
  }
  
  .option-group label {
    font-size: 0.875rem;
    font-weight: 500;
  }
  
  .vector-results {
    display: grid;
    grid-template-columns: 1fr 2fr;
    gap: 1rem;
    min-height: 400px;
  }
  
  .search-results, .node-details {
    border: 1px solid var(--pico-muted-border-color);
    border-radius: 8px;
    padding: 1rem;
    overflow-y: auto;
    max-height: 600px;
  }
  
  .result-list {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }
  
  .result-item {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    width: 100%;
    padding: 0.75rem;
    text-align: left;
    border: 1px solid var(--pico-muted-border-color);
    border-radius: 4px;
    background: transparent;
    cursor: pointer;
  }
  
  .result-item:hover {
    background: var(--pico-muted-border-color);
  }
  
  .result-item.selected {
    background: var(--pico-primary);
    color: var(--pico-primary-inverse);
  }
  
  .result-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    width: 100%;
    margin-bottom: 0.25rem;
  }
  
  .result-id {
    font-weight: 600;
    font-size: 0.875rem;
  }
  
  .result-score {
    font-size: 0.75rem;
    opacity: 0.8;
  }
  
  .result-preview {
    font-size: 0.75rem;
    opacity: 0.7;
    font-family: monospace;
  }
  
  .node-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 1rem;
  }
  
  .node-actions {
    display: flex;
    gap: 0.5rem;
    align-items: center;
  }
  
  .vector-stats {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
    gap: 0.75rem;
    margin-bottom: 1rem;
  }
  
  .stat-item {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }
  
  .stat-label {
    font-size: 0.875rem;
    font-weight: 500;
    color: var(--pico-muted-color);
  }
  
  .stat-value {
    font-family: monospace;
    font-size: 0.875rem;
  }
  
  .vector-raw {
    margin-top: 1rem;
  }
  
  .vector-preview {
    background: var(--pico-muted-border-color);
    padding: 0.75rem;
    border-radius: 4px;
    font-family: monospace;
    font-size: 0.75rem;
    overflow-x: auto;
    max-height: 200px;
    overflow-y: auto;
  }
  
  .neighbors-list {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    max-height: 200px;
    overflow-y: auto;
  }
  
  .neighbor-item {
    padding: 0.5rem;
    border: 1px solid var(--pico-muted-border-color);
    border-radius: 4px;
    background: rgba(0, 0, 0, 0.02);
  }
  
  .neighbor-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 0.25rem;
  }
  
  .neighbor-id {
    font-weight: 600;
    font-size: 0.875rem;
  }
  
  .neighbor-score {
    font-size: 0.75rem;
    color: var(--pico-muted-color);
  }
  
  .neighbor-preview {
    font-size: 0.75rem;
    color: var(--pico-muted-color);
    font-family: monospace;
  }
  
  .no-vector, .no-neighbors {
    color: var(--pico-muted-color);
    font-style: italic;
    text-align: center;
    padding: 1rem;
  }
  
  .vector-info {
    margin-top: 1rem;
    padding: 0.75rem;
    background: var(--pico-muted-border-color);
    border-radius: 4px;
    font-size: 0.875rem;
  }
  
  @media (max-width: 768px) {
    .vector-results {
      grid-template-columns: 1fr;
    }
    
    .search-options {
      flex-direction: column;
    }
    
    .option-group {
      min-width: auto;
    }
  }
</style>
