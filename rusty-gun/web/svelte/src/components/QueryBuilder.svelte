<script lang="ts">
  import { onMount } from 'svelte'
  import { push as toast } from '../lib/toasts'
  import JsonEditor from './JsonEditor.svelte'
  
  let dark = false
  
  // Type definitions
  interface QueryNode {
    type: 'and' | 'or'
    conditions: Array<{
      type: 'field' | 'group'
      field?: string
      operator?: string
      value?: string
      conditions?: QueryNode['conditions']
    }>
  }
  
  let queries: Array<{
    id: string
    name: string
    description: string
    query: QueryNode
    results: any[]
    createdAt: number
    updatedAt: number
  }> = []
  let selectedQuery: any = null
  let showNewQuery = false
  let newQueryName = ''
  let newQueryDescription = ''
  let showRawMode = false
  let rawQuery = ''
  let loading = false
  
  // Query builder state
  let rootQuery: QueryNode = {
    type: 'and',
    conditions: []
  }
  
  // Available fields for building queries
  let availableFields: string[] = []
  let availableTypes: string[] = []
  
  // Query execution results
  let queryResults: any[] = []
  let executionTime = 0
  
  $: dark = (document.documentElement.getAttribute('data-theme') === 'dark')
  
  onMount(() => {
    loadQueries()
    loadAvailableFields()
  })
  
  function loadQueries() {
    try {
      const saved = localStorage.getItem('pluresdb-queries')
      if (saved) {
        queries = JSON.parse(saved)
      }
    } catch (error) {
      console.error('Error loading queries:', error)
    }
  }
  
  function saveQueries() {
    try {
      localStorage.setItem('pluresdb-queries', JSON.stringify(queries))
    } catch (error) {
      console.error('Error saving queries:', error)
    }
  }
  
  async function loadAvailableFields() {
    try {
      const res = await fetch('/api/list')
      const nodes = await res.json()
      
      const fields = new Set<string>()
      const types = new Set<string>()
      
      for (const node of nodes) {
        if (node.data.type) types.add(node.data.type)
        Object.keys(node.data).forEach(field => fields.add(field))
      }
      
      availableFields = Array.from(fields).sort()
      availableTypes = Array.from(types).sort()
    } catch (error) {
      console.error('Error loading fields:', error)
    }
  }
  
  function createQuery() {
    if (!newQueryName.trim()) {
      toast('Please enter a query name', 'error')
      return
    }
    
    const query = {
      id: `query-${Date.now()}`,
      name: newQueryName,
      description: newQueryDescription,
      query: JSON.parse(JSON.stringify(rootQuery)), // Deep copy
      results: [],
      createdAt: Date.now(),
      updatedAt: Date.now()
    }
    
    queries = [...queries, query]
    selectedQuery = query
    newQueryName = ''
    newQueryDescription = ''
    showNewQuery = false
    saveQueries()
    toast('Query created successfully', 'success')
  }
  
  function selectQuery(query: any) {
    selectedQuery = query
    rootQuery = JSON.parse(JSON.stringify(query.query))
    showRawMode = false
    rawQuery = JSON.stringify(query.query, null, 2)
  }
  
  function deleteQuery(queryId: string) {
    if (confirm('Are you sure you want to delete this query?')) {
      queries = queries.filter(q => q.id !== queryId)
      if (selectedQuery?.id === queryId) {
        selectedQuery = null
        rootQuery = { type: 'and', conditions: [] }
      }
      saveQueries()
      toast('Query deleted', 'success')
    }
  }
  
  function addCondition() {
    rootQuery.conditions.push({
      type: 'field',
      field: availableFields[0] || '',
      operator: 'equals',
      value: ''
    })
  }
  
  function addGroup() {
    rootQuery.conditions.push({
      type: 'group',
      operator: 'and',
      conditions: []
    })
  }
  
  function removeCondition(index: number) {
    rootQuery.conditions.splice(index, 1)
  }
  
  function updateCondition(index: number, condition: any) {
    rootQuery.conditions[index] = condition
  }
  
  function toggleRawMode() {
    showRawMode = !showRawMode
    if (showRawMode) {
      rawQuery = JSON.stringify(rootQuery, null, 2)
    } else {
      try {
        rootQuery = JSON.parse(rawQuery)
      } catch (error) {
        toast('Invalid JSON in raw mode', 'error')
        showRawMode = false
      }
    }
  }
  
  async function executeQuery() {
    if (!rootQuery || rootQuery.conditions.length === 0) {
      toast('Please add some conditions to your query', 'error')
      return
    }
    
    loading = true
    const startTime = Date.now()
    
    try {
      // Convert query to API format and execute
      const apiQuery = convertQueryToAPI(rootQuery)
      const res = await fetch('/api/query', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(apiQuery)
      })
      
      if (!res.ok) throw new Error('Query execution failed')
      
      queryResults = await res.json()
      executionTime = Date.now() - startTime
      
      if (selectedQuery) {
        selectedQuery.results = queryResults
        selectedQuery.updatedAt = Date.now()
        saveQueries()
      }
      
      toast(`Query executed successfully (${queryResults.length} results, ${executionTime}ms)`, 'success')
    } catch (error) {
      toast('Query execution failed', 'error')
      console.error('Query execution error:', error)
    } finally {
      loading = false
    }
  }
  
  function convertQueryToAPI(query: QueryNode): any {
    // Convert the visual query to API format
    // This is a simplified conversion - in a real implementation,
    // you'd have more sophisticated query translation
    return {
      type: query.type,
      conditions: query.conditions.map(condition => {
        if (condition.type === 'field') {
          return {
            field: condition.field,
            operator: condition.operator,
            value: condition.value
          }
        } else if (condition.type === 'group') {
          return convertQueryToAPI(condition)
        }
        return condition
      })
    }
  }
  
  function exportQuery(query: any) {
    const data = {
      name: query.name,
      description: query.description,
      query: query.query,
      results: query.results,
      exportedAt: new Date().toISOString()
    }
    
    const blob = new Blob([JSON.stringify(data, null, 2)], { type: 'application/json' })
    const url = URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.href = url
    a.download = `${query.name.replace(/[^a-z0-9]/gi, '_').toLowerCase()}.json`
    document.body.appendChild(a)
    a.click()
    document.body.removeChild(a)
    URL.revokeObjectURL(url)
    
    toast('Query exported', 'success')
  }
  
  function saveQuery() {
    if (!selectedQuery) return
    
    selectedQuery.query = JSON.parse(JSON.stringify(rootQuery))
    selectedQuery.updatedAt = Date.now()
    saveQueries()
    toast('Query saved', 'success')
  }
