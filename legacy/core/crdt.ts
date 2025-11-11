import type { NodeRecord, VectorClock } from "../types/index.ts";

export function mergeVectorClocks(a: VectorClock, b: VectorClock): VectorClock {
  const merged: VectorClock = {};
  const keys = new Set([...Object.keys(a ?? {}), ...Object.keys(b ?? {})]);
  for (const key of keys) {
    merged[key] = Math.max(a?.[key] ?? 0, b?.[key] ?? 0);
  }
  return merged;
}

function isPlainObject(val: unknown): val is Record<string, unknown> {
  return typeof val === "object" && val !== null && !Array.isArray(val);
}

function deepMergeWithDeletes(
  base: Record<string, unknown>,
  incoming: Record<string, unknown>,
  baseState: Record<string, number> | undefined,
  incomingState: Record<string, number> | undefined,
  now: number,
): { data: Record<string, unknown>; state: Record<string, number> } {
  const out: Record<string, unknown> = { ...base };
  const outState: Record<string, number> = { ...(baseState ?? {}) };
  for (const [key, incVal] of Object.entries(incoming)) {
    const baseVal = out[key];
    const incTs = (incomingState ?? {})[key] ?? now; // default to now if missing
    const baseTs = (baseState ?? {})[key] ?? 0;
    if (incTs < baseTs) continue; // base wins

    if (incVal === null) {
      delete out[key];
      outState[key] = incTs;
      continue;
    }
    if (isPlainObject(baseVal) && isPlainObject(incVal)) {
      const merged = deepMergeWithDeletes(
        baseVal as Record<string, unknown>,
        incVal as Record<string, unknown>,
        baseState,
        incomingState,
        now,
      );
      out[key] = merged.data;
      outState[key] = incTs;
    } else {
      out[key] = incVal as unknown;
      outState[key] = incTs;
    }
  }
  return { data: out, state: outState };
}

export function mergeNodes(
  local: NodeRecord | null,
  incoming: NodeRecord,
): NodeRecord {
  if (!local) return incoming;
  if (local.id !== incoming.id) {
    throw new Error("mergeNodes called with mismatched ids");
  }

  if (incoming.timestamp > local.timestamp) {
    const merged = deepMergeWithDeletes(
      local.data,
      incoming.data,
      local.state,
      incoming.state,
      incoming.timestamp,
    );
    return {
      id: local.id,
      data: merged.data,
      vector: incoming.vector ?? local.vector,
      type: incoming.type ?? local.type,
      timestamp: incoming.timestamp,
      state: merged.state,
      vectorClock: mergeVectorClocks(local.vectorClock, incoming.vectorClock),
    };
  }
  if (incoming.timestamp < local.timestamp) {
    return {
      ...local,
      vectorClock: mergeVectorClocks(local.vectorClock, incoming.vectorClock),
    };
  }

  // Equal timestamps: deterministic field-wise merge with per-field state
  const merged = deepMergeWithDeletes(
    local.data,
    incoming.data,
    local.state,
    incoming.state,
    incoming.timestamp,
  );
  const mergedVector = incoming.vector ?? local.vector;
  const mergedType = incoming.type ?? local.type;
  return {
    id: local.id,
    data: merged.data,
    vector: mergedVector,
    type: mergedType,
    timestamp: incoming.timestamp, // ties keep timestamp
    state: merged.state,
    vectorClock: mergeVectorClocks(local.vectorClock, incoming.vectorClock),
  };
}
