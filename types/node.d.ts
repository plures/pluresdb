/**
 * PluresDB Node.js N-API bindings.
 * For use in Node.js applications.
 */

export {
  NodeId,
  ActorId,
  VectorClock,
  NodeRecord,
  NodeListItem,
  PluresDBOptions,
  DatabaseOptions,
  QueryResult,
  ExecutionResult,
  CrdtOperation
} from './index';

/** PluresDB database instance for Node.js */
export declare class PluresDatabase {
  /**
   * Create a new PluresDB instance.
   * @param actorId - Actor ID for CRDT operations. Defaults to "node-actor".
   * @param dbPath - Path to database file. Omit for in-memory.
   */
  constructor(actorId?: string, dbPath?: string);

  /**
   * Create a PluresDB instance with automatic text embedding.
   * @param model - HuggingFace model ID such as "BAAI/bge-small-en-v1.5"
   * @param actorId - Actor ID for CRDT operations. Defaults to "node-actor".
   * @param dbPath - Path to database file. Omit for in-memory.
   */
  static newWithEmbeddings(model: string, actorId?: string, dbPath?: string): PluresDatabase;

  /** Insert or update a node using CRDT semantics. Returns the node ID. */
  put(id: string, data: unknown): string;

  /** Insert or update a node with an embedding vector. Returns the node ID. */
  putWithEmbedding(id: string, data: unknown, embedding: number[]): string;

  /** Fetch a node by identifier. Returns only the data payload. */
  get(id: string): unknown | null;

  /** Fetch a node with full metadata (clock, timestamp). */
  getWithMetadata(id: string): {
    id: string;
    data: unknown;
    clock: Record<string, number>;
    timestamp: string;
  } | null;

  /** Remove a node from the store. */
  delete(id: string): void;

  /** List all nodes currently stored. */
  list(): Array<{ id: string; data: unknown; timestamp: string }>;

  /** List nodes filtered by a specific type field value. */
  listByType(nodeType: string): Array<{ id: string; data: unknown; timestamp: string }>;

  /** Perform vector similarity search. */
  vectorSearch(
    embedding: number[],
    limit?: number,
    threshold?: number
  ): Array<{ id: string; data: unknown; score: number; timestamp: string }>;

  /** Search nodes by text content (simple string matching). */
  search(query: string, limit?: number): Array<{ id: string; data: unknown; score: number; timestamp: string }>;

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
  query(sql: string, params?: unknown[]): {
    columns: string[];
    rows: unknown[][];
    changes: number;
    lastInsertRowid: number;
  };

  /** Execute SQL statements (requires sqlite-compat feature). */
  exec(sql: string): {
    changes: number;
    lastInsertRowid: number;
  };
}

/** Initialize the module (no-op in current version). */
export declare function init(): void;
