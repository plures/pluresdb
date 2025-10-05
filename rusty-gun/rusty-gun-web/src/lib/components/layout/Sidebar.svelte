<script lang="ts">
	import { page } from '$app/stores';
	import { sidebarOpen } from '$lib/stores/app';
	import { apiConnected } from '$lib/stores/api';
	
	// Import icons
	import {
		Home,
		Database,
		Search,
		Network,
		Settings,
		BarChart3,
		Code,
		Graph,
		Cpu,
		Globe,
		ChevronLeft,
		ChevronRight
	} from 'lucide-svelte';
	
	// Navigation items
	const navigation = [
		{
			name: 'Dashboard',
			href: '/',
			icon: Home,
			current: false
		},
		{
			name: 'Nodes',
			href: '/nodes',
			icon: Database,
			current: false
		},
		{
			name: 'Graph',
			href: '/graph',
			icon: Graph,
			current: false
		},
		{
			name: 'Vector Search',
			href: '/vector',
			icon: Search,
			current: false
		},
		{
			name: 'SQL Query',
			href: '/sql',
			icon: Code,
			current: false
		},
		{
			name: 'Network',
			href: '/network',
			icon: Network,
			current: false
		},
		{
			name: 'Analytics',
			href: '/analytics',
			icon: BarChart3,
			current: false
		},
		{
			name: 'Settings',
			href: '/settings',
			icon: Settings,
			current: false
		}
	];
	
	// Reactive state
	let isCollapsed = $state(false);
	
	// Toggle sidebar
	function toggleSidebar() {
		sidebarOpen.update(open => !open);
	}
	
	// Toggle collapsed state
	function toggleCollapsed() {
		isCollapsed = !isCollapsed;
	}
	
	// Check if current page matches navigation item
	function isCurrentPage(href: string) {
		return $page.url.pathname === href;
	}
</script>

<!-- Sidebar -->
<div class="flex flex-col h-full bg-white dark:bg-gray-800 border-r border-gray-200 dark:border-gray-700 transition-all duration-300 {isCollapsed ? 'w-16' : 'w-64'}">
	<!-- Header -->
	<div class="flex items-center justify-between p-4 border-b border-gray-200 dark:border-gray-700">
		{#if !isCollapsed}
			<div class="flex items-center space-x-2">
				<div class="w-8 h-8 bg-primary-600 rounded-lg flex items-center justify-center">
					<Cpu class="w-5 h-5 text-white" />
				</div>
				<div>
					<h1 class="text-lg font-semibold text-gray-900 dark:text-white">Rusty Gun</h1>
					<p class="text-xs text-gray-500 dark:text-gray-400">Graph Database</p>
				</div>
			</div>
		{:else}
			<div class="w-8 h-8 bg-primary-600 rounded-lg flex items-center justify-center mx-auto">
				<Cpu class="w-5 h-5 text-white" />
			</div>
		{/if}
		
		<button
			onclick={toggleCollapsed}
			class="p-1 rounded-md hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors"
		>
			{#if isCollapsed}
				<ChevronRight class="w-4 h-4 text-gray-500" />
			{:else}
				<ChevronLeft class="w-4 h-4 text-gray-500" />
			{/if}
		</button>
	</div>
	
	<!-- Navigation -->
	<nav class="flex-1 px-4 py-4 space-y-1">
		{#each navigation as item}
			<a
				href={item.href}
				class="flex items-center px-3 py-2 text-sm font-medium rounded-lg transition-colors duration-200 {
					isCurrentPage(item.href)
						? 'bg-primary-100 text-primary-700 dark:bg-primary-900 dark:text-primary-200'
						: 'text-gray-700 hover:bg-gray-100 dark:text-gray-300 dark:hover:bg-gray-700'
				}"
			>
				<svelte:component this={item.icon} class="w-5 h-5 {isCollapsed ? 'mx-auto' : 'mr-3'}" />
				{#if !isCollapsed}
					<span>{item.name}</span>
				{/if}
			</a>
		{/each}
	</nav>
	
	<!-- Footer -->
	<div class="p-4 border-t border-gray-200 dark:border-gray-700">
		<!-- Connection status -->
		<div class="flex items-center space-x-2 mb-4">
			<div class="w-2 h-2 rounded-full {$apiConnected ? 'bg-green-500' : 'bg-red-500'}"></div>
			{#if !isCollapsed}
				<span class="text-xs text-gray-500 dark:text-gray-400">
					{$apiConnected ? 'Connected' : 'Disconnected'}
				</span>
			{/if}
		</div>
		
		<!-- Version info -->
		{#if !isCollapsed}
			<div class="text-xs text-gray-400 dark:text-gray-500">
				v0.1.0
			</div>
		{/if}
	</div>
</div>

<!-- Mobile overlay -->
{#if $sidebarOpen}
	<div
		class="fixed inset-0 z-40 bg-gray-600 bg-opacity-75 lg:hidden"
		onclick={() => sidebarOpen.set(false)}
	></div>
{/if}