</script>

<section aria-labelledby="query-builder-heading">
  <h3 id="query-builder-heading">Query Builder</h3>
  
  <div class="query-builder-layout">
    <!-- Query List -->
    <div class="queries-sidebar">
      <div class="sidebar-header">
        <h4>Saved Queries ({queries.length})</h4>
        <button on:click={() => showNewQuery = true} class="primary">
          New Query
        </button>
      </div>
      
      <div class="query-list">
        {#each queries as query}
          <div 
            class="query-item"
            class:selected={selectedQuery?.id === query.id}
            role="button"
            tabindex="0"
            on:click={() => selectQuery(query)}
            on:keydown={(e) => e.key === 'Enter' && selectQuery(query)}
          >
            <div class="query-header">
              <span class="query-name">{query.name}</span>
              <div class="query-actions">
                <button 
                  on:click|stopPropagation={() => exportQuery(query)}
                  class="small"
                  title="Export query"
                >
                  📤
                </button>
                <button 
                  on:click|stopPropagation={() => deleteQuery(query.id)}
                  class="small"
                  title="Delete query"
                >
                  🗑️
                </button>
              </div>
            </div>
            <div class="query-meta">
              <span class="result-count">{query.results.length} results</span>
              <span class="updated-time">
                {new Date(query.updatedAt).toLocaleDateString()}
              </span>
            </div>
            {#if query.description}
              <div class="query-description">
                {query.description}
              </div>
            {/if}
          </div>
        {/each}
      </div>
    </div>
    
    <!-- Query Builder -->
    <div class="query-builder">
      {#if selectedQuery}
        <div class="builder-header">
          <h4>{selectedQuery.name}</h4>
          <div class="builder-actions">
            <button on:click={toggleRawMode} class="secondary">
              {showRawMode ? 'Visual Mode' : 'Raw Mode'}
            </button>
            <button on:click={saveQuery} class="secondary">
              Save
            </button>
            <button on:click={executeQuery} disabled={loading} class="primary">
              {loading ? 'Executing...' : 'Execute Query'}
            </button>
          </div>
        </div>
        
        <div class="builder-content">
          {#if showRawMode}
            <div class="raw-editor">
              <h5>Raw Query JSON</h5>
              <div role="region" aria-label="Raw query editor">
                <JsonEditor 
                  {dark} 
                  value={rawQuery}
                  onChange={(value) => rawQuery = value}
                />
              </div>
            </div>
          {:else}
            <div class="visual-builder">
              <div class="query-tree">
                <!-- Query Node Builder Component -->
                <div class="query-node">
                  <div class="node-header">
                    <select bind:value={rootQuery.type} on:change={() => {}}>
                      <option value="and">AND</option>
                      <option value="or">OR</option>
                    </select>
                    <button on:click={addCondition} class="small">Add Condition</button>
                    <button on:click={addGroup} class="small">Add Group</button>
                  </div>
                  
                  <div class="node-conditions">
                    {#each rootQuery.conditions as condition, index}
                      <div class="condition-item">
                        {#if condition.type === 'field'}
                          <div class="field-condition">
                            <select bind:value={condition.field} on:change={() => updateCondition(index, condition)}>
                              {#each availableFields as field}
                                <option value={field}>{field}</option>
                              {/each}
                            </select>
                            <select bind:value={condition.operator} on:change={() => updateCondition(index, condition)}>
                              <option value="equals">equals</option>
                              <option value="not_equals">not equals</option>
                              <option value="contains">contains</option>
                              <option value="not_contains">not contains</option>
                              <option value="starts_with">starts with</option>
                              <option value="ends_with">ends with</option>
                              <option value="greater_than">greater than</option>
                              <option value="less_than">less than</option>
                              <option value="is_empty">is empty</option>
                              <option value="is_not_empty">is not empty</option>
                            </select>
                            <input 
                              type="text" 
                              bind:value={condition.value} 
                              on:input={() => updateCondition(index, condition)}
                              placeholder="Value..."
                            />
                          </div>
                        {:else if condition.type === 'group'}
                          <div class="group-condition">
                            <select bind:value={condition.operator} on:change={() => updateCondition(index, condition)}>
                              <option value="and">AND</option>
                              <option value="or">OR</option>
                            </select>
                            <span class="group-label">Group</span>
                          </div>
                        {/if}
                        <button on:click={() => removeCondition(index)} class="small remove">
                          Remove
                        </button>
                      </div>
                    {/each}
                  </div>
                </div>
              </div>
            </div>
          {/if}
        </div>
        
        {#if queryResults.length > 0}
          <div class="query-results">
            <div class="results-header">
              <h5>Results ({queryResults.length})</h5>
              <span class="execution-time">Executed in {executionTime}ms</span>
            </div>
            <div class="results-list">
              {#each queryResults as result, index}
                <div class="result-item">
                  <div class="result-header">
                    <span class="result-id">{result.id}</span>
                    <span class="result-type">{result.data.type || 'unknown'}</span>
                  </div>
                  <div class="result-preview">
                    {JSON.stringify(result.data).substring(0, 100)}...
                  </div>
                </div>
              {/each}
            </div>
          </div>
        {/if}
      {:else}
        <div class="no-query">
          <p>Select a query or create a new one to get started</p>
        </div>
      {/if}
    </div>
  </div>
  
  <!-- New Query Dialog -->
  {#if showNewQuery}
    <div class="dialog-overlay">
      <div class="dialog">
        <h4>Create New Query</h4>
        <input 
          type="text" 
          bind:value={newQueryName} 
          placeholder="Enter query name..."
          on:keydown={(e) => e.key === 'Enter' && createQuery()}
        />
        <textarea 
          bind:value={newQueryDescription} 
          placeholder="Enter query description (optional)..."
          rows="3"
        ></textarea>
        <div class="dialog-actions">
          <button on:click={createQuery} disabled={!newQueryName.trim()}>
            Create
          </button>
          <button on:click={() => showNewQuery = false} class="secondary">
            Cancel
          </button>
        </div>
      </div>
    </div>
  {/if}
</section>


<style>
  .query-builder-layout {
    display: grid;
    grid-template-columns: 300px 1fr;
    gap: 1rem;
    height: 600px;
  }
  
  .queries-sidebar {
    border: 1px solid var(--pico-muted-border-color);
    border-radius: 8px;
    padding: 1rem;
    overflow-y: auto;
    background: var(--pico-muted-border-color);
  }
  
  .sidebar-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 1rem;
  }
  
  .query-list {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }
  
  .query-item {
    padding: 0.75rem;
    border: 1px solid var(--pico-muted-border-color);
    border-radius: 4px;
    background: var(--pico-background-color);
    cursor: pointer;
    transition: all 0.2s;
  }
  
  .query-item:hover {
    background: var(--pico-muted-border-color);
  }
  
  .query-item.selected {
    background: var(--pico-primary);
    color: var(--pico-primary-inverse);
  }
  
  .query-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 0.25rem;
  }
  
  .query-name {
    font-weight: 600;
    font-size: 0.875rem;
  }
  
  .query-actions {
    display: flex;
    gap: 0.25rem;
  }
  
  .query-meta {
    display: flex;
    justify-content: space-between;
    font-size: 0.75rem;
    opacity: 0.7;
    margin-bottom: 0.25rem;
  }
  
  .query-description {
    font-size: 0.75rem;
    opacity: 0.8;
    font-style: italic;
  }
  
  .query-builder {
    border: 1px solid var(--pico-muted-border-color);
    border-radius: 8px;
    padding: 1rem;
    overflow-y: auto;
    background: var(--pico-background-color);
  }
  
  .builder-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 1rem;
    padding-bottom: 1rem;
    border-bottom: 1px solid var(--pico-muted-border-color);
  }
  
  .builder-actions {
    display: flex;
    gap: 0.5rem;
  }
  
  .raw-editor {
    margin-bottom: 1rem;
  }
  
  .visual-builder {
    margin-bottom: 1rem;
  }
  
  .query-node {
    border: 1px solid var(--pico-muted-border-color);
    border-radius: 8px;
    padding: 1rem;
    background: var(--pico-muted-border-color);
  }
  
  .node-header {
    display: flex;
    gap: 0.5rem;
    align-items: center;
    margin-bottom: 1rem;
  }
  
  .node-conditions {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }
  
  .condition-item {
    display: flex;
    gap: 0.5rem;
    align-items: center;
    padding: 0.5rem;
    border: 1px solid var(--pico-muted-border-color);
    border-radius: 4px;
    background: var(--pico-background-color);
  }
  
  .field-condition {
    display: flex;
    gap: 0.5rem;
    align-items: center;
    flex: 1;
  }
  
  .field-condition select,
  .field-condition input {
    flex: 1;
  }
  
  .query-results {
    margin-top: 1rem;
    border-top: 1px solid var(--pico-muted-border-color);
    padding-top: 1rem;
  }
  
  .results-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 1rem;
  }
  
  .execution-time {
    font-size: 0.875rem;
    color: var(--pico-muted-color);
  }
  
  .results-list {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    max-height: 300px;
    overflow-y: auto;
  }
  
  .result-item {
    padding: 0.75rem;
    border: 1px solid var(--pico-muted-border-color);
    border-radius: 4px;
    background: var(--pico-muted-border-color);
  }
  
  .result-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 0.25rem;
  }
  
  .result-id {
    font-weight: 600;
    font-size: 0.875rem;
  }
  
  .result-type {
    font-size: 0.75rem;
    opacity: 0.8;
    background: var(--pico-primary);
    color: var(--pico-primary-inverse);
    padding: 0.125rem 0.375rem;
    border-radius: 3px;
  }
  
  .result-preview {
    font-size: 0.75rem;
    opacity: 0.7;
    font-family: monospace;
  }
  
  .no-query {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
    color: var(--pico-muted-color);
    font-style: italic;
  }
  
  .dialog-overlay {
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
  
  .dialog {
    background: var(--pico-background-color);
    padding: 2rem;
    border-radius: 8px;
    min-width: 400px;
    max-width: 500px;
  }
  
  .dialog h4 {
    margin-bottom: 1rem;
  }
  
  .dialog input,
  .dialog textarea {
    width: 100%;
    margin-bottom: 1rem;
  }
  
  .dialog-actions {
    display: flex;
    gap: 0.5rem;
    justify-content: flex-end;
  }
  
  .small {
    padding: 0.25rem 0.5rem;
    font-size: 0.75rem;
  }
  
  .remove {
    background: var(--error-color);
    color: white;
    border: none;
  }
  
  @media (max-width: 768px) {
    .query-builder-layout {
      grid-template-columns: 1fr;
      height: auto;
    }
    
    .queries-sidebar {
      max-height: 200px;
    }
    
    .field-condition {
      flex-direction: column;
    }
  }
</style>
