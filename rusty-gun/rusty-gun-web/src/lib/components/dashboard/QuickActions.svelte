<script lang="ts">
	import { Plus, Search, Code, Network, Database, BarChart3 } from 'lucide-svelte';
	
	// Quick actions
	const actions = [
		{
			name: 'Create Node',
			description: 'Add a new node to the graph',
			icon: Plus,
			href: '/nodes?action=create',
			color: 'bg-blue-500'
		},
		{
			name: 'Vector Search',
			description: 'Search using semantic similarity',
			icon: Search,
			href: '/vector',
			color: 'bg-purple-500'
		},
		{
			name: 'SQL Query',
			description: 'Execute SQL queries',
			icon: Code,
			href: '/sql',
			color: 'bg-green-500'
		},
		{
			name: 'Network Status',
			description: 'View network connections',
			icon: Network,
			href: '/network',
			color: 'bg-orange-500'
		},
		{
			name: 'Database Stats',
			description: 'View database statistics',
			icon: Database,
			href: '/analytics',
			color: 'bg-indigo-500'
		},
		{
			name: 'Analytics',
			description: 'View performance metrics',
			icon: BarChart3,
			href: '/analytics',
			color: 'bg-pink-500'
		}
	];
</script>

<div class="card">
	<div class="card-header">
		<h3 class="text-lg font-semibold text-gray-900 dark:text-white">
			Quick Actions
		</h3>
	</div>
	
	<div class="grid grid-cols-1 gap-3">
		{#each actions as action}
			<a
				href={action.href}
				class="flex items-center p-3 bg-gray-50 dark:bg-gray-700 rounded-lg hover:bg-gray-100 dark:hover:bg-gray-600 transition-colors group"
			>
				<div class="p-2 rounded-lg {action.color} bg-opacity-10 group-hover:bg-opacity-20 transition-all">
					<svelte:component this={action.icon} class="w-5 h-5 {action.color.replace('bg-', 'text-')}" />
				</div>
				<div class="ml-3 flex-1 min-w-0">
					<h4 class="text-sm font-medium text-gray-900 dark:text-white group-hover:text-primary-600 dark:group-hover:text-primary-400">
						{action.name}
					</h4>
					<p class="text-xs text-gray-500 dark:text-gray-400">
						{action.description}
					</p>
				</div>
			</a>
		{/each}
	</div>
</div>


