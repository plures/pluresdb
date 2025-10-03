<script lang="ts">
  import { onMount } from 'svelte'
  import { push as toast } from '../lib/toasts'
  import JsonEditor from './JsonEditor.svelte'
  
  let searchResults: Array<{
    id: string
    data: Record<string, unknown>
    type?: string
    timestamp?: number
  }> = []
  let filteredResults: typeof searchResults = []
  let selectedNode: any = null
  let loading = false
  let dark = false
  
  // Facet filters
  let typeFilter = ''
  let timeFilter = ''
  let tagFilter = ''
  let textFilter = ''
  let dateFrom = ''
  let dateTo = ''
  
  // Available facets
  let availableTypes: string[] = []
  let availableTags: string[] = []
  let timeRanges = [
    { value: '', label: 'All Time' },
    { value: '1h', label: 'Last Hour' },
    { value: '24h', label: 'Last 24 Hours' },
    { value: '7d', label: 'Last 7 Days' },
    { value: '30d', label: 'Last 30 Days' },
    { value: '90d', label: 'Last 90 Days' }
  ]
  
  // Saved searches
  let savedSearches: Array<{
    id: string
    name: string
    filters: Record<string, any>
    createdAt: number
  }> = []
  let searchName = ''
  let showSaveDialog = false
  
  $: dark = (document.documentElement.getAttribute('data-theme') === 'dark')
  $: filteredResults = applyFilters()
  
  onMount(() => {
    loadAllData()
    loadSavedSearches()
  })
  
  async function loadAllData() {
    loading = true
    try {
      const res = await fetch('/api/list')
      const nodes = await res.json()
      
      searchResults = nodes.map((node: any) => ({
        id: node.id,
        data: node.data,
        type: node.data.type || 'unknown',
        timestamp: node.data.timestamp || Date.now()
      }))
      
      extractFacets()
      toast(`Loaded ${searchResults.length} nodes`, 'success')
    } catch (error) {
      toast('Failed to load data', 'error')
      console.error('Error loading data:', error)
    } finally {
      loading = false
    }
  }
  
  function extractFacets() {
    const types = new Set<string>()
    const tags = new Set<string>()
    
    for (const node of searchResults) {
      if (node.type) types.add(node.type)
      
      // Extract tags from data
      for (const [key, value] of Object.entries(node.data)) {
        if (key.toLowerCase().includes('tag') && typeof value === 'string') {
          tags.add(value)
        }
        if (Array.isArray(value) && value.every(v => typeof v === 'string')) {
          value.forEach(tag => tags.add(tag))
        }
      }
    }
    
    availableTypes = Array.from(types).sort()
    availableTags = Array.from(tags).sort()
  }
  
  function applyFilters() {
    let results = [...searchResults]
    
    // Type filter
    if (typeFilter) {
      results = results.filter(node => node.type === typeFilter)
    }
    
    // Time filter
    if (timeFilter) {
      const now = Date.now()
      const timeMap: Record<string, number> = {
        '1h': 60 * 60 * 1000,
        '24h': 24 * 60 * 60 * 1000,
        '7d': 7 * 24 * 60 * 60 * 1000,
        '30d': 30 * 24 * 60 * 60 * 1000,
        '90d': 90 * 24 * 60 * 60 * 1000
      }
      
      if (timeMap[timeFilter]) {
        const cutoff = now - timeMap[timeFilter]
        results = results.filter(node => node.timestamp >= cutoff)
      }
    }
    
    // Date range filter
    if (dateFrom) {
      const fromDate = new Date(dateFrom).getTime()
      results = results.filter(node => node.timestamp >= fromDate)
    }
    
    if (dateTo) {
      const toDate = new Date(dateTo).getTime()
      results = results.filter(node => node.timestamp <= toDate)
    }
    
    // Tag filter
    if (tagFilter) {
      results = results.filter(node => {
        const dataStr = JSON.stringify(node.data).toLowerCase()
        return dataStr.includes(tagFilter.toLowerCase())
      })
    }
    
    // Text filter
    if (textFilter) {
      const query = textFilter.toLowerCase()
      results = results.filter(node => {
        const dataStr = JSON.stringify(node.data).toLowerCase()
        const idStr = node.id.toLowerCase()
        return dataStr.includes(query) || idStr.includes(query)
      })
    }
    
    return results
  }
  
  function clearFilters() {
    typeFilter = ''
    timeFilter = ''
    tagFilter = ''
    textFilter = ''
    dateFrom = ''
    dateTo = ''
  }
  
  function selectNode(node: any) {
    selectedNode = node
  }
  
  function saveSearch() {
    if (!searchName.trim()) {
      toast('Please enter a search name', 'error')
      return
    }
    
    const search = {
      id: `search-${Date.now()}`,
      name: searchName,
      filters: {
        typeFilter,
        timeFilter,
        tagFilter,
        textFilter,
        dateFrom,
        dateTo
      },
      createdAt: Date.now()
    }
    
    savedSearches = [...savedSearches, search]
    localStorage.setItem('pluresdb-saved-searches', JSON.stringify(savedSearches))
    
    searchName = ''
    showSaveDialog = false
    toast('Search saved successfully', 'success')
  }
  
  function loadSavedSearches() {
    try {
      const saved = localStorage.getItem('pluresdb-saved-searches')
      if (saved) {
        savedSearches = JSON.parse(saved)
      }
    } catch (error) {
      console.error('Error loading saved searches:', error)
    }
  }
  
  function applySavedSearch(search: any) {
    typeFilter = search.filters.typeFilter || ''
    timeFilter = search.filters.timeFilter || ''
    tagFilter = search.filters.tagFilter || ''
    textFilter = search.filters.textFilter || ''
    dateFrom = search.filters.dateFrom || ''
    dateTo = search.filters.dateTo || ''
    
    toast(`Applied saved search: ${search.name}`, 'success')
  }
  
  function deleteSavedSearch(searchId: string) {
    savedSearches = savedSearches.filter(s => s.id !== searchId)
    localStorage.setItem('pluresdb-saved-searches', JSON.stringify(savedSearches))
    toast('Search deleted', 'success')
  }
  
  function exportResults() {
    const data = {
      filters: {
        typeFilter,
        timeFilter,
        tagFilter,
        textFilter,
        dateFrom,
        dateTo
      },
      results: filteredResults,
      totalCount: filteredResults.length,
      exportedAt: new Date().toISOString()
    }
    
    const blob = new Blob([JSON.stringify(data, null, 2)], { type: 'application/json' })
    const url = URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.href = url
    a.download = `pluresdb-search-${Date.now()}.json`
    document.body.appendChild(a)
    a.click()
    document.body.removeChild(a)
    URL.revokeObjectURL(url)
    
    toast('Search results exported', 'success')
  }
  
  function formatTimestamp(timestamp: number): string {
    return new Date(timestamp).toLocaleString()
  }
  
  function getQuickActions(node: any) {
    const actions = []
    
    if (node.data.type) {
      actions.push({
        label: `Filter by ${node.data.type}`,
        action: () => {
          typeFilter = node.data.type
        }
      })
    }
    
    if (node.data.tags) {
      actions.push({
        label: 'View tags',
        action: () => {
          tagFilter = Array.isArray(node.data.tags) 
            ? node.data.tags.join(' ') 
            : node.data.tags
        }
      })
    }
    
    return actions
  }
