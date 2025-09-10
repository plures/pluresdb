export interface VectorClock {
  [peerId: string]: number;
}

export interface NodeRecord {
  id: string;
  data: Record<string, unknown>;
  vector?: number[];
  type?: string;
  timestamp: number;
  state?: Record<string, number>;
  vectorClock: VectorClock;
}

export interface PutMessage {
  type: "put";
  originId?: string;
  node: NodeRecord;
}

export interface DeleteMessage {
  type: "delete";
  originId?: string;
  id: string;
}

export interface SyncRequestMessage {
  type: "sync_request";
  originId?: string;
}

export type MeshMessage = PutMessage | DeleteMessage | SyncRequestMessage;
