/**
 * NAPI-based entry point for PluresDB Node.js bindings.
 *
 * This is the primary interface for Node.js consumers.
 * The native Rust bindings provide direct in-process access to the database,
 * eliminating the need to spawn a separate Deno server process.
 *
 * For the legacy HTTP-based interface, use '@plures/pluresdb/legacy'.
 */

import * as path from "node:path";

export interface VectorSearchItem {
  id: string;
  data: unknown;
  score: number;
  timestamp: string;
}

export interface NativePluresDatabaseConstructor {
  new (actorId?: string | null, dbPath?: string | null): NativePluresDatabase;
  newWithEmbeddings(model: string, actorId?: string | null): NativePluresDatabase;
}

export interface NativePluresDatabase {
  put(id: string, data: unknown): string;
  putWithEmbedding(id: string, data: unknown, embedding: number[]): string;
  get(id: string): unknown | null;
  getWithMetadata(id: string): unknown | null;
  delete(id: string): void;
  list(): unknown[];
  listByType(nodeType: string): unknown[];
  query(sql: string, params?: unknown[] | null): unknown;
  exec(sql: string): unknown;
  search(query: string, limit?: number | null): unknown[];
  vectorSearch(
    embedding: number[],
    limit?: number | null,
    threshold?: number | null,
  ): VectorSearchItem[];
  subscribe(): string;
  getActorId(): string;
  stats(): unknown;
}

// The crates/pluresdb-node/index.js loader selects the correct platform-specific
// .node binary and falls back to scoped npm packages (e.g.
// @plures/pluresdb-native-linux-x64-gnu) when no local binary is present.
//
// Path calculation: this file compiles to dist/napi/index.js, so __dirname at
// runtime is <pkg-root>/dist/napi/ and ../../ resolves to <pkg-root>/, which is
// exactly where crates/pluresdb-node/index.js lives (included in the npm package
// via the "files" field in package.json).
const nativeLoaderPath = path.resolve(__dirname, "../../crates/pluresdb-node/index.js");

// eslint-disable-next-line @typescript-eslint/no-require-imports
const native = require(nativeLoaderPath) as {
  PluresDatabase: NativePluresDatabaseConstructor;
};

export const PluresDatabase: NativePluresDatabaseConstructor = native.PluresDatabase;
export default PluresDatabase;
