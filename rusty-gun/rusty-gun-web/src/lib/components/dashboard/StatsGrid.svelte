<script lang="ts">
	import { serverStatus, graphStats, vectorStats } from '$lib/stores/api';
	import { Database, Search, Network, Cpu } from 'lucide-svelte';
	
	// Reactive state
	let stats = $state({
		nodes: 0,
		relationships: 0,
		vectors: 0,
		connections: 0
	});
	
	// Update stats when stores change
	$effect(() => {
		if ($graphStats) {
			stats.nodes = $graphStats.node_count || 0;
			stats.relationships = $graphStats.relationship_count || 0;
		}
		
		if ($vectorStats) {
			stats.vectors = $vectorStats.vector_count || 0;
		}
		
		if ($serverStatus) {
			stats.connections = $serverStatus.services?.network === 'healthy' ? 1 : 0;
		}
	});
	
	// Stat cards data
	const statCards = [
		{
			name: 'Nodes',
			value: stats.nodes,
			icon: Database,
			color: 'bg-blue-500',
			change: '+12%',
			changeType: 'positive'
		},
		{
			name: 'Relationships',
			value: stats.relationships,
			icon: Network,
			color: 'bg-green-500',
			change: '+8%',
			changeType: 'positive'
		},
		{
			name: 'Vector Index',
			value: stats.vectors,
			icon: Search,
			color: 'bg-purple-500',
			change: '+15%',
			changeType: 'positive'
		},
		{
			name: 'Active Connections',
			value: stats.connections,
			icon: Cpu,
			color: 'bg-orange-500',
			change: '0%',
			changeType: 'neutral'
		}
	];
</script>

<div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6">
	{#each statCards as card}
		<div class="card hover:shadow-md transition-shadow duration-200">
			<div class="flex items-center justify-between">
				<div>
					<p class="text-sm font-medium text-gray-600 dark:text-gray-400">
						{card.name}
					</p>
					<p class="text-2xl font-bold text-gray-900 dark:text-white">
						{card.value.toLocaleString()}
					</p>
					<div class="flex items-center mt-2">
						<span class="text-sm {
							card.changeType === 'positive' ? 'text-green-600' : 
							card.changeType === 'negative' ? 'text-red-600' : 
							'text-gray-600'
						}">
							{card.change}
						</span>
						<span class="text-sm text-gray-500 dark:text-gray-400 ml-1">
							from last month
						</span>
					</div>
				</div>
				<div class="p-3 rounded-full {card.color} bg-opacity-10">
					<svelte:component this={card.icon} class="w-6 h-6 {card.color.replace('bg-', 'text-')}" />
				</div>
			</div>
		</div>
	{/each}
</div>


