<script lang="ts">
  import { onMount } from 'svelte'
  import { push as toast } from '../lib/toasts'

  // Sync Status
  let syncStatus = {
    isOnline: true,
    lastSync: null as Date | null,
    pendingChanges: 0,
    conflictsResolved: 0,
    dataTransferred: 0,
    syncMode: 'realtime' as 'realtime' | 'batch' | 'manual',
    compressionEnabled: true,
    deltaSync: true
  }

  // Connected Devices
  let connectedDevices = [] as any[]
  let deviceForm = {
    name: '',
    type: 'laptop',
    location: '',
    description: ''
  }

  // Sync Queues
  let outgoingQueue = [] as any[]
  let incomingQueue = [] as any[]
  let conflictQueue = [] as any[]

  // Sync Settings
  let syncSettings = {
    autoSync: true,
    syncInterval: 30, // seconds
    maxRetries: 3,
    conflictResolution: 'manual' as 'manual' | 'automatic' | 'last-write-wins',
    compressionLevel: 6,
    encryptionEnabled: true,
    bandwidthLimit: 'unlimited' as 'unlimited' | '1MB' | '10MB' | '100MB',
    syncOnWiFiOnly: false
  }

  // Sync History
  let syncHistory = [] as any[]

  // Performance Metrics
  let performanceMetrics = {
    averageSyncTime: 0,
    totalDataSynced: 0,
    syncSuccessRate: 100,
    conflictRate: 0,
    bandwidthUsage: 0
  }

  onMount(() => {
    loadSyncStatus()
    loadConnectedDevices()
    loadSyncQueues()
    loadSyncSettings()
    loadSyncHistory()
    loadPerformanceMetrics()
    startSyncMonitoring()
  })

  function loadSyncStatus() {
    const stored = localStorage.getItem('rusty-gun-sync-status')
    if (stored) {
      syncStatus = { ...syncStatus, ...JSON.parse(stored) }
    }
  }

  function loadConnectedDevices() {
    const stored = localStorage.getItem('rusty-gun-connected-devices')
    if (stored) {
      connectedDevices = JSON.parse(stored)
    } else {
      // Add current device
      addCurrentDevice()
    }
  }

  function loadSyncQueues() {
    const stored = localStorage.getItem('rusty-gun-sync-queues')
    if (stored) {
      const queues = JSON.parse(stored)
      outgoingQueue = queues.outgoing || []
      incomingQueue = queues.incoming || []
      conflictQueue = queues.conflicts || []
    }
  }

  function loadSyncSettings() {
    const stored = localStorage.getItem('rusty-gun-sync-settings')
    if (stored) {
      syncSettings = { ...syncSettings, ...JSON.parse(stored) }
    }
  }

  function loadSyncHistory() {
    const stored = localStorage.getItem('rusty-gun-sync-history')
    if (stored) {
      syncHistory = JSON.parse(stored)
    }
  }

  function loadPerformanceMetrics() {
    const stored = localStorage.getItem('rusty-gun-performance-metrics')
    if (stored) {
      performanceMetrics = { ...performanceMetrics, ...JSON.parse(stored) }
    }
  }

  function addCurrentDevice() {
    const device = {
      id: 'device_current',
      name: 'Current Device',
      type: 'laptop',
      location: 'Local',
      description: 'This device',
      isOnline: true,
      lastSeen: new Date(),
      syncStatus: 'connected',
      dataVersion: 1
    }
    connectedDevices.push(device)
    saveConnectedDevices()
  }

  function addDevice() {
    const device = {
      id: 'device_' + Math.random().toString(36).substr(2, 9),
      name: deviceForm.name,
      type: deviceForm.type,
      location: deviceForm.location,
      description: deviceForm.description,
      isOnline: false,
      lastSeen: new Date(),
      syncStatus: 'disconnected',
      dataVersion: 0
    }
    
    connectedDevices.push(device)
    saveConnectedDevices()
    
    // Reset form
    deviceForm = {
      name: '',
      type: 'laptop',
      location: '',
      description: ''
    }
    
    toast.success('Device added')
  }

  function removeDevice(deviceId: string) {
    connectedDevices = connectedDevices.filter(d => d.id !== deviceId)
    saveConnectedDevices()
    toast.info('Device removed')
  }

  function connectDevice(deviceId: string) {
    const device = connectedDevices.find(d => d.id === deviceId)
    if (device) {
      device.isOnline = true
      device.lastSeen = new Date()
      device.syncStatus = 'connected'
      saveConnectedDevices()
      toast.success(`Connected to ${device.name}`)
    }
  }

  function disconnectDevice(deviceId: string) {
    const device = connectedDevices.find(d => d.id === deviceId)
    if (device) {
      device.isOnline = false
      device.syncStatus = 'disconnected'
      saveConnectedDevices()
      toast.info(`Disconnected from ${device.name}`)
    }
  }

  function saveSyncStatus() {
    localStorage.setItem('rusty-gun-sync-status', JSON.stringify(syncStatus))
  }

  function saveConnectedDevices() {
    localStorage.setItem('rusty-gun-connected-devices', JSON.stringify(connectedDevices))
  }

  function saveSyncQueues() {
    localStorage.setItem('rusty-gun-sync-queues', JSON.stringify({
      outgoing: outgoingQueue,
      incoming: incomingQueue,
      conflicts: conflictQueue
    }))
  }

  function saveSyncSettings() {
    localStorage.setItem('rusty-gun-sync-settings', JSON.stringify(syncSettings))
  }

  function saveSyncHistory() {
    localStorage.setItem('rusty-gun-sync-history', JSON.stringify(syncHistory))
  }

  function savePerformanceMetrics() {
    localStorage.setItem('rusty-gun-performance-metrics', JSON.stringify(performanceMetrics))
  }

  function startSyncMonitoring() {
    // Simulate sync monitoring
    setInterval(() => {
      if (syncSettings.autoSync && syncStatus.isOnline) {
        performSync()
      }
    }, syncSettings.syncInterval * 1000)
  }

  async function performSync() {
    if (!syncStatus.isOnline) return

    try {
      // Simulate sync process
      await new Promise(resolve => setTimeout(resolve, 1000))
      
      // Update sync status
      syncStatus.lastSync = new Date()
      syncStatus.pendingChanges = 0
      syncStatus.dataTransferred += Math.random() * 1000
      
      // Add to history
      syncHistory.unshift({
        id: 'sync_' + Math.random().toString(36).substr(2, 9),
        timestamp: new Date(),
        status: 'success',
        dataTransferred: Math.random() * 1000,
        duration: Math.random() * 2000,
        conflictsResolved: Math.floor(Math.random() * 5)
      })
      
      // Keep only last 100 entries
      if (syncHistory.length > 100) {
        syncHistory = syncHistory.slice(0, 100)
      }
      
      saveSyncStatus()
      saveSyncHistory()
      
      toast.success('Sync completed successfully')
    } catch (error) {
      toast.error('Sync failed: ' + error.message)
    }
  }

  function forceSync() {
    performSync()
  }

  function pauseSync() {
    syncSettings.autoSync = false
    saveSyncSettings()
    toast.info('Auto-sync paused')
  }

  function resumeSync() {
    syncSettings.autoSync = true
    saveSyncSettings()
    toast.success('Auto-sync resumed')
  }

  function resolveConflict(conflictId: string, resolution: 'mine' | 'theirs' | 'merge') {
    const conflict = conflictQueue.find(c => c.id === conflictId)
    if (conflict) {
      conflict.resolution = resolution
      conflict.resolvedAt = new Date()
      conflictQueue = conflictQueue.filter(c => c.id !== conflictId)
      saveSyncQueues()
      toast.success('Conflict resolved')
    }
  }

  function clearSyncHistory() {
    syncHistory = []
    saveSyncHistory()
    toast.info('Sync history cleared')
  }

  function updateSyncSettings() {
    saveSyncSettings()
    toast.success('Sync settings updated')
  }

  function addToOutgoingQueue(nodeId: string, operation: string) {
    const item = {
      id: 'outgoing_' + Math.random().toString(36).substr(2, 9),
      nodeId,
      operation,
      timestamp: new Date(),
      status: 'pending',
      retries: 0
    }
    outgoingQueue.push(item)
    saveSyncQueues()
  }

  function addToIncomingQueue(nodeId: string, operation: string) {
    const item = {
      id: 'incoming_' + Math.random().toString(36).substr(2, 9),
      nodeId,
      operation,
      timestamp: new Date(),
      status: 'pending',
      retries: 0
    }
    incomingQueue.push(item)
    saveSyncQueues()
  }

  function addConflict(nodeId: string, localVersion: any, remoteVersion: any) {
    const conflict = {
      id: 'conflict_' + Math.random().toString(36).substr(2, 9),
      nodeId,
      localVersion,
      remoteVersion,
      timestamp: new Date(),
      status: 'pending'
    }
    conflictQueue.push(conflict)
    saveSyncQueues()
  }

  function getDeviceIcon(type: string): string {
    switch (type) {
      case 'laptop': return '💻'
      case 'phone': return '📱'
      case 'tablet': return '📱'
      case 'server': return '🖥️'
      case 'desktop': return '🖥️'
      default: return '📱'
    }
  }

  function getSyncStatusColor(status: string): string {
    switch (status) {
      case 'connected': return 'var(--success)'
      case 'disconnected': return 'var(--muted)'
      case 'syncing': return 'var(--warning)'
      case 'error': return 'var(--danger)'
      default: return 'var(--muted)'
    }
  }
