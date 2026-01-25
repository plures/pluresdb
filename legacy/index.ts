/**
 * Deno-first entry point for PluresDB.
 *
 * This module intentionally re-exports only browser/Deno compatible code.
 * Node.js consumers should use the compiled entry at `pluresdb/node` instead.
 */

export { GunDB } from "./core/database.ts";
export type { DatabaseOptions, ServeOptions } from "./core/database.ts";

export { mergeNodes } from "./core/crdt.ts";
export type { MeshMessage, NodeRecord, VectorClock } from "./types/index.ts";

export { startApiServer } from "./http/api-server.ts";
export type { ApiServerHandle } from "./http/api-server.ts";

export { loadConfig, saveConfig } from "./config.ts";

export { connectToPeer, startMeshServer } from "./network/websocket-server.ts";
export type { MeshServer } from "./network/websocket-server.ts";

export { RuleEngine } from "./logic/rules.ts";
export type { Rule, RuleContext } from "./logic/rules.ts";

export { BruteForceVectorIndex } from "./vector/index.ts";
export type { VectorIndex, VectorIndexResult } from "./vector/index.ts";

export { debugLog } from "./util/debug.ts";

export { PluresDBLocalFirst } from "./local-first/unified-api.ts";
export type { LocalFirstOptions, LocalFirstBackend } from "./local-first/unified-api.ts";
