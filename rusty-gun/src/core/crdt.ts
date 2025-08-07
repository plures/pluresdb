import type { NodeRecord, VectorClock } from "../types/index.ts";

export function mergeVectorClocks(a: VectorClock, b: VectorClock): VectorClock {
  const merged: VectorClock = {};
  const keys = new Set([...Object.keys(a ?? {}), ...Object.keys(b ?? {})]);
  for (const key of keys) {
    merged[key] = Math.max(a?.[key] ?? 0, b?.[key] ?? 0);
  }
  return merged;
}

export function mergeNodes(local: NodeRecord | null, incoming: NodeRecord): NodeRecord {
  if (!local) return incoming;
  if (local.id !== incoming.id) {
    throw new Error("mergeNodes called with mismatched ids");
  }

  if (incoming.timestamp > local.timestamp) {
    return { ...incoming, vectorClock: mergeVectorClocks(local.vectorClock, incoming.vectorClock) };
  }
  if (incoming.timestamp < local.timestamp) {
    return { ...local, vectorClock: mergeVectorClocks(local.vectorClock, incoming.vectorClock) };
  }

  // Equal timestamps: deterministic field-wise merge
  const mergedData = { ...local.data, ...incoming.data };
  const mergedVector = incoming.vector ?? local.vector;
  const mergedType = incoming.type ?? local.type;
  return {
    id: local.id,
    data: mergedData,
    vector: mergedVector,
    type: mergedType,
    timestamp: incoming.timestamp, // ties keep timestamp
    vectorClock: mergeVectorClocks(local.vectorClock, incoming.vectorClock),
  };
}
