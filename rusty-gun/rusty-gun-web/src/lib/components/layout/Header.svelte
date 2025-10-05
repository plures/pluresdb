<script lang="ts">
	import { page } from '$app/stores';
	import { sidebarOpen, theme, mobileMenuOpen } from '$lib/stores/app';
	import { apiConnected, serverStatus } from '$lib/stores/api';
	
	// Import icons
	import {
		Menu,
		Sun,
		Moon,
		Bell,
		Settings,
		User,
		Wifi,
		WifiOff,
		RefreshCw
	} from 'lucide-svelte';
	
	// Reactive state
	let refreshing = $state(false);
	
	// Toggle sidebar
	function toggleSidebar() {
		sidebarOpen.update(open => !open);
	}
	
	// Toggle theme
	function toggleTheme() {
		theme.update(current => current === 'light' ? 'dark' : 'light');
	}
	
	// Toggle mobile menu
	function toggleMobileMenu() {
		mobileMenuOpen.update(open => !open);
	}
	
	// Refresh data
	async function refreshData() {
		refreshing = true;
		try {
			// Refresh server status
			const response = await fetch('/api/health');
			const data = await response.json();
			
			if (data.success) {
				serverStatus.set(data.data);
			}
		} catch (error) {
			console.error('Failed to refresh data:', error);
		} finally {
			refreshing = false;
		}
	}
	
	// Get page title
	function getPageTitle() {
		const path = $page.url.pathname;
		switch (path) {
			case '/':
				return 'Dashboard';
			case '/nodes':
				return 'Nodes';
			case '/graph':
				return 'Graph';
			case '/vector':
				return 'Vector Search';
			case '/sql':
				return 'SQL Query';
			case '/network':
				return 'Network';
			case '/analytics':
				return 'Analytics';
			case '/settings':
				return 'Settings';
			default:
				return 'Rusty Gun';
		}
	}
</script>

<header class="bg-white dark:bg-gray-800 border-b border-gray-200 dark:border-gray-700 px-4 py-3">
	<div class="flex items-center justify-between">
		<!-- Left side -->
		<div class="flex items-center space-x-4">
			<!-- Mobile menu button -->
			<button
				onclick={toggleSidebar}
				class="p-2 rounded-md text-gray-500 hover:text-gray-700 hover:bg-gray-100 dark:text-gray-400 dark:hover:text-gray-200 dark:hover:bg-gray-700 lg:hidden"
			>
				<Menu class="w-5 h-5" />
			</button>
			
			<!-- Page title -->
			<div>
				<h1 class="text-xl font-semibold text-gray-900 dark:text-white">
					{getPageTitle()}
				</h1>
				{#if $serverStatus}
					<p class="text-sm text-gray-500 dark:text-gray-400">
						Server: {$serverStatus.status || 'Unknown'}
					</p>
				{/if}
			</div>
		</div>
		
		<!-- Right side -->
		<div class="flex items-center space-x-2">
			<!-- Connection status -->
			<div class="flex items-center space-x-2 px-3 py-1 rounded-full text-sm {
				$apiConnected ? 'bg-green-100 text-green-800 dark:bg-green-900 dark:text-green-200' : 'bg-red-100 text-red-800 dark:bg-red-900 dark:text-red-200'
			}">
				{#if $apiConnected}
					<Wifi class="w-4 h-4" />
					<span class="hidden sm:inline">Connected</span>
				{:else}
					<WifiOff class="w-4 h-4" />
					<span class="hidden sm:inline">Disconnected</span>
				{/if}
			</div>
			
			<!-- Refresh button -->
			<button
				onclick={refreshData}
				disabled={refreshing}
				class="p-2 rounded-md text-gray-500 hover:text-gray-700 hover:bg-gray-100 dark:text-gray-400 dark:hover:text-gray-200 dark:hover:bg-gray-700 disabled:opacity-50 disabled:cursor-not-allowed"
			>
				<RefreshCw class="w-5 h-5 {refreshing ? 'animate-spin' : ''}" />
			</button>
			
			<!-- Theme toggle -->
			<button
				onclick={toggleTheme}
				class="p-2 rounded-md text-gray-500 hover:text-gray-700 hover:bg-gray-100 dark:text-gray-400 dark:hover:text-gray-200 dark:hover:bg-gray-700"
			>
				{#if $theme === 'light'}
					<Moon class="w-5 h-5" />
				{:else}
					<Sun class="w-5 h-5" />
				{/if}
			</button>
			
			<!-- Notifications -->
			<button class="p-2 rounded-md text-gray-500 hover:text-gray-700 hover:bg-gray-100 dark:text-gray-400 dark:hover:text-gray-200 dark:hover:bg-gray-700 relative">
				<Bell class="w-5 h-5" />
				<!-- Notification badge -->
				<span class="absolute -top-1 -right-1 w-3 h-3 bg-red-500 rounded-full"></span>
			</button>
			
			<!-- User menu -->
			<div class="relative">
				<button
					onclick={toggleMobileMenu}
					class="p-2 rounded-md text-gray-500 hover:text-gray-700 hover:bg-gray-100 dark:text-gray-400 dark:hover:text-gray-200 dark:hover:bg-gray-700"
				>
					<User class="w-5 h-5" />
				</button>
				
				<!-- Dropdown menu -->
				{#if $mobileMenuOpen}
					<div class="absolute right-0 mt-2 w-48 bg-white dark:bg-gray-800 rounded-md shadow-lg border border-gray-200 dark:border-gray-700 z-50">
						<div class="py-1">
							<a
								href="/settings"
								class="flex items-center px-4 py-2 text-sm text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700"
							>
								<Settings class="w-4 h-4 mr-3" />
								Settings
							</a>
							<button
								onclick={toggleTheme}
								class="flex items-center w-full px-4 py-2 text-sm text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700"
							>
								{#if $theme === 'light'}
									<Moon class="w-4 h-4 mr-3" />
									Dark Mode
								{:else}
									<Sun class="w-4 h-4 mr-3" />
									Light Mode
								{/if}
							</button>
						</div>
					</div>
				{/if}
			</div>
		</div>
	</div>
</header>


