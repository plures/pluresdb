<script lang="ts">
	import '../app.css';
	import { onMount } from 'svelte';
	import { page } from '$app/stores';
	import { browser } from '$app/environment';
	
	// Import components
	import Sidebar from '$lib/components/layout/Sidebar.svelte';
	import Header from '$lib/components/layout/Header.svelte';
	import ToastContainer from '$lib/components/ui/ToastContainer.svelte';
	import LoadingOverlay from '$lib/components/ui/LoadingOverlay.svelte';
	
	// Import stores
	import { theme, sidebarOpen, loading } from '$lib/stores/app';
	
	// Reactive state
	let mounted = $state(false);
	
	onMount(() => {
		mounted = true;
		
		// Initialize theme
		if (browser) {
			const savedTheme = localStorage.getItem('theme');
			if (savedTheme) {
				theme.set(savedTheme as 'light' | 'dark');
			}
		}
	});
	
	// Apply theme to document
	$effect(() => {
		if (mounted && browser) {
			document.documentElement.classList.toggle('dark', $theme === 'dark');
			localStorage.setItem('theme', $theme);
		}
	});
</script>

<svelte:head>
	<title>Rusty Gun - Graph Database with Vector Search</title>
	<meta name="description" content="High-performance graph database with vector search capabilities" />
</svelte:head>

<div class="h-full flex bg-gray-50 dark:bg-gray-900 transition-colors duration-200">
	<!-- Sidebar -->
	<Sidebar />
	
	<!-- Main content area -->
	<div class="flex-1 flex flex-col min-w-0">
		<!-- Header -->
		<Header />
		
		<!-- Page content -->
		<main class="flex-1 overflow-auto">
			<div class="h-full">
				<slot />
			</div>
		</main>
	</div>
</div>

<!-- Toast notifications -->
<ToastContainer />

<!-- Loading overlay -->
{#if $loading}
	<LoadingOverlay />
{/if}

<!-- Global styles for theme -->
<style>
	:global(html) {
		@apply transition-colors duration-200;
	}
	
	:global(body) {
		@apply transition-colors duration-200;
	}
</style>


