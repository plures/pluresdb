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

/**
 * A single result returned by {@link NativePluresDatabase.vectorSearch}.
 */
export interface VectorSearchItem {
  /** Stable unique identifier of the matching node. */
  id: string;
  /** Arbitrary JSON payload stored with the node. */
  data: unknown;
  /** Similarity score (higher is more similar). */
  score: number;
  /** RFC 3339 timestamp of the last write that touched this node. */
  timestamp: string;
}

/**
 * Constructor interface for the native {@link NativePluresDatabase} class.
 *
 * Exposed as a typed handle so the platform-specific `.node` binary can be
 * loaded at runtime while still providing full TypeScript type information.
 */
export interface NativePluresDatabaseConstructor {
  /** Create a new database instance, optionally opening a file at `dbPath`. */
  new (actorId?: string | null, dbPath?: string | null): NativePluresDatabase;
  /**
   * Create a new database instance with automatic text embedding enabled.
   *
   * @param model   - HuggingFace model ID (e.g. `"BAAI/bge-small-en-v1.5"`).
   * @param actorId - Optional actor identifier for CRDT attribution.
   */
  newWithEmbeddings(model: string, actorId?: string | null): NativePluresDatabase;
}

/**
 * Native PluresDB database handle (implemented in Rust via N-API).
 *
 * All methods are **synchronous** because they call directly into the Rust
 * library without spawning a subprocess or making network requests.
 */
export interface NativePluresDatabase {
  /** Insert or update a node and return its node ID. */
  put(id: string, data: unknown): string;
  /** Insert or update a node together with a pre-computed embedding vector. */
  putWithEmbedding(id: string, data: unknown, embedding: number[]): string;
  /** Return the node payload for `id`, or `null` if not found. */
  get(id: string): unknown | null;
  /** Return the node payload plus CRDT metadata, or `null` if not found. */
  getWithMetadata(id: string): unknown | null;
  /** Delete the node with the given `id`. */
  delete(id: string): void;
  /** Return all nodes as an array of JSON objects. */
  list(): unknown[];
  /** Return all nodes whose `type` field matches `nodeType`. */
  listByType(nodeType: string): unknown[];
  /** Execute a SQL SELECT and return the result set. */
  query(sql: string, params?: unknown[] | null): unknown;
  /** Execute a SQL statement (INSERT / UPDATE / DELETE) and return write stats. */
  exec(sql: string): unknown;
  /** Full-text search across all node data, returning ranked results. */
  search(query: string, limit?: number | null): unknown[];
  /** Vector similarity search using a pre-computed query embedding. */
  vectorSearch(
    embedding: number[],
    limit?: number | null,
    threshold?: number | null,
  ): VectorSearchItem[];
  /** Subscribe to real-time node change events; returns a subscription token. */
  subscribe(): string;
  /** Return the actor ID associated with this database instance. */
  getActorId(): string;
  /** Return aggregate statistics about the database. */
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

/** The native PluresDB database constructor, loaded from the platform-specific `.node` binary. */
export const PluresDatabase: NativePluresDatabaseConstructor = native.PluresDatabase;
export default PluresDatabase;
