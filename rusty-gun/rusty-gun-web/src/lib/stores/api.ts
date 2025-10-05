// API state management using Svelte 5 runes

import { writable } from 'svelte/store';

// API connection state
export const apiConnected = writable(false);
export const serverStatus = writable<any>(null);
export const lastError = writable<string | null>(null);

// API base URL
const API_BASE = '/api';

// Generic API request function
export async function apiRequest<T>(
	endpoint: string,
	options: RequestInit = {}
): Promise<T> {
	try {
		const response = await fetch(`${API_BASE}${endpoint}`, {
			headers: {
				'Content-Type': 'application/json',
				...options.headers,
			},
			...options,
		});

		if (!response.ok) {
			throw new Error(`HTTP ${response.status}: ${response.statusText}`);
		}

		const data = await response.json();
		
		if (!data.success) {
			throw new Error(data.error || 'API request failed');
		}

		return data.data;
	} catch (error) {
		console.error('API request failed:', error);
		lastError.set(error instanceof Error ? error.message : 'Unknown error');
		throw error;
	}
}

// Health check
export async function checkHealth() {
	try {
		const status = await apiRequest('/health');
		apiConnected.set(true);
		serverStatus.set(status);
		return status;
	} catch (error) {
		apiConnected.set(false);
		serverStatus.set(null);
		throw error;
	}
}

// Node operations
export const nodes = writable<any[]>([]);
export const selectedNode = writable<any>(null);

export async function fetchNodes(limit = 100, offset = 0) {
	const data = await apiRequest(`/nodes?limit=${limit}&offset=${offset}`);
	nodes.set(data);
	return data;
}

export async function createNode(nodeData: any) {
	const data = await apiRequest('/nodes', {
		method: 'POST',
		body: JSON.stringify(nodeData),
	});
	nodes.update(nodes => [...nodes, data]);
	return data;
}

export async function updateNode(id: string, nodeData: any) {
	const data = await apiRequest(`/nodes/${id}`, {
		method: 'PUT',
		body: JSON.stringify(nodeData),
	});
	nodes.update(nodes => nodes.map(node => node.id === id ? data : node));
	return data;
}

export async function deleteNode(id: string) {
	await apiRequest(`/nodes/${id}`, {
		method: 'DELETE',
	});
	nodes.update(nodes => nodes.filter(node => node.id !== id));
}

export async function searchNodes(query: string, limit = 10) {
	const data = await apiRequest('/nodes/search', {
		method: 'POST',
		body: JSON.stringify({ query, limit }),
	});
	return data;
}

// Vector search operations
export const vectorResults = writable<any[]>([]);
export const vectorStats = writable<any>(null);

export async function searchVectors(query: string, limit = 5, threshold = 0.3) {
	const data = await apiRequest('/vector/search/text', {
		method: 'POST',
		body: JSON.stringify({ query, limit, threshold }),
	});
	vectorResults.set(data.results || []);
	return data;
}

export async function addVectorText(id: string, text: string, metadata: any = {}) {
	const data = await apiRequest('/vector/text', {
		method: 'POST',
		body: JSON.stringify({ id, text, metadata }),
	});
	return data;
}

export async function fetchVectorStats() {
	const data = await apiRequest('/vector/stats');
	vectorStats.set(data);
	return data;
}

// Graph operations
export const graphStats = writable<any>(null);
export const relationships = writable<any[]>([]);

export async function fetchGraphStats() {
	const data = await apiRequest('/graph/stats');
	graphStats.set(data);
	return data;
}

export async function createRelationship(from: string, to: string, type: string, metadata: any = {}) {
	const data = await apiRequest('/relationships', {
		method: 'POST',
		body: JSON.stringify({ from, to, relation_type: type, metadata }),
	});
	return data;
}

export async function deleteRelationship(from: string, to: string, type: string) {
	await apiRequest(`/relationships/${from}/${to}/${type}`, {
		method: 'DELETE',
	});
}

export async function findPath(from: string, to: string) {
	const data = await apiRequest(`/graph/path/${from}/${to}`);
	return data;
}

// SQL operations
export const sqlResults = writable<any[]>([]);

export async function executeSQL(query: string, params: any[] = []) {
	const data = await apiRequest('/sql/query', {
		method: 'POST',
		body: JSON.stringify({ query, params }),
	});
	sqlResults.set(data.rows || []);
	return data;
}

// WebSocket connection
export const wsConnected = writable(false);
export const wsMessages = writable<any[]>([]);

let ws: WebSocket | null = null;

export function connectWebSocket() {
	if (ws?.readyState === WebSocket.OPEN) return;
	
	ws = new WebSocket(`ws://${window.location.host}/ws`);
	
	ws.onopen = () => {
		wsConnected.set(true);
		console.log('WebSocket connected');
	};
	
	ws.onmessage = (event) => {
		try {
			const message = JSON.parse(event.data);
			wsMessages.update(messages => [...messages, message]);
		} catch (error) {
			console.error('Failed to parse WebSocket message:', error);
		}
	};
	
	ws.onclose = () => {
		wsConnected.set(false);
		console.log('WebSocket disconnected');
		// Attempt to reconnect after 3 seconds
		setTimeout(connectWebSocket, 3000);
	};
	
	ws.onerror = (error) => {
		console.error('WebSocket error:', error);
		wsConnected.set(false);
	};
}

export function disconnectWebSocket() {
	if (ws) {
		ws.close();
		ws = null;
		wsConnected.set(false);
	}
}

export function sendWebSocketMessage(message: any) {
	if (ws?.readyState === WebSocket.OPEN) {
		ws.send(JSON.stringify(message));
	}
}


