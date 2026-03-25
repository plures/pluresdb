/**
 * Logical clock used for CRDT conflict resolution.
 *
 * Each key is a peer identifier; the value is the peer's write counter.
 * On every write the local peer's counter is incremented by one.
 */
export interface VectorClock {
  [peerId: string]: number;
}

/**
 * Core data record stored in the PluresDB graph database.
 *
 * Every node has a stable `id`, arbitrary user-supplied `data`, an optional
 * floating-point embedding `vector` for similarity search, an optional `type`
 * label, and CRDT metadata (`timestamp`, `state`, `vectorClock`) used to
 * merge concurrent writes without conflicts.
 */
export interface NodeRecord {
  /** Stable, unique identifier for this node. */
  id: string;
  /** Arbitrary key/value payload stored with the node. */
  data: Record<string, unknown>;
  /** Optional embedding vector used for similarity/vector search. */
  vector?: number[];
  /** Optional type label (e.g. `"User"`, `"Product"`). */
  type?: string;
  /** Unix millisecond timestamp of the last write that touched this node. */
  timestamp: number;
  /**
   * Per-field last-write timestamp used for field-level CRDT merges.
   * Keys are field names; values are the Unix ms timestamp of the latest write
   * that updated that field.
   */
  state?: Record<string, number>;
  /** Vector clock tracking per-peer write counts for this node. */
  vectorClock: VectorClock;
}

/**
 * Mesh network message sent when a node is inserted or updated.
 */
export interface PutMessage {
  /** Discriminant for this message type. */
  type: "put";
  /** Identifier of the peer that originated this write. */
  originId?: string;
  /** The node that was inserted or updated. */
  node: NodeRecord;
}

/**
 * Mesh network message sent when a node is deleted.
 */
export interface DeleteMessage {
  /** Discriminant for this message type. */
  type: "delete";
  /** Identifier of the peer that originated this deletion. */
  originId?: string;
  /** Identifier of the node that was deleted. */
  id: string;
}

/**
 * Mesh network message requesting a full-state snapshot from a peer.
 *
 * Upon receiving this message a peer should reply with a {@link PutMessage}
 * for every node it holds.
 */
export interface SyncRequestMessage {
  /** Discriminant for this message type. */
  type: "sync_request";
  /** Identifier of the peer requesting the snapshot. */
  originId?: string;
}

/**
 * Union of all messages exchanged over the WebSocket mesh network.
 */
export type MeshMessage = PutMessage | DeleteMessage | SyncRequestMessage;
