<script lang="ts">
  import { onMount } from 'svelte'
  import { push as toast } from '../lib/toasts'
  
  let dark = false
  let profilingData = {
    slowOperations: [] as Array<{
      id: string
      operation: string
      duration: number
      timestamp: number
      nodeId: string
      details: string
    }>,
    largeNodes: [] as Array<{
      id: string
      size: number
      type: string
      lastAccessed: number
      accessCount: number
    }>,
    topTalkers: [] as Array<{
      id: string
      name: string
      messageCount: number
      bandwidth: number
      lastActivity: number
    }>,
    suggestions: [] as Array<{
      id: string
      type: 'index' | 'split' | 'optimize' | 'cleanup'
      priority: 'high' | 'medium' | 'low'
      title: string
      description: string
      impact: string
      action: string
    }>
  }
  let selectedTab: 'operations' | 'nodes' | 'talkers' | 'suggestions' = 'operations'
  let autoRefresh = true
  let refreshInterval: number | null = null
  
  $: dark = (document.documentElement.getAttribute('data-theme') === 'dark')
  
  onMount(() => {
    loadProfilingData()
    if (autoRefresh) {
      startAutoRefresh()
    }
  })
  
  function startAutoRefresh() {
    refreshInterval = setInterval(() => {
      loadProfilingData()
    }, 10000) // Refresh every 10 seconds
  }
  
  function stopAutoRefresh() {
    if (refreshInterval) {
      clearInterval(refreshInterval)
      refreshInterval = null
    }
  }
  
  function toggleAutoRefresh() {
    autoRefresh = !autoRefresh
    if (autoRefresh) {
      startAutoRefresh()
    } else {
      stopAutoRefresh()
    }
  }
  
  async function loadProfilingData() {
    try {
      // Simulate profiling data
      profilingData = {
        slowOperations: [
          {
            id: 'op-1',
            operation: 'vector_search',
            duration: 2500,
            timestamp: Date.now() - 1000,
            nodeId: 'node-123',
            details: 'Vector search with 1000+ dimensions'
          },
          {
            id: 'op-2',
            operation: 'crud_update',
            duration: 1800,
            timestamp: Date.now() - 2000,
            nodeId: 'node-456',
            details: 'Large document update with 50+ fields'
          },
          {
            id: 'op-3',
            operation: 'text_search',
            duration: 1200,
            timestamp: Date.now() - 3000,
            nodeId: 'node-789',
            details: 'Full-text search across 10,000+ documents'
          }
        ],
        largeNodes: [
          {
            id: 'node-123',
            size: 50 * 1024 * 1024, // 50MB
            type: 'document',
            lastAccessed: Date.now() - 1000,
            accessCount: 150
          },
          {
            id: 'node-456',
            size: 35 * 1024 * 1024, // 35MB
            type: 'vector',
            lastAccessed: Date.now() - 2000,
            accessCount: 75
          },
          {
            id: 'node-789',
            size: 25 * 1024 * 1024, // 25MB
            type: 'document',
            lastAccessed: Date.now() - 3000,
            accessCount: 200
          }
        ],
        topTalkers: [
          {
            id: 'peer-1',
            name: 'Node Alpha',
            messageCount: 1500,
            bandwidth: 1024 * 1024, // 1MB
            lastActivity: Date.now() - 1000
          },
          {
            id: 'peer-2',
            name: 'Node Beta',
            messageCount: 1200,
            bandwidth: 800 * 1024, // 800KB
            lastActivity: Date.now() - 2000
          },
          {
            id: 'peer-3',
            name: 'Node Gamma',
            messageCount: 900,
            bandwidth: 600 * 1024, // 600KB
            lastActivity: Date.now() - 3000
          }
        ],
        suggestions: [
          {
            id: 'sug-1',
            type: 'index',
            priority: 'high',
            title: 'Create Vector Index',
            description: 'Vector search operations are slow. Consider creating a vector index.',
            impact: 'Reduce search time by 80%',
            action: 'Create vector index for node-123'
          },
          {
            id: 'sug-2',
            type: 'split',
            priority: 'medium',
            title: 'Split Large Node',
            description: 'Node node-123 is 50MB and frequently accessed. Consider splitting it.',
            impact: 'Improve performance and reduce memory usage',
            action: 'Split node-123 into smaller chunks'
          },
          {
            id: 'sug-3',
            type: 'optimize',
            priority: 'low',
            title: 'Optimize Text Search',
            description: 'Text search operations could benefit from optimization.',
            impact: 'Reduce search time by 30%',
            action: 'Optimize text search configuration'
          },
          {
            id: 'sug-4',
            type: 'cleanup',
            priority: 'medium',
            title: 'Cleanup Old Data',
            description: 'Remove unused or old data to free up space.',
            impact: 'Free up 100MB of storage',
            action: 'Run cleanup operation'
          }
        ]
      }
    } catch (error) {
      toast('Failed to load profiling data', 'error')
      console.error('Error loading profiling data:', error)
    }
  }
  
  function formatBytes(bytes: number): string {
    if (bytes === 0) return '0 B'
    const k = 1024
    const sizes = ['B', 'KB', 'MB', 'GB']
    const i = Math.floor(Math.log(bytes) / Math.log(k))
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i]
  }
  
  function formatDuration(ms: number): string {
    if (ms < 1000) return `${ms}ms`
    return `${(ms / 1000).toFixed(2)}s`
  }
  
  function formatTimestamp(timestamp: number): string {
    return new Date(timestamp).toLocaleString()
  }
  
  function getPriorityColor(priority: string): string {
    switch (priority) {
      case 'high': return 'var(--error-color)'
      case 'medium': return 'var(--warning-color)'
      case 'low': return 'var(--success-color)'
      default: return 'var(--muted-color)'
    }
  }
  
  function getPriorityIcon(priority: string): string {
    switch (priority) {
      case 'high': return '🔴'
      case 'medium': return '🟡'
      case 'low': return '🟢'
      default: return '⚪'
    }
  }
  
  function getTypeIcon(type: string): string {
    switch (type) {
      case 'index': return '📊'
      case 'split': return '✂️'
      case 'optimize': return '⚡'
      case 'cleanup': return '🧹'
      default: return '💡'
    }
  }
  
  async function applySuggestion(suggestionId: string) {
    try {
      // Simulate applying suggestion
      await new Promise(resolve => setTimeout(resolve, 1000))
      toast('Suggestion applied successfully', 'success')
    } catch (error) {
      toast('Failed to apply suggestion', 'error')
      console.error('Apply suggestion error:', error)
    }
  }
  
  async function dismissSuggestion(suggestionId: string) {
    try {
      profilingData.suggestions = profilingData.suggestions.filter(s => s.id !== suggestionId)
      toast('Suggestion dismissed', 'success')
    } catch (error) {
      toast('Failed to dismiss suggestion', 'error')
      console.error('Dismiss suggestion error:', error)
    }
  }
