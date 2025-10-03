<script lang="ts">
  import { onMount } from "svelte";
  import { push as toast } from "../lib/toasts";

  let dark = false;
  let tasks: Array<{
    id: string;
    name: string;
    description: string;
    type: "re-embed" | "cleanup" | "backup" | "custom";
    schedule: string;
    enabled: boolean;
    lastRun?: number;
    nextRun?: number;
    status: "idle" | "running" | "success" | "error";
    logs: Array<{
      timestamp: number;
      level: "info" | "warn" | "error";
      message: string;
    }>;
    createdAt: number;
    updatedAt: number;
  }> = [];
  let selectedTask: any = null;
  let showNewTask = false;
  let newTaskName = "";
  let newTaskDescription = "";
  let newTaskType: "re-embed" | "cleanup" | "backup" | "custom" = "re-embed";
  let newTaskSchedule = "0 0 * * *"; // Daily at midnight
  let showLogs = false;
  let runningTasks = new Set<string>();

  // Schedule presets
  let schedulePresets = [
    { value: "0 0 * * *", label: "Daily at midnight" },
    { value: "0 0 * * 0", label: "Weekly on Sunday" },
    { value: "0 0 1 * *", label: "Monthly on 1st" },
    { value: "0 */6 * * *", label: "Every 6 hours" },
    { value: "0 */1 * * *", label: "Every hour" },
    { value: "*/15 * * * *", label: "Every 15 minutes" },
  ];

  $: dark = document.documentElement.getAttribute("data-theme") === "dark";

  onMount(() => {
    loadTasks();
    startScheduler();
  });

  function loadTasks() {
    try {
      const saved = localStorage.getItem("pluresdb-tasks");
      if (saved) {
        tasks = JSON.parse(saved);
        updateNextRuns();
      }
    } catch (error) {
      console.error("Error loading tasks:", error);
    }
  }

  function saveTasks() {
    try {
      localStorage.setItem("pluresdb-tasks", JSON.stringify(tasks));
    } catch (error) {
      console.error("Error saving tasks:", error);
    }
  }

  function updateNextRuns() {
    const now = Date.now();
    tasks.forEach((task) => {
      if (task.enabled) {
        // Simple cron-like calculation (in a real app, use a proper cron library)
        task.nextRun = calculateNextRun(task.schedule, now);
      }
    });
    saveTasks();
  }

  function calculateNextRun(schedule: string, from: number): number {
    // Simplified cron calculation - in production, use a proper cron library
    const now = new Date(from);
    const next = new Date(now);

    if (schedule === "0 0 * * *") {
      // Daily at midnight
      next.setDate(next.getDate() + 1);
      next.setHours(0, 0, 0, 0);
    } else if (schedule === "0 */6 * * *") {
      // Every 6 hours
      next.setHours(next.getHours() + 6);
      next.setMinutes(0, 0, 0);
    } else if (schedule === "0 */1 * * *") {
      // Every hour
      next.setHours(next.getHours() + 1);
      next.setMinutes(0, 0, 0);
    } else if (schedule === "*/15 * * * *") {
      // Every 15 minutes
      next.setMinutes(next.getMinutes() + 15);
      next.setSeconds(0, 0);
    } else {
      // Default to daily
      next.setDate(next.getDate() + 1);
      next.setHours(0, 0, 0, 0);
    }

    return next.getTime();
  }

  function startScheduler() {
    // Check for tasks that need to run every minute
    setInterval(() => {
      const now = Date.now();
      tasks.forEach((task) => {
        if (task.enabled && task.nextRun && task.nextRun <= now && !runningTasks.has(task.id)) {
          runTask(task);
        }
      });
    }, 60000); // Check every minute
  }

  function createTask() {
    if (!newTaskName.trim()) {
      toast("Please enter a task name", "error");
      return;
    }

    const task = {
      id: `task-${Date.now()}`,
      name: newTaskName,
      description: newTaskDescription,
      type: newTaskType,
      schedule: newTaskSchedule,
      enabled: true,
      status: "idle" as const,
      logs: [],
      createdAt: Date.now(),
      updatedAt: Date.now(),
    };

    task.nextRun = calculateNextRun(task.schedule, Date.now());

    tasks = [...tasks, task];
    selectedTask = task;
    newTaskName = "";
    newTaskDescription = "";
    newTaskType = "re-embed";
    newTaskSchedule = "0 0 * * *";
    showNewTask = false;
    saveTasks();
    toast("Task created successfully", "success");
  }

  function selectTask(task: any) {
    selectedTask = task;
  }

  function deleteTask(taskId: string) {
    if (confirm("Are you sure you want to delete this task?")) {
      tasks = tasks.filter((t) => t.id !== taskId);
      runningTasks.delete(taskId);
      if (selectedTask?.id === taskId) {
        selectedTask = null;
      }
      saveTasks();
      toast("Task deleted", "success");
    }
  }

  function toggleTask(taskId: string) {
    const task = tasks.find((t) => t.id === taskId);
    if (task) {
      task.enabled = !task.enabled;
      task.updatedAt = Date.now();
      if (task.enabled) {
        task.nextRun = calculateNextRun(task.schedule, Date.now());
      } else {
        task.nextRun = undefined;
      }
      saveTasks();
      toast(`Task ${task.enabled ? "enabled" : "disabled"}`, "success");
    }
  }

  async function runTask(task: any) {
    if (runningTasks.has(task.id)) return;

    runningTasks.add(task.id);
    task.status = "running";
    task.lastRun = Date.now();
    task.updatedAt = Date.now();

    addLog(task, "info", `Task started: ${task.name}`);
    saveTasks();

    try {
      let result: any = {};

      switch (task.type) {
        case "re-embed":
          result = await executeReEmbedTask(task);
          break;
        case "cleanup":
          result = await executeCleanupTask(task);
          break;
        case "backup":
          result = await executeBackupTask(task);
          break;
        case "custom":
          result = await executeCustomTask(task);
          break;
        default:
          throw new Error(`Unknown task type: ${task.type}`);
      }

      task.status = "success";
      addLog(task, "info", `Task completed successfully: ${JSON.stringify(result)}`);

      // Schedule next run
      task.nextRun = calculateNextRun(task.schedule, Date.now());

      toast(`Task "${task.name}" completed successfully`, "success");
    } catch (error) {
      task.status = "error";
      addLog(task, "error", `Task failed: ${error.message}`);
      toast(`Task "${task.name}" failed: ${error.message}`, "error");
      console.error("Task execution error:", error);
    } finally {
      runningTasks.delete(task.id);
      task.updatedAt = Date.now();
      saveTasks();
    }
  }

  async function executeReEmbedTask(task: any) {
    // Simulate re-embedding task
    addLog(task, "info", "Starting re-embedding process...");

    const res = await fetch("/api/list");
    const nodes = await res.json();

    let processed = 0;
    for (const node of nodes) {
      if (node.data.vector) {
        // Simulate re-embedding
        await new Promise((resolve) => setTimeout(resolve, 100));
        processed++;
      }
    }

    addLog(task, "info", `Re-embedded ${processed} nodes`);
    return { processed };
  }

  async function executeCleanupTask(task: any) {
    // Simulate cleanup task
    addLog(task, "info", "Starting cleanup process...");

    const res = await fetch("/api/list");
    const nodes = await res.json();

    let cleaned = 0;
    for (const node of nodes) {
      // Simulate cleanup logic
      if (node.data.temp || node.data.old) {
        await new Promise((resolve) => setTimeout(resolve, 50));
        cleaned++;
      }
    }

    addLog(task, "info", `Cleaned up ${cleaned} nodes`);
    return { cleaned };
  }

  async function executeBackupTask(task: any) {
    // Simulate backup task
    addLog(task, "info", "Starting backup process...");

    const res = await fetch("/api/list");
    const nodes = await res.json();

    // Simulate backup creation
    await new Promise((resolve) => setTimeout(resolve, 1000));

    addLog(task, "info", `Backed up ${nodes.length} nodes`);
    return { backedUp: nodes.length };
  }

  async function executeCustomTask(task: any) {
    // Simulate custom task
    addLog(task, "info", "Executing custom task...");

    // In a real implementation, this would execute custom JavaScript
    await new Promise((resolve) => setTimeout(resolve, 500));

    addLog(task, "info", "Custom task completed");
    return { executed: true };
  }

  function addLog(task: any, level: "info" | "warn" | "error", message: string) {
    task.logs.push({
      timestamp: Date.now(),
      level,
      message,
    });

    // Keep only last 100 logs
    if (task.logs.length > 100) {
      task.logs = task.logs.slice(-100);
    }
  }

  function exportTask(task: any) {
    const data = {
      name: task.name,
      description: task.description,
      type: task.type,
      schedule: task.schedule,
      exportedAt: new Date().toISOString(),
    };

    const blob = new Blob([JSON.stringify(data, null, 2)], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `${task.name.replace(/[^a-z0-9]/gi, "_").toLowerCase()}.json`;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);

    toast("Task exported", "success");
  }

  function formatTimestamp(timestamp: number): string {
    return new Date(timestamp).toLocaleString();
  }

  function formatNextRun(nextRun: number): string {
    const now = Date.now();
    const diff = nextRun - now;

    if (diff < 0) return "Overdue";
    if (diff < 60000) return "In less than a minute";
    if (diff < 3600000) return `In ${Math.floor(diff / 60000)} minutes`;
    if (diff < 86400000) return `In ${Math.floor(diff / 3600000)} hours`;
    return `In ${Math.floor(diff / 86400000)} days`;
  }

  function getStatusIcon(status: string): string {
    switch (status) {
      case "running":
        return "⏳";
      case "success":
        return "✅";
      case "error":
        return "❌";
      default:
        return "⏸️";
    }
  }
