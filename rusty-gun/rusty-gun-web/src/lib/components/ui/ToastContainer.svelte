<script lang="ts">
	import { notifications, removeNotification } from '$lib/stores/app';
	import { CheckCircle, XCircle, AlertTriangle, Info } from 'lucide-svelte';
	
	// Get icon for notification type
	function getIcon(type: string) {
		switch (type) {
			case 'success':
				return CheckCircle;
			case 'error':
				return XCircle;
			case 'warning':
				return AlertTriangle;
			case 'info':
				return Info;
			default:
				return Info;
		}
	}
	
	// Get color classes for notification type
	function getColorClasses(type: string) {
		switch (type) {
			case 'success':
				return 'border-green-200 bg-green-50 dark:bg-green-900 dark:border-green-700';
			case 'error':
				return 'border-red-200 bg-red-50 dark:bg-red-900 dark:border-red-700';
			case 'warning':
				return 'border-yellow-200 bg-yellow-50 dark:bg-yellow-900 dark:border-yellow-700';
			case 'info':
				return 'border-blue-200 bg-blue-50 dark:bg-blue-900 dark:border-blue-700';
			default:
				return 'border-gray-200 bg-gray-50 dark:bg-gray-800 dark:border-gray-700';
		}
	}
	
	// Get icon color for notification type
	function getIconColor(type: string) {
		switch (type) {
			case 'success':
				return 'text-green-500';
			case 'error':
				return 'text-red-500';
			case 'warning':
				return 'text-yellow-500';
			case 'info':
				return 'text-blue-500';
			default:
				return 'text-gray-500';
		}
	}
</script>

<!-- Toast container -->
<div class="fixed top-4 right-4 z-50 space-y-2">
	{#each $notifications as notification}
		<div
			class="toast {getColorClasses(notification.type)} animate-slide-in"
			role="alert"
		>
			<div class="flex items-start">
				<div class="flex-shrink-0">
					<svelte:component
						this={getIcon(notification.type)}
						class="w-5 h-5 {getIconColor(notification.type)}"
					/>
				</div>
				<div class="ml-3 flex-1">
					<h4 class="text-sm font-medium text-gray-900 dark:text-white">
						{notification.title}
					</h4>
					<p class="text-sm text-gray-600 dark:text-gray-300 mt-1">
						{notification.message}
					</p>
				</div>
				<div class="ml-4 flex-shrink-0">
					<button
						onclick={() => removeNotification(notification.id)}
						class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
					>
						<XCircle class="w-4 h-4" />
					</button>
				</div>
			</div>
		</div>
	{/each}
</div>


