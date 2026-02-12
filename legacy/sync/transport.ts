/**
 * Sync Transport Trait
 * 
 * Pluggable transport layer for P2P sync with three modes:
 * - Azure relay (primary): WSS on port 443, corporate-safe
 * - Vercel relay (backup): Edge WebSocket functions, universally whitelisted
 * - Hyperswarm (direct): DHT + UDP holepunching for home networks
 */

/**
 * Connection interface for bidirectional data transfer
 */
export interface SyncConnection {
  /**
   * Send data to the connected peer
   */
  send(data: Uint8Array): Promise<void>;

  /**
   * Receive data from the connected peer as an async iterable stream
   */
  receive(): AsyncIterable<Uint8Array>;

  /**
   * Close the connection
   */
  close(): Promise<void>;
}

/**
 * Pluggable sync transport interface
 */
export interface SyncTransport {
  /**
   * Transport name (e.g., "azure-relay", "vercel-relay", "hyperswarm")
   */
  readonly name: string;

  /**
   * Connect to a peer using this transport
   * @param peerId Unique identifier for the peer to connect to
   * @returns Promise that resolves to a sync connection
   */
  connect(peerId: string): Promise<SyncConnection>;

  /**
   * Listen for incoming connections
   * @param onConnection Callback invoked when a peer connects
   * @returns Promise that resolves when listening starts
   */
  listen(onConnection: (conn: SyncConnection) => void): Promise<void>;

  /**
   * Close the transport and cleanup resources
   */
  close(): Promise<void>;
}

/**
 * Transport configuration options
 */
export interface TransportConfig {
  /**
   * Transport mode: "auto", "azure-relay", "vercel-relay", "direct"
   */
  mode: "auto" | "azure-relay" | "vercel-relay" | "direct";

  /**
   * Azure relay server URL (WSS endpoint)
   */
  azureRelayUrl?: string;

  /**
   * Vercel relay server URL (fallback WSS endpoint)
   */
  vercelRelayUrl?: string;

  /**
   * Shared secret key for P2P encryption
   */
  syncKey?: string;

  /**
   * Timeout for connection attempts in milliseconds
   */
  connectionTimeoutMs?: number;
}

/**
 * Default transport configuration
 */
export const defaultTransportConfig: TransportConfig = {
  mode: "auto",
  azureRelayUrl: "wss://pluresdb-relay.azurewebsites.net",
  vercelRelayUrl: "wss://pluresdb-relay.vercel.app",
  connectionTimeoutMs: 30000,
};
