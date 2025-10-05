<script lang="ts">
	import { onMount } from 'svelte';
	import { nodes, fetchNodes } from '$lib/stores/api';
	import { Network, RefreshCw } from 'lucide-svelte';
	
	// Reactive state
	let container: HTMLDivElement;
	let cy: any = null;
	let loading = $state(true);
	let error = $state<string | null>(null);
	
	onMount(async () => {
		try {
			// Load Cytoscape dynamically
			const cytoscape = await import('cytoscape');
			const coseBilkent = await import('cytoscape-cose-bilkent');
			const dagre = await import('cytoscape-dagre');
			
			// Register layouts
			cytoscape.default.use(coseBilkent.default);
			cytoscape.default.use(dagre.default);
			
			// Initialize Cytoscape
			cy = cytoscape.default({
				container: container,
				elements: [],
				style: [
					{
						selector: 'node',
						style: {
							'background-color': '#3b82f6',
							'label': 'data(label)',
							'text-valign': 'center',
							'text-halign': 'center',
							'color': 'white',
							'font-size': '12px',
							'font-weight': 'bold',
							'width': '40px',
							'height': '40px',
							'border-width': 2,
							'border-color': '#1e40af'
						}
					},
					{
						selector: 'edge',
						style: {
							'width': 2,
							'line-color': '#6b7280',
							'label': 'data(label)',
							'font-size': '10px',
							'text-rotation': 'autorotate',
							'text-margin-y': -10
						}
					}
				],
				layout: {
					name: 'cose-bilkent',
					animate: true,
					animationDuration: 1000
				}
			});
			
			// Load initial data
			await loadGraphData();
			
		} catch (err) {
			console.error('Failed to initialize graph:', err);
			error = 'Failed to load graph visualization';
		} finally {
			loading = false;
		}
	});
	
	// Load graph data
	async function loadGraphData() {
		try {
			await fetchNodes(50, 0);
			
			if (cy && $nodes.length > 0) {
				// Convert nodes to Cytoscape format
				const elements = $nodes.map(node => ({
					data: {
						id: node.id,
						label: node.data?.name || node.data?.title || node.id,
						type: node.data?.type || 'unknown'
					}
				}));
				
				// Add some sample relationships
				const relationships = [];
				for (let i = 0; i < Math.min($nodes.length - 1, 10); i++) {
					relationships.push({
						data: {
							id: `edge-${i}`,
							source: $nodes[i].id,
							target: $nodes[i + 1].id,
							label: 'related'
						}
					});
				}
				
				cy.add([...elements, ...relationships]);
				cy.layout({ name: 'cose-bilkent' }).run();
			}
		} catch (err) {
			console.error('Failed to load graph data:', err);
			error = 'Failed to load graph data';
		}
	}
	
	// Refresh graph
	async function refreshGraph() {
		loading = true;
		error = null;
		
		try {
			if (cy) {
				cy.elements().remove();
			}
			await loadGraphData();
		} catch (err) {
			console.error('Failed to refresh graph:', err);
			error = 'Failed to refresh graph';
		} finally {
			loading = false;
		}
	}
</script>

<div class="card">
	<div class="card-header">
		<div class="flex items-center justify-between">
			<h3 class="text-lg font-semibold text-gray-900 dark:text-white flex items-center">
				<Network class="w-5 h-5 mr-2" />
				Graph Visualization
			</h3>
			<button
				onclick={refreshGraph}
				disabled={loading}
				class="p-2 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 disabled:opacity-50 disabled:cursor-not-allowed"
			>
				<RefreshCw class="w-4 h-4 {loading ? 'animate-spin' : ''}" />
			</button>
		</div>
	</div>
	
	<div class="h-96 relative">
		{#if loading}
			<div class="absolute inset-0 flex items-center justify-center bg-gray-50 dark:bg-gray-700 rounded-lg">
				<div class="text-center">
					<div class="animate-spin rounded-full h-8 w-8 border-b-2 border-primary-600 mx-auto mb-2"></div>
					<p class="text-sm text-gray-600 dark:text-gray-400">Loading graph...</p>
				</div>
			</div>
		{:else if error}
			<div class="absolute inset-0 flex items-center justify-center bg-gray-50 dark:bg-gray-700 rounded-lg">
				<div class="text-center">
					<Network class="w-12 h-12 text-gray-300 dark:text-gray-600 mx-auto mb-2" />
					<p class="text-sm text-gray-600 dark:text-gray-400">{error}</p>
					<button
						onclick={refreshGraph}
						class="mt-2 text-sm text-primary-600 hover:text-primary-700 dark:text-primary-400 dark:hover:text-primary-300"
					>
						Try again
					</button>
				</div>
			</div>
		{:else}
			<div bind:this={container} class="w-full h-full rounded-lg"></div>
		{/if}
	</div>
	
	<!-- Graph info -->
	{#if $nodes.length > 0}
		<div class="mt-4 pt-4 border-t border-gray-200 dark:border-gray-600">
			<div class="flex items-center justify-between text-sm text-gray-600 dark:text-gray-400">
				<span>{$nodes.length} nodes</span>
				<a
					href="/graph"
					class="text-primary-600 hover:text-primary-700 dark:text-primary-400 dark:hover:text-primary-300 font-medium"
				>
					View full graph →
				</a>
			</div>
		</div>
	{/if}
</div>