</script>

<section aria-labelledby="faceted-search-heading">
  <h3 id="faceted-search-heading">Faceted Search</h3>
  
  <div class="search-controls">
    <div class="filters-section">
      <div class="filter-group">
        <label for="type-filter">Type</label>
        <select id="type-filter" bind:value={typeFilter}>
          <option value="">All Types</option>
          {#each availableTypes as type}
            <option value={type}>{type}</option>
          {/each}
        </select>
      </div>
      
      <div class="filter-group">
        <label for="time-filter">Time Range</label>
        <select id="time-filter" bind:value={timeFilter}>
          {#each timeRanges as range}
            <option value={range.value}>{range.label}</option>
          {/each}
        </select>
      </div>
      
      <div class="filter-group">
        <label for="tag-filter">Tags</label>
        <input 
          id="tag-filter"
          type="text" 
          bind:value={tagFilter} 
          placeholder="Filter by tags..."
        />
      </div>
      
      <div class="filter-group">
        <label for="text-filter">Text Search</label>
        <input 
          id="text-filter"
          type="text" 
          bind:value={textFilter} 
          placeholder="Search in data..."
        />
      </div>
      
      <div class="filter-group">
        <label for="date-from">From Date</label>
        <input 
          id="date-from"
          type="date" 
          bind:value={dateFrom}
        />
      </div>
      
      <div class="filter-group">
        <label for="date-to">To Date</label>
        <input 
          id="date-to"
          type="date" 
          bind:value={dateTo}
        />
      </div>
    </div>
    
    <div class="actions-section">
      <button on:click={clearFilters} class="secondary">
        Clear Filters
      </button>
      <button on:click={() => showSaveDialog = true} class="secondary">
        Save Search
      </button>
      <button on:click={exportResults} class="secondary" disabled={filteredResults.length === 0}>
        Export Results
      </button>
    </div>
  </div>
  
  {#if showSaveDialog}
    <div class="save-dialog">
      <div class="dialog-content">
        <h4>Save Search</h4>
        <input 
          type="text" 
          bind:value={searchName} 
          placeholder="Enter search name..."
          on:keydown={(e) => e.key === 'Enter' && saveSearch()}
        />
        <div class="dialog-actions">
          <button on:click={saveSearch} disabled={!searchName.trim()}>
            Save
          </button>
          <button on:click={() => showSaveDialog = false} class="secondary">
            Cancel
          </button>
        </div>
      </div>
    </div>
  {/if}
  
  <div class="saved-searches">
    <h4>Saved Searches ({savedSearches.length})</h4>
    {#if savedSearches.length > 0}
      <div class="saved-list">
        {#each savedSearches as search}
          <div class="saved-item">
            <div class="saved-info">
              <span class="saved-name">{search.name}</span>
              <span class="saved-date">{formatTimestamp(search.createdAt)}</span>
            </div>
            <div class="saved-actions">
              <button on:click={() => applySavedSearch(search)} class="small">
                Apply
              </button>
              <button on:click={() => deleteSavedSearch(search.id)} class="small outline">
                Delete
              </button>
            </div>
          </div>
        {/each}
      </div>
    {:else}
      <p class="no-saved">No saved searches yet</p>
    {/if}
  </div>
  
  <div class="search-results">
    <div class="results-header">
      <h4>Results ({filteredResults.length})</h4>
      {#if loading}
        <span class="loading">Loading...</span>
      {/if}
    </div>
    
    <div class="results-grid">
      <div class="results-list">
        {#each filteredResults as node}
          <button
            class="result-item"
            class:selected={selectedNode?.id === node.id}
            on:click={() => selectNode(node)}
            on:keydown={(e) => e.key === 'Enter' && selectNode(node)}
          >
            <div class="result-header">
              <span class="result-id">{node.id}</span>
              <span class="result-type">{node.type}</span>
            </div>
            <div class="result-preview">
              {JSON.stringify(node.data).substring(0, 100)}...
            </div>
            <div class="result-meta">
              <span class="result-time">{formatTimestamp(node.timestamp)}</span>
            </div>
            <div class="quick-actions">
              {#each getQuickActions(node) as action}
                <button 
                  class="quick-action"
                  on:click|stopPropagation={() => action.action()}
                >
                  {action.label}
                </button>
              {/each}
            </div>
          </button>
        {/each}
      </div>
      
      {#if selectedNode}
        <div class="result-detail">
          <div class="detail-header">
            <h5>Node Details: {selectedNode.id}</h5>
            <button on:click={() => selectedNode = null} class="close-button">
              ×
            </button>
          </div>
          <div class="detail-content">
            <div role="region" aria-label="Node data editor">
              <JsonEditor 
                {dark} 
                value={JSON.stringify(selectedNode.data, null, 2)}
                onChange={() => {}} 
              />
            </div>
          </div>
        </div>
      {/if}
    </div>
  </div>
</section>

<style>
  .search-controls {
    display: flex;
    flex-direction: column;
    gap: 1rem;
    margin-bottom: 1.5rem;
    padding: 1rem;
    background: var(--pico-muted-border-color);
    border-radius: 8px;
  }
  
  .filters-section {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
    gap: 1rem;
  }
  
  .filter-group {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }
  
  .filter-group label {
    font-size: 0.875rem;
    font-weight: 500;
  }
  
  .actions-section {
    display: flex;
    gap: 0.5rem;
    flex-wrap: wrap;
  }
  
  .save-dialog {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(0, 0, 0, 0.5);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
  }
  
  .dialog-content {
    background: var(--pico-background-color);
    padding: 2rem;
    border-radius: 8px;
    min-width: 300px;
  }
  
  .dialog-content h4 {
    margin-bottom: 1rem;
  }
  
  .dialog-content input {
    width: 100%;
    margin-bottom: 1rem;
  }
  
  .dialog-actions {
    display: flex;
    gap: 0.5rem;
    justify-content: flex-end;
  }
  
  .saved-searches {
    margin-bottom: 1.5rem;
  }
  
  .saved-list {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }
  
  .saved-item {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.75rem;
    border: 1px solid var(--pico-muted-border-color);
    border-radius: 4px;
    background: rgba(0, 0, 0, 0.02);
  }
  
  .saved-info {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }
  
  .saved-name {
    font-weight: 600;
  }
  
  .saved-date {
    font-size: 0.75rem;
    color: var(--pico-muted-color);
  }
  
  .saved-actions {
    display: flex;
    gap: 0.5rem;
  }
  
  .small {
    padding: 0.25rem 0.5rem;
    font-size: 0.75rem;
  }
  
  .no-saved {
    color: var(--pico-muted-color);
    font-style: italic;
    text-align: center;
    padding: 1rem;
  }
  
  .search-results {
    margin-top: 1.5rem;
  }
  
  .results-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 1rem;
  }
  
  .loading {
    color: var(--pico-muted-color);
    font-style: italic;
  }
  
  .results-grid {
    display: grid;
    grid-template-columns: 1fr 2fr;
    gap: 1rem;
    min-height: 400px;
  }
  
  .results-list {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    overflow-y: auto;
    max-height: 600px;
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
  
  .result-type {
    font-size: 0.75rem;
    opacity: 0.8;
    background: rgba(0, 0, 0, 0.1);
    padding: 0.125rem 0.375rem;
    border-radius: 3px;
  }
  
  .result-preview {
    font-size: 0.75rem;
    opacity: 0.7;
    font-family: monospace;
    margin-bottom: 0.25rem;
  }
  
  .result-meta {
    font-size: 0.75rem;
    opacity: 0.6;
  }
  
  .quick-actions {
    display: flex;
    gap: 0.25rem;
    margin-top: 0.5rem;
    flex-wrap: wrap;
  }
  
  .quick-action {
    padding: 0.125rem 0.375rem;
    font-size: 0.625rem;
    background: rgba(0, 0, 0, 0.1);
    border: none;
    border-radius: 3px;
    cursor: pointer;
  }
  
  .result-detail {
    border: 1px solid var(--pico-muted-border-color);
    border-radius: 8px;
    padding: 1rem;
    overflow-y: auto;
    max-height: 600px;
  }
  
  .detail-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 1rem;
  }
  
  .close-button {
    background: none;
    border: none;
    font-size: 1.5rem;
    cursor: pointer;
    padding: 0;
    width: 2rem;
    height: 2rem;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  
  @media (max-width: 768px) {
    .filters-section {
      grid-template-columns: 1fr;
    }
    
    .results-grid {
      grid-template-columns: 1fr;
    }
    
    .saved-item {
      flex-direction: column;
      align-items: flex-start;
      gap: 0.5rem;
    }
    
    .saved-actions {
      align-self: flex-end;
    }
  }
</style>
