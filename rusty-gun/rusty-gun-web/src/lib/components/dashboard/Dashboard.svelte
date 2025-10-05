<script lang="ts">
	import { onMount } from 'svelte';
	import { apiConnected, serverStatus, graphStats, vectorStats } from '$lib/stores/api';
	
	// Import components
	import StatsGrid from './StatsGrid.svelte';
	import RecentNodes from './RecentNodes.svelte';
	import VectorSearchWidget from './VectorSearchWidget.svelte';
	import GraphVisualization from './GraphVisualization.svelte';
	import SystemStatus from './SystemStatus.svelte';
	import QuickActions from './QuickActions.svelte';
	
	// Reactive state
	let mounted = $state(false);
	
	onMount(async () => {
		mounted = true;
		
		// Fetch initial data
		try {
			// Fetch graph stats
			const graphResponse = await fetch('/api/graph/stats');
			const graphData = await graphResponse.json();
			if (graphData.success) {
				graphStats.set(graphData.data);
			}
			
			// Fetch vector stats
			const vectorResponse = await fetch('/api/vector/stats');
			const vectorData = await vectorResponse.json();
			if (vectorData.success) {
				vectorStats.set(vectorData.data);
			}
		} catch (error) {
			console.error('Failed to fetch dashboard data:', error);
		}
	});
</script>

<div class="h-full overflow-auto">
	<div class="max-w-7xl mx-auto px-4 py-6">
		<!-- Welcome section -->
		<div class="mb-8">
			<h1 class="text-3xl font-bold text-gray-900 dark:text-white mb-2">
				Welcome to Rusty Gun
			</h1>
			<p class="text-gray-600 dark:text-gray-400">
				Your high-performance graph database with vector search capabilities
			</p>
		</div>
		
		<!-- Stats grid -->
		<StatsGrid />
		
		<!-- Main content grid -->
		<div class="grid grid-cols-1 lg:grid-cols-3 gap-6 mt-8">
			<!-- Left column -->
			<div class="lg:col-span-2 space-y-6">
				<!-- Graph visualization -->
				<GraphVisualization />
				
				<!-- Recent nodes -->
				<RecentNodes />
			</div>
			
			<!-- Right column -->
			<div class="space-y-6">
				<!-- System status -->
				<SystemStatus />
				
				<!-- Vector search widget -->
				<VectorSearchWidget />
				
				<!-- Quick actions -->
				<QuickActions />
			</div>
		</div>
	</div>
</div>


