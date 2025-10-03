<script lang="ts">
  import { onMount, onDestroy } from 'svelte'
  import { nodes, selectedId } from '../lib/stores'
  import { push as toast } from '../lib/toasts'
  
  let graphContainer: HTMLDivElement
  let cy: any = null
  let dark = false
  let layoutType = 'cose-bilkent'
  let showLabels = true
  let showEdges = true
  let filterType = ''
  let searchQuery = ''
  let selectedNodes: Set<string> = new Set()
  let isLassoMode = false
  
  // Layout options
  const layouts = [
    { value: 'cose-bilkent', label: 'Force-directed' },
    { value: 'dagre', label: 'Hierarchical' },
    { value: 'cola', label: 'Constraint-based' },
    { value: 'grid', label: 'Grid' },
    { value: 'circle', label: 'Circle' }
  ]
  
  // Available types for filtering
  let availableTypes: string[] = []
  
  $: dark = (document.documentElement.getAttribute('data-theme') === 'dark')
  $: if (cy) {
    updateTheme()
  }
  
  onMount(async () => {
    await loadGraphData()
    setupGraph()
    loadAvailableTypes()
  })
  
  onDestroy(() => {
    if (cy) {
      cy.destroy()
    }
  })
  
  async function loadGraphData() {
    try {
      const res = await fetch('/api/list')
      const nodeData = await res.json()
      
      // Convert to Cytoscape format
      const elements = []
      const nodeIds = new Set()
      
      // Add nodes
      for (const node of nodeData) {
        if (!nodeIds.has(node.id)) {
          elements.push({
            data: {
              id: node.id,
              label: node.id,
              type: node.data.type || 'unknown',
              data: node.data,
              ...node.data
            }
          })
          nodeIds.add(node.id)
        }
      }
      
      // Add edges based on relationships in data
      for (const node of nodeData) {
        for (const [key, value] of Object.entries(node.data)) {
          if (typeof value === 'string' && value.startsWith('node:')) {
            const targetId = value.replace('node:', '')
            if (nodeIds.has(targetId)) {
              elements.push({
                data: {
                  id: `${node.id}-${targetId}`,
                  source: node.id,
                  target: targetId,
                  label: key,
                  relationship: key
                }
              })
            }
          }
        }
      }
      
      return elements
    } catch (error) {
      toast('Failed to load graph data', 'error')
      console.error('Error loading graph data:', error)
      return []
    }
  }
  
  async function setupGraph() {
    if (!graphContainer) return
    
    const elements = await loadGraphData()
    
    // Initialize Cytoscape
    const Cytoscape = (await import('cytoscape')).default
    const dagre = (await import('cytoscape-dagre')).default
    const cola = (await import('cytoscape-cola')).default
    const coseBilkent = (await import('cytoscape-cose-bilkent')).default
    
    // Register extensions
    Cytoscape.use(dagre)
    Cytoscape.use(cola)
    Cytoscape.use(coseBilkent)
    
    cy = Cytoscape({
      container: graphContainer,
      elements,
      style: getGraphStyle(),
      layout: { name: layoutType },
      userZoomingEnabled: true,
      userPanningEnabled: true,
      boxSelectionEnabled: true,
      selectionType: 'additive'
    })
    
    // Event handlers
    cy.on('tap', 'node', (event: any) => {
      const nodeId = event.target.id()
      selectedId.set(nodeId)
      selectedNodes.add(nodeId)
      updateSelection()
    })
    
    cy.on('tap', 'edge', (event: any) => {
      const edge = event.target
      const source = edge.source().id()
      const target = edge.target().id()
      toast(`Edge: ${source} → ${target}`, 'info')
    })
    
    cy.on('boxend', (event: any) => {
      if (isLassoMode) {
        const selected = event.cy.elements(':selected')
        selected.forEach((node: any) => {
          selectedNodes.add(node.id())
        })
        updateSelection()
      }
    })
    
    // Responsive sizing
    cy.resize()
    window.addEventListener('resize', () => cy.resize())
  }
  
  function getGraphStyle() {
    const baseColor = dark ? '#f6f8fa' : '#24292f'
    const bgColor = dark ? '#0d1117' : '#ffffff'
    const primaryColor = dark ? '#58a6ff' : '#0969da'
    
    return [
      {
        selector: 'node',
        style: {
          'background-color': primaryColor,
          'border-color': baseColor,
          'border-width': 2,
          'label': showLabels ? 'data(label)' : '',
          'text-valign': 'center',
          'text-halign': 'center',
          'color': baseColor,
          'font-size': '12px',
          'font-weight': 'bold',
          'width': '40px',
          'height': '40px',
          'text-wrap': 'wrap',
          'text-max-width': '80px'
        }
      },
      {
        selector: 'node:selected',
        style: {
          'background-color': dark ? '#f85149' : '#cf222e',
          'border-color': dark ? '#f85149' : '#cf222e',
          'border-width': 3
        }
      },
      {
        selector: 'node[type]',
        style: {
          'background-color': (ele: any) => getTypeColor(ele.data('type'))
        }
      },
      {
        selector: 'edge',
        style: {
          'display': showEdges ? 'element' : 'none',
          'width': 2,
          'line-color': dark ? '#8b949e' : '#57606a',
          'target-arrow-color': dark ? '#8b949e' : '#57606a',
          'target-arrow-shape': 'triangle',
          'curve-style': 'bezier',
          'label': 'data(label)',
          'font-size': '10px',
          'color': dark ? '#8b949e' : '#57606a'
        }
      },
      {
        selector: 'edge:selected',
        style: {
          'line-color': dark ? '#f85149' : '#cf222e',
          'target-arrow-color': dark ? '#f85149' : '#cf222e',
          'width': 3
        }
      }
    ]
  }
  
  function getTypeColor(type: string): string {
    const colors = [
      '#0969da', '#1a7f37', '#9a6700', '#8250df', '#cf222e',
      '#bf8700', '#0969da', '#1a7f37', '#9a6700', '#8250df'
    ]
    const hash = type.split('').reduce((a, b) => {
      a = ((a << 5) - a) + b.charCodeAt(0)
      return a & a
    }, 0)
    return colors[Math.abs(hash) % colors.length]
  }
  
  function updateTheme() {
    if (cy) {
      cy.style(getGraphStyle())
    }
  }
  
  function updateSelection() {
    if (cy) {
      cy.elements().unselect()
      selectedNodes.forEach(nodeId => {
        cy.getElementById(nodeId).select()
      })
    }
  }
  
  function changeLayout() {
    if (cy) {
      cy.layout({ name: layoutType }).run()
    }
  }
  
  function toggleLabels() {
    showLabels = !showLabels
    updateTheme()
  }
  
  function toggleEdges() {
    showEdges = !showEdges
    updateTheme()
  }
  
  function filterByType() {
    if (cy) {
      if (filterType) {
        cy.elements().style('display', 'none')
        cy.elements(`node[type="${filterType}"]`).style('display', 'element')
        cy.elements(`edge`).style('display', showEdges ? 'element' : 'none')
      } else {
        cy.elements().style('display', 'element')
      }
    }
  }
  
  function searchNodes() {
    if (cy && searchQuery) {
      cy.elements().unselect()
      const matching = cy.elements(`node[label*="${searchQuery}" i]`)
      matching.select()
      if (matching.length > 0) {
        cy.fit(matching)
        toast(`Found ${matching.length} matching nodes`, 'success')
      } else {
        toast('No matching nodes found', 'info')
      }
    }
  }
  
  function clearSelection() {
    selectedNodes.clear()
    if (cy) {
      cy.elements().unselect()
    }
  }
  
  function fitToScreen() {
    if (cy) {
      cy.fit()
    }
  }
  
  function centerGraph() {
    if (cy) {
      cy.center()
    }
  }
  
  function toggleLassoMode() {
    isLassoMode = !isLassoMode
    if (cy) {
      cy.boxSelectionEnabled(isLassoMode)
    }
  }
  
  async function loadAvailableTypes() {
    try {
      const res = await fetch('/api/list')
      const nodeData = await res.json()
      const types = new Set<string>()
      
      for (const node of nodeData) {
        if (node.data.type) {
          types.add(node.data.type)
        }
      }
      
      availableTypes = Array.from(types).sort()
    } catch (error) {
      console.error('Error loading types:', error)
    }
  }
  
  function exportGraph() {
    if (cy) {
      const png = cy.png({ scale: 2, full: true })
      const link = document.createElement('a')
      link.download = `pluresdb-graph-${Date.now()}.png`
      link.href = png
      link.click()
      toast('Graph exported as PNG', 'success')
    }
  }
