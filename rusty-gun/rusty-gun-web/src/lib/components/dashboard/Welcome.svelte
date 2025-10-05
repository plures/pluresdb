<script lang="ts">
	import { onMount } from 'svelte';
	import { apiConnected, checkHealth } from '$lib/stores/api';
	import { Cpu, Database, Search, Network, RefreshCw, AlertCircle } from 'lucide-svelte';
	
	// Reactive state
	let checking = $state(false);
	let retryCount = $state(0);
	
	// Check connection
	async function checkConnection() {
		checking = true;
		try {
			await checkHealth();
		} catch (error) {
			console.error('Health check failed:', error);
			retryCount++;
		} finally {
			checking = false;
		}
	}
	
	// Auto-retry connection
	onMount(() => {
		const interval = setInterval(() => {
			if (!$apiConnected && retryCount < 5) {
				checkConnection();
			} else if ($apiConnected) {
				clearInterval(interval);
			}
		}, 3000);
		
		return () => clearInterval(interval);
	});
	
	// Features list
	const features = [
		{
			icon: Database,
			title: 'Graph Database',
			description: 'High-performance graph operations with CRDT-based conflict resolution'
		},
		{
			icon: Search,
			title: 'Vector Search',
			description: 'Semantic search powered by HNSW algorithm and multiple embedding models'
		},
		{
			icon: Network,
			title: 'P2P Networking',
			description: 'Distributed networking with WebRTC, QUIC, and LibP2P support'
		},
		{
			icon: Cpu,
			title: 'Real-time Sync',
			description: 'Local-first applications with real-time synchronization'
		}
	];
</script>

<div class="h-full flex items-center justify-center">
	<div class="max-w-4xl mx-auto px-4 py-12 text-center">
		<!-- Logo and title -->
		<div class="mb-8">
			<div class="w-20 h-20 bg-primary-600 rounded-2xl flex items-center justify-center mx-auto mb-6">
				<Cpu class="w-10 h-10 text-white" />
			</div>
			<h1 class="text-4xl font-bold text-gray-900 dark:text-white mb-4">
				Rusty Gun
			</h1>
			<p class="text-xl text-gray-600 dark:text-gray-400 mb-8">
				High-performance graph database with vector search capabilities
			</p>
		</div>
		
		<!-- Connection status -->
		<div class="mb-12">
			{#if checking}
				<div class="flex items-center justify-center space-x-2 text-gray-600 dark:text-gray-400">
					<RefreshCw class="w-5 h-5 animate-spin" />
					<span>Connecting to server...</span>
				</div>
			{:else if !$apiConnected}
				<div class="flex items-center justify-center space-x-2 text-red-600 dark:text-red-400 mb-4">
					<AlertCircle class="w-5 h-5" />
					<span>Unable to connect to server</span>
				</div>
				<button
					onclick={checkConnection}
					class="btn-primary"
				>
					<RefreshCw class="w-4 h-4 mr-2" />
					Retry Connection
				</button>
			{/if}
		</div>
		
		<!-- Features grid -->
		<div class="grid grid-cols-1 md:grid-cols-2 gap-8 mb-12">
			{#each features as feature}
				<div class="text-left">
					<div class="flex items-center space-x-3 mb-3">
						<div class="p-2 bg-primary-100 dark:bg-primary-900 rounded-lg">
							<svelte:component this={feature.icon} class="w-6 h-6 text-primary-600 dark:text-primary-400" />
						</div>
						<h3 class="text-lg font-semibold text-gray-900 dark:text-white">
							{feature.title}
						</h3>
					</div>
					<p class="text-gray-600 dark:text-gray-400">
						{feature.description}
					</p>
				</div>
			{/each}
		</div>
		
		<!-- Getting started -->
		<div class="bg-gray-50 dark:bg-gray-800 rounded-2xl p-8">
			<h2 class="text-2xl font-bold text-gray-900 dark:text-white mb-4">
				Getting Started
			</h2>
			<p class="text-gray-600 dark:text-gray-400 mb-6">
				Make sure the Rusty Gun server is running and accessible at the configured endpoint.
			</p>
			
			<div class="grid grid-cols-1 md:grid-cols-3 gap-4 text-left">
				<div class="p-4 bg-white dark:bg-gray-700 rounded-lg">
					<h4 class="font-semibold text-gray-900 dark:text-white mb-2">1. Start Server</h4>
					<p class="text-sm text-gray-600 dark:text-gray-400">
						Run the Rusty Gun server using the CLI or API server
					</p>
				</div>
				
				<div class="p-4 bg-white dark:bg-gray-700 rounded-lg">
					<h4 class="font-semibold text-gray-900 dark:text-white mb-2">2. Configure</h4>
					<p class="text-sm text-gray-600 dark:text-gray-400">
						Set up your database and vector search configuration
					</p>
				</div>
				
				<div class="p-4 bg-white dark:bg-gray-700 rounded-lg">
					<h4 class="font-semibold text-gray-900 dark:text-white mb-2">3. Explore</h4>
					<p class="text-sm text-gray-600 dark:text-gray-400">
						Start creating nodes, relationships, and vector searches
					</p>
				</div>
			</div>
		</div>
		
		<!-- Documentation link -->
		<div class="mt-8">
			<a
				href="https://github.com/rusty-gun/rusty-gun"
				target="_blank"
				rel="noopener noreferrer"
				class="inline-flex items-center text-primary-600 hover:text-primary-700 dark:text-primary-400 dark:hover:text-primary-300 font-medium"
			>
				View Documentation
				<svg class="w-4 h-4 ml-1" fill="none" stroke="currentColor" viewBox="0 0 24 24">
					<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10 6H6a2 2 0 00-2 2v10a2 2 0 002 2h10a2 2 0 002-2v-4M14 4h6m0 0v6m0-6L10 14" />
				</svg>
			</a>
		</div>
	</div>
</div>


