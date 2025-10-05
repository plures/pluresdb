<script lang="ts">
	import { serverStatus, apiConnected } from '$lib/stores/api';
	import { CheckCircle, XCircle, AlertCircle, Clock } from 'lucide-svelte';
	
	// Get service status
	function getServiceStatus(service: string) {
		if (!$serverStatus?.services) return 'unknown';
		return $serverStatus.services[service] || 'unknown';
	}
	
	// Get status icon and color
	function getStatusInfo(status: string) {
		switch (status) {
			case 'healthy':
				return {
					icon: CheckCircle,
					color: 'text-green-500',
					bgColor: 'bg-green-100 dark:bg-green-900',
					text: 'Healthy'
				};
			case 'unhealthy':
				return {
					icon: XCircle,
					color: 'text-red-500',
					bgColor: 'bg-red-100 dark:bg-red-900',
					text: 'Unhealthy'
				};
			case 'warning':
				return {
					icon: AlertCircle,
					color: 'text-yellow-500',
					bgColor: 'bg-yellow-100 dark:bg-yellow-900',
					text: 'Warning'
				};
			default:
				return {
					icon: Clock,
					color: 'text-gray-500',
					bgColor: 'bg-gray-100 dark:bg-gray-700',
					text: 'Unknown'
				};
		}
	}
	
	// Services to monitor
	const services = [
		{ name: 'Storage', key: 'storage' },
		{ name: 'Vector Search', key: 'vector_search' },
		{ name: 'Network', key: 'network' },
		{ name: 'API Server', key: 'api' }
	];
</script>

<div class="card">
	<div class="card-header">
		<h3 class="text-lg font-semibold text-gray-900 dark:text-white flex items-center">
			<CheckCircle class="w-5 h-5 mr-2" />
			System Status
		</h3>
	</div>
	
	<div class="space-y-4">
		<!-- Overall status -->
		<div class="flex items-center justify-between p-3 bg-gray-50 dark:bg-gray-700 rounded-lg">
			<div class="flex items-center space-x-2">
				{#if $apiConnected}
					<CheckCircle class="w-5 h-5 text-green-500" />
					<span class="text-sm font-medium text-gray-900 dark:text-white">
						All Systems Operational
					</span>
				{:else}
					<XCircle class="w-5 h-5 text-red-500" />
					<span class="text-sm font-medium text-gray-900 dark:text-white">
						Connection Issues
					</span>
				{/if}
			</div>
			{#if $serverStatus?.timestamp}
				<span class="text-xs text-gray-500 dark:text-gray-400">
					{new Date($serverStatus.timestamp).toLocaleTimeString()}
				</span>
			{/if}
		</div>
		
		<!-- Service status -->
		<div class="space-y-2">
			{#each services as service}
				{@const status = getServiceStatus(service.key)}
				{@const statusInfo = getStatusInfo(status)}
				
				<div class="flex items-center justify-between p-2 rounded-lg {statusInfo.bgColor}">
					<div class="flex items-center space-x-2">
						<svelte:component this={statusInfo.icon} class="w-4 h-4 {statusInfo.color}" />
						<span class="text-sm font-medium text-gray-900 dark:text-white">
							{service.name}
						</span>
					</div>
					<span class="text-xs {statusInfo.color} font-medium">
						{statusInfo.text}
					</span>
				</div>
			{/each}
		</div>
		
		<!-- Server info -->
		{#if $serverStatus}
			<div class="pt-4 border-t border-gray-200 dark:border-gray-600">
				<div class="grid grid-cols-2 gap-4 text-sm">
					<div>
						<span class="text-gray-500 dark:text-gray-400">Version:</span>
						<span class="ml-2 font-medium text-gray-900 dark:text-white">
							{$serverStatus.version || 'Unknown'}
						</span>
					</div>
					<div>
						<span class="text-gray-500 dark:text-gray-400">Uptime:</span>
						<span class="ml-2 font-medium text-gray-900 dark:text-white">
							Running
						</span>
					</div>
				</div>
			</div>
		{/if}
	</div>
</div>