</script>

<section aria-labelledby="graph-heading">
  <h3 id="graph-heading">Graph View</h3>
  
  <div class="graph-controls">
    <div class="control-group">
      <label for="layout-select">Layout</label>
      <select id="layout-select" bind:value={layoutType} on:change={changeLayout}>
        {#each layouts as layout}
          <option value={layout.value}>{layout.label}</option>
        {/each}
      </select>
    </div>
    
    <div class="control-group">
      <label for="type-filter">Filter by Type</label>
      <select id="type-filter" bind:value={filterType} on:change={filterByType}>
        <option value="">All Types</option>
        {#each availableTypes as type}
          <option value={type}>{type}</option>
        {/each}
      </select>
    </div>
    
    <div class="control-group">
      <label for="search-input">Search</label>
      <input 
        id="search-input"
        type="text" 
        bind:value={searchQuery} 
        placeholder="Search nodes..."
        on:keydown={(e) => e.key === 'Enter' && searchNodes()}
      />
      <button on:click={searchNodes} disabled={!searchQuery.trim()}>
        Search
      </button>
    </div>
    
    <div class="control-group">
      <label>
        <input type="checkbox" bind:checked={showLabels} on:change={toggleLabels} />
        Show Labels
      </label>
    </div>
    
    <div class="control-group">
      <label>
        <input type="checkbox" bind:checked={showEdges} on:change={toggleEdges} />
        Show Edges
      </label>
    </div>
    
    <div class="control-group">
      <label>
        <input type="checkbox" bind:checked={isLassoMode} on:change={toggleLassoMode} />
        Lasso Mode
      </label>
    </div>
  </div>
  
  <div class="graph-actions">
    <button on:click={clearSelection} class="secondary">
      Clear Selection
    </button>
    <button on:click={fitToScreen} class="secondary">
      Fit to Screen
    </button>
    <button on:click={centerGraph} class="secondary">
      Center
    </button>
    <button on:click={exportGraph} class="secondary">
      Export PNG
    </button>
  </div>
  
  <div class="graph-container">
    <div 
      bind:this={graphContainer}
      class="cytoscape-container"
      role="img"
      aria-label="Interactive graph visualization"
    ></div>
  </div>
  
  <div class="graph-info">
    <p>
      <strong>Selected:</strong> {selectedNodes.size} nodes | 
      <strong>Total:</strong> {cy ? cy.nodes().length : 0} nodes | 
      <strong>Edges:</strong> {cy ? cy.edges().length : 0} edges
    </p>
  </div>
</section>

<style>
  .graph-controls {
    display: flex;
    flex-wrap: wrap;
    gap: 1rem;
    margin-bottom: 1rem;
    padding: 1rem;
    background: var(--pico-muted-border-color);
    border-radius: 8px;
  }
  
  .control-group {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    min-width: 120px;
  }
  
  .control-group label {
    font-size: 0.875rem;
    font-weight: 500;
  }
  
  .control-group input[type="checkbox"] {
    margin-right: 0.5rem;
  }
  
  .graph-actions {
    display: flex;
    gap: 0.5rem;
    margin-bottom: 1rem;
    flex-wrap: wrap;
  }
  
  .graph-container {
    width: 100%;
    height: 600px;
    border: 1px solid var(--pico-muted-border-color);
    border-radius: 8px;
    overflow: hidden;
    background: var(--pico-background-color);
  }
  
  .cytoscape-container {
    width: 100%;
    height: 100%;
  }
  
  .graph-info {
    margin-top: 1rem;
    padding: 0.75rem;
    background: var(--pico-muted-border-color);
    border-radius: 4px;
    font-size: 0.875rem;
  }
  
  @media (max-width: 768px) {
    .graph-controls {
      flex-direction: column;
    }
    
    .control-group {
      min-width: auto;
    }
    
    .graph-container {
      height: 400px;
    }
  }
</style>
