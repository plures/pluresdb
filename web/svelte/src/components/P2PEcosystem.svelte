<script lang="ts">
  import { onMount } from "svelte";
  import { push as toast } from "../lib/toasts";

  let dark = false;
  let activeTab: "network" | "security" | "sync" | "offline" | "ecosystem" = "network";
  let networkNodes: Array<{
    id: string;
    address: string;
    status: "online" | "offline" | "syncing";
    lastSeen: number;
    latency: number;
    bandwidth: { incoming: number; outgoing: number };
    capabilities: string[];
    trustLevel: "high" | "medium" | "low";
    dataShared: number;
    encryptionEnabled: boolean;
  }> = [];
  let securityConfig = {
    encryption: "AES-256-GCM",
    keyExchange: "ECDH-P256",
    authentication: "Ed25519",
    dataIntegrity: "SHA-256",
    forwardSecrecy: true,
    perfectForwardSecrecy: true,
    zeroKnowledgeProofs: false,
    homomorphicEncryption: false,
  };
  let syncStatus = {
    lastSync: null as number | null,
    pendingChanges: 0,
    conflictsResolved: 0,
    dataTransferred: 0,
    syncMode: "realtime" as "realtime" | "batch" | "manual",
    compressionEnabled: true,
    deltaSync: true,
  };
  let offlineCapabilities = {
    localStorage: true,
    conflictResolution: true,
    queueOperations: true,
    backgroundSync: true,
    dataReplication: true,
    offlineQueries: true,
    localIndexing: true,
    cacheManagement: true,
  };
  let ecosystemApps: Array<{
    id: string;
    name: string;
    version: string;
    status: "active" | "inactive" | "updating";
    permissions: string[];
    dataAccess: string[];
    lastActivity: number;
    trustScore: number;
  }> = [];
  let selectedNode: any = null;
  let showNodeDialog = false;
  let newNode = { address: "", capabilities: [], trustLevel: "medium" as const };

  $: dark = document.documentElement.getAttribute("data-theme") === "dark";

  onMount(() => {
    loadP2PData();
  });

  async function loadP2PData() {
    try {
      // Simulate P2P ecosystem data
      networkNodes = [
        {
          id: "node-1",
          address: "192.168.1.100:34567",
          status: "online",
          lastSeen: Date.now() - 30000, // 30 seconds ago
          latency: 15,
          bandwidth: { incoming: 1024, outgoing: 512 },
          capabilities: ["data-sync", "encryption", "offline-support"],
          trustLevel: "high",
          dataShared: 1024000, // 1MB
          encryptionEnabled: true,
        },
        {
          id: "node-2",
          address: "10.0.0.50:34567",
          status: "syncing",
          lastSeen: Date.now() - 60000, // 1 minute ago
          latency: 45,
          bandwidth: { incoming: 2048, outgoing: 1024 },
          capabilities: ["data-sync", "encryption", "offline-support", "mesh-routing"],
          trustLevel: "medium",
          dataShared: 2048000, // 2MB
          encryptionEnabled: true,
        },
        {
          id: "node-3",
          address: "172.16.0.25:34567",
          status: "offline",
          lastSeen: Date.now() - 300000, // 5 minutes ago
          latency: 0,
          bandwidth: { incoming: 0, outgoing: 0 },
          capabilities: ["data-sync", "encryption"],
          trustLevel: "low",
          dataShared: 512000, // 512KB
          encryptionEnabled: false,
        },
      ];

      ecosystemApps = [
        {
          id: "app-1",
          name: "Data Explorer",
          version: "1.2.0",
          status: "active",
          permissions: ["read:data", "write:data", "sync:data"],
          dataAccess: ["users", "posts", "analytics"],
          lastActivity: Date.now() - 120000, // 2 minutes ago
          trustScore: 95,
        },
        {
          id: "app-2",
          name: "Analytics Dashboard",
          version: "2.1.0",
          status: "active",
          permissions: ["read:data", "read:analytics"],
          dataAccess: ["analytics", "metrics"],
          lastActivity: Date.now() - 300000, // 5 minutes ago
          trustScore: 88,
        },
        {
          id: "app-3",
          name: "Mobile Sync",
          version: "1.0.5",
          status: "updating",
          permissions: ["read:data", "write:data", "sync:data", "offline:access"],
          dataAccess: ["users", "posts"],
          lastActivity: Date.now() - 600000, // 10 minutes ago
          trustScore: 92,
        },
      ];
    } catch (error) {
      toast("Failed to load P2P ecosystem data", "error");
      console.error("Error loading P2P data:", error);
    }
  }

  function selectNode(node: any) {
    selectedNode = node;
  }

  async function connectToNode(address: string) {
    try {
      // Simulate node connection
      await new Promise((resolve) => setTimeout(resolve, 2000));
      toast("Connected to node successfully", "success");
    } catch (error) {
      toast("Failed to connect to node", "error");
      console.error("Connect to node error:", error);
    }
  }

  async function disconnectFromNode(nodeId: string) {
    try {
      const node = networkNodes.find((n) => n.id === nodeId);
      if (node) {
        node.status = "offline";
        toast("Disconnected from node", "success");
      }
    } catch (error) {
      toast("Failed to disconnect from node", "error");
      console.error("Disconnect from node error:", error);
    }
  }

  async function startSync() {
    try {
      syncStatus.syncMode = "realtime";
      syncStatus.lastSync = Date.now();
      toast("Sync started", "success");
    } catch (error) {
      toast("Failed to start sync", "error");
      console.error("Start sync error:", error);
    }
  }

  async function stopSync() {
    try {
      syncStatus.syncMode = "manual";
      toast("Sync stopped", "success");
    } catch (error) {
      toast("Failed to stop sync", "error");
      console.error("Stop sync error:", error);
    }
  }

  async function resolveConflicts() {
    try {
      // Simulate conflict resolution
      await new Promise((resolve) => setTimeout(resolve, 1000));
      syncStatus.conflictsResolved += 1;
      toast("Conflicts resolved successfully", "success");
    } catch (error) {
      toast("Failed to resolve conflicts", "error");
      console.error("Resolve conflicts error:", error);
    }
  }

  async function enableOfflineMode() {
    try {
      offlineCapabilities.localStorage = true;
      offlineCapabilities.queueOperations = true;
      toast("Offline mode enabled", "success");
    } catch (error) {
      toast("Failed to enable offline mode", "error");
      console.error("Enable offline mode error:", error);
    }
  }

  async function disableOfflineMode() {
    try {
      offlineCapabilities.localStorage = false;
      offlineCapabilities.queueOperations = false;
      toast("Offline mode disabled", "success");
    } catch (error) {
      toast("Failed to disable offline mode", "error");
      console.error("Disable offline mode error:", error);
    }
  }

  async function addEcosystemApp(app: any) {
    try {
      const newApp = {
        id: `app-${Date.now()}`,
        name: app.name,
        version: app.version,
        status: "active" as const,
        permissions: app.permissions || [],
        dataAccess: app.dataAccess || [],
        lastActivity: Date.now(),
        trustScore: 85,
      };

      ecosystemApps = [...ecosystemApps, newApp];
      toast("Ecosystem app added successfully", "success");
    } catch (error) {
      toast("Failed to add ecosystem app", "error");
      console.error("Add ecosystem app error:", error);
    }
  }

  async function removeEcosystemApp(appId: string) {
    try {
      ecosystemApps = ecosystemApps.filter((app) => app.id !== appId);
      toast("Ecosystem app removed", "success");
    } catch (error) {
      toast("Failed to remove ecosystem app", "error");
      console.error("Remove ecosystem app error:", error);
    }
  }

  function formatBytes(bytes: number): string {
    if (bytes === 0) return "0 B";
    const k = 1024;
    const sizes = ["B", "KB", "MB", "GB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + " " + sizes[i];
  }

  function formatTimestamp(timestamp: number): string {
    return new Date(timestamp).toLocaleString();
  }

  function getStatusColor(status: string): string {
    switch (status) {
      case "online":
        return "var(--success-color)";
      case "offline":
        return "var(--error-color)";
      case "syncing":
        return "var(--warning-color)";
      case "active":
        return "var(--success-color)";
      case "inactive":
        return "var(--muted-color)";
      case "updating":
        return "var(--warning-color)";
      default:
        return "var(--muted-color)";
    }
  }

  function getStatusIcon(status: string): string {
    switch (status) {
      case "online":
        return "🟢";
      case "offline":
        return "🔴";
      case "syncing":
        return "🟡";
      case "active":
        return "✅";
      case "inactive":
        return "⏸️";
      case "updating":
        return "🔄";
      default:
        return "⚪";
    }
  }

  function getTrustColor(trustLevel: string): string {
    switch (trustLevel) {
      case "high":
        return "var(--success-color)";
      case "medium":
        return "var(--warning-color)";
      case "low":
        return "var(--error-color)";
      default:
        return "var(--muted-color)";
    }
  }
</script>

<section aria-labelledby="p2p-ecosystem-heading">
  <h3 id="p2p-ecosystem-heading">P2P Ecosystem Foundation</h3>

  <div class="p2p-layout">
    <!-- Tabs -->
    <div class="p2p-tabs">
      <button
        class="tab-button"
        class:active={activeTab === "network"}
        on:click={() => (activeTab = "network")}
      >
        Network ({networkNodes.length})
      </button>
      <button
        class="tab-button"
        class:active={activeTab === "security"}
        on:click={() => (activeTab = "security")}
      >
        Security
      </button>
      <button
        class="tab-button"
        class:active={activeTab === "sync"}
        on:click={() => (activeTab = "sync")}
      >
        Sync
      </button>
      <button
        class="tab-button"
        class:active={activeTab === "offline"}
        on:click={() => (activeTab = "offline")}
      >
        Offline
      </button>
      <button
        class="tab-button"
        class:active={activeTab === "ecosystem"}
        on:click={() => (activeTab = "ecosystem")}
      >
        Ecosystem ({ecosystemApps.length})
      </button>
    </div>

    <!-- Content -->
    <div class="p2p-content">
      {#if activeTab === "network"}
        <div class="network-section">
          <div class="section-header">
            <h4>P2P Network</h4>
            <button on:click={() => (showNodeDialog = true)} class="primary"> Add Node </button>
          </div>

          <div class="network-stats">
            <div class="stat-card">
              <div class="stat-title">Online Nodes</div>
              <div class="stat-value">
                {networkNodes.filter((n) => n.status === "online").length}
              </div>
            </div>
            <div class="stat-card">
              <div class="stat-title">Total Bandwidth</div>
              <div class="stat-value">
                {formatBytes(
                  networkNodes.reduce(
                    (sum, n) => sum + n.bandwidth.incoming + n.bandwidth.outgoing,
                    0,
                  ),
                )}/s
              </div>
            </div>
            <div class="stat-card">
              <div class="stat-title">Data Shared</div>
              <div class="stat-value">
                {formatBytes(networkNodes.reduce((sum, n) => sum + n.dataShared, 0))}
              </div>
            </div>
          </div>

          <div class="nodes-list">
            {#each networkNodes as node}
              <div
                class="node-item"
                class:selected={selectedNode?.id === node.id}
                role="button"
                tabindex="0"
                on:click={() => selectNode(node)}
                on:keydown={(e) => e.key === "Enter" && selectNode(node)}
              >
                <div class="node-header">
                  <div class="node-info">
                    <span class="node-address">{node.address}</span>
                    <span class="node-status" style="color: {getStatusColor(node.status)}">
                      {getStatusIcon(node.status)}
                      {node.status}
                    </span>
                  </div>
                  <div class="node-trust" style="color: {getTrustColor(node.trustLevel)}">
                    Trust: {node.trustLevel}
                  </div>
                </div>
                <div class="node-details">
                  <div class="node-metrics">
                    <span>Latency: {node.latency}ms</span>
                    <span
                      >Bandwidth: {formatBytes(node.bandwidth.incoming)}/s in, {formatBytes(
                        node.bandwidth.outgoing,
                      )}/s out</span
                    >
                    <span>Data Shared: {formatBytes(node.dataShared)}</span>
                  </div>
                  <div class="node-capabilities">
                    {#each node.capabilities as capability}
                      <span class="capability-tag">{capability}</span>
                    {/each}
                  </div>
                </div>
                <div class="node-actions">
                  {#if node.status === "online"}
                    <button
                      on:click|stopPropagation={() => disconnectFromNode(node.id)}
                      class="small error"
                    >
                      Disconnect
                    </button>
                  {:else}
                    <button
                      on:click|stopPropagation={() => connectToNode(node.address)}
                      class="small success"
                    >
                      Connect
                    </button>
                  {/if}
                </div>
              </div>
            {/each}
          </div>
        </div>
      {:else if activeTab === "security"}
        <div class="security-section">
          <h4>Security Configuration</h4>

          <div class="security-grid">
            <div class="security-item">
              <label for="encryption-algorithm">Encryption Algorithm</label>
              <select id="encryption-algorithm" bind:value={securityConfig.encryption}>
                <option value="AES-256-GCM">AES-256-GCM</option>
                <option value="ChaCha20-Poly1305">ChaCha20-Poly1305</option>
                <option value="AES-128-GCM">AES-128-GCM</option>
              </select>
            </div>

            <div class="security-item">
              <label for="key-exchange">Key Exchange</label>
              <select id="key-exchange" bind:value={securityConfig.keyExchange}>
                <option value="ECDH-P256">ECDH-P256</option>
                <option value="ECDH-P384">ECDH-P384</option>
                <option value="ECDH-P521">ECDH-P521</option>
                <option value="X25519">X25519</option>
              </select>
            </div>

            <div class="security-item">
              <label for="authentication-method">Authentication</label>
              <select id="authentication-method" bind:value={securityConfig.authentication}>
                <option value="Ed25519">Ed25519</option>
                <option value="RSA-2048">RSA-2048</option>
                <option value="RSA-4096">RSA-4096</option>
                <option value="ECDSA-P256">ECDSA-P256</option>
              </select>
            </div>

            <div class="security-item">
              <label for="data-integrity">Data Integrity</label>
              <select id="data-integrity" bind:value={securityConfig.dataIntegrity}>
                <option value="SHA-256">SHA-256</option>
                <option value="SHA-384">SHA-384</option>
                <option value="SHA-512">SHA-512</option>
                <option value="BLAKE3">BLAKE3</option>
              </select>
            </div>

            <div class="security-item">
              <label>
                <input type="checkbox" bind:checked={securityConfig.forwardSecrecy} />
                Forward Secrecy
              </label>
            </div>

            <div class="security-item">
              <label>
                <input type="checkbox" bind:checked={securityConfig.perfectForwardSecrecy} />
                Perfect Forward Secrecy
              </label>
            </div>

            <div class="security-item">
              <label>
                <input type="checkbox" bind:checked={securityConfig.zeroKnowledgeProofs} />
                Zero-Knowledge Proofs
              </label>
            </div>

            <div class="security-item">
              <label>
                <input type="checkbox" bind:checked={securityConfig.homomorphicEncryption} />
                Homomorphic Encryption
              </label>
            </div>
          </div>
        </div>
      {:else if activeTab === "sync"}
        <div class="sync-section">
          <h4>Data Synchronization</h4>

          <div class="sync-status">
            <div class="status-card">
              <div class="status-title">Sync Mode</div>
              <div class="status-value">{syncStatus.syncMode}</div>
            </div>

            <div class="status-card">
              <div class="status-title">Last Sync</div>
              <div class="status-value">
                {syncStatus.lastSync ? formatTimestamp(syncStatus.lastSync) : "Never"}
              </div>
            </div>

            <div class="status-card">
              <div class="status-title">Pending Changes</div>
              <div class="status-value">{syncStatus.pendingChanges}</div>
            </div>

            <div class="status-card">
              <div class="status-title">Conflicts Resolved</div>
              <div class="status-value">{syncStatus.conflictsResolved}</div>
            </div>

            <div class="status-card">
              <div class="status-title">Data Transferred</div>
              <div class="status-value">{formatBytes(syncStatus.dataTransferred)}</div>
            </div>
          </div>

          <div class="sync-controls">
            <button
              on:click={startSync}
              disabled={syncStatus.syncMode === "realtime"}
              class="primary"
            >
              Start Sync
            </button>
            <button
              on:click={stopSync}
              disabled={syncStatus.syncMode === "manual"}
              class="secondary"
            >
              Stop Sync
            </button>
            <button
              on:click={resolveConflicts}
              disabled={syncStatus.pendingChanges === 0}
              class="secondary"
            >
              Resolve Conflicts
            </button>
          </div>

          <div class="sync-settings">
            <label>
              <input type="checkbox" bind:checked={syncStatus.compressionEnabled} />
              Enable Compression
            </label>
            <label>
              <input type="checkbox" bind:checked={syncStatus.deltaSync} />
              Delta Sync
            </label>
          </div>
        </div>
      {:else if activeTab === "offline"}
        <div class="offline-section">
          <h4>Offline-First Capabilities</h4>

          <div class="offline-grid">
            {#each Object.entries(offlineCapabilities) as [capability, enabled]}
              <div class="offline-item">
                <label>
                  <input type="checkbox" bind:checked={offlineCapabilities[capability]} />
                  {capability.replace(/([A-Z])/g, " $1").replace(/^./, (str) => str.toUpperCase())}
                </label>
              </div>
            {/each}
          </div>

          <div class="offline-actions">
            <button on:click={enableOfflineMode} class="primary"> Enable Offline Mode </button>
            <button on:click={disableOfflineMode} class="secondary"> Disable Offline Mode </button>
          </div>
        </div>
      {:else if activeTab === "ecosystem"}
        <div class="ecosystem-section">
          <div class="section-header">
            <h4>Ecosystem Applications</h4>
            <button on:click={() => addEcosystemApp({})} class="primary"> Add App </button>
          </div>

          <div class="apps-list">
            {#each ecosystemApps as app}
              <div class="app-item">
                <div class="app-header">
                  <div class="app-info">
                    <span class="app-name">{app.name}</span>
                    <span class="app-version">v{app.version}</span>
                  </div>
                  <div class="app-status" style="color: {getStatusColor(app.status)}">
                    {getStatusIcon(app.status)}
                    {app.status}
                  </div>
                </div>
                <div class="app-details">
                  <div class="app-metrics">
                    <span>Trust Score: {app.trustScore}%</span>
                    <span>Last Activity: {formatTimestamp(app.lastActivity)}</span>
                  </div>
                  <div class="app-permissions">
                    <span>Permissions: {app.permissions.join(", ")}</span>
                    <span>Data Access: {app.dataAccess.join(", ")}</span>
                  </div>
                </div>
                <div class="app-actions">
                  <button on:click={() => removeEcosystemApp(app.id)} class="small error">
                    Remove
                  </button>
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
  .p2p-layout {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  .p2p-tabs {
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

  .p2p-content {
    flex: 1;
    padding: 1rem;
    background: var(--pico-background-color);
    border-radius: 8px;
    border: 1px solid var(--pico-muted-border-color);
  }

  .section-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 1rem;
  }

  .network-stats {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
    gap: 1rem;
    margin-bottom: 1.5rem;
  }

  .stat-card {
    padding: 1rem;
    border: 1px solid var(--pico-muted-border-color);
    border-radius: 8px;
    background: var(--pico-muted-border-color);
    text-align: center;
  }

  .stat-title {
    font-size: 0.875rem;
    color: var(--pico-muted-color);
    margin-bottom: 0.5rem;
  }

  .stat-value {
    font-size: 1.5rem;
    font-weight: bold;
    color: var(--pico-primary);
  }

  .nodes-list,
  .apps-list {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .node-item,
  .app-item {
    padding: 1rem;
    border: 1px solid var(--pico-muted-border-color);
    border-radius: 8px;
    background: var(--pico-muted-border-color);
    cursor: pointer;
    transition: all 0.2s;
  }

  .node-item:hover,
  .app-item:hover {
    background: var(--pico-primary);
    color: var(--pico-primary-inverse);
  }

  .node-item.selected {
    background: var(--pico-primary);
    color: var(--pico-primary-inverse);
  }

  .node-header,
  .app-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 0.5rem;
  }

  .node-info,
  .app-info {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }

  .node-address,
  .app-name {
    font-weight: 600;
    font-size: 1rem;
  }

  .app-version {
    font-size: 0.875rem;
    opacity: 0.8;
  }

  .node-status,
  .app-status {
    font-size: 0.875rem;
  }

  .node-details,
  .app-details {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    margin-bottom: 0.5rem;
  }

  .node-metrics,
  .app-metrics {
    display: flex;
    gap: 1rem;
    font-size: 0.875rem;
    opacity: 0.8;
  }

  .node-capabilities {
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

  .app-permissions {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    font-size: 0.875rem;
    opacity: 0.8;
  }

  .node-actions,
  .app-actions {
    display: flex;
    gap: 0.5rem;
  }

  .security-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(250px, 1fr));
    gap: 1rem;
  }

  .security-item {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .security-item label {
    font-weight: 600;
  }

  .sync-status {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
    gap: 1rem;
    margin-bottom: 1.5rem;
  }

  .status-card {
    padding: 1rem;
    border: 1px solid var(--pico-muted-border-color);
    border-radius: 8px;
    background: var(--pico-muted-border-color);
    text-align: center;
  }

  .status-title {
    font-size: 0.875rem;
    color: var(--pico-muted-color);
    margin-bottom: 0.5rem;
  }

  .status-value {
    font-size: 1.25rem;
    font-weight: bold;
    color: var(--pico-primary);
  }

  .sync-controls {
    display: flex;
    gap: 0.5rem;
    margin-bottom: 1rem;
  }

  .sync-settings {
    display: flex;
    gap: 1rem;
  }

  .sync-settings label {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .offline-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(250px, 1fr));
    gap: 1rem;
    margin-bottom: 1.5rem;
  }

  .offline-item {
    padding: 1rem;
    border: 1px solid var(--pico-muted-border-color);
    border-radius: 8px;
    background: var(--pico-muted-border-color);
  }

  .offline-item label {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-weight: 600;
  }

  .offline-actions {
    display: flex;
    gap: 0.5rem;
  }

  .small {
    padding: 0.25rem 0.5rem;
    font-size: 0.75rem;
  }

  .success {
    background: var(--success-color);
    color: white;
  }

  .error {
    background: var(--error-color);
    color: white;
  }

  @media (max-width: 768px) {
    .p2p-tabs {
      flex-wrap: wrap;
    }

    .tab-button {
      flex: 1;
      min-width: 120px;
    }

    .network-stats,
    .sync-status {
      grid-template-columns: 1fr;
    }

    .security-grid,
    .offline-grid {
      grid-template-columns: 1fr;
    }

    .sync-controls,
    .offline-actions {
      flex-direction: column;
    }
  }
</style>
