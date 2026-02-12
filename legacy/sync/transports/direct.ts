/**
 * Direct Transport (Hyperswarm/Mesh Network)
 * 
 * Wraps the existing mesh network WebSocket implementation.
 * Uses direct P2P connections - best for home networks.
 * Not corporate-safe (non-standard ports, direct connections).
 */

import type { SyncConnection, SyncTransport } from "../transport.ts";
import { connectToPeer, startMeshServer } from "../../network/websocket-server.ts";

/**
 * Direct mesh network connection implementation
 */
class DirectConnection implements SyncConnection {
  private socket: WebSocket;
  private receiverQueue: Uint8Array[] = [];
  private resolvers: ((value: IteratorResult<Uint8Array>) => void)[] = [];
  private closed = false;

  constructor(socket: WebSocket) {
    this.socket = socket;

    // Handle incoming messages
    this.socket.addEventListener("message", (event: MessageEvent) => {
      try {
        let data: Uint8Array;

        // Handle both string and binary data
        if (typeof event.data === "string") {
          // Convert string to Uint8Array
          const encoder = new TextEncoder();
          data = encoder.encode(event.data);
        } else if (event.data instanceof ArrayBuffer) {
          data = new Uint8Array(event.data);
        } else if (event.data instanceof Uint8Array) {
          data = event.data;
        } else {
          console.error("Unknown message data type:", typeof event.data);
          return;
        }

        // If there's a waiting resolver, give it the data immediately
        const resolver = this.resolvers.shift();
        if (resolver) {
          resolver({ value: data, done: false });
        } else {
          // Otherwise queue it
          this.receiverQueue.push(data);
        }
      } catch (error) {
        console.error("Direct connection message error:", error);
      }
    });

    this.socket.addEventListener("close", () => {
      this.closed = true;
      // Resolve all pending receivers with done
      for (const resolver of this.resolvers) {
        resolver({ value: new Uint8Array(0), done: true });
      }
      this.resolvers = [];
    });
  }

  async send(data: Uint8Array): Promise<void> {
    if (this.closed || this.socket.readyState !== WebSocket.OPEN) {
      throw new Error("Connection is closed");
    }

    this.socket.send(data);
  }

  async *receive(): AsyncIterable<Uint8Array> {
    while (!this.closed) {
      // If we have queued data, yield it immediately
      if (this.receiverQueue.length > 0) {
        const data = this.receiverQueue.shift();
        if (data) {
          yield data;
        }
        continue;
      }

      // Otherwise wait for new data
      const data = await new Promise<Uint8Array | null>((resolve) => {
        if (this.closed) {
          resolve(null);
          return;
        }

        this.resolvers.push((result) => {
          if (result.done) {
            resolve(null);
          } else {
            resolve(result.value);
          }
        });
      });

      if (data === null) {
        break;
      }

      yield data;
    }
  }

  async close(): Promise<void> {
    if (!this.closed) {
      this.closed = true;
      this.socket.close();
    }
  }
}

/**
 * Direct mesh network transport implementation
 */
export class DirectTransport implements SyncTransport {
  readonly name = "direct";

  private port?: number;
  private server?: { close: () => void };
  private connections = new Set<WebSocket>();

  constructor(options?: { port?: number }) {
    this.port = options?.port;
  }

  async connect(peerId: string): Promise<SyncConnection> {
    // Parse peerId as URL (e.g., "ws://localhost:8080")
    return new Promise((resolve, reject) => {
      const socket = connectToPeer(peerId, {
        onOpen: (socket) => {
          resolve(new DirectConnection(socket));
        },
        onClose: () => {
          reject(new Error("Direct connection closed before establishment"));
        },
      });

      // Set timeout for connection
      setTimeout(() => {
        if (socket.readyState !== WebSocket.OPEN) {
          socket.close();
          reject(new Error("Direct connection timeout"));
        }
      }, 30000);
    });
  }

  async listen(onConnection: (conn: SyncConnection) => void): Promise<void> {
    if (!this.port) {
      throw new Error("Port is required for direct transport listen");
    }

    this.server = startMeshServer({
      port: this.port,
      onMessage: ({ source }) => {
        // Track new connections - only call onConnection for new WebSockets
        if (!this.connections.has(source)) {
          this.connections.add(source);
          const connection = new DirectConnection(source);
          onConnection(connection);

          // Clean up tracking when connection closes
          source.addEventListener("close", () => {
            this.connections.delete(source);
          });
        }
      },
    });

    // Give the server time to start
    await new Promise((resolve) => setTimeout(resolve, 100));
  }

  async close(): Promise<void> {
    if (this.server) {
      this.server.close();
      this.server = undefined;
    }
    this.connections.clear();
  }
}
