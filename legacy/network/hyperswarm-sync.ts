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
  on(event: "data", handler: (data: Uint8Array) => void): void;
  on(event: "error", handler: (error: Error) => void): void;
  on(event: "close", handler: () => void): void;
  write(data: Uint8Array | string): void;
  end(): void;
  remotePublicKey?: Uint8Array;
}

interface HyperswarmInstance {
  join(topic: Uint8Array, options?: { server?: boolean; client?: boolean }): void;
  leave(topic: Uint8Array): Promise<void>;
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
  onMessage?: (payload: {
    msg: MeshMessage;
    peerId: string;
    send: (obj: unknown) => void;
  }) => void;
}

/**
 * Generate a new sync key (32 bytes as hex string)
 * Works in both Node.js and Deno environments
 */
export function generateSyncKey(): string {
  // Use WebCrypto for cross-platform compatibility
  if (typeof crypto !== "undefined" && crypto.getRandomValues) {
    const bytes = new Uint8Array(32);
    crypto.getRandomValues(bytes);
    return Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
  }

  // Fallback to Node.js crypto (dynamic import for compatibility)
  try {
    // eslint-disable-next-line @typescript-eslint/no-var-requires
    const nodeCrypto = require("crypto");
    return nodeCrypto.randomBytes(32).toString("hex");
  } catch {
    throw new Error(
      "Crypto not available. Please use a Node.js or Web environment with crypto support.",
    );
  }
}

/**
 * Derive a DHT topic from a sync key
 * Works in both Node.js and Deno environments
 */
async function deriveTopicFromKey(key: string): Promise<Uint8Array> {
  // Use WebCrypto for cross-platform compatibility
  if (typeof crypto !== "undefined" && crypto.subtle) {
    const encoder = new TextEncoder();
    const keyData = encoder.encode(key);
    const hashBuffer = await crypto.subtle.digest("SHA-256", keyData);
    return new Uint8Array(hashBuffer);
  }

  // Fallback to Node.js crypto
  try {
    // eslint-disable-next-line @typescript-eslint/no-var-requires
    const nodeCrypto = require("crypto");
    const hash = nodeCrypto.createHash("sha256").update(key, "hex").digest();
    return new Uint8Array(hash);
  } catch {
    throw new Error(
      "Crypto not available. Please use a Node.js or Web environment with crypto support.",
    );
  }
}

/**
 * Hyperswarm P2P Sync Manager
 */
export class HyperswarmSync {
  private swarm: HyperswarmInstance | null = null;
  private syncKey: string | null = null;
  private topic: Uint8Array | null = null;
  private connections = new Map<string, HyperswarmConnection>();
  private connectionBuffers = new Map<string, Uint8Array>();
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
    this.topic = await deriveTopicFromKey(key);

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
    debugLog("hyperswarm:enabled", {
      topic: Array.from(this.topic.slice(0, 8), (b) => b.toString(16).padStart(2, "0")).join(""),
    });
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
    this.connectionBuffers.clear();

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
      ? Array.from(connection.remotePublicKey, (b) => b.toString(16).padStart(2, "0")).join("")
      : `peer-${Date.now()}`;

    debugLog("hyperswarm:connection", { peerId: peerId.slice(0, 16) });

    this.connections.set(peerId, connection);
    this.connectionBuffers.set(peerId, new Uint8Array(0));
    this.stats.peersConnected = this.connections.size;

    // Notify handler
    if (this.handlers.onPeerConnected) {
      this.handlers.onPeerConnected({
        peerId,
        connected: true,
        remotePublicKey: connection.remotePublicKey
          ? Array.from(connection.remotePublicKey, (b) => b.toString(16).padStart(2, "0")).join("")
          : undefined,
      });
    }

    // Handle incoming data with buffering for message framing
    connection.on("data", (data: Uint8Array) => {
      this.handleIncomingData(data, peerId, connection);
    });

    connection.on("close", () => {
      debugLog("hyperswarm:peer disconnected", { peerId: peerId.slice(0, 16) });
      this.connections.delete(peerId);
      this.connectionBuffers.delete(peerId);
      this.stats.peersConnected = this.connections.size;

      if (this.handlers.onPeerDisconnected) {
        this.handlers.onPeerDisconnected({
          peerId,
          connected: false,
          remotePublicKey: connection.remotePublicKey
            ? Array.from(connection.remotePublicKey, (b) => b.toString(16).padStart(2, "0")).join(
                "",
              )
            : undefined,
        });
      }
    });

    connection.on("error", (error: Error) => {
      debugLog("hyperswarm:connection error", { peerId: peerId.slice(0, 16), error });
    });
  }

  /**
   * Handle incoming data from a peer with message framing
   * Uses newline-delimited JSON for message boundaries
   */
  private handleIncomingData(
    data: Uint8Array,
    peerId: string,
    connection: HyperswarmConnection,
  ): void {
    try {
      this.stats.bytesReceived += data.length;

      // Append to buffer
      const existingBuffer = this.connectionBuffers.get(peerId) || new Uint8Array(0);
      const combined = new Uint8Array(existingBuffer.length + data.length);
      combined.set(existingBuffer);
      combined.set(data, existingBuffer.length);

      // Process complete messages (newline-delimited)
      const decoder = new TextDecoder();
      const text = decoder.decode(combined);
      const lines = text.split("\n");

      // Keep the last incomplete line in the buffer
      const incompleteText = lines.pop() || "";
      const incompleteBuffer = new TextEncoder().encode(incompleteText);
      this.connectionBuffers.set(peerId, incompleteBuffer);

      // Process each complete message
      for (const line of lines) {
        if (!line.trim()) continue;

        try {
          const message = JSON.parse(line) as MeshMessage;
          this.stats.messagesReceived++;

          debugLog("hyperswarm:received", {
            peerId: peerId.slice(0, 16),
            type: message.type,
          });

          if (this.handlers.onMessage) {
            this.handlers.onMessage({
              msg: message,
              peerId,
              send: (obj: unknown) => this.sendToPeer(obj, peerId, connection),
            });
          }
        } catch (parseError) {
          debugLog("hyperswarm:parse error", {
            peerId: peerId.slice(0, 16),
            error: parseError,
            line,
          });
        }
      }
    } catch (error) {
      debugLog("hyperswarm:data handling error", { peerId: peerId.slice(0, 16), error });
    }
  }

  /**
   * Send a message to a specific peer with newline framing
   */
  private sendToPeer(obj: unknown, peerId: string, connection: HyperswarmConnection): void {
    try {
      const data = JSON.stringify(obj) + "\n"; // Add newline delimiter
      const encoder = new TextEncoder();
      const buffer = encoder.encode(data);

      connection.write(buffer);

      this.stats.bytesTransmitted += buffer.length;
      this.stats.messagesSent++;
    } catch (error) {
      debugLog("hyperswarm:send error", { peerId: peerId.slice(0, 16), error });
    }
  }

  /**
   * Broadcast a message to all connected peers (excluding specified peer if provided)
   */
  broadcast(obj: unknown, excludePeerId?: string): void {
    for (const [peerId, connection] of this.connections.entries()) {
      if (excludePeerId && peerId === excludePeerId) {
        continue; // Skip the excluded peer
      }
      this.sendToPeer(obj, peerId, connection);
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
