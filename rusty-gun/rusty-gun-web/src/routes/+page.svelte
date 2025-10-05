<script lang="ts">
	import { onMount } from 'svelte';
	import { browser } from '$app/environment';
	
	// Import components
	import Dashboard from '$lib/components/dashboard/Dashboard.svelte';
	import Welcome from '$lib/components/dashboard/Welcome.svelte';
	
	// Import stores
	import { apiConnected, serverStatus } from '$lib/stores/api';
	
	// Reactive state
	let mounted = $state(false);
	
	onMount(async () => {
		mounted = true;
		
		if (browser) {
			// Check server connection
			try {
				const response = await fetch('/api/health');
				const data = await response.json();
				
				if (data.success) {
					apiConnected.set(true);
					serverStatus.set(data.data);
				} else {
					apiConnected.set(false);
				}
			} catch (error) {
				console.error('Failed to connect to server:', error);
				apiConnected.set(false);
			}
		}
	});
</script>

<svelte:head>
	<title>Dashboard - Rusty Gun</title>
</svelte:head>

<div class="h-full">
	{#if mounted}
		{#if $apiConnected}
			<Dashboard />
		{:else}
			<Welcome />
		{/if}
	{:else}
		<!-- Loading state -->
		<div class="h-full flex items-center justify-center">
			<div class="text-center">
				<div class="animate-spin rounded-full h-12 w-12 border-b-2 border-primary-600 mx-auto mb-4"></div>
				<p class="text-gray-600 dark:text-gray-400">Loading Rusty Gun...</p>
			</div>
		</div>
	{/if}
</div>