</script>

<section aria-labelledby="tasks-scheduler-heading">
  <h3 id="tasks-scheduler-heading">Tasks Scheduler</h3>

  <div class="tasks-layout">
    <!-- Tasks List -->
    <div class="tasks-sidebar">
      <div class="sidebar-header">
        <h4>Tasks ({tasks.length})</h4>
        <button on:click={() => (showNewTask = true)} class="primary"> New Task </button>
      </div>

      <div class="tasks-list">
        {#each tasks as task}
          <div
            class="task-item"
            class:selected={selectedTask?.id === task.id}
            class:disabled={!task.enabled}
            class:running={task.status === "running"}
            role="button"
            tabindex="0"
            on:click={() => selectTask(task)}
            on:keydown={(e) => e.key === "Enter" && selectTask(task)}
          >
            <div class="task-header">
              <span class="task-name">{task.name}</span>
              <div class="task-actions">
                <span class="task-status" title={task.status}>
                  {getStatusIcon(task.status)}
                </span>
                <button
                  on:click|stopPropagation={() => toggleTask(task.id)}
                  class="small"
                  title={task.enabled ? "Disable task" : "Enable task"}
                >
                  {task.enabled ? "✅" : "❌"}
                </button>
                <button
                  on:click|stopPropagation={() => runTask(task)}
                  class="small"
                  disabled={task.status === "running"}
                  title="Run now"
                >
                  ▶️
                </button>
                <button
                  on:click|stopPropagation={() => exportTask(task)}
                  class="small"
                  title="Export task"
                >
                  📤
                </button>
                <button
                  on:click|stopPropagation={() => deleteTask(task.id)}
                  class="small"
                  title="Delete task"
                >
                  🗑️
                </button>
              </div>
            </div>
            <div class="task-meta">
              <span class="task-type">{task.type}</span>
              <span class="task-schedule">{task.schedule}</span>
            </div>
            <div class="task-timing">
              {#if task.lastRun}
                <span class="last-run">Last: {formatTimestamp(task.lastRun)}</span>
              {/if}
              {#if task.nextRun}
                <span class="next-run">Next: {formatNextRun(task.nextRun)}</span>
              {/if}
            </div>
            {#if task.description}
              <div class="task-description">
                {task.description}
              </div>
            {/if}
          </div>
        {/each}
      </div>
    </div>

    <!-- Task Details -->
    <div class="task-details">
      {#if selectedTask}
        <div class="details-header">
          <h4>{selectedTask.name}</h4>
          <div class="details-actions">
            <button on:click={() => (showLogs = !showLogs)} class="secondary">
              {showLogs ? "Hide Logs" : "Show Logs"}
            </button>
            <button
              on:click={() => runTask(selectedTask)}
              disabled={selectedTask.status === "running"}
              class="primary"
            >
              {selectedTask.status === "running" ? "Running..." : "Run Now"}
            </button>
          </div>
        </div>

        <div class="task-info">
          <div class="info-grid">
            <div class="info-item">
              <span class="info-label">Type</span>
              <span>{selectedTask.type}</span>
            </div>
            <div class="info-item">
              <span class="info-label">Schedule</span>
              <span>{selectedTask.schedule}</span>
            </div>
            <div class="info-item">
              <span class="info-label">Status</span>
              <span
                class="status-badge"
                class:running={selectedTask.status === "running"}
                class:success={selectedTask.status === "success"}
                class:error={selectedTask.status === "error"}
              >
                {selectedTask.status}
              </span>
            </div>
            <div class="info-item">
              <span class="info-label">Enabled</span>
              <span>{selectedTask.enabled ? "Yes" : "No"}</span>
            </div>
            {#if selectedTask.lastRun}
              <div class="info-item">
                <span class="info-label">Last Run</span>
                <span>{formatTimestamp(selectedTask.lastRun)}</span>
              </div>
            {/if}
            {#if selectedTask.nextRun}
              <div class="info-item">
                <span class="info-label">Next Run</span>
                <span>{formatNextRun(selectedTask.nextRun)}</span>
              </div>
            {/if}
          </div>

          {#if selectedTask.description}
            <div class="task-description-full">
              <span class="info-label">Description</span>
              <p>{selectedTask.description}</p>
            </div>
          {/if}
        </div>

        {#if showLogs}
          <div class="task-logs">
            <h5>Execution Logs ({selectedTask.logs.length})</h5>
            <div class="logs-list">
              {#each selectedTask.logs as log}
                <div
                  class="log-item"
                  class:info={log.level === "info"}
                  class:warn={log.level === "warn"}
                  class:error={log.level === "error"}
                >
                  <span class="log-timestamp">{formatTimestamp(log.timestamp)}</span>
                  <span class="log-level">{log.level.toUpperCase()}</span>
                  <span class="log-message">{log.message}</span>
                </div>
              {/each}
            </div>
          </div>
        {/if}
      {:else}
        <div class="no-task">
          <p>Select a task or create a new one to get started</p>
        </div>
      {/if}
    </div>
  </div>

  <!-- New Task Dialog -->
  {#if showNewTask}
    <div class="dialog-overlay">
      <div class="dialog">
        <h4>Create New Task</h4>
        <input
          type="text"
          bind:value={newTaskName}
          placeholder="Enter task name..."
          on:keydown={(e) => e.key === "Enter" && createTask()}
        />
        <textarea
          bind:value={newTaskDescription}
          placeholder="Enter task description (optional)..."
          rows="3"
        ></textarea>
        <div class="form-group">
          <label for="task-type">Task Type</label>
          <select id="task-type" bind:value={newTaskType}>
            <option value="re-embed">Re-embed Vectors</option>
            <option value="cleanup">Cleanup Data</option>
            <option value="backup">Backup Data</option>
            <option value="custom">Custom Task</option>
          </select>
        </div>
        <div class="form-group">
          <label for="task-schedule">Schedule (Cron)</label>
          <select id="task-schedule" bind:value={newTaskSchedule}>
            {#each schedulePresets as preset}
              <option value={preset.value}>{preset.label}</option>
            {/each}
          </select>
          <input type="text" bind:value={newTaskSchedule} placeholder="Custom cron expression..." />
        </div>
        <div class="dialog-actions">
          <button on:click={createTask} disabled={!newTaskName.trim()}> Create </button>
          <button on:click={() => (showNewTask = false)} class="secondary"> Cancel </button>
        </div>
      </div>
    </div>
  {/if}
</section>

<style>
  .tasks-layout {
    display: grid;
    grid-template-columns: 350px 1fr;
    gap: 1rem;
    height: 600px;
  }

  .tasks-sidebar {
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

  .tasks-list {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .task-item {
    padding: 0.75rem;
    border: 1px solid var(--pico-muted-border-color);
    border-radius: 4px;
    background: var(--pico-background-color);
    cursor: pointer;
    transition: all 0.2s;
  }

  .task-item:hover {
    background: var(--pico-muted-border-color);
  }

  .task-item.selected {
    background: var(--pico-primary);
    color: var(--pico-primary-inverse);
  }

  .task-item.disabled {
    opacity: 0.6;
  }

  .task-item.running {
    border-color: var(--warning-color);
    background: rgba(255, 193, 7, 0.1);
  }

  .task-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 0.25rem;
  }

  .task-name {
    font-weight: 600;
    font-size: 0.875rem;
  }

  .task-actions {
    display: flex;
    gap: 0.25rem;
    align-items: center;
  }

  .task-status {
    font-size: 0.875rem;
  }

  .task-meta {
    display: flex;
    justify-content: space-between;
    font-size: 0.75rem;
    opacity: 0.7;
    margin-bottom: 0.25rem;
  }

  .task-type {
    background: var(--pico-primary);
    color: var(--pico-primary-inverse);
    padding: 0.125rem 0.375rem;
    border-radius: 3px;
  }

  .task-timing {
    display: flex;
    flex-direction: column;
    gap: 0.125rem;
    font-size: 0.75rem;
    opacity: 0.8;
  }

  .task-description {
    font-size: 0.75rem;
    opacity: 0.8;
    font-style: italic;
    margin-top: 0.25rem;
  }

  .task-details {
    border: 1px solid var(--pico-muted-border-color);
    border-radius: 8px;
    padding: 1rem;
    overflow-y: auto;
    background: var(--pico-background-color);
  }

  .details-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 1rem;
    padding-bottom: 1rem;
    border-bottom: 1px solid var(--pico-muted-border-color);
  }

  .details-actions {
    display: flex;
    gap: 0.5rem;
  }

  .task-info {
    margin-bottom: 1rem;
  }

  .info-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
    gap: 1rem;
    margin-bottom: 1rem;
  }

  .info-item {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }

  .info-label {
    font-size: 0.875rem;
    font-weight: 600;
    color: var(--pico-muted-color);
  }

  .status-badge {
    padding: 0.25rem 0.5rem;
    border-radius: 4px;
    font-size: 0.75rem;
    font-weight: 600;
    text-transform: uppercase;
    background: var(--pico-muted-border-color);
  }

  .status-badge.running {
    background: var(--warning-color);
    color: white;
  }

  .status-badge.success {
    background: var(--success-color);
    color: white;
  }

  .status-badge.error {
    background: var(--error-color);
    color: white;
  }

  .task-description-full {
    margin-top: 1rem;
  }

  .task-description-full .info-label {
    font-size: 0.875rem;
    font-weight: 600;
    color: var(--pico-muted-color);
    display: block;
    margin-bottom: 0.5rem;
  }

  .task-logs {
    border-top: 1px solid var(--pico-muted-border-color);
    padding-top: 1rem;
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
    grid-template-columns: 150px 60px 1fr;
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

  .no-task {
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
    min-width: 500px;
    max-width: 600px;
  }

  .dialog h4 {
    margin-bottom: 1rem;
  }

  .dialog input,
  .dialog textarea,
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
    .tasks-layout {
      grid-template-columns: 1fr;
      height: auto;
    }

    .tasks-sidebar {
      max-height: 200px;
    }

    .info-grid {
      grid-template-columns: 1fr;
    }

    .log-item {
      grid-template-columns: 1fr;
      gap: 0.25rem;
    }
  }
</style>