</script>

<div class="cross-device-sync">
  <div class="header">
    <h2>Cross-Device Sync</h2>
    <p>Manage data synchronization across all your devices</p>
  </div>

  <div class="tabs">
    <button class="tab active">Sync Status</button>
    <button class="tab">Devices</button>
    <button class="tab">Queues</button>
    <button class="tab">Settings</button>
    <button class="tab">History</button>
  </div>

  <div class="tab-content">
    <!-- Sync Status Tab -->
    <div class="tab-panel active">
      <div class="sync-status">
        <div class="status-cards">
          <div class="status-card">
            <div class="status-icon">🔄</div>
            <div class="status-info">
              <h3>Sync Status</h3>
              <p class="status-text {syncStatus.isOnline ? 'online' : 'offline'}">
                {syncStatus.isOnline ? 'Online' : 'Offline'}
              </p>
            </div>
          </div>

          <div class="status-card">
            <div class="status-icon">⏰</div>
            <div class="status-info">
              <h3>Last Sync</h3>
              <p class="status-text">
                {syncStatus.lastSync ? syncStatus.lastSync.toLocaleString() : 'Never'}
              </p>
            </div>
          </div>

          <div class="status-card">
            <div class="status-icon">📊</div>
            <div class="status-info">
              <h3>Pending Changes</h3>
              <p class="status-text">{syncStatus.pendingChanges}</p>
            </div>
          </div>

          <div class="status-card">
            <div class="status-icon">🔧</div>
            <div class="status-info">
              <h3>Sync Mode</h3>
              <p class="status-text">{syncStatus.syncMode}</p>
            </div>
          </div>
        </div>

        <div class="sync-controls">
          <button 
            class="btn btn-primary"
            on:click={forceSync}
            disabled={!syncStatus.isOnline}
          >
            Force Sync
          </button>
          <button 
            class="btn btn-secondary"
            on:click={syncSettings.autoSync ? pauseSync : resumeSync}
          >
            {syncSettings.autoSync ? 'Pause Auto-Sync' : 'Resume Auto-Sync'}
          </button>
        </div>

        <div class="performance-metrics">
          <h3>Performance Metrics</h3>
          <div class="metrics-grid">
            <div class="metric">
              <span class="metric-label">Average Sync Time</span>
              <span class="metric-value">{performanceMetrics.averageSyncTime.toFixed(0)}ms</span>
            </div>
            <div class="metric">
              <span class="metric-label">Total Data Synced</span>
              <span class="metric-value">{(performanceMetrics.totalDataSynced / 1024).toFixed(1)}KB</span>
            </div>
            <div class="metric">
              <span class="metric-label">Sync Success Rate</span>
              <span class="metric-value">{performanceMetrics.syncSuccessRate.toFixed(1)}%</span>
            </div>
            <div class="metric">
              <span class="metric-label">Conflict Rate</span>
              <span class="metric-value">{performanceMetrics.conflictRate.toFixed(1)}%</span>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- Devices Tab -->
    <div class="tab-panel">
      <div class="device-form">
        <h3>Add New Device</h3>
        <div class="form-group">
          <label for="device-name">Device Name</label>
          <input 
            id="device-name"
            type="text" 
            bind:value={deviceForm.name}
            placeholder="Enter device name"
          />
        </div>

        <div class="form-row">
          <div class="form-group">
            <label for="device-type">Device Type</label>
            <select id="device-type" bind:value={deviceForm.type}>
              <option value="laptop">Laptop</option>
              <option value="phone">Phone</option>
              <option value="tablet">Tablet</option>
              <option value="desktop">Desktop</option>
              <option value="server">Server</option>
            </select>
          </div>

          <div class="form-group">
            <label for="device-location">Location</label>
            <input 
              id="device-location"
              type="text" 
              bind:value={deviceForm.location}
              placeholder="Enter location"
            />
          </div>
        </div>

        <div class="form-group">
          <label for="device-description">Description</label>
          <textarea 
            id="device-description"
            bind:value={deviceForm.description}
            placeholder="Optional description"
            rows="2"
          ></textarea>
        </div>

        <button class="btn btn-primary" on:click={addDevice}>
          Add Device
        </button>
      </div>

      <div class="connected-devices">
        <h3>Connected Devices</h3>
        {#if connectedDevices.length === 0}
          <p class="empty">No devices connected</p>
        {:else}
          {#each connectedDevices as device}
            <div class="device-card">
              <div class="device-info">
                <div class="device-icon">{getDeviceIcon(device.type)}</div>
                <div class="device-details">
                  <h4>{device.name}</h4>
                  <p>{device.type} • {device.location}</p>
                  <p>{device.description}</p>
                  <p class="last-seen">Last seen: {device.lastSeen.toLocaleString()}</p>
                </div>
              </div>
              <div class="device-status">
                <span 
                  class="status-indicator"
                  style="background-color: {getSyncStatusColor(device.syncStatus)}"
                ></span>
                <span class="status-text">{device.syncStatus}</span>
              </div>
              <div class="device-actions">
                {#if device.syncStatus === 'connected'}
                  <button 
                    class="btn btn-danger"
                    on:click={() => disconnectDevice(device.id)}
                  >
                    Disconnect
                  </button>
                {:else}
                  <button 
                    class="btn btn-success"
                    on:click={() => connectDevice(device.id)}
                  >
                    Connect
                  </button>
                {/if}
                <button 
                  class="btn btn-danger"
                  on:click={() => removeDevice(device.id)}
                >
                  Remove
                </button>
              </div>
            </div>
          {/each}
        {/if}
      </div>
    </div>

    <!-- Queues Tab -->
    <div class="tab-panel">
      <div class="sync-queues">
        <div class="queue-section">
          <h3>Outgoing Queue ({outgoingQueue.length})</h3>
          {#if outgoingQueue.length === 0}
            <p class="empty">No outgoing items</p>
          {:else}
            {#each outgoingQueue as item}
              <div class="queue-item">
                <div class="item-info">
                  <h4>Node: {item.nodeId}</h4>
                  <p>Operation: {item.operation}</p>
                  <p>Status: <span class="status {item.status}">{item.status}</span></p>
                  <p class="timestamp">Queued: {item.timestamp.toLocaleString()}</p>
                </div>
              </div>
            {/each}
          {/if}
        </div>

        <div class="queue-section">
          <h3>Incoming Queue ({incomingQueue.length})</h3>
          {#if incomingQueue.length === 0}
            <p class="empty">No incoming items</p>
          {:else}
            {#each incomingQueue as item}
              <div class="queue-item">
                <div class="item-info">
                  <h4>Node: {item.nodeId}</h4>
                  <p>Operation: {item.operation}</p>
                  <p>Status: <span class="status {item.status}">{item.status}</span></p>
                  <p class="timestamp">Queued: {item.timestamp.toLocaleString()}</p>
                </div>
              </div>
            {/each}
          {/if}
        </div>

        <div class="queue-section">
          <h3>Conflict Queue ({conflictQueue.length})</h3>
          {#if conflictQueue.length === 0}
            <p class="empty">No conflicts</p>
          {:else}
            {#each conflictQueue as conflict}
              <div class="conflict-item">
                <div class="conflict-info">
                  <h4>Node: {conflict.nodeId}</h4>
                  <p>Local Version: {conflict.localVersion}</p>
                  <p>Remote Version: {conflict.remoteVersion}</p>
                  <p class="timestamp">Detected: {conflict.timestamp.toLocaleString()}</p>
                </div>
                <div class="conflict-actions">
                  <button 
                    class="btn btn-success"
                    on:click={() => resolveConflict(conflict.id, 'mine')}
                  >
                    Use Mine
                  </button>
                  <button 
                    class="btn btn-warning"
                    on:click={() => resolveConflict(conflict.id, 'theirs')}
                  >
                    Use Theirs
                  </button>
                  <button 
                    class="btn btn-primary"
                    on:click={() => resolveConflict(conflict.id, 'merge')}
                  >
                    Merge
                  </button>
                </div>
              </div>
            {/each}
          {/if}
        </div>
      </div>
    </div>

    <!-- Settings Tab -->
    <div class="tab-panel">
      <div class="sync-settings">
        <h3>Sync Settings</h3>
        <div class="settings-form">
          <div class="form-group">
            <label>
              <input 
                type="checkbox" 
                bind:checked={syncSettings.autoSync}
              />
              Enable Auto-Sync
            </label>
          </div>

          <div class="form-group">
            <label for="sync-interval">Sync Interval (seconds)</label>
            <input 
              id="sync-interval"
              type="number" 
              bind:value={syncSettings.syncInterval}
              min="10"
              max="3600"
            />
          </div>

          <div class="form-group">
            <label for="max-retries">Max Retries</label>
            <input 
              id="max-retries"
              type="number" 
              bind:value={syncSettings.maxRetries}
              min="0"
              max="10"
            />
          </div>

          <div class="form-group">
            <label for="conflict-resolution">Conflict Resolution</label>
            <select id="conflict-resolution" bind:value={syncSettings.conflictResolution}>
              <option value="manual">Manual</option>
              <option value="automatic">Automatic</option>
              <option value="last-write-wins">Last Write Wins</option>
            </select>
          </div>

          <div class="form-group">
            <label for="compression-level">Compression Level</label>
            <input 
              id="compression-level"
              type="range" 
              bind:value={syncSettings.compressionLevel}
              min="1"
              max="9"
            />
            <span class="range-value">{syncSettings.compressionLevel}</span>
          </div>

          <div class="form-group">
            <label>
              <input 
                type="checkbox" 
                bind:checked={syncSettings.encryptionEnabled}
              />
              Enable Encryption
            </label>
          </div>

          <div class="form-group">
            <label for="bandwidth-limit">Bandwidth Limit</label>
            <select id="bandwidth-limit" bind:value={syncSettings.bandwidthLimit}>
              <option value="unlimited">Unlimited</option>
              <option value="1MB">1 MB/s</option>
              <option value="10MB">10 MB/s</option>
              <option value="100MB">100 MB/s</option>
            </select>
          </div>

          <div class="form-group">
            <label>
              <input 
                type="checkbox" 
                bind:checked={syncSettings.syncOnWiFiOnly}
              />
              Sync on WiFi Only
            </label>
          </div>

          <button class="btn btn-primary" on:click={updateSyncSettings}>
            Save Settings
          </button>
        </div>
      </div>
    </div>

    <!-- History Tab -->
    <div class="tab-panel">
      <div class="sync-history">
        <div class="history-header">
          <h3>Sync History</h3>
          <button 
            class="btn btn-danger"
            on:click={clearSyncHistory}
          >
            Clear History
          </button>
        </div>
        {#if syncHistory.length === 0}
          <p class="empty">No sync history</p>
        {:else}
          {#each syncHistory as entry}
            <div class="history-item">
              <div class="history-info">
                <h4>Sync {entry.status}</h4>
                <p>Data Transferred: {(entry.dataTransferred / 1024).toFixed(1)}KB</p>
                <p>Duration: {entry.duration.toFixed(0)}ms</p>
                <p>Conflicts Resolved: {entry.conflictsResolved}</p>
                <p class="timestamp">{entry.timestamp.toLocaleString()}</p>
              </div>
              <div class="history-status">
                <span class="status {entry.status}">{entry.status}</span>
              </div>
            </div>
          {/each}
        {/if}
      </div>
    </div>
  </div>
</div>

<style>
  .cross-device-sync {
    padding: 1rem;
    max-width: 1200px;
    margin: 0 auto;
  }

  .header {
    margin-bottom: 2rem;
  }

  .header h2 {
    margin: 0 0 0.5rem 0;
    color: var(--primary);
  }

  .header p {
    margin: 0;
    color: var(--muted);
  }

  .tabs {
    display: flex;
    border-bottom: 1px solid var(--border);
    margin-bottom: 2rem;
  }

  .tab {
    padding: 0.75rem 1.5rem;
    border: none;
    background: none;
    cursor: pointer;
    border-bottom: 2px solid transparent;
    transition: all 0.2s;
  }

  .tab:hover {
    background: var(--hover);
  }

  .tab.active {
    border-bottom-color: var(--primary);
    color: var(--primary);
  }

  .tab-content {
    min-height: 400px;
  }

  .tab-panel {
    display: none;
  }

  .tab-panel.active {
    display: block;
  }

  .status-cards {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(250px, 1fr));
    gap: 1rem;
    margin-bottom: 2rem;
  }

  .status-card {
    display: flex;
    align-items: center;
    padding: 1.5rem;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--background);
  }

  .status-icon {
    font-size: 2rem;
    margin-right: 1rem;
  }

  .status-info h3 {
    margin: 0 0 0.5rem 0;
    font-size: 1.1rem;
  }

  .status-text {
    margin: 0;
    font-weight: 500;
  }

  .status-text.online {
    color: var(--success);
  }

  .status-text.offline {
    color: var(--danger);
  }

  .sync-controls {
    display: flex;
    gap: 1rem;
    margin-bottom: 2rem;
  }

  .performance-metrics {
    background: var(--background);
    padding: 1.5rem;
    border-radius: 8px;
  }

  .performance-metrics h3 {
    margin: 0 0 1rem 0;
  }

  .metrics-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
    gap: 1rem;
  }

  .metric {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }

  .metric-label {
    font-size: 0.875rem;
    color: var(--muted);
  }

  .metric-value {
    font-size: 1.25rem;
    font-weight: 600;
    color: var(--primary);
  }

  .device-form {
    max-width: 600px;
    margin-bottom: 2rem;
  }

  .form-group {
    margin-bottom: 1.5rem;
  }

  .form-group label {
    display: block;
    margin-bottom: 0.5rem;
    font-weight: 500;
  }

  .form-group input,
  .form-group textarea,
  .form-group select {
    width: 100%;
    padding: 0.75rem;
    border: 1px solid var(--border);
    border-radius: 4px;
    font-size: 1rem;
  }

  .form-group textarea {
    resize: vertical;
    min-height: 80px;
  }

  .form-row {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 1rem;
  }

  .range-value {
    margin-left: 1rem;
    font-weight: 500;
  }

  .connected-devices {
    margin-top: 2rem;
  }

  .device-card {
    display: flex;
    align-items: center;
    padding: 1rem;
    border: 1px solid var(--border);
    border-radius: 8px;
    margin-bottom: 1rem;
  }

  .device-info {
    display: flex;
    align-items: center;
    flex: 1;
  }

  .device-icon {
    font-size: 2rem;
    margin-right: 1rem;
  }

  .device-details h4 {
    margin: 0 0 0.5rem 0;
  }

  .device-details p {
    margin: 0.25rem 0;
    color: var(--muted);
  }

  .device-details .last-seen {
    font-size: 0.875rem;
  }

  .device-status {
    display: flex;
    align-items: center;
    margin: 0 1rem;
  }

  .status-indicator {
    width: 12px;
    height: 12px;
    border-radius: 50%;
    margin-right: 0.5rem;
  }

  .device-actions {
    display: flex;
    gap: 0.5rem;
  }

  .sync-queues {
    display: grid;
    grid-template-columns: 1fr;
    gap: 2rem;
  }

  .queue-section h3 {
    margin: 0 0 1rem 0;
  }

  .queue-item,
  .conflict-item {
    padding: 1rem;
    border: 1px solid var(--border);
    border-radius: 4px;
    margin-bottom: 1rem;
  }

  .queue-item h4,
  .conflict-item h4 {
    margin: 0 0 0.5rem 0;
  }

  .queue-item p,
  .conflict-item p {
    margin: 0.25rem 0;
    color: var(--muted);
  }

  .queue-item .timestamp,
  .conflict-item .timestamp {
    font-size: 0.875rem;
  }

  .conflict-actions {
    display: flex;
    gap: 0.5rem;
    margin-top: 1rem;
  }

  .sync-settings {
    max-width: 600px;
  }

  .settings-form {
    background: var(--background);
    padding: 1.5rem;
    border-radius: 8px;
  }

  .history-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 1rem;
  }

  .history-item {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 1rem;
    border: 1px solid var(--border);
    border-radius: 4px;
    margin-bottom: 1rem;
  }

  .history-info h4 {
    margin: 0 0 0.5rem 0;
  }

  .history-info p {
    margin: 0.25rem 0;
    color: var(--muted);
  }

  .history-info .timestamp {
    font-size: 0.875rem;
  }

  .history-status .status {
    padding: 0.25rem 0.75rem;
    border-radius: 20px;
    font-size: 0.875rem;
    font-weight: 500;
  }

  .status.success {
    background: var(--success);
    color: white;
  }

  .status.error {
    background: var(--danger);
    color: white;
  }

  .status.pending {
    background: var(--warning);
    color: white;
  }

  .empty {
    text-align: center;
    color: var(--muted);
    font-style: italic;
    padding: 2rem;
  }

  .btn {
    padding: 0.5rem 1rem;
    border: none;
    border-radius: 4px;
    cursor: pointer;
    font-size: 0.875rem;
    font-weight: 500;
    transition: all 0.2s;
  }

  .btn:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  .btn-primary {
    background: var(--primary);
    color: white;
  }

  .btn-primary:hover:not(:disabled) {
    background: var(--primary-dark);
  }

  .btn-secondary {
    background: var(--secondary);
    color: var(--text);
  }

  .btn-secondary:hover {
    background: var(--secondary-dark);
  }

  .btn-success {
    background: var(--success);
    color: white;
  }

  .btn-success:hover {
    background: var(--success-dark);
  }

  .btn-danger {
    background: var(--danger);
    color: white;
  }

  .btn-danger:hover {
    background: var(--danger-dark);
  }

  .btn-warning {
    background: var(--warning);
    color: white;
  }

  .btn-warning:hover {
    background: var(--warning-dark);
  }
</style>
