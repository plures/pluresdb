<script lang="ts">
  import { onMount, onDestroy } from 'svelte'
  import { push as toast } from '../lib/toasts'
  
  let dark = false
  let peers: Array<{
    id: string
    name: string
    address: string
    status: 'connected' | 'disconnected' | 'connecting' | 'error'
    lastSeen: number
    bandwidth: {
      incoming: number
      outgoing: number
    }
    messageRate: {
      incoming: number
      outgoing: number
    }
    latency: number
    version: string
    capabilities: string[]
  }> = []
  let selectedPeer: any = null
  let meshStats = {
    totalPeers: 0,
    connectedPeers: 0,
    totalBandwidth: { incoming: 0, outgoing: 0 },
    totalMessages: { incoming: 0, outgoing: 0 },
    averageLatency: 0
  }
  let meshLogs: Array<{
    timestamp: number
    level: 'info' | 'warn' | 'error' | 'debug'
    message: string
    peerId?: string
  }> = []
  let showLogs = false
  let autoRefresh = true
  let refreshInterval: number | null = null
  
  // Snapshot and sync controls
  let isSnapshotting = false
  let isSyncing = false
  let lastSnapshot: number | null = null
  let syncProgress = 0
  
  $: dark = (document.documentElement.getAttribute('data-theme') === 'dark')
  
  onMount(() => {
    loadMeshData()
    if (autoRefresh) {
      startAutoRefresh()
    }
  })
  
  onDestroy(() => {
    if (refreshInterval) {
      clearInterval(refreshInterval)
    }
  })
  
  async function loadMeshData() {
    try {
      // Simulate mesh data - in a real implementation, this would come from the mesh API
      const mockPeers = [
        {
          id: 'peer-1',
          name: 'Node Alpha',
          address: '192.168.1.100:34567',
          status: 'connected' as const,
          lastSeen: Date.now() - 1000,
          bandwidth: { incoming: 1024, outgoing: 512 },
          messageRate: { incoming: 150, outgoing: 75 },
          latency: 25,
          version: '1.0.0',
          capabilities: ['crud', 'search', 'vector']
        },
        {
          id: 'peer-2',
          name: 'Node Beta',
          address: '192.168.1.101:34567',
          status: 'connected' as const,
          lastSeen: Date.now() - 2000,
          bandwidth: { incoming: 2048, outgoing: 1024 },
          messageRate: { incoming: 300, outgoing: 150 },
          latency: 45,
          version: '1.0.0',
          capabilities: ['crud', 'search', 'vector', 'rules']
        },
        {
          id: 'peer-3',
          name: 'Node Gamma',
          address: '192.168.1.102:34567',
          status: 'disconnected' as const,
          lastSeen: Date.now() - 30000,
          bandwidth: { incoming: 0, outgoing: 0 },
          messageRate: { incoming: 0, outgoing: 0 },
          latency: 0,
          version: '0.9.0',
          capabilities: ['crud', 'search']
        }
      ]
      
      peers = mockPeers
      updateMeshStats()
      loadMeshLogs()
    } catch (error) {
      toast('Failed to load mesh data', 'error')
      console.error('Error loading mesh data:', error)
    }
  }
  
  function updateMeshStats() {
    const connectedPeers = peers.filter(p => p.status === 'connected')
    meshStats = {
      totalPeers: peers.length,
      connectedPeers: connectedPeers.length,
      totalBandwidth: {
        incoming: peers.reduce((sum, p) => sum + p.bandwidth.incoming, 0),
        outgoing: peers.reduce((sum, p) => sum + p.bandwidth.outgoing, 0)
      },
      totalMessages: {
        incoming: peers.reduce((sum, p) => sum + p.messageRate.incoming, 0),
        outgoing: peers.reduce((sum, p) => sum + p.messageRate.outgoing, 0)
      },
      averageLatency: connectedPeers.length > 0 
        ? connectedPeers.reduce((sum, p) => sum + p.latency, 0) / connectedPeers.length 
        : 0
    }
  }
  
  function loadMeshLogs() {
    // Simulate mesh logs
    const mockLogs = [
      {
        timestamp: Date.now() - 1000,
        level: 'info' as const,
        message: 'Peer connected: Node Alpha',
        peerId: 'peer-1'
      },
      {
        timestamp: Date.now() - 2000,
        level: 'info' as const,
        message: 'Peer connected: Node Beta',
        peerId: 'peer-2'
      },
      {
        timestamp: Date.now() - 5000,
        level: 'warn' as const,
        message: 'Peer disconnected: Node Gamma',
        peerId: 'peer-3'
      },
      {
        timestamp: Date.now() - 10000,
        level: 'debug' as const,
        message: 'Mesh synchronization started'
      },
      {
        timestamp: Date.now() - 15000,
        level: 'info' as const,
        message: 'Mesh synchronization completed'
      }
    ]
    
    meshLogs = mockLogs.sort((a, b) => b.timestamp - a.timestamp)
  }
  
  function startAutoRefresh() {
    refreshInterval = setInterval(() => {
      loadMeshData()
    }, 5000) // Refresh every 5 seconds
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
  
  function selectPeer(peer: any) {
    selectedPeer = peer
  }
  
  async function connectPeer(peerId: string) {
    try {
      // Simulate peer connection
      const peer = peers.find(p => p.id === peerId)
      if (peer) {
        peer.status = 'connecting'
        await new Promise(resolve => setTimeout(resolve, 1000))
        peer.status = 'connected'
        peer.lastSeen = Date.now()
        updateMeshStats()
        toast(`Connected to ${peer.name}`, 'success')
      }
    } catch (error) {
      toast('Failed to connect to peer', 'error')
      console.error('Connection error:', error)
    }
  }
  
  async function disconnectPeer(peerId: string) {
    try {
      const peer = peers.find(p => p.id === peerId)
      if (peer) {
        peer.status = 'disconnected'
        peer.bandwidth = { incoming: 0, outgoing: 0 }
        peer.messageRate = { incoming: 0, outgoing: 0 }
        updateMeshStats()
        toast(`Disconnected from ${peer.name}`, 'success')
      }
    } catch (error) {
      toast('Failed to disconnect from peer', 'error')
      console.error('Disconnection error:', error)
    }
  }
  
  async function createSnapshot() {
    if (isSnapshotting) return
    
    isSnapshotting = true
    try {
      // Simulate snapshot creation
      await new Promise(resolve => setTimeout(resolve, 2000))
      lastSnapshot = Date.now()
      toast('Snapshot created successfully', 'success')
    } catch (error) {
      toast('Failed to create snapshot', 'error')
      console.error('Snapshot error:', error)
    } finally {
      isSnapshotting = false
    }
  }
  
  async function startSync() {
    if (isSyncing) return
    
    isSyncing = true
    syncProgress = 0
    
    try {
      // Simulate sync process
      for (let i = 0; i <= 100; i += 10) {
        syncProgress = i
        await new Promise(resolve => setTimeout(resolve, 200))
      }
      toast('Synchronization completed', 'success')
    } catch (error) {
      toast('Synchronization failed', 'error')
      console.error('Sync error:', error)
    } finally {
      isSyncing = false
      syncProgress = 0
    }
  }
  
  function formatBytes(bytes: number): string {
    if (bytes === 0) return '0 B'
    const k = 1024
    const sizes = ['B', 'KB', 'MB', 'GB']
    const i = Math.floor(Math.log(bytes) / Math.log(k))
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i]
  }
  
  function formatTimestamp(timestamp: number): string {
    return new Date(timestamp).toLocaleString()
  }
  
  function getStatusColor(status: string): string {
    switch (status) {
      case 'connected': return 'var(--success-color)'
      case 'disconnected': return 'var(--error-color)'
      case 'connecting': return 'var(--warning-color)'
      case 'error': return 'var(--error-color)'
      default: return 'var(--muted-color)'
    }
  }
  
  function getStatusIcon(status: string): string {
    switch (status) {
      case 'connected': return '🟢'
      case 'disconnected': return '🔴'
      case 'connecting': return '🟡'
      case 'error': return '❌'
      default: return '⚪'
    }
  }
