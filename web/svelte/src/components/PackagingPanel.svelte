<script lang="ts">
  import { onMount } from 'svelte'
  import { push as toast } from '../lib/toasts'
  
  let dark = false
  let activeTab: 'docker' | 'windows' | 'updates' | 'deploy' = 'docker'
  let dockerStatus = {
    imageBuilt: false,
    containerRunning: false,
    lastBuild: null as number | null,
    imageSize: '0 MB',
    containerId: ''
  }
  let windowsStatus = {
    msiBuilt: false,
    installerSize: '0 MB',
    lastBuild: null as number | null,
    wingetReady: false
  }
  let updateStatus = {
    currentVersion: '1.0.0',
    latestVersion: '1.0.0',
    updateAvailable: false,
    lastCheck: null as number | null,
    releaseChannel: 'stable'
  }
  let deploymentStatus = {
    environment: 'local',
    status: 'running',
    lastDeploy: null as number | null,
    healthCheck: 'healthy'
  }
  let isBuilding = false
  let buildProgress = 0
  let buildLogs: string[] = []
  let showLogs = false
  
  $: dark = (document.documentElement.getAttribute('data-theme') === 'dark')
  
  onMount(() => {
    loadPackagingData()
  })
  
  async function loadPackagingData() {
    try {
      // Simulate packaging data
      dockerStatus = {
        imageBuilt: true,
        containerRunning: true,
        lastBuild: Date.now() - 3600000, // 1 hour ago
        imageSize: '245 MB',
        containerId: 'pluresdb-123'
      }
      
      windowsStatus = {
        msiBuilt: false,
        installerSize: '0 MB',
        lastBuild: null,
        wingetReady: false
      }
      
      updateStatus = {
        currentVersion: '1.0.0',
        latestVersion: '1.1.0',
        updateAvailable: true,
        lastCheck: Date.now() - 1800000, // 30 minutes ago
        releaseChannel: 'stable'
      }
      
      deploymentStatus = {
        environment: 'local',
        status: 'running',
        lastDeploy: Date.now() - 7200000, // 2 hours ago
        healthCheck: 'healthy'
      }
    } catch (error) {
      toast('Failed to load packaging data', 'error')
      console.error('Error loading packaging data:', error)
    }
  }
  
  async function buildDockerImage() {
    if (isBuilding) return
    
    isBuilding = true
    buildProgress = 0
    buildLogs = []
    showLogs = true
    
    try {
      // Simulate Docker build
      const steps = [
        'Building Docker image...',
        'Installing dependencies...',
        'Copying source code...',
        'Building application...',
        'Optimizing image...',
        'Pushing to registry...'
      ]
      
      for (let i = 0; i < steps.length; i++) {
        buildLogs.push(steps[i])
        buildProgress = ((i + 1) / steps.length) * 100
        await new Promise(resolve => setTimeout(resolve, 1000))
      }
      
      dockerStatus.imageBuilt = true
      dockerStatus.lastBuild = Date.now()
      dockerStatus.imageSize = '245 MB'
      dockerStatus.containerId = `pluresdb-${Date.now()}`
      
      toast('Docker image built successfully', 'success')
    } catch (error) {
      toast('Docker build failed', 'error')
      console.error('Docker build error:', error)
    } finally {
      isBuilding = false
    }
  }
  
  async function runDockerContainer() {
    try {
      // Simulate container start
      await new Promise(resolve => setTimeout(resolve, 2000))
      dockerStatus.containerRunning = true
      toast('Docker container started', 'success')
    } catch (error) {
      toast('Failed to start container', 'error')
      console.error('Container start error:', error)
    }
  }
  
  async function stopDockerContainer() {
    try {
      // Simulate container stop
      await new Promise(resolve => setTimeout(resolve, 1000))
      dockerStatus.containerRunning = false
      toast('Docker container stopped', 'success')
    } catch (error) {
      toast('Failed to stop container', 'error')
      console.error('Container stop error:', error)
    }
  }
  
  async function buildWindowsInstaller() {
    if (isBuilding) return
    
    isBuilding = true
    buildProgress = 0
    buildLogs = []
    showLogs = true
    
    try {
      // Simulate Windows installer build
      const steps = [
        'Preparing Windows installer...',
        'Collecting files...',
        'Creating MSI package...',
        'Signing installer...',
        'Creating Winget manifest...',
        'Finalizing package...'
      ]
      
      for (let i = 0; i < steps.length; i++) {
        buildLogs.push(steps[i])
        buildProgress = ((i + 1) / steps.length) * 100
        await new Promise(resolve => setTimeout(resolve, 1500))
      }
      
      windowsStatus.msiBuilt = true
      windowsStatus.lastBuild = Date.now()
      windowsStatus.installerSize = '156 MB'
      windowsStatus.wingetReady = true
      
      toast('Windows installer built successfully', 'success')
    } catch (error) {
      toast('Windows installer build failed', 'error')
      console.error('Windows build error:', error)
    } finally {
      isBuilding = false
    }
  }
  
  async function checkForUpdates() {
    try {
      // Simulate update check
      await new Promise(resolve => setTimeout(resolve, 2000))
      updateStatus.lastCheck = Date.now()
      updateStatus.latestVersion = '1.1.0'
      updateStatus.updateAvailable = updateStatus.currentVersion !== updateStatus.latestVersion
      toast('Update check completed', 'success')
    } catch (error) {
      toast('Update check failed', 'error')
      console.error('Update check error:', error)
    }
  }
  
  async function installUpdate() {
    try {
      // Simulate update installation
      await new Promise(resolve => setTimeout(resolve, 5000))
      updateStatus.currentVersion = updateStatus.latestVersion
      updateStatus.updateAvailable = false
      toast('Update installed successfully', 'success')
    } catch (error) {
      toast('Update installation failed', 'error')
      console.error('Update installation error:', error)
    }
  }
  
  async function deployToProduction() {
    try {
      // Simulate production deployment
      await new Promise(resolve => setTimeout(resolve, 3000))
      deploymentStatus.environment = 'production'
      deploymentStatus.lastDeploy = Date.now()
      deploymentStatus.healthCheck = 'healthy'
      toast('Deployed to production successfully', 'success')
    } catch (error) {
      toast('Production deployment failed', 'error')
      console.error('Deployment error:', error)
    }
  }
  
  function formatTimestamp(timestamp: number): string {
    return new Date(timestamp).toLocaleString()
  }
  
  function getStatusColor(status: string): string {
    switch (status) {
      case 'running': return 'var(--success-color)'
      case 'stopped': return 'var(--error-color)'
      case 'healthy': return 'var(--success-color)'
      case 'unhealthy': return 'var(--error-color)'
      default: return 'var(--muted-color)'
    }
  }
  
  function getStatusIcon(status: string): string {
    switch (status) {
      case 'running': return '🟢'
      case 'stopped': return '🔴'
      case 'healthy': return '✅'
      case 'unhealthy': return '❌'
      default: return '⚪'
    }
  }
