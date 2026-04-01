/**
 * PluresDB — P2P Graph Database
 * Shared type definitions for Node.js (N-API) and Browser (WASM) bindings.
 */

/** Unique identifier for a stored node. */
export type NodeId = string;

/** Logical actor identifier used when merging CRDT updates. */
export type ActorId = string;

/** A key-value map of logical clocks per actor. */
export type VectorClock = Record<string, number>;

/** Metadata associated with a persisted node in the CRDT store. */
export interface NodeRecord {
  /** Unique node identifier. */
  id: NodeId;
  /** Arbitrary JSON payload stored with this node. */
  data: unknown;
  /** Per-actor logical write counters used for CRDT merges. */
  clock: VectorClock;
  /** ISO 8601 timestamp of the last write that touched this node. */
  timestamp: string;
  /** Optional embedding vector for vector similarity search. */
  embedding?: number[];
}

/** A search result from vector similarity search. */
export interface VectorSearchResult {
  /** The full node record that matched the query. */
  record: NodeRecord;
  /** Cosine similarity score in [0, 1] where 1 = identical direction. */
  score: number;
}

/** Options for creating a PluresDB instance. */
export interface PluresDBOptions {
  /** Actor ID for CRDT operations. Defaults to a random UUID. */
  actorId?: string;
  /** Path to database file (Node.js only). Omit for in-memory. */
  dbPath?: string;
}

/** Options for opening a SQLite-compatible database. */
export interface DatabaseOptions {
  /** Path to SQLite database file. Omit or ':memory:' for in-memory. */
  path?: string;
  /** Open in read-only mode. */
  readOnly?: boolean;
  /** Apply default performance PRAGMAs (WAL mode, etc.). Default: true. */
  applyDefaultPragmas?: boolean;
}

/** Result of a SQL query. */
export interface QueryResult {
  /** Ordered list of column names. */
  columns: string[];
  /** All matching rows. Each row is an array of column values. */
  rows: unknown[][];
  /** Number of rows affected by the last DML statement. */
  changes: number;
  /** Row-id of the last INSERT, or 0. */
  lastInsertRowid: number;
}

/** Result of executing a DML statement. */
export interface ExecutionResult {
  /** Number of rows modified. */
  changes: number;
  /** Row-id of the last INSERT, or 0. */
  lastInsertRowid: number;
}

/** CRDT operations that can be applied to the store. */
export type CrdtOperation =
  | { type: 'put'; id: NodeId; actor: ActorId; data: unknown }
  | { type: 'delete'; id: NodeId };

/** The main PluresDB database interface. */
export interface PluresDB {
  /** Insert or update a node using CRDT semantics. */
  put(id: string, data: unknown): string;
  
  /** Insert or update a node with an embedding vector. */
  putWithEmbedding(id: string, data: unknown, embedding: number[]): string;
  
  /** Fetch a node by identifier. */
  get(id: string): NodeRecord | null;
  
  /** Remove a node from the store. */
  delete(id: string): void;
  
  /** List all nodes currently stored. */
  list(): NodeRecord[];
  
  /** Perform vector similarity search. */
  vectorSearch(queryEmbedding: number[], limit: number, minScore?: number): VectorSearchResult[];
  
  /** Apply a CRDT operation. */
  apply(op: CrdtOperation): string | null;
}

/** SQLite-compatible database interface. */
export interface Database {
  /** Execute SQL statements that don't return rows (DDL, multi-statement). */
  exec(sql: string): ExecutionResult;
  
  /** Execute a SQL query with parameters. */
  query(sql: string, params?: unknown[]): QueryResult;
  
  /** Execute a PRAGMA statement. */
  pragma(pragma: string): QueryResult;
}