</script>

<section aria-labelledby="profiling-heading">
  <h3 id="profiling-heading">Profiling & Performance</h3>
  
  <div class="profiling-layout">
    <!-- Controls -->
    <div class="profiling-controls">
      <div class="control-group">
        <button on:click={loadProfilingData} class="secondary">
          Refresh
        </button>
        <button on:click={toggleAutoRefresh} class:active={autoRefresh}>
          {autoRefresh ? 'Auto Refresh ON' : 'Auto Refresh OFF'}
        </button>
      </div>
    </div>
    
    <!-- Tabs -->
    <div class="profiling-tabs">
      <button 
        class="tab-button"
        class:active={selectedTab === 'operations'}
        on:click={() => selectedTab = 'operations'}
      >
        Slow Operations ({profilingData.slowOperations.length})
      </button>
      <button 
        class="tab-button"
        class:active={selectedTab === 'nodes'}
        on:click={() => selectedTab = 'nodes'}
      >
        Large Nodes ({profilingData.largeNodes.length})
      </button>
      <button 
        class="tab-button"
        class:active={selectedTab === 'talkers'}
        on:click={() => selectedTab = 'talkers'}
      >
        Top Talkers ({profilingData.topTalkers.length})
      </button>
      <button 
        class="tab-button"
        class:active={selectedTab === 'suggestions'}
        on:click={() => selectedTab = 'suggestions'}
      >
        Suggestions ({profilingData.suggestions.length})
      </button>
    </div>
    
    <!-- Content -->
    <div class="profiling-content">
      {#if selectedTab === 'operations'}
        <div class="operations-section">
          <h4>Slow Operations</h4>
          <div class="operations-list">
            {#each profilingData.slowOperations as operation}
              <div class="operation-item">
                <div class="operation-header">
                  <div class="operation-info">
                    <span class="operation-name">{operation.operation}</span>
                    <span class="operation-duration">{formatDuration(operation.duration)}</span>
                  </div>
                  <div class="operation-meta">
                    <span class="operation-time">{formatTimestamp(operation.timestamp)}</span>
                  </div>
                </div>
                <div class="operation-details">
                  <div class="operation-node">Node: {operation.nodeId}</div>
                  <div class="operation-description">{operation.details}</div>
                </div>
              </div>
            {/each}
          </div>
        </div>
      {:else if selectedTab === 'nodes'}
        <div class="nodes-section">
          <h4>Large Nodes</h4>
          <div class="nodes-list">
            {#each profilingData.largeNodes as node}
              <div class="node-item">
                <div class="node-header">
                  <div class="node-info">
                    <span class="node-id">{node.id}</span>
                    <span class="node-type">{node.type}</span>
                    <span class="node-size">{formatBytes(node.size)}</span>
                  </div>
                  <div class="node-meta">
                    <span class="node-accesses">{node.accessCount} accesses</span>
                  </div>
                </div>
                <div class="node-details">
                  <div class="node-last-accessed">
                    Last accessed: {formatTimestamp(node.lastAccessed)}
                  </div>
                </div>
              </div>
            {/each}
          </div>
        </div>
      {:else if selectedTab === 'talkers'}
        <div class="talkers-section">
          <h4>Top Talkers</h4>
          <div class="talkers-list">
            {#each profilingData.topTalkers as talker}
              <div class="talker-item">
                <div class="talker-header">
                  <div class="talker-info">
                    <span class="talker-name">{talker.name}</span>
                    <span class="talker-id">{talker.id}</span>
                  </div>
                  <div class="talker-metrics">
                    <span class="talker-messages">{talker.messageCount} messages</span>
                    <span class="talker-bandwidth">{formatBytes(talker.bandwidth)}</span>
                  </div>
                </div>
                <div class="talker-details">
                  <div class="talker-last-activity">
                    Last activity: {formatTimestamp(talker.lastActivity)}
                  </div>
                </div>
              </div>
            {/each}
          </div>
        </div>
      {:else if selectedTab === 'suggestions'}
        <div class="suggestions-section">
          <h4>Performance Suggestions</h4>
          <div class="suggestions-list">
            {#each profilingData.suggestions as suggestion}
              <div class="suggestion-item">
                <div class="suggestion-header">
                  <div class="suggestion-info">
                    <span class="suggestion-title">
                      {getTypeIcon(suggestion.type)} {suggestion.title}
                    </span>
                    <span class="suggestion-priority" style="color: {getPriorityColor(suggestion.priority)}">
                      {getPriorityIcon(suggestion.priority)} {suggestion.priority}
                    </span>
                  </div>
                  <div class="suggestion-actions">
                    <button 
                      on:click={() => applySuggestion(suggestion.id)}
                      class="small primary"
                    >
                      Apply
                    </button>
                    <button 
                      on:click={() => dismissSuggestion(suggestion.id)}
                      class="small secondary"
                    >
                      Dismiss
                    </button>
                  </div>
                </div>
                <div class="suggestion-details">
                  <div class="suggestion-description">{suggestion.description}</div>
                  <div class="suggestion-impact">
                    <strong>Impact:</strong> {suggestion.impact}
                  </div>
                  <div class="suggestion-action">
                    <strong>Action:</strong> {suggestion.action}
                  </div>
                </div>
              </div>
            {/each}
          </div>
        </div>
      {/if}
    </div>
  </div>
</section>

<style>
  .profiling-layout {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }
  
  .profiling-controls {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 1rem;
    background: var(--pico-muted-border-color);
    border-radius: 8px;
  }
  
  .control-group {
    display: flex;
    gap: 0.5rem;
    align-items: center;
  }
  
  .profiling-tabs {
    display: flex;
    gap: 0.5rem;
    border-bottom: 1px solid var(--pico-muted-border-color);
  }
  
  .tab-button {
    padding: 0.75rem 1rem;
    border: none;
    background: transparent;
    color: var(--pico-muted-color);
    cursor: pointer;
    border-bottom: 2px solid transparent;
    transition: all 0.2s;
  }
  
  .tab-button:hover {
    color: var(--pico-primary);
  }
  
  .tab-button.active {
    color: var(--pico-primary);
    border-bottom-color: var(--pico-primary);
  }
  
  .profiling-content {
    flex: 1;
    padding: 1rem;
    background: var(--pico-background-color);
    border-radius: 8px;
    border: 1px solid var(--pico-muted-border-color);
  }
  
  .operations-list, .nodes-list, .talkers-list, .suggestions-list {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }
  
  .operation-item, .node-item, .talker-item, .suggestion-item {
    padding: 1rem;
    border: 1px solid var(--pico-muted-border-color);
    border-radius: 8px;
    background: var(--pico-muted-border-color);
  }
  
  .operation-header, .node-header, .talker-header, .suggestion-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 0.5rem;
  }
  
  .operation-info, .node-info, .talker-info, .suggestion-info {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }
  
  .operation-name, .node-id, .talker-name, .suggestion-title {
    font-weight: 600;
    font-size: 1rem;
  }
  
  .operation-duration {
    font-size: 0.875rem;
    color: var(--pico-primary);
    font-weight: 600;
  }
  
  .node-type, .talker-id {
    font-size: 0.875rem;
    color: var(--pico-muted-color);
  }
  
  .node-size, .talker-bandwidth {
    font-size: 0.875rem;
    color: var(--pico-primary);
    font-weight: 600;
  }
  
  .suggestion-priority {
    font-size: 0.875rem;
    font-weight: 600;
  }
  
  .operation-meta, .node-meta, .talker-metrics {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    text-align: right;
  }
  
  .operation-time, .node-accesses, .talker-messages {
    font-size: 0.875rem;
    color: var(--pico-muted-color);
  }
  
  .operation-details, .node-details, .talker-details, .suggestion-details {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    font-size: 0.875rem;
  }
  
  .operation-node, .node-last-accessed, .talker-last-activity {
    color: var(--pico-muted-color);
  }
  
  .operation-description, .suggestion-description {
    font-style: italic;
  }
  
  .suggestion-impact, .suggestion-action {
    margin-top: 0.5rem;
  }
  
  .suggestion-actions {
    display: flex;
    gap: 0.5rem;
  }
  
  .small {
    padding: 0.25rem 0.5rem;
    font-size: 0.75rem;
  }
  
  .active {
    background: var(--pico-primary);
    color: var(--pico-primary-inverse);
  }
  
  @media (max-width: 768px) {
    .profiling-tabs {
      flex-wrap: wrap;
    }
    
    .tab-button {
      flex: 1;
      min-width: 120px;
    }
    
    .operation-header, .node-header, .talker-header, .suggestion-header {
      flex-direction: column;
      align-items: flex-start;
      gap: 0.5rem;
    }
    
    .operation-meta, .node-meta, .talker-metrics {
      text-align: left;
    }
  }
</style>
