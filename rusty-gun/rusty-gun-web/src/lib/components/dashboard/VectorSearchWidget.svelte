<script lang="ts">
	import { vectorResults, searchVectors } from '$lib/stores/api';
	import { Search, Loader2 } from 'lucide-svelte';
	
	// Reactive state
	let query = $state('');
	let searching = $state(false);
	let threshold = $state(0.3);
	let limit = $state(5);
	
	// Search function
	async function handleSearch() {
		if (!query.trim()) return;
		
		searching = true;
		try {
			await searchVectors(query, limit, threshold);
		} catch (error) {
			console.error('Search failed:', error);
		} finally {
			searching = false;
		}
	}
	
	// Handle enter key
	function handleKeydown(event: KeyboardEvent) {
		if (event.key === 'Enter') {
			handleSearch();
		}
	}
</script>

<div class="card">
	<div class="card-header">
		<h3 class="text-lg font-semibold text-gray-900 dark:text-white flex items-center">
			<Search class="w-5 h-5 mr-2" />
			Vector Search
		</h3>
	</div>
	
	<div class="space-y-4">
		<!-- Search input -->
		<div class="form-group">
			<label for="search-query" class="form-label">Search Query</label>
			<div class="relative">
				<input
					id="search-query"
					type="text"
					bind:value={query}
					onkeydown={handleKeydown}
					placeholder="Enter your search query..."
					class="form-input pr-10"
					disabled={searching}
				/>
				<button
					onclick={handleSearch}
					disabled={searching || !query.trim()}
					class="absolute right-2 top-1/2 transform -translate-y-1/2 p-1 text-gray-400 hover:text-gray-600 disabled:opacity-50 disabled:cursor-not-allowed"
				>
					{#if searching}
						<Loader2 class="w-4 h-4 animate-spin" />
					{:else}
						<Search class="w-4 h-4" />
					{/if}
				</button>
			</div>
		</div>
		
		<!-- Search parameters -->
		<div class="grid grid-cols-2 gap-4">
			<div class="form-group">
				<label for="threshold" class="form-label">Threshold</label>
				<input
					id="threshold"
					type="range"
					min="0"
					max="1"
					step="0.1"
					bind:value={threshold}
					class="w-full"
				/>
				<div class="text-sm text-gray-500 dark:text-gray-400">
					{Math.round(threshold * 100)}%
				</div>
			</div>
			
			<div class="form-group">
				<label for="limit" class="form-label">Results</label>
				<select id="limit" bind:value={limit} class="form-select">
					<option value={5}>5 results</option>
					<option value={10}>10 results</option>
					<option value={20}>20 results</option>
				</select>
			</div>
		</div>
		
		<!-- Search results -->
		{#if $vectorResults.length > 0}
			<div class="space-y-2">
				<h4 class="text-sm font-medium text-gray-700 dark:text-gray-300">
					Search Results ({$vectorResults.length})
				</h4>
				
				<div class="space-y-2 max-h-64 overflow-y-auto">
					{#each $vectorResults as result, index}
						<div class="p-3 bg-gray-50 dark:bg-gray-700 rounded-lg">
							<div class="flex items-center justify-between mb-2">
								<span class="text-sm font-medium text-gray-900 dark:text-white">
									{result.id}
								</span>
								<span class="text-xs text-gray-500 dark:text-gray-400">
									{Math.round(result.score * 100)}% similar
								</span>
							</div>
							
							{#if result.metadata?.title}
								<p class="text-sm text-gray-700 dark:text-gray-300">
									{result.metadata.title}
								</p>
							{/if}
							
							{#if result.metadata?.category}
								<span class="inline-block mt-1 px-2 py-1 text-xs bg-primary-100 text-primary-800 dark:bg-primary-900 dark:text-primary-200 rounded">
									{result.metadata.category}
								</span>
							{/if}
						</div>
					{/each}
				</div>
			</div>
		{:else if query && !searching}
			<div class="text-center py-4 text-gray-500 dark:text-gray-400">
				No results found. Try adjusting your search query or threshold.
			</div>
		{/if}
	</div>
</div>