</script>

<section aria-labelledby="packaging-panel-heading">
  <h3 id="packaging-panel-heading">Packaging & Deployment</h3>
  
  <div class="packaging-layout">
    <!-- Tabs -->
    <div class="packaging-tabs">
      <button 
        class="tab-button"
        class:active={activeTab === 'docker'}
        on:click={() => activeTab = 'docker'}
      >
        Docker
      </button>
      <button 
        class="tab-button"
        class:active={activeTab === 'windows'}
        on:click={() => activeTab = 'windows'}
      >
        Windows
      </button>
      <button 
        class="tab-button"
        class:active={activeTab === 'updates'}
        on:click={() => activeTab = 'updates'}
      >
        Updates
      </button>
      <button 
        class="tab-button"
        class:active={activeTab === 'deploy'}
        on:click={() => activeTab = 'deploy'}
      >
        Deploy
      </button>
    </div>
    
    <!-- Content -->
    <div class="packaging-content">
      {#if activeTab === 'docker'}
        <div class="docker-section">
          <h4>Docker Containerization</h4>
          
          <div class="docker-status">
            <div class="status-card">
              <div class="status-header">
                <span class="status-title">Image Status</span>
                <span class="status-value" style="color: {getStatusColor(dockerStatus.imageBuilt ? 'running' : 'stopped')}">
                  {getStatusIcon(dockerStatus.imageBuilt ? 'running' : 'stopped')} 
                  {dockerStatus.imageBuilt ? 'Built' : 'Not Built'}
                </span>
              </div>
              <div class="status-details">
                <span>Size: {dockerStatus.imageSize}</span>
                {#if dockerStatus.lastBuild}
                  <span>Last build: {formatTimestamp(dockerStatus.lastBuild)}</span>
                {/if}
              </div>
            </div>
            
            <div class="status-card">
              <div class="status-header">
                <span class="status-title">Container Status</span>
                <span class="status-value" style="color: {getStatusColor(dockerStatus.containerRunning ? 'running' : 'stopped')}">
                  {getStatusIcon(dockerStatus.containerRunning ? 'running' : 'stopped')} 
                  {dockerStatus.containerRunning ? 'Running' : 'Stopped'}
                </span>
              </div>
              <div class="status-details">
                {#if dockerStatus.containerId}
                  <span>ID: {dockerStatus.containerId}</span>
                {/if}
              </div>
            </div>
          </div>
          
          <div class="docker-actions">
            <button on:click={buildDockerImage} disabled={isBuilding} class="primary">
              {isBuilding ? 'Building...' : 'Build Image'}
            </button>
            {#if dockerStatus.imageBuilt}
              {#if dockerStatus.containerRunning}
                <button on:click={stopDockerContainer} class="secondary">
                  Stop Container
                </button>
              {:else}
                <button on:click={runDockerContainer} class="secondary">
                  Run Container
                </button>
              {/if}
            {/if}
          </div>
          
          {#if showLogs && buildLogs.length > 0}
            <div class="build-logs">
              <h5>Build Logs</h5>
              <div class="logs-content">
                {#each buildLogs as log}
                  <div class="log-line">{log}</div>
                {/each}
              </div>
            </div>
          {/if}
        </div>
      {:else if activeTab === 'windows'}
        <div class="windows-section">
          <h4>Windows Packaging</h4>
          
          <div class="windows-status">
            <div class="status-card">
              <div class="status-header">
                <span class="status-title">MSI Installer</span>
                <span class="status-value" style="color: {getStatusColor(windowsStatus.msiBuilt ? 'running' : 'stopped')}">
                  {getStatusIcon(windowsStatus.msiBuilt ? 'running' : 'stopped')} 
                  {windowsStatus.msiBuilt ? 'Built' : 'Not Built'}
                </span>
              </div>
              <div class="status-details">
                <span>Size: {windowsStatus.installerSize}</span>
                {#if windowsStatus.lastBuild}
                  <span>Last build: {formatTimestamp(windowsStatus.lastBuild)}</span>
                {/if}
              </div>
            </div>
            
            <div class="status-card">
              <div class="status-header">
                <span class="status-title">Winget Package</span>
                <span class="status-value" style="color: {getStatusColor(windowsStatus.wingetReady ? 'running' : 'stopped')}">
                  {getStatusIcon(windowsStatus.wingetReady ? 'running' : 'stopped')} 
                  {windowsStatus.wingetReady ? 'Ready' : 'Not Ready'}
                </span>
              </div>
              <div class="status-details">
                <span>Package: pluresdb</span>
              </div>
            </div>
          </div>
          
          <div class="windows-actions">
            <button on:click={buildWindowsInstaller} disabled={isBuilding} class="primary">
              {isBuilding ? 'Building...' : 'Build MSI'}
            </button>
            {#if windowsStatus.msiBuilt}
              <button class="secondary">
                Download MSI
              </button>
            {/if}
            {#if windowsStatus.wingetReady}
              <button class="secondary">
                Publish to Winget
              </button>
            {/if}
          </div>
        </div>
      {:else if activeTab === 'updates'}
        <div class="updates-section">
          <h4>Update Management</h4>
          
          <div class="update-status">
            <div class="status-card">
              <div class="status-header">
                <span class="status-title">Current Version</span>
                <span class="status-value">{updateStatus.currentVersion}</span>
              </div>
            </div>
            
            <div class="status-card">
              <div class="status-header">
                <span class="status-title">Latest Version</span>
                <span class="status-value">{updateStatus.latestVersion}</span>
              </div>
            </div>
            
            <div class="status-card">
              <div class="status-header">
                <span class="status-title">Update Available</span>
                <span class="status-value" style="color: {updateStatus.updateAvailable ? 'var(--warning-color)' : 'var(--success-color)'}">
                  {updateStatus.updateAvailable ? '🔄 Yes' : '✅ No'}
                </span>
              </div>
            </div>
            
            <div class="status-card">
              <div class="status-header">
                <span class="status-title">Release Channel</span>
                <span class="status-value">{updateStatus.releaseChannel}</span>
              </div>
            </div>
          </div>
          
          <div class="update-actions">
            <button on:click={checkForUpdates} class="primary">
              Check for Updates
            </button>
            {#if updateStatus.updateAvailable}
              <button on:click={installUpdate} class="secondary">
                Install Update
              </button>
            {/if}
          </div>
          
          {#if updateStatus.lastCheck}
            <div class="update-info">
              <p>Last checked: {formatTimestamp(updateStatus.lastCheck)}</p>
            </div>
          {/if}
        </div>
      {:else if activeTab === 'deploy'}
        <div class="deploy-section">
          <h4>Deployment Management</h4>
          
          <div class="deploy-status">
            <div class="status-card">
              <div class="status-header">
                <span class="status-title">Environment</span>
                <span class="status-value">{deploymentStatus.environment}</span>
              </div>
            </div>
            
            <div class="status-card">
              <div class="status-header">
                <span class="status-title">Status</span>
                <span class="status-value" style="color: {getStatusColor(deploymentStatus.status)}">
                  {getStatusIcon(deploymentStatus.status)} {deploymentStatus.status}
                </span>
              </div>
            </div>
            
            <div class="status-card">
              <div class="status-header">
                <span class="status-title">Health Check</span>
                <span class="status-value" style="color: {getStatusColor(deploymentStatus.healthCheck)}">
                  {getStatusIcon(deploymentStatus.healthCheck)} {deploymentStatus.healthCheck}
                </span>
              </div>
            </div>
            
            <div class="status-card">
              <div class="status-header">
                <span class="status-title">Last Deploy</span>
                <span class="status-value">
                  {deploymentStatus.lastDeploy ? formatTimestamp(deploymentStatus.lastDeploy) : 'Never'}
                </span>
              </div>
            </div>
          </div>
          
          <div class="deploy-actions">
            <button on:click={deployToProduction} class="primary">
              Deploy to Production
            </button>
            <button class="secondary">
              Deploy to Staging
            </button>
            <button class="secondary">
              Rollback
            </button>
          </div>
        </div>
      {/if}
    </div>
  </div>
</section>

<style>
  .packaging-layout {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }
  
  .packaging-tabs {
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
  
  .packaging-content {
    flex: 1;
    padding: 1rem;
    background: var(--pico-background-color);
    border-radius: 8px;
    border: 1px solid var(--pico-muted-border-color);
  }
  
  .docker-status, .windows-status, .update-status, .deploy-status {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(250px, 1fr));
    gap: 1rem;
    margin-bottom: 1.5rem;
  }
  
  .status-card {
    padding: 1rem;
    border: 1px solid var(--pico-muted-border-color);
    border-radius: 8px;
    background: var(--pico-muted-border-color);
  }
  
  .status-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 0.5rem;
  }
  
  .status-title {
    font-weight: 600;
    font-size: 0.875rem;
  }
  
  .status-value {
    font-weight: 600;
    font-size: 0.875rem;
  }
  
  .status-details {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    font-size: 0.75rem;
    opacity: 0.8;
  }
  
  .docker-actions, .windows-actions, .update-actions, .deploy-actions {
    display: flex;
    gap: 0.5rem;
    margin-bottom: 1rem;
  }
  
  .build-logs {
    border: 1px solid var(--pico-muted-border-color);
    border-radius: 8px;
    padding: 1rem;
    background: var(--pico-muted-border-color);
  }
  
  .build-logs h5 {
    margin-bottom: 0.5rem;
  }
  
  .logs-content {
    max-height: 200px;
    overflow-y: auto;
    font-family: monospace;
    font-size: 0.75rem;
  }
  
  .log-line {
    padding: 0.25rem 0;
    border-bottom: 1px solid var(--pico-muted-border-color);
  }
  
  .update-info {
    margin-top: 1rem;
    padding: 1rem;
    background: var(--pico-muted-border-color);
    border-radius: 8px;
  }
  
  .update-info p {
    margin: 0;
    font-size: 0.875rem;
    opacity: 0.8;
  }
  
  @media (max-width: 768px) {
    .packaging-tabs {
      flex-wrap: wrap;
    }
    
    .tab-button {
      flex: 1;
      min-width: 120px;
    }
    
    .docker-status, .windows-status, .update-status, .deploy-status {
      grid-template-columns: 1fr;
    }
    
    .docker-actions, .windows-actions, .update-actions, .deploy-actions {
      flex-direction: column;
    }
  }
</style>
