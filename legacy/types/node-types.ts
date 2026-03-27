/**
 * Node.js-specific types for PluresDB
 */

/**
 * Server-side configuration for the embedded PluresDB Deno process.
 */
export interface PluresDBConfig {
  /** TCP port the PluresDB HTTP API listens on. Defaults to `34567`. */
  port?: number;
  /** Hostname to bind to. Defaults to `"localhost"`. */
  host?: string;
  /** File-system directory for persisted data. Defaults to `~/.pluresdb`. */
  dataDir?: string;
  /** TCP port for the web UI server. Defaults to `34568`. */
  webPort?: number;
  /** Log verbosity level. Defaults to `"info"`. */
  logLevel?: "debug" | "info" | "warn" | "error";
}

/**
 * Options for constructing a {@link PluresNode} wrapper.
 */
export interface PluresDBOptions {
  /** Override the default server configuration. */
  config?: PluresDBConfig;
  /** When `true`, start the server immediately in the constructor. */
  autoStart?: boolean;
  /** Path to the `deno` executable. Auto-detected when omitted. */
  denoPath?: string;
}

/**
 * Result returned by a SQL query executed via {@link PluresNode}.
 */
export interface QueryResult {
  /** Array of row objects matching the query. */
  rows: Record<string, unknown>[];
  /** Column names in result order. */
  columns: string[];
  /** Number of rows affected by the last DML statement. */
  changes: number;
  /** Row-id of the last INSERT, or 0 if not applicable. */
  lastInsertRowId: number;
}

/**
 * Options for the `better-sqlite3`-compatible API layer.
 */
export interface BetterSQLite3Options extends PluresDBOptions {
  /** Path to the database file. Ignored — PluresDB manages storage. */
  filename?: string;
  /** When `true`, use an in-memory database. */
  memory?: boolean;
  /** Open the database in read-only mode. */
  readonly?: boolean;
  /** Throw if the database file does not exist. */
  fileMustExist?: boolean;
  /** Verbose logging function (mirrors the `better-sqlite3` API). */
  verbose?: (...args: unknown[]) => void;
}

/**
 * Result returned by `better-sqlite3`-compatible write operations.
 */
export interface BetterSQLite3RunResult {
  /** Number of rows affected. */
  changes: number;
  /** Row-id of the last INSERT, or `null` if not applicable. */
  lastInsertRowid: number | null;
  /** Column names for the executed statement, if available. */
  columns?: string[];
}

/**
 * A single result from a vector similarity search.
 */
export interface VectorSearchResult {
  /** Node identifier. */
  id: string;
  /** Text content of the matching node. */
  content: string;
  /** Cosine similarity score in `[0, 1]`; higher is more similar. */
  score: number;
  /** Optional arbitrary metadata associated with the node. */
  metadata?: Record<string, unknown>;
}

/**
 * Information about a remote P2P peer.
 */
export interface Peer {
  /** Unique peer identifier. */
  id: string;
  /** Human-readable display name. */
  name: string;
  /** Contact email address. */
  email: string;
  /** Public key used for end-to-end encryption (base64 or hex). */
  publicKey: string;
  /** Timestamp of the most recent activity from this peer. */
  lastSeen: Date;
  /** Current connectivity status. */
  status: "online" | "offline" | "connecting";
}

/**
 * A node shared with a remote peer, including access-control metadata.
 */
export interface SharedNode {
  /** Unique sharing record identifier. */
  id: string;
  /** Identifier of the PluresDB node that is being shared. */
  nodeId: string;
  /** Identifier of the peer the node is shared with. */
  peerId: string;
  /** Permission level granted to the peer. */
  accessLevel: "read-only" | "read-write" | "admin";
  /** When `true`, the node data is encrypted before sharing. */
  encrypted: boolean;
  /** When the sharing record was created. */
  createdAt: Date;
  /** Optional expiry time after which the share is revoked. */
  expiresAt?: Date;
}

/**
 * Information about a local device participating in P2P sync.
 */
export interface Device {
  /** Unique device identifier. */
  id: string;
  /** Human-readable device name. */
  name: string;
  /** Device category. */
  type: "laptop" | "phone" | "server" | "desktop";
  /** Timestamp of the last successful sync with this device. */
  lastSync: Date;
  /** Current sync status. */
  status: "online" | "offline" | "syncing";
}

/**
 * Snapshot of the P2P synchronization status.
 */
export interface SyncStatus {
  /** Whether the local node is currently reachable by peers. */
  isOnline: boolean;
  /** Timestamp of the last completed sync, or `null` if never synced. */
  lastSync: Date | null;
  /** Number of local changes not yet replicated to any peer. */
  pendingChanges: number;
  /** Number of currently connected peers. */
  connectedPeers: number;
  /** Sync progress as a fraction in `[0, 1]` (1 = fully synced). */
  syncProgress: number;
}