</script>

<section aria-labelledby="mesh-panel-heading">
  <h3 id="mesh-panel-heading">Mesh Panel</h3>
  
  <div class="mesh-layout">
    <!-- Mesh Stats -->
    <div class="mesh-stats">
      <div class="stats-grid">
        <div class="stat-card">
          <div class="stat-value">{meshStats.totalPeers}</div>
          <div class="stat-label">Total Peers</div>
        </div>
        <div class="stat-card">
          <div class="stat-value">{meshStats.connectedPeers}</div>
          <div class="stat-label">Connected</div>
        </div>
        <div class="stat-card">
          <div class="stat-value">{formatBytes(meshStats.totalBandwidth.incoming)}/s</div>
          <div class="stat-label">Incoming</div>
        </div>
        <div class="stat-card">
          <div class="stat-value">{formatBytes(meshStats.totalBandwidth.outgoing)}/s</div>
          <div class="stat-label">Outgoing</div>
        </div>
        <div class="stat-card">
          <div class="stat-value">{meshStats.totalMessages.incoming}/s</div>
          <div class="stat-label">Messages In</div>
        </div>
        <div class="stat-card">
          <div class="stat-value">{meshStats.totalMessages.outgoing}/s</div>
          <div class="stat-label">Messages Out</div>
        </div>
        <div class="stat-card">
          <div class="stat-value">{meshStats.averageLatency.toFixed(0)}ms</div>
          <div class="stat-label">Avg Latency</div>
        </div>
      </div>
    </div>
    
    <!-- Controls -->
    <div class="mesh-controls">
      <div class="control-group">
        <button on:click={loadMeshData} class="secondary">
          Refresh
        </button>
        <button on:click={toggleAutoRefresh} class:active={autoRefresh}>
          {autoRefresh ? 'Auto Refresh ON' : 'Auto Refresh OFF'}
        </button>
        <button on:click={() => showLogs = !showLogs} class="secondary">
          {showLogs ? 'Hide Logs' : 'Show Logs'}
        </button>
      </div>
      
      <div class="control-group">
        <button on:click={createSnapshot} disabled={isSnapshotting} class="primary">
          {isSnapshotting ? 'Creating...' : 'Create Snapshot'}
        </button>
        <button on:click={startSync} disabled={isSyncing} class="primary">
          {isSyncing ? 'Syncing...' : 'Start Sync'}
        </button>
      </div>
      
      {#if lastSnapshot}
        <div class="snapshot-info">
          Last snapshot: {formatTimestamp(lastSnapshot)}
        </div>
      {/if}
      
      {#if isSyncing}
        <div class="sync-progress">
          <div class="progress-bar">
            <div class="progress-fill" style="width: {syncProgress}%"></div>
          </div>
          <span class="progress-text">{syncProgress}%</span>
        </div>
      {/if}
    </div>
    
    <!-- Peers List -->
    <div class="peers-section">
      <h4>Peers ({peers.length})</h4>
      <div class="peers-list">
        {#each peers as peer}
          <div 
            class="peer-item"
            class:selected={selectedPeer?.id === peer.id}
            role="button"
            tabindex="0"
            on:click={() => selectPeer(peer)}
            on:keydown={(e) => e.key === 'Enter' && selectPeer(peer)}
          >
            <div class="peer-header">
              <div class="peer-info">
                <span class="peer-name">{peer.name}</span>
                <span class="peer-status" style="color: {getStatusColor(peer.status)}">
                  {getStatusIcon(peer.status)} {peer.status}
                </span>
              </div>
              <div class="peer-actions">
                {#if peer.status === 'connected'}
                  <button 
                    on:click|stopPropagation={() => disconnectPeer(peer.id)}
                    class="small"
                    title="Disconnect"
                  >
                    Disconnect
                  </button>
                {:else}
                  <button 
                    on:click|stopPropagation={() => connectPeer(peer.id)}
                    class="small"
                    title="Connect"
                  >
                    Connect
                  </button>
                {/if}
              </div>
            </div>
            <div class="peer-details">
              <div class="peer-address">{peer.address}</div>
              <div class="peer-metrics">
                <span>Latency: {peer.latency}ms</span>
                <span>Version: {peer.version}</span>
                <span>Last seen: {formatTimestamp(peer.lastSeen)}</span>
              </div>
              <div class="peer-bandwidth">
                <span>In: {formatBytes(peer.bandwidth.incoming)}/s</span>
                <span>Out: {formatBytes(peer.bandwidth.outgoing)}/s</span>
              </div>
              <div class="peer-messages">
                <span>Messages: {peer.messageRate.incoming}/s in, {peer.messageRate.outgoing}/s out</span>
              </div>
              <div class="peer-capabilities">
                {#each peer.capabilities as capability}
                  <span class="capability-tag">{capability}</span>
                {/each}
              </div>
            </div>
          </div>
        {/each}
      </div>
    </div>
    
    <!-- Peer Details -->
    {#if selectedPeer}
      <div class="peer-details-panel">
        <h4>Peer Details: {selectedPeer.name}</h4>
        <div class="details-grid">
          <div class="detail-item">
            <label>ID</label>
            <span>{selectedPeer.id}</span>
          </div>
          <div class="detail-item">
            <label>Address</label>
            <span>{selectedPeer.address}</span>
          </div>
          <div class="detail-item">
            <label>Status</label>
            <span style="color: {getStatusColor(selectedPeer.status)}">
              {getStatusIcon(selectedPeer.status)} {selectedPeer.status}
            </span>
          </div>
          <div class="detail-item">
            <label>Version</label>
            <span>{selectedPeer.version}</span>
          </div>
          <div class="detail-item">
            <label>Latency</label>
            <span>{selectedPeer.latency}ms</span>
          </div>
          <div class="detail-item">
            <label>Last Seen</label>
            <span>{formatTimestamp(selectedPeer.lastSeen)}</span>
          </div>
          <div class="detail-item">
            <label>Bandwidth In</label>
            <span>{formatBytes(selectedPeer.bandwidth.incoming)}/s</span>
          </div>
          <div class="detail-item">
            <label>Bandwidth Out</label>
            <span>{formatBytes(selectedPeer.bandwidth.outgoing)}/s</span>
          </div>
          <div class="detail-item">
            <label>Messages In</label>
            <span>{selectedPeer.messageRate.incoming}/s</span>
          </div>
          <div class="detail-item">
            <label>Messages Out</label>
            <span>{selectedPeer.messageRate.outgoing}/s</span>
          </div>
        </div>
        <div class="capabilities-section">
          <label>Capabilities</label>
          <div class="capabilities-list">
            {#each selectedPeer.capabilities as capability}
              <span class="capability-tag">{capability}</span>
            {/each}
          </div>
        </div>
      </div>
    {/if}
    
    <!-- Mesh Logs -->
    {#if showLogs}
      <div class="mesh-logs">
        <h4>Mesh Logs ({meshLogs.length})</h4>
        <div class="logs-list">
          {#each meshLogs as log}
            <div class="log-item" class:info={log.level === 'info'} class:warn={log.level === 'warn'} class:error={log.level === 'error'} class:debug={log.level === 'debug'}>
              <span class="log-timestamp">{formatTimestamp(log.timestamp)}</span>
              <span class="log-level">{log.level.toUpperCase()}</span>
              <span class="log-message">{log.message}</span>
              {#if log.peerId}
                <span class="log-peer">Peer: {log.peerId}</span>
              {/if}
            </div>
          {/each}
        </div>
      </div>
    {/if}
  </div>
</section>

<style>
  .mesh-layout {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }
  
  .mesh-stats {
    background: var(--pico-muted-border-color);
    border-radius: 8px;
    padding: 1rem;
  }
  
  .stats-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
    gap: 1rem;
  }
  
  .stat-card {
    text-align: center;
    padding: 1rem;
    background: var(--pico-background-color);
    border-radius: 8px;
    border: 1px solid var(--pico-muted-border-color);
  }
  
  .stat-value {
    font-size: 2rem;
    font-weight: bold;
    color: var(--pico-primary);
  }
  
  .stat-label {
    font-size: 0.875rem;
    color: var(--pico-muted-color);
    margin-top: 0.25rem;
  }
  
  .mesh-controls {
    display: flex;
    flex-wrap: wrap;
    gap: 1rem;
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
  
  .snapshot-info {
    font-size: 0.875rem;
    color: var(--pico-muted-color);
  }
  
  .sync-progress {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }
  
  .progress-bar {
    width: 200px;
    height: 8px;
    background: var(--pico-muted-border-color);
    border-radius: 4px;
    overflow: hidden;
  }
  
  .progress-fill {
    height: 100%;
    background: var(--pico-primary);
    transition: width 0.3s ease;
  }
  
  .progress-text {
    font-size: 0.875rem;
    font-weight: 600;
  }
  
  .peers-section {
    border: 1px solid var(--pico-muted-border-color);
    border-radius: 8px;
    padding: 1rem;
    background: var(--pico-background-color);
  }
  
  .peers-list {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    max-height: 400px;
    overflow-y: auto;
  }
  
  .peer-item {
    padding: 1rem;
    border: 1px solid var(--pico-muted-border-color);
    border-radius: 8px;
    background: var(--pico-muted-border-color);
    cursor: pointer;
    transition: all 0.2s;
  }
  
  .peer-item:hover {
    background: var(--pico-primary);
    color: var(--pico-primary-inverse);
  }
  
  .peer-item.selected {
    background: var(--pico-primary);
    color: var(--pico-primary-inverse);
  }
  
  .peer-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 0.5rem;
  }
  
  .peer-info {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }
  
  .peer-name {
    font-weight: 600;
    font-size: 1rem;
  }
  
  .peer-status {
    font-size: 0.875rem;
  }
  
  .peer-actions {
    display: flex;
    gap: 0.5rem;
  }
  
  .peer-details {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    font-size: 0.875rem;
  }
  
  .peer-address {
    font-family: monospace;
    color: var(--pico-muted-color);
  }
  
  .peer-metrics {
    display: flex;
    gap: 1rem;
    flex-wrap: wrap;
  }
  
  .peer-bandwidth {
    display: flex;
    gap: 1rem;
  }
  
  .peer-messages {
    color: var(--pico-muted-color);
  }
  
  .peer-capabilities {
    display: flex;
    gap: 0.25rem;
    flex-wrap: wrap;
  }
  
  .capability-tag {
    background: var(--pico-primary);
    color: var(--pico-primary-inverse);
    padding: 0.125rem 0.375rem;
    border-radius: 3px;
    font-size: 0.75rem;
  }
  
  .peer-details-panel {
    border: 1px solid var(--pico-muted-border-color);
    border-radius: 8px;
    padding: 1rem;
    background: var(--pico-background-color);
  }
  
  .details-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
    gap: 1rem;
    margin-bottom: 1rem;
  }
  
  .detail-item {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }
  
  .detail-item label {
    font-size: 0.875rem;
    font-weight: 600;
    color: var(--pico-muted-color);
  }
  
  .capabilities-section {
    margin-top: 1rem;
  }
  
  .capabilities-section label {
    display: block;
    margin-bottom: 0.5rem;
    font-weight: 600;
  }
  
  .capabilities-list {
    display: flex;
    gap: 0.5rem;
    flex-wrap: wrap;
  }
  
  .mesh-logs {
    border: 1px solid var(--pico-muted-border-color);
    border-radius: 8px;
    padding: 1rem;
    background: var(--pico-background-color);
  }
  
  .logs-list {
    max-height: 300px;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }
  
  .log-item {
    display: grid;
    grid-template-columns: 150px 60px 1fr auto;
    gap: 0.5rem;
    padding: 0.5rem;
    border-radius: 4px;
    font-size: 0.75rem;
    background: var(--pico-muted-border-color);
  }
  
  .log-item.info {
    background: rgba(0, 123, 255, 0.1);
  }
  
  .log-item.warn {
    background: rgba(255, 193, 7, 0.1);
  }
  
  .log-item.error {
    background: rgba(220, 53, 69, 0.1);
  }
  
  .log-item.debug {
    background: rgba(108, 117, 125, 0.1);
  }
  
  .log-timestamp {
    font-family: monospace;
    opacity: 0.7;
  }
  
  .log-level {
    font-weight: 600;
    text-transform: uppercase;
  }
  
  .log-message {
    font-family: monospace;
  }
  
  .log-peer {
    font-size: 0.625rem;
    opacity: 0.7;
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
    .stats-grid {
      grid-template-columns: repeat(auto-fit, minmax(120px, 1fr));
    }
    
    .mesh-controls {
      flex-direction: column;
      align-items: flex-start;
    }
    
    .details-grid {
      grid-template-columns: 1fr;
    }
    
    .log-item {
      grid-template-columns: 1fr;
      gap: 0.25rem;
    }
  }
</style>
