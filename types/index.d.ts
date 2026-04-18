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
  /** Optional memory quality score in [0, 1]. */
  quality_score?: number;
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

/** Simplified node data returned by list() and search operations. */
export interface NodeListItem {
  /** Unique node identifier. */
  id: NodeId;
  /** Arbitrary JSON payload stored with this node. */
  data: unknown;
  /** ISO 8601 timestamp of the last write that touched this node. */
  timestamp: string;
  /** Optional: Similarity score (for vector_search results). */
  score?: number;
}

/** The main PluresDB database interface. */
export interface PluresDB {
  /** Insert or update a node using CRDT semantics. Returns the node ID. */
  put(id: string, data: unknown): string;
  
  /** Insert or update a node with an embedding vector. Returns the node ID. */
  putWithEmbedding(id: string, data: unknown, embedding: number[]): string;
  
  /** Fetch a node by identifier. Returns only the data payload. */
  get(id: string): unknown | null;
  
  /** Fetch a node with full metadata (clock, timestamp). */
  getWithMetadata(id: string): NodeRecord | null;
  
  /** Remove a node from the store. */
  delete(id: string): void;
  
  /** List all nodes currently stored. */
  list(): NodeListItem[];
  
  /** List nodes filtered by a specific type field value. */
  listByType(nodeType: string): NodeListItem[];
  
  /** Perform vector similarity search. */
  vectorSearch(embedding: number[], limit?: number, threshold?: number): NodeListItem[];
  
  /** Search nodes by text content (simple string matching). */
  search(query: string, limit?: number): NodeListItem[];
  
  /** Subscribe to node changes. Returns a subscription ID. */
  subscribe(): string;
  
  /** Embed text using the configured embedding model (only if created with newWithEmbeddings). */
  embed(texts: string[]): number[][];
  
  /** Get the embedding dimension, or null if no embedder is configured. */
  embeddingDimension(): number | null;
  
  /** Get the actor ID for this database instance. */
  getActorId(): string;
  
  /** Execute a DSL query string against the CRDT store. */
  execDsl(query: string): unknown;
  
  /** Execute a JSON IR query against the CRDT store. */
  execIr(steps: unknown): unknown;
  
  /** Build the HNSW vector index from hydrated embeddings. */
  buildVectorIndex(): number;
  
  /** Get database statistics. */
  stats(): { totalNodes: number; typeCounts: Record<string, number> };
  
  /** Execute a SQL query with optional parameters (requires sqlite-compat feature). */
  query(sql: string, params?: unknown[]): QueryResult;
  
  /** Execute SQL statements (requires sqlite-compat feature). */
  exec(sql: string): ExecutionResult;
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
