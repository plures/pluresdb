/**
 * Azure Relay Transport
 * 
 * WSS-based relay transport that looks like normal HTTPS traffic (port 443).
 * Corporate firewall-safe, uses Azure Container Instance relay server.
 */

import type { SyncConnection, SyncTransport } from "../transport.ts";

/**
 * Relay protocol message types
 */
interface RelayMessage {
  type: "join" | "offer" | "answer" | "data" | "peer-joined" | "peer-left" | "error";
  peerId?: string;
  topic?: string;
  data?: string; // Base64-encoded binary data
  payload?: unknown;
}

/**
 * Azure relay connection implementation
 */
class AzureRelayConnection implements SyncConnection {
  private socket: WebSocket;
  private receiverQueue: Uint8Array[] = [];
  private resolvers: ((value: IteratorResult<Uint8Array>) => void)[] = [];
  private closed = false;

  constructor(socket: WebSocket) {
    this.socket = socket;

    // Handle incoming data messages
    this.socket.addEventListener("message", (event: MessageEvent) => {
      try {
        const msg = JSON.parse(event.data as string) as RelayMessage;

        if (msg.type === "data" && msg.data) {
          // Decode base64 to Uint8Array
          const binaryString = atob(msg.data);
          const bytes = new Uint8Array(binaryString.length);
          for (let i = 0; i < binaryString.length; i++) {
            bytes[i] = binaryString.charCodeAt(i);
          }

          // If there's a waiting resolver, give it the data immediately
          const resolver = this.resolvers.shift();
          if (resolver) {
            resolver({ value: bytes, done: false });
          } else {
            // Otherwise queue it
            this.receiverQueue.push(bytes);
          }
        }
      } catch (error) {
        console.error("Azure relay connection error:", error);
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

    // Encode binary data as base64
    let binaryString = "";
    for (let i = 0; i < data.length; i++) {
      binaryString += String.fromCharCode(data[i]);
    }
    const base64Data = btoa(binaryString);

    const msg: RelayMessage = {
      type: "data",
      data: base64Data,
    };

    this.socket.send(JSON.stringify(msg));
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
 * Azure relay transport implementation
 */
export class AzureRelayTransport implements SyncTransport {
  readonly name = "azure-relay";

  private relayUrl: string;
  private topic: string;
  private socket?: WebSocket;
  private connectionHandlers: ((conn: SyncConnection) => void)[] = [];

  constructor(options: {
    relayUrl: string;
    topic?: string;
  }) {
    this.relayUrl = options.relayUrl;
    // Use topic as a namespace for matching peers
    this.topic = options.topic || "pluresdb-default";
  }

  async connect(peerId: string): Promise<SyncConnection> {
    return new Promise((resolve, reject) => {
      const socket = new WebSocket(this.relayUrl);
      let connectionEstablished = false;

      const timeout = setTimeout(() => {
        if (!connectionEstablished) {
          socket.close();
          reject(new Error("Azure relay connection timeout"));
        }
      }, 30000);

      socket.addEventListener("open", () => {
        // Join the topic/room with our peer ID
        const joinMsg: RelayMessage = {
          type: "join",
          topic: this.topic,
          peerId,
        };
        socket.send(JSON.stringify(joinMsg));
      });

      socket.addEventListener("message", (event: MessageEvent) => {
        try {
          const msg = JSON.parse(event.data as string) as RelayMessage;

          // When peer joins, connection is established
          if (msg.type === "peer-joined" && !connectionEstablished) {
            connectionEstablished = true;
            clearTimeout(timeout);
            resolve(new AzureRelayConnection(socket));
          } else if (msg.type === "error") {
            clearTimeout(timeout);
            socket.close();
            reject(new Error(`Azure relay error: ${JSON.stringify(msg.payload)}`));
          }
        } catch (error) {
          console.error("Azure relay message error:", error);
        }
      });

      socket.addEventListener("error", (error) => {
        clearTimeout(timeout);
        reject(new Error(`Azure relay WebSocket error: ${error}`));
      });

      socket.addEventListener("close", () => {
        clearTimeout(timeout);
        if (!connectionEstablished) {
          reject(new Error("Azure relay connection closed before establishment"));
        }
      });
    });
  }

  async listen(onConnection: (conn: SyncConnection) => void): Promise<void> {
    this.connectionHandlers.push(onConnection);

    // Create WebSocket connection to relay server
    this.socket = new WebSocket(this.relayUrl);

    return new Promise((resolve, reject) => {
      if (!this.socket) {
        reject(new Error("Failed to create socket"));
        return;
      }

      this.socket.addEventListener("open", () => {
        if (!this.socket) return;

        // Join the topic as a listening peer
        const joinMsg: RelayMessage = {
          type: "join",
          topic: this.topic,
        };
        this.socket.send(JSON.stringify(joinMsg));
        resolve();
      });

      this.socket.addEventListener("message", (event: MessageEvent) => {
        try {
          const msg = JSON.parse(event.data as string) as RelayMessage;

          // When a peer joins, create a connection for them
          if (msg.type === "peer-joined" && this.socket) {
            const connection = new AzureRelayConnection(this.socket);
            // Notify all registered handlers
            for (const handler of this.connectionHandlers) {
              handler(connection);
            }
          }
        } catch (error) {
          console.error("Azure relay listen message error:", error);
        }
      });

      this.socket.addEventListener("error", (error) => {
        reject(new Error(`Azure relay listen error: ${error}`));
      });
    });
  }

  async close(): Promise<void> {
    if (this.socket) {
      this.socket.close();
      this.socket = undefined;
    }
    this.connectionHandlers = [];
  }
}
