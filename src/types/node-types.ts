/**
 * Node.js-specific types for PluresDB
 */

export interface PluresDBConfig {
  port?: number;
  host?: string;
  dataDir?: string;
  webPort?: number;
  logLevel?: "debug" | "info" | "warn" | "error";
}

export interface PluresDBOptions {
  config?: PluresDBConfig;
  autoStart?: boolean;
  denoPath?: string;
}

export interface QueryResult {
  rows: any[];
  columns: string[];
  changes: number;
  lastInsertRowId: number;
}

export interface VectorSearchResult {
  id: string;
  content: string;
  score: number;
  metadata?: any;
}

export interface Peer {
  id: string;
  name: string;
  email: string;
  publicKey: string;
  lastSeen: Date;
  status: "online" | "offline" | "connecting";
}

export interface SharedNode {
  id: string;
  nodeId: string;
  peerId: string;
  accessLevel: "read-only" | "read-write" | "admin";
  encrypted: boolean;
  createdAt: Date;
  expiresAt?: Date;
}

export interface Device {
  id: string;
  name: string;
  type: "laptop" | "phone" | "server" | "desktop";
  lastSync: Date;
  status: "online" | "offline" | "syncing";
}

export interface SyncStatus {
  isOnline: boolean;
  lastSync: Date | null;
  pendingChanges: number;
  connectedPeers: number;
  syncProgress: number;
}
