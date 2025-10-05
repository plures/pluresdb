<script lang="ts">
	import { Loader2 } from 'lucide-svelte';
	
	// Props
	let message = 'Loading...';
	let showSpinner = true;
</script>

<div class="fixed inset-0 z-50 flex items-center justify-center bg-black bg-opacity-50">
	<div class="bg-white dark:bg-gray-800 rounded-lg p-6 shadow-xl max-w-sm w-full mx-4">
		<div class="text-center">
			{#if showSpinner}
				<div class="flex justify-center mb-4">
					<Loader2 class="w-8 h-8 text-primary-600 animate-spin" />
				</div>
			{/if}
			
			<h3 class="text-lg font-semibold text-gray-900 dark:text-white mb-2">
				{message}
			</h3>
			
			<p class="text-sm text-gray-600 dark:text-gray-400">
				Please wait while we process your request...
			</p>
		</div>
	</div>
</div>


