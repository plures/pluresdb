/**
 * Hyperswarm P2P Sync Transport for PluresDB
 *
 * Provides DHT-based peer discovery and NAT traversal for database synchronization.
 *
 * @module hyperswarm-sync
 */

import type { MeshMessage } from "../types/index.ts";
import { debugLog } from "../util/debug.ts";

// Type definitions for Hyperswarm (for TypeScript/Deno compatibility)

interface HyperswarmConnection {
  on(event: "data", handler: (data: Buffer) => void): void;
  on(event: "error", handler: (error: Error) => void): void;
  on(event: "close", handler: () => void): void;
  write(data: Buffer | string): void;
  end(): void;
  remotePublicKey?: Buffer;
}

interface HyperswarmInstance {
  join(topic: Buffer, options?: { server?: boolean; client?: boolean }): void;
  leave(topic: Buffer): Promise<void>;
  on(event: "connection", handler: (connection: HyperswarmConnection) => void): void;
  on(event: "error", handler: (error: Error) => void): void;
  flush(): Promise<void>;
  destroy(): Promise<void>;
}

export interface SyncKeyOptions {
  /**
   * Custom key (32 bytes hex string). If not provided, generates a new random key.
   */
  key?: string;
}

export interface SyncStats {
  peersConnected: number;
  messagesSent: number;
  messagesReceived: number;
  bytesTransmitted: number;
  bytesReceived: number;
}

export interface PeerInfo {
  peerId: string;
  connected: boolean;
  remotePublicKey?: string;
}

/**
 * Event handler types for Hyperswarm sync
 */
export interface HyperswarmSyncHandlers {
  onPeerConnected?: (info: PeerInfo) => void;
  onPeerDisconnected?: (info: PeerInfo) => void;
  onSyncComplete?: (stats: SyncStats) => void;
  onMessage?: (payload: {
    msg: MeshMessage;
    peerId: string;
    send: (obj: unknown) => void;
  }) => void;
}

/**
 * Generate a new sync key (32 bytes as hex string)
 */
export function generateSyncKey(): string {
  // Use Node.js crypto for key generation
  const crypto = require("crypto");
  return crypto.randomBytes(32).toString("hex");
}

/**
 * Derive a DHT topic from a sync key
 */
function deriveTopicFromKey(key: string): Buffer {
  const crypto = require("crypto");
  // Hash the key to derive a consistent topic
  return crypto.createHash("sha256").update(key, "hex").digest();
}

/**
 * Hyperswarm P2P Sync Manager
 */
export class HyperswarmSync {
  private swarm: HyperswarmInstance | null = null;
  private syncKey: string | null = null;
  private topic: Buffer | null = null;
  private connections = new Map<string, HyperswarmConnection>();
  private handlers: HyperswarmSyncHandlers;
  private stats: SyncStats = {
    peersConnected: 0,
    messagesSent: 0,
    messagesReceived: 0,
    bytesTransmitted: 0,
    bytesReceived: 0,
  };
  private enabled = false;

  constructor(handlers: HyperswarmSyncHandlers = {}) {
    this.handlers = handlers;
  }

  /**
   * Enable sync with a given key.
   * Joins the DHT and starts discovering peers.
   */
  async enableSync(options: SyncKeyOptions): Promise<void> {
    if (this.enabled) {
      throw new Error("Sync already enabled. Call disableSync() first.");
    }

    // Validate or generate sync key
    const key = options.key || generateSyncKey();
    if (!/^[0-9a-f]{64}$/i.test(key)) {
      throw new Error("Sync key must be a 64-character hex string (32 bytes)");
    }

    this.syncKey = key;
    this.topic = deriveTopicFromKey(key);

    debugLog("hyperswarm:enableSync", { keyPrefix: key.slice(0, 8) });

    // Dynamically import Hyperswarm (Node.js only)
    let Hyperswarm: any;
    try {
      Hyperswarm = (await import("hyperswarm")).default;
    } catch (error) {
      throw new Error(
        "Hyperswarm is only available in Node.js environment. Cannot enable P2P sync in Deno/Browser.",
      );
    }

    // Create Hyperswarm instance
    this.swarm = new Hyperswarm() as HyperswarmInstance;

    // Handle incoming connections
    this.swarm.on("connection", (connection: HyperswarmConnection) => {
      this.handleConnection(connection);
    });

    this.swarm.on("error", (error: Error) => {
      debugLog("hyperswarm:error", { error: error.message });
    });

    // Join the DHT topic (both as client and server)
    this.swarm.join(this.topic, { server: true, client: true });

    await this.swarm.flush(); // Wait for initial DHT announcements

    this.enabled = true;
    debugLog("hyperswarm:enabled", { topic: this.topic.toString("hex").slice(0, 16) });
  }

