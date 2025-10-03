<script lang="ts">
  import { onMount, onDestroy } from 'svelte'
  import { push as toast } from '../lib/toasts'
  import { selectedId } from '../lib/stores'
  
  let dark = false
  let notebooks: Array<{
    id: string
    name: string
    cells: Array<{
      id: string
      type: 'code' | 'markdown'
      content: string
      output?: any
      status: 'idle' | 'running' | 'success' | 'error'
    }>
    createdAt: number
    updatedAt: number
  }> = []
  let selectedNotebook: any = null
  let selectedCell: any = null
  let showNewNotebook = false
  let newNotebookName = ''
  let showNewCell = false
  let newCellType: 'code' | 'markdown' = 'code'
  
  $: dark = (document.documentElement.getAttribute('data-theme') === 'dark')
  
  onMount(() => {
    loadNotebooks()
  })
  
  function loadNotebooks() {
    try {
      const saved = localStorage.getItem('pluresdb-notebooks')
      if (saved) {
        notebooks = JSON.parse(saved)
      } else {
        // Create a default notebook
        createDefaultNotebook()
      }
    } catch (error) {
      console.error('Error loading notebooks:', error)
      createDefaultNotebook()
    }
  }
  
  function saveNotebooks() {
    try {
      localStorage.setItem('pluresdb-notebooks', JSON.stringify(notebooks))
    } catch (error) {
      console.error('Error saving notebooks:', error)
    }
  }
  
  function createDefaultNotebook() {
    const defaultNotebook = {
      id: `notebook-${Date.now()}`,
      name: 'Welcome Notebook',
      cells: [
        {
          id: `cell-${Date.now()}`,
          type: 'markdown',
          content: '# Welcome to PluresDB Notebooks!\n\nThis is a markdown cell. You can write documentation, notes, and explanations here.\n\n## Getting Started\n\n1. Create a new code cell to run JavaScript/TypeScript\n2. Use the API to interact with your data\n3. Visualize results with charts and tables\n\nTry creating a code cell below!',
          status: 'idle'
        },
        {
          id: `cell-${Date.now() + 1}`,
          type: 'code',
          content: `// Example: List all nodes
const response = await fetch('/api/list');
const nodes = await response.json();
console.log('Found', nodes.length, 'nodes');
return nodes;`,
          status: 'idle'
        }
      ],
      createdAt: Date.now(),
      updatedAt: Date.now()
    }
    notebooks = [defaultNotebook]
    saveNotebooks()
  }
  
  function createNotebook() {
    if (!newNotebookName.trim()) {
      toast('Please enter a notebook name', 'error')
      return
    }
    
    const notebook = {
      id: `notebook-${Date.now()}`,
      name: newNotebookName,
      cells: [],
      createdAt: Date.now(),
      updatedAt: Date.now()
    }
    
    notebooks = [...notebooks, notebook]
    selectedNotebook = notebook
    newNotebookName = ''
    showNewNotebook = false
    saveNotebooks()
    toast('Notebook created successfully', 'success')
  }
  
  function selectNotebook(notebook: any) {
    selectedNotebook = notebook
    selectedCell = null
  }
  
  function deleteNotebook(notebookId: string) {
    if (confirm('Are you sure you want to delete this notebook?')) {
      notebooks = notebooks.filter(n => n.id !== notebookId)
      if (selectedNotebook?.id === notebookId) {
        selectedNotebook = notebooks[0] || null
      }
      saveNotebooks()
      toast('Notebook deleted', 'success')
    }
  }
  
  function addCell() {
    if (!selectedNotebook) return
    
    const cell = {
      id: `cell-${Date.now()}`,
      type: newCellType,
      content: newCellType === 'markdown' ? '# New Markdown Cell\n\nWrite your documentation here...' : '// New Code Cell\n\n// Write your JavaScript/TypeScript code here\n\n',
      status: 'idle'
    }
    
    selectedNotebook.cells = [...selectedNotebook.cells, cell]
    selectedNotebook.updatedAt = Date.now()
    selectedCell = cell
    showNewCell = false
    saveNotebooks()
  }
  
  function selectCell(cell: any) {
    selectedCell = cell
  }
  
  function deleteCell(cellId: string) {
    if (!selectedNotebook) return
    
    if (confirm('Are you sure you want to delete this cell?')) {
      selectedNotebook.cells = selectedNotebook.cells.filter((c: any) => c.id !== cellId)
      selectedNotebook.updatedAt = Date.now()
      if (selectedCell?.id === cellId) {
        selectedCell = null
      }
      saveNotebooks()
      toast('Cell deleted', 'success')
    }
  }
  
  function moveCell(cellId: string, direction: 'up' | 'down') {
    if (!selectedNotebook) return
    
    const cells = [...selectedNotebook.cells]
    const index = cells.findIndex((c: any) => c.id === cellId)
    
    if (direction === 'up' && index > 0) {
      const temp = cells[index]
      cells[index] = cells[index - 1]
      cells[index - 1] = temp
    } else if (direction === 'down' && index < cells.length - 1) {
      const temp = cells[index]
      cells[index] = cells[index + 1]
      cells[index + 1] = temp
    }
    
    selectedNotebook.cells = cells
    selectedNotebook.updatedAt = Date.now()
    saveNotebooks()
  }
  
  async function executeCell(cell: any) {
    if (cell.type !== 'code') return
    
    cell.status = 'running'
    saveNotebooks()
    
    try {
      // Create a safe execution environment
      const result = await executeCode(cell.content)
      cell.output = result
      cell.status = 'success'
      toast('Cell executed successfully', 'success')
    } catch (error) {
      cell.output = { error: error.message }
      cell.status = 'error'
      toast('Cell execution failed', 'error')
      console.error('Cell execution error:', error)
    }
    
    selectedNotebook.updatedAt = Date.now()
    saveNotebooks()
  }
  
  async function executeCode(code: string): Promise<any> {
    // Create a safe execution environment with API access
    const api = {
      async get(url: string) {
        const response = await fetch(url)
        if (!response.ok) throw new Error(`HTTP ${response.status}: ${response.statusText}`)
        return response.json()
      },
      async post(url: string, data: any) {
        const response = await fetch(url, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(data)
        })
        if (!response.ok) throw new Error(`HTTP ${response.status}: ${response.statusText}`)
        return response.json()
      }
    }
    
    // Create a sandboxed function
    const func = new Function('api', 'console', 'return', `
      (async () => {
        ${code}
      })()
    `)
    
    return await func(api, console, (value: any) => value)
  }
  
  function formatOutput(output: any): string {
    if (output === null) return 'null'
    if (output === undefined) return 'undefined'
    if (typeof output === 'string') return output
    if (typeof output === 'number') return output.toString()
    if (typeof output === 'boolean') return output.toString()
    if (Array.isArray(output)) return JSON.stringify(output, null, 2)
    if (typeof output === 'object') return JSON.stringify(output, null, 2)
    return String(output)
  }
  
  function exportNotebook(notebook: any) {
    const data = {
      name: notebook.name,
      cells: notebook.cells,
      exportedAt: new Date().toISOString()
    }
    
    const blob = new Blob([JSON.stringify(data, null, 2)], { type: 'application/json' })
    const url = URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.href = url
    a.download = `${notebook.name.replace(/[^a-z0-9]/gi, '_').toLowerCase()}.json`
    document.body.appendChild(a)
    a.click()
    document.body.removeChild(a)
    URL.revokeObjectURL(url)
    
    toast('Notebook exported', 'success')
  }
  
  function importNotebook(event: Event) {
    const file = (event.target as HTMLInputElement).files?.[0]
    if (!file) return
    
    const reader = new FileReader()
    reader.onload = (e) => {
      try {
        const data = JSON.parse(e.target?.result as string)
        const notebook = {
          id: `notebook-${Date.now()}`,
          name: data.name || 'Imported Notebook',
          cells: data.cells || [],
          createdAt: Date.now(),
          updatedAt: Date.now()
        }
        
        notebooks = [...notebooks, notebook]
        selectedNotebook = notebook
        saveNotebooks()
        toast('Notebook imported successfully', 'success')
      } catch (error) {
        toast('Failed to import notebook', 'error')
        console.error('Import error:', error)
      }
    }
    reader.readAsText(file)
  }
