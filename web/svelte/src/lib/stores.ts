import { writable, derived } from 'svelte/store'

export type NodeItem = { id: string; data: Record<string, unknown> }

export const nodes = writable<Record<string, NodeItem>>({})
export const selectedId = writable<string | null>(null)
export const selected = derived([nodes, selectedId], ([$nodes, $id]) => $id ? $nodes[$id] ?? null : null)

export const settings = writable<{ kvPath?: string; port?: number; apiPortOffset?: number; peers?: string[] }>({})

export function upsertNode(item: NodeItem) {
  nodes.update(map => ({ ...map, [item.id]: item }))
}
export function removeNode(id: string) {
  nodes.update(map => { const copy = { ...map }; delete copy[id]; return copy })
}
export function setAll(items: NodeItem[]) {
  const map: Record<string, NodeItem> = {}
  for (const it of items) map[it.id] = it
  nodes.set(map)
}