  /**
   * Disable sync and disconnect from all peers
   */
  async disableSync(): Promise<void> {
    if (!this.enabled || !this.swarm || !this.topic) {
      return;
    }

    debugLog("hyperswarm:disableSync");

    // Close all connections
    for (const [peerId, conn] of this.connections.entries()) {
      try {
        conn.end();
      } catch (error) {
        debugLog("hyperswarm:connection close error", { peerId, error });
      }
    }
    this.connections.clear();

    // Leave the DHT topic
    await this.swarm.leave(this.topic);

    // Destroy the swarm instance
    await this.swarm.destroy();

    this.swarm = null;
    this.syncKey = null;
    this.topic = null;
    this.enabled = false;
    this.stats.peersConnected = 0;
  }

  /**
   * Handle a new peer connection
   */
  private handleConnection(connection: HyperswarmConnection): void {
    const peerId = connection.remotePublicKey
      ? connection.remotePublicKey.toString("hex")
      : `peer-${Date.now()}`;

    debugLog("hyperswarm:connection", { peerId: peerId.slice(0, 16) });

    this.connections.set(peerId, connection);
    this.stats.peersConnected = this.connections.size;

    // Notify handler
    if (this.handlers.onPeerConnected) {
      this.handlers.onPeerConnected({
        peerId,
        connected: true,
        remotePublicKey: connection.remotePublicKey?.toString("hex"),
      });
    }

    // Handle incoming data
    connection.on("data", (data: Buffer) => {
      this.handleIncomingData(data, peerId, connection);
    });

    connection.on("close", () => {
      debugLog("hyperswarm:peer disconnected", { peerId: peerId.slice(0, 16) });
      this.connections.delete(peerId);
      this.stats.peersConnected = this.connections.size;

      if (this.handlers.onPeerDisconnected) {
        this.handlers.onPeerDisconnected({
          peerId,
          connected: false,
          remotePublicKey: connection.remotePublicKey?.toString("hex"),
        });
      }
    });

    connection.on("error", (error: Error) => {
      debugLog("hyperswarm:connection error", { peerId: peerId.slice(0, 16), error });
    });
  }

  /**
   * Handle incoming data from a peer
   */
  private handleIncomingData(data: Buffer, peerId: string, connection: HyperswarmConnection): void {
    try {
      this.stats.bytesReceived += data.length;
      this.stats.messagesReceived++;

      const message = JSON.parse(data.toString("utf-8")) as MeshMessage;

      debugLog("hyperswarm:received", {
        peerId: peerId.slice(0, 16),
        type: message.type,
      });

      if (this.handlers.onMessage) {
        this.handlers.onMessage({
          msg: message,
          peerId,
          send: (obj: unknown) => this.sendToPeer(obj, connection),
        });
      }
    } catch (error) {
      debugLog("hyperswarm:parse error", { peerId: peerId.slice(0, 16), error });
    }
  }

  /**
   * Send a message to a specific peer
   */
  private sendToPeer(obj: unknown, connection: HyperswarmConnection): void {
    try {
      const data = JSON.stringify(obj);
      const buffer = Buffer.from(data, "utf-8");

      connection.write(buffer);

      this.stats.bytesTransmitted += buffer.length;
      this.stats.messagesSent++;
    } catch (error) {
      debugLog("hyperswarm:send error", { error });
    }
  }

  /**
   * Broadcast a message to all connected peers
   */
  broadcast(obj: unknown): void {
    for (const connection of this.connections.values()) {
      this.sendToPeer(obj, connection);
    }
  }

  /**
   * Get current sync statistics
   */
  getStats(): SyncStats {
    return { ...this.stats };
  }

  /**
   * Get list of connected peers
   */
  getPeers(): PeerInfo[] {
    return Array.from(this.connections.entries()).map(([peerId, conn]) => ({
      peerId,
      connected: true,
      remotePublicKey: conn.remotePublicKey?.toString("hex"),
    }));
  }

  /**
   * Check if sync is currently enabled
   */
  isEnabled(): boolean {
    return this.enabled;
  }

  /**
   * Get the current sync key (if enabled)
   */
  getSyncKey(): string | null {
    return this.syncKey;
  }
}