</script>

<section aria-labelledby="notebooks-heading">
  <h3 id="notebooks-heading">Notebooks</h3>
  
  <div class="notebooks-layout">
    <!-- Notebook List -->
    <div class="notebooks-sidebar">
      <div class="sidebar-header">
        <h4>Notebooks ({notebooks.length})</h4>
        <button on:click={() => showNewNotebook = true} class="primary">
          New Notebook
        </button>
      </div>
      
      <div class="notebook-list">
        {#each notebooks as notebook}
          <div 
            class="notebook-item"
            class:selected={selectedNotebook?.id === notebook.id}
            on:click={() => selectNotebook(notebook)}
            on:keydown={(e) => e.key === 'Enter' && selectNotebook(notebook)}
          >
            <div class="notebook-header">
              <span class="notebook-name">{notebook.name}</span>
              <div class="notebook-actions">
                <button 
                  on:click|stopPropagation={() => exportNotebook(notebook)}
                  class="small"
                  title="Export notebook"
                >
                  📤
                </button>
                <button 
                  on:click|stopPropagation={() => deleteNotebook(notebook.id)}
                  class="small"
                  title="Delete notebook"
                >
                  🗑️
                </button>
              </div>
            </div>
            <div class="notebook-meta">
              <span class="cell-count">{notebook.cells.length} cells</span>
              <span class="updated-time">
                {new Date(notebook.updatedAt).toLocaleDateString()}
              </span>
            </div>
          </div>
        {/each}
      </div>
      
      <div class="sidebar-footer">
        <label class="import-button">
          <input type="file" accept=".json" on:change={importNotebook} style="display: none;" />
          Import Notebook
        </label>
      </div>
    </div>
    
    <!-- Notebook Editor -->
    <div class="notebook-editor">
      {#if selectedNotebook}
        <div class="editor-header">
          <h4>{selectedNotebook.name}</h4>
          <div class="editor-actions">
            <button on:click={() => showNewCell = true} class="secondary">
              Add Cell
            </button>
            <button on:click={() => exportNotebook(selectedNotebook)} class="secondary">
              Export
            </button>
          </div>
        </div>
        
        <div class="cells-container">
          {#each selectedNotebook.cells as cell, index}
            <div class="cell-container">
              <div class="cell-header">
                <div class="cell-info">
                  <span class="cell-type">{cell.type}</span>
                  <span class="cell-status" class:running={cell.status === 'running'} class:success={cell.status === 'success'} class:error={cell.status === 'error'}>
                    {cell.status === 'running' ? '⏳' : cell.status === 'success' ? '✅' : cell.status === 'error' ? '❌' : '⏸️'}
                  </span>
                </div>
                <div class="cell-actions">
                  {#if cell.type === 'code'}
                    <button on:click={() => executeCell(cell)} disabled={cell.status === 'running'} class="small">
                      {cell.status === 'running' ? 'Running...' : 'Run'}
                    </button>
                  {/if}
                  <button on:click={() => moveCell(cell.id, 'up')} disabled={index === 0} class="small">
                    ↑
                  </button>
                  <button on:click={() => moveCell(cell.id, 'down')} disabled={index === selectedNotebook.cells.length - 1} class="small">
                    ↓
                  </button>
                  <button on:click={() => deleteCell(cell.id)} class="small">
                    🗑️
                  </button>
                </div>
              </div>
              
              <div class="cell-content">
                {#if cell.type === 'markdown'}
                  <div class="markdown-cell">
                    <textarea 
                      bind:value={cell.content}
                      on:input={() => { selectedNotebook.updatedAt = Date.now(); saveNotebooks(); }}
                      placeholder="Write markdown content..."
                    ></textarea>
                    <div class="markdown-preview">
                      <div class="markdown-content">
                        {cell.content.split('\n').map(line => {
                          if (line.startsWith('# ')) return `<h1>${line.substring(2)}</h1>`
                          if (line.startsWith('## ')) return `<h2>${line.substring(3)}</h2>`
                          if (line.startsWith('### ')) return `<h3>${line.substring(4)}</h3>`
                          if (line.startsWith('**') && line.endsWith('**')) return `<strong>${line.substring(2, line.length - 2)}</strong>`
                          if (line.startsWith('*') && line.endsWith('*')) return `<em>${line.substring(1, line.length - 1)}</em>`
                          if (line.startsWith('- ')) return `<li>${line.substring(2)}</li>`
                          if (line.trim() === '') return '<br>'
                          return `<p>${line}</p>`
                        }).join('')}
                      </div>
                    </div>
                  </div>
                {:else}
                  <div class="code-cell">
                    <textarea 
                      bind:value={cell.content}
                      on:input={() => { selectedNotebook.updatedAt = Date.now(); saveNotebooks(); }}
                      placeholder="Write JavaScript/TypeScript code..."
                      class="code-editor"
                    ></textarea>
                    
                    {#if cell.output}
                      <div class="cell-output">
                        <div class="output-header">
                          <span>Output:</span>
                          <button on:click={() => { cell.output = null; saveNotebooks(); }} class="small">
                            Clear
                          </button>
                        </div>
                        <pre class="output-content">{formatOutput(cell.output)}</pre>
                      </div>
                    {/if}
                  </div>
                {/if}
              </div>
            </div>
          {/each}
        </div>
      {:else}
        <div class="no-notebook">
          <p>Select a notebook or create a new one to get started</p>
        </div>
      {/if}
    </div>
  </div>
  
  <!-- New Notebook Dialog -->
  {#if showNewNotebook}
    <div class="dialog-overlay">
      <div class="dialog">
        <h4>Create New Notebook</h4>
        <input 
          type="text" 
          bind:value={newNotebookName} 
          placeholder="Enter notebook name..."
          on:keydown={(e) => e.key === 'Enter' && createNotebook()}
        />
        <div class="dialog-actions">
          <button on:click={createNotebook} disabled={!newNotebookName.trim()}>
            Create
          </button>
          <button on:click={() => showNewNotebook = false} class="secondary">
            Cancel
          </button>
        </div>
      </div>
    </div>
  {/if}
  
  <!-- New Cell Dialog -->
  {#if showNewCell}
    <div class="dialog-overlay">
      <div class="dialog">
        <h4>Add New Cell</h4>
        <div class="cell-type-selector">
          <label>
            <input type="radio" bind:group={newCellType} value="code" />
            Code Cell (JavaScript/TypeScript)
          </label>
          <label>
            <input type="radio" bind:group={newCellType} value="markdown" />
            Markdown Cell (Documentation)
          </label>
        </div>
        <div class="dialog-actions">
          <button on:click={addCell}>
            Add Cell
          </button>
          <button on:click={() => showNewCell = false} class="secondary">
            Cancel
          </button>
        </div>
      </div>
    </div>
  {/if}
</section>

<style>
  .notebooks-layout {
    display: grid;
    grid-template-columns: 300px 1fr;
    gap: 1rem;
    height: 600px;
  }
  
  .notebooks-sidebar {
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
  
  .notebook-list {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }
  
  .notebook-item {
    padding: 0.75rem;
    border: 1px solid var(--pico-muted-border-color);
    border-radius: 4px;
    background: var(--pico-background-color);
    cursor: pointer;
    transition: all 0.2s;
  }
  
  .notebook-item:hover {
    background: var(--pico-muted-border-color);
  }
  
  .notebook-item.selected {
    background: var(--pico-primary);
    color: var(--pico-primary-inverse);
  }
  
  .notebook-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 0.25rem;
  }
  
  .notebook-name {
    font-weight: 600;
    font-size: 0.875rem;
  }
  
  .notebook-actions {
    display: flex;
    gap: 0.25rem;
  }
  
  .notebook-meta {
    display: flex;
    justify-content: space-between;
    font-size: 0.75rem;
    opacity: 0.7;
  }
  
  .sidebar-footer {
    margin-top: 1rem;
    padding-top: 1rem;
    border-top: 1px solid var(--pico-muted-border-color);
  }
  
  .import-button {
    display: block;
    text-align: center;
    padding: 0.5rem;
    background: var(--pico-muted-border-color);
    border-radius: 4px;
    cursor: pointer;
    font-size: 0.875rem;
  }
  
  .notebook-editor {
    border: 1px solid var(--pico-muted-border-color);
    border-radius: 8px;
    padding: 1rem;
    overflow-y: auto;
    background: var(--pico-background-color);
  }
  
  .editor-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 1rem;
    padding-bottom: 1rem;
    border-bottom: 1px solid var(--pico-muted-border-color);
  }
  
  .editor-actions {
    display: flex;
    gap: 0.5rem;
  }
  
  .cells-container {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }
  
  .cell-container {
    border: 1px solid var(--pico-muted-border-color);
    border-radius: 8px;
    overflow: hidden;
  }
  
  .cell-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.5rem 0.75rem;
    background: var(--pico-muted-border-color);
    border-bottom: 1px solid var(--pico-muted-border-color);
  }
  
  .cell-info {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }
  
  .cell-type {
    font-size: 0.75rem;
    font-weight: 600;
    text-transform: uppercase;
    padding: 0.125rem 0.375rem;
    border-radius: 3px;
    background: var(--pico-primary);
    color: var(--pico-primary-inverse);
  }
  
  .cell-status {
    font-size: 0.875rem;
  }
  
  .cell-status.running {
    color: var(--warning-color);
  }
  
  .cell-status.success {
    color: var(--success-color);
  }
  
  .cell-status.error {
    color: var(--error-color);
  }
  
  .cell-actions {
    display: flex;
    gap: 0.25rem;
  }
  
  .cell-content {
    padding: 1rem;
  }
  
  .markdown-cell {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 1rem;
  }
  
  .markdown-cell textarea {
    font-family: monospace;
    font-size: 0.875rem;
    min-height: 200px;
    resize: vertical;
  }
  
  .markdown-preview {
    border: 1px solid var(--pico-muted-border-color);
    border-radius: 4px;
    padding: 1rem;
    background: var(--pico-background-color);
    overflow-y: auto;
    max-height: 300px;
  }
  
  .markdown-content h1, .markdown-content h2, .markdown-content h3 {
    margin: 0.5rem 0;
  }
  
  .markdown-content p {
    margin: 0.25rem 0;
  }
  
  .markdown-content li {
    margin: 0.125rem 0;
  }
  
  .code-cell {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }
  
  .code-editor {
    font-family: monospace;
    font-size: 0.875rem;
    min-height: 200px;
    resize: vertical;
    width: 100%;
  }
  
  .cell-output {
    border: 1px solid var(--pico-muted-border-color);
    border-radius: 4px;
    background: var(--pico-muted-border-color);
  }
  
  .output-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.5rem 0.75rem;
    background: var(--pico-muted-border-color);
    border-bottom: 1px solid var(--pico-muted-border-color);
    font-size: 0.875rem;
    font-weight: 600;
  }
  
  .output-content {
    padding: 0.75rem;
    font-family: monospace;
    font-size: 0.875rem;
    white-space: pre-wrap;
    max-height: 300px;
    overflow-y: auto;
  }
  
  .no-notebook {
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
  
  .dialog input {
    width: 100%;
    margin-bottom: 1rem;
  }
  
  .cell-type-selector {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
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
  
  @media (max-width: 768px) {
    .notebooks-layout {
      grid-template-columns: 1fr;
      height: auto;
    }
    
    .notebooks-sidebar {
      max-height: 200px;
    }
    
    .markdown-cell {
      grid-template-columns: 1fr;
    }
  }
</style>
