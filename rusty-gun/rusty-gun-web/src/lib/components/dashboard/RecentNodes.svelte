<script lang="ts">
	import { onMount } from 'svelte';
	import { nodes, fetchNodes } from '$lib/stores/api';
	import { Database, Eye, Edit, Trash2 } from 'lucide-svelte';
	
	// Reactive state
	let loading = $state(true);
	
	onMount(async () => {
		try {
			await fetchNodes(10, 0);
		} catch (error) {
			console.error('Failed to fetch nodes:', error);
		} finally {
			loading = false;
		}
	});
	
	// Format date
	function formatDate(dateString: string) {
		return new Date(dateString).toLocaleDateString('en-US', {
			month: 'short',
			day: 'numeric',
			hour: '2-digit',
			minute: '2-digit'
		});
	}
	
	// Get node type from data
	function getNodeType(node: any) {
		return node.data?.type || 'Unknown';
	}
	
	// Get node name from data
	function getNodeName(node: any) {
		return node.data?.name || node.data?.title || node.id;
	}
</script>

<div class="card">
	<div class="card-header">
		<h3 class="text-lg font-semibold text-gray-900 dark:text-white flex items-center">
			<Database class="w-5 h-5 mr-2" />
			Recent Nodes
		</h3>
	</div>
	
	{#if loading}
		<div class="space-y-3">
			{#each Array(5) as _}
				<div class="animate-pulse">
					<div class="h-16 bg-gray-200 dark:bg-gray-700 rounded-lg"></div>
				</div>
			{/each}
		</div>
	{:else if $nodes.length > 0}
		<div class="space-y-3">
			{#each $nodes.slice(0, 5) as node}
				<div class="flex items-center justify-between p-3 bg-gray-50 dark:bg-gray-700 rounded-lg hover:bg-gray-100 dark:hover:bg-gray-600 transition-colors">
					<div class="flex-1 min-w-0">
						<div class="flex items-center space-x-2">
							<h4 class="text-sm font-medium text-gray-900 dark:text-white truncate">
								{getNodeName(node)}
							</h4>
							<span class="inline-flex items-center px-2 py-1 text-xs font-medium bg-primary-100 text-primary-800 dark:bg-primary-900 dark:text-primary-200 rounded">
								{getNodeType(node)}
							</span>
						</div>
						<p class="text-xs text-gray-500 dark:text-gray-400 mt-1">
							Created {formatDate(node.created_at)}
						</p>
					</div>
					
					<div class="flex items-center space-x-1 ml-4">
						<button class="p-1 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300">
							<Eye class="w-4 h-4" />
						</button>
						<button class="p-1 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300">
							<Edit class="w-4 h-4" />
						</button>
						<button class="p-1 text-gray-400 hover:text-red-600 dark:hover:text-red-400">
							<Trash2 class="w-4 h-4" />
						</button>
					</div>
				</div>
			{/each}
		</div>
		
		<div class="mt-4 pt-4 border-t border-gray-200 dark:border-gray-600">
			<a
				href="/nodes"
				class="text-sm text-primary-600 hover:text-primary-700 dark:text-primary-400 dark:hover:text-primary-300 font-medium"
			>
				View all nodes →
			</a>
		</div>
	{:else}
		<div class="text-center py-8 text-gray-500 dark:text-gray-400">
			<Database class="w-12 h-12 mx-auto mb-4 text-gray-300 dark:text-gray-600" />
			<p class="text-sm">No nodes found</p>
			<p class="text-xs mt-1">Create your first node to get started</p>
		</div>
	{/if}
</div>


