<script lang="ts">
  import { onMount } from "svelte";
  import { push as toast } from "../lib/toasts";

  let dark = false;
  let storageStats = {
    totalSize: 0,
    usedSize: 0,
    freeSize: 0,
    nodeCount: 0,
    keyCount: 0,
    compactionLevel: 0,
    lastCompaction: null as number | null,
  };
  let indexes: Array<{
    id: string;
    name: string;
    type: "vector" | "text" | "numeric" | "composite";
    dimensions: number;
    size: number;
    status: "active" | "building" | "error" | "disabled";
    createdAt: number;
    lastUpdated: number;
    performance: {
      queryTime: number;
      buildTime: number;
      memoryUsage: number;
    };
  }> = [];
  let backups: Array<{
    id: string;
    name: string;
    size: number;
    createdAt: number;
    status: "completed" | "in_progress" | "failed";
    type: "full" | "incremental";
  }> = [];
  let showBackupDialog = false;
  let backupName = "";
  let backupType: "full" | "incremental" = "full";
  let isBackingUp = false;
  let isCompacting = false;

  $: dark = document.documentElement.getAttribute("data-theme") === "dark";

  onMount(() => {
    loadStorageData();
    loadIndexes();
    loadBackups();
  });

  async function loadStorageData() {
    try {
      // Simulate storage stats
      storageStats = {
        totalSize: 1024 * 1024 * 1024, // 1GB
        usedSize: 512 * 1024 * 1024, // 512MB
        freeSize: 512 * 1024 * 1024, // 512MB
        nodeCount: 1250,
        keyCount: 5000,
        compactionLevel: 75,
        lastCompaction: Date.now() - 3600000, // 1 hour ago
      };
    } catch (error) {
      toast("Failed to load storage data", "error");
      console.error("Error loading storage data:", error);
    }
  }

  async function loadIndexes() {
    try {
      // Simulate indexes data
      indexes = [
        {
          id: "idx-1",
          name: "vector_index",
          type: "vector",
          dimensions: 768,
          size: 50 * 1024 * 1024, // 50MB
          status: "active",
          createdAt: Date.now() - 86400000, // 1 day ago
          lastUpdated: Date.now() - 3600000, // 1 hour ago
          performance: {
            queryTime: 15,
            buildTime: 30000,
            memoryUsage: 25 * 1024 * 1024, // 25MB
          },
        },
        {
          id: "idx-2",
          name: "text_search",
          type: "text",
          dimensions: 0,
          size: 30 * 1024 * 1024, // 30MB
          status: "active",
          createdAt: Date.now() - 172800000, // 2 days ago
          lastUpdated: Date.now() - 7200000, // 2 hours ago
          performance: {
            queryTime: 8,
            buildTime: 15000,
            memoryUsage: 15 * 1024 * 1024, // 15MB
          },
        },
        {
          id: "idx-3",
          name: "numeric_index",
          type: "numeric",
          dimensions: 0,
          size: 20 * 1024 * 1024, // 20MB
          status: "building",
          createdAt: Date.now() - 3600000, // 1 hour ago
          lastUpdated: Date.now() - 1800000, // 30 minutes ago
          performance: {
            queryTime: 0,
            buildTime: 0,
            memoryUsage: 10 * 1024 * 1024, // 10MB
          },
        },
      ];
    } catch (error) {
      toast("Failed to load indexes data", "error");
      console.error("Error loading indexes data:", error);
    }
  }

  async function loadBackups() {
    try {
      // Simulate backups data
      backups = [
        {
          id: "backup-1",
          name: "full_backup_2024_01_01",
          size: 500 * 1024 * 1024, // 500MB
          createdAt: Date.now() - 86400000, // 1 day ago
          status: "completed",
          type: "full",
        },
        {
          id: "backup-2",
          name: "incremental_backup_2024_01_02",
          size: 50 * 1024 * 1024, // 50MB
          createdAt: Date.now() - 3600000, // 1 hour ago
          status: "completed",
          type: "incremental",
        },
        {
          id: "backup-3",
          name: "backup_in_progress",
          size: 0,
          createdAt: Date.now() - 300000, // 5 minutes ago
          status: "in_progress",
          type: "full",
        },
      ];
    } catch (error) {
      toast("Failed to load backups data", "error");
      console.error("Error loading backups data:", error);
    }
  }

  async function createBackup() {
    if (!backupName.trim()) {
      toast("Please enter a backup name", "error");
      return;
    }

    isBackingUp = true;
    try {
      // Simulate backup creation
      await new Promise((resolve) => setTimeout(resolve, 2000));

      const backup = {
        id: `backup-${Date.now()}`,
        name: backupName,
        size: backupType === "full" ? 500 * 1024 * 1024 : 50 * 1024 * 1024,
        createdAt: Date.now(),
        status: "completed" as const,
        type: backupType,
      };

      backups = [backup, ...backups];
      backupName = "";
      showBackupDialog = false;
      toast("Backup created successfully", "success");
    } catch (error) {
      toast("Failed to create backup", "error");
      console.error("Backup error:", error);
    } finally {
      isBackingUp = false;
    }
  }

  async function restoreBackup(backupId: string) {
    if (
      confirm("Are you sure you want to restore this backup? This will overwrite current data.")
    ) {
      try {
        // Simulate backup restoration
        await new Promise((resolve) => setTimeout(resolve, 3000));
        toast("Backup restored successfully", "success");
      } catch (error) {
        toast("Failed to restore backup", "error");
        console.error("Restore error:", error);
      }
    }
  }

  async function deleteBackup(backupId: string) {
    if (confirm("Are you sure you want to delete this backup?")) {
      try {
        backups = backups.filter((b) => b.id !== backupId);
        toast("Backup deleted", "success");
      } catch (error) {
        toast("Failed to delete backup", "error");
        console.error("Delete error:", error);
      }
    }
  }

  async function compactStorage() {
    if (isCompacting) return;

    isCompacting = true;
    try {
      // Simulate compaction
      await new Promise((resolve) => setTimeout(resolve, 5000));
      storageStats.compactionLevel = 0;
      storageStats.lastCompaction = Date.now();
      toast("Storage compaction completed", "success");
    } catch (error) {
      toast("Storage compaction failed", "error");
      console.error("Compaction error:", error);
    } finally {
      isCompacting = false;
    }
  }

  async function createIndex() {
    // Simulate index creation
    const newIndex = {
      id: `idx-${Date.now()}`,
      name: "new_index",
      type: "vector" as const,
      dimensions: 768,
      size: 0,
      status: "building" as const,
      createdAt: Date.now(),
      lastUpdated: Date.now(),
      performance: {
        queryTime: 0,
        buildTime: 0,
        memoryUsage: 0,
      },
    };

    indexes = [...indexes, newIndex];
    toast("Index creation started", "success");
  }

  async function deleteIndex(indexId: string) {
    if (confirm("Are you sure you want to delete this index?")) {
      try {
        indexes = indexes.filter((i) => i.id !== indexId);
        toast("Index deleted", "success");
      } catch (error) {
        toast("Failed to delete index", "error");
        console.error("Delete error:", error);
      }
    }
  }

  function formatBytes(bytes: number): string {
    if (bytes === 0) return "0 B";
    const k = 1024;
    const sizes = ["B", "KB", "MB", "GB", "TB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + " " + sizes[i];
  }

  function formatTimestamp(timestamp: number): string {
    return new Date(timestamp).toLocaleString();
  }

  function getStatusColor(status: string): string {
    switch (status) {
      case "active":
        return "var(--success-color)";
      case "building":
        return "var(--warning-color)";
      case "error":
        return "var(--error-color)";
      case "disabled":
        return "var(--muted-color)";
      default:
        return "var(--muted-color)";
    }
  }

  function getStatusIcon(status: string): string {
    switch (status) {
      case "active":
        return "✅";
      case "building":
        return "⏳";
      case "error":
        return "❌";
      case "disabled":
        return "⏸️";
      default:
        return "⚪";
    }
  }

  function getTypeIcon(type: string): string {
    switch (type) {
      case "vector":
        return "🔢";
      case "text":
        return "📝";
      case "numeric":
        return "🔢";
      case "composite":
        return "🔗";
      default:
        return "📊";
    }
  }
</script>

<section aria-labelledby="storage-indexes-heading">
  <h3 id="storage-indexes-heading">Storage & Indexes</h3>

  <div class="storage-layout">
    <!-- Storage Stats -->
    <div class="storage-stats">
      <h4>Storage Statistics</h4>
      <div class="stats-grid">
        <div class="stat-card">
          <div class="stat-value">{formatBytes(storageStats.totalSize)}</div>
          <div class="stat-label">Total Size</div>
        </div>
        <div class="stat-card">
          <div class="stat-value">{formatBytes(storageStats.usedSize)}</div>
          <div class="stat-label">Used Size</div>
        </div>
        <div class="stat-card">
          <div class="stat-value">{formatBytes(storageStats.freeSize)}</div>
          <div class="stat-label">Free Size</div>
        </div>
        <div class="stat-card">
          <div class="stat-value">{storageStats.nodeCount.toLocaleString()}</div>
          <div class="stat-label">Nodes</div>
        </div>
        <div class="stat-card">
          <div class="stat-value">{storageStats.keyCount.toLocaleString()}</div>
          <div class="stat-label">Keys</div>
        </div>
        <div class="stat-card">
          <div class="stat-value">{storageStats.compactionLevel}%</div>
          <div class="stat-label">Compaction Level</div>
        </div>
      </div>

      <div class="storage-usage">
        <div class="usage-bar">
          <div
            class="usage-fill"
            style="width: {(storageStats.usedSize / storageStats.totalSize) * 100}%"
          ></div>
        </div>
        <div class="usage-text">
          {((storageStats.usedSize / storageStats.totalSize) * 100).toFixed(1)}% used
        </div>
      </div>

      <div class="storage-actions">
        <button on:click={compactStorage} disabled={isCompacting} class="primary">
          {isCompacting ? "Compacting..." : "Compact Storage"}
        </button>
        {#if storageStats.lastCompaction}
          <span class="last-compaction">
            Last compaction: {formatTimestamp(storageStats.lastCompaction)}
          </span>
        {/if}
      </div>
    </div>

    <!-- Indexes Section -->
    <div class="indexes-section">
      <div class="section-header">
        <h4>Indexes ({indexes.length})</h4>
        <button on:click={createIndex} class="primary"> Create Index </button>
      </div>

      <div class="indexes-list">
        {#each indexes as index}
          <div class="index-item">
            <div class="index-header">
              <div class="index-info">
                <span class="index-name">
                  {getTypeIcon(index.type)}
                  {index.name}
                </span>
                <span class="index-status" style="color: {getStatusColor(index.status)}">
                  {getStatusIcon(index.status)}
                  {index.status}
                </span>
              </div>
              <div class="index-actions">
                <button on:click={() => deleteIndex(index.id)} class="small" title="Delete index">
                  🗑️
                </button>
              </div>
            </div>
            <div class="index-details">
              <div class="index-metrics">
                <span>Type: {index.type}</span>
                {#if index.dimensions > 0}
                  <span>Dimensions: {index.dimensions}</span>
                {/if}
                <span>Size: {formatBytes(index.size)}</span>
                <span>Query Time: {index.performance.queryTime}ms</span>
              </div>
              <div class="index-meta">
                <span>Created: {formatTimestamp(index.createdAt)}</span>
                <span>Updated: {formatTimestamp(index.lastUpdated)}</span>
              </div>
            </div>
          </div>
        {/each}
      </div>
    </div>

    <!-- Backups Section -->
    <div class="backups-section">
      <div class="section-header">
        <h4>Backups ({backups.length})</h4>
        <button on:click={() => (showBackupDialog = true)} class="primary"> Create Backup </button>
      </div>

      <div class="backups-list">
        {#each backups as backup}
          <div class="backup-item">
            <div class="backup-header">
              <div class="backup-info">
                <span class="backup-name">{backup.name}</span>
                <span class="backup-type">{backup.type}</span>
                <span
                  class="backup-status"
                  class:completed={backup.status === "completed"}
                  class:in-progress={backup.status === "in_progress"}
                  class:failed={backup.status === "failed"}
                >
                  {backup.status === "completed"
                    ? "✅"
                    : backup.status === "in_progress"
                      ? "⏳"
                      : "❌"}
                  {backup.status}
                </span>
              </div>
              <div class="backup-actions">
                {#if backup.status === "completed"}
                  <button
                    on:click={() => restoreBackup(backup.id)}
                    class="small"
                    title="Restore backup"
                  >
                    🔄
                  </button>
                {/if}
                <button
                  on:click={() => deleteBackup(backup.id)}
                  class="small"
                  title="Delete backup"
                >
                  🗑️
                </button>
              </div>
            </div>
            <div class="backup-details">
              <span>Size: {formatBytes(backup.size)}</span>
              <span>Created: {formatTimestamp(backup.createdAt)}</span>
            </div>
          </div>
        {/each}
      </div>
    </div>
  </div>

  <!-- Backup Dialog -->
  {#if showBackupDialog}
    <div class="dialog-overlay">
      <div class="dialog">
        <h4>Create Backup</h4>
        <input
          type="text"
          bind:value={backupName}
          placeholder="Enter backup name..."
          on:keydown={(e) => e.key === "Enter" && createBackup()}
        />
        <div class="form-group">
          <label for="backup-type">Backup Type</label>
          <select id="backup-type" bind:value={backupType}>
            <option value="full">Full Backup</option>
            <option value="incremental">Incremental Backup</option>
          </select>
        </div>
        <div class="dialog-actions">
          <button on:click={createBackup} disabled={!backupName.trim() || isBackingUp}>
            {isBackingUp ? "Creating..." : "Create Backup"}
          </button>
          <button on:click={() => (showBackupDialog = false)} class="secondary"> Cancel </button>
        </div>
      </div>
    </div>
  {/if}
</section>

<style>
  .storage-layout {
    display: flex;
    flex-direction: column;
    gap: 1.5rem;
  }

  .storage-stats {
    background: var(--pico-muted-border-color);
    border-radius: 8px;
    padding: 1.5rem;
  }

  .stats-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
    gap: 1rem;
    margin-bottom: 1rem;
  }

  .stat-card {
    text-align: center;
    padding: 1rem;
    background: var(--pico-background-color);
    border-radius: 8px;
    border: 1px solid var(--pico-muted-border-color);
  }

  .stat-value {
    font-size: 1.5rem;
    font-weight: bold;
    color: var(--pico-primary);
  }

  .stat-label {
    font-size: 0.875rem;
    color: var(--pico-muted-color);
    margin-top: 0.25rem;
  }

  .storage-usage {
    margin-bottom: 1rem;
  }

  .usage-bar {
    width: 100%;
    height: 12px;
    background: var(--pico-muted-border-color);
    border-radius: 6px;
    overflow: hidden;
    margin-bottom: 0.5rem;
  }

  .usage-fill {
    height: 100%;
    background: var(--pico-primary);
    transition: width 0.3s ease;
  }

  .usage-text {
    text-align: center;
    font-size: 0.875rem;
    color: var(--pico-muted-color);
  }

  .storage-actions {
    display: flex;
    align-items: center;
    gap: 1rem;
  }

  .last-compaction {
    font-size: 0.875rem;
    color: var(--pico-muted-color);
  }

  .indexes-section,
  .backups-section {
    border: 1px solid var(--pico-muted-border-color);
    border-radius: 8px;
    padding: 1.5rem;
    background: var(--pico-background-color);
  }

  .section-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 1rem;
  }

  .indexes-list,
  .backups-list {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .index-item,
  .backup-item {
    padding: 1rem;
    border: 1px solid var(--pico-muted-border-color);
    border-radius: 8px;
    background: var(--pico-muted-border-color);
  }

  .index-header,
  .backup-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 0.5rem;
  }

  .index-info,
  .backup-info {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }

  .index-name,
  .backup-name {
    font-weight: 600;
    font-size: 1rem;
  }

  .index-status,
  .backup-status {
    font-size: 0.875rem;
  }

  .backup-type {
    font-size: 0.75rem;
    background: var(--pico-primary);
    color: var(--pico-primary-inverse);
    padding: 0.125rem 0.375rem;
    border-radius: 3px;
  }

  .backup-status.completed {
    color: var(--success-color);
  }

  .backup-status.in-progress {
    color: var(--warning-color);
  }

  .backup-status.failed {
    color: var(--error-color);
  }

  .index-actions,
  .backup-actions {
    display: flex;
    gap: 0.5rem;
  }

  .index-details,
  .backup-details {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    font-size: 0.875rem;
  }

  .index-metrics {
    display: flex;
    gap: 1rem;
    flex-wrap: wrap;
  }

  .index-meta {
    display: flex;
    gap: 1rem;
    color: var(--pico-muted-color);
  }

  .backup-details {
    display: flex;
    gap: 1rem;
    color: var(--pico-muted-color);
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
  .dialog select {
    width: 100%;
    margin-bottom: 1rem;
  }

  .form-group {
    margin-bottom: 1rem;
  }

  .form-group label {
    display: block;
    margin-bottom: 0.5rem;
    font-weight: 600;
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

  @media (max-width: 768px) {
    .stats-grid {
      grid-template-columns: repeat(auto-fit, minmax(120px, 1fr));
    }

    .section-header {
      flex-direction: column;
      align-items: flex-start;
      gap: 0.5rem;
    }

    .index-metrics,
    .index-meta,
    .backup-details {
      flex-direction: column;
      gap: 0.25rem;
    }
  }
</style>
