import { debugLog } from "../util/debug.ts";

/**
 * Handle returned by {@link startMeshServer}.
 *
 * Provides the server URL, a broadcast helper, and a shutdown method.
 */
export interface MeshServer {
  /** WebSocket URL clients should connect to (e.g. `"ws://localhost:8080"`). */
  url: string;
  /**
   * Send a message to all connected clients.
   *
   * @param obj     - Value to serialise and broadcast.
   * @param exclude - Optional socket to skip (e.g. the sender).
   */
  broadcast: (obj: unknown, exclude?: WebSocket) => void;
  /** Shut down the server and close all open client connections. */
  close: () => void;
}

/**
 * Start a WebSocket mesh server.
 *
 * The server accepts incoming WebSocket connections and routes each JSON
 * message to the provided `onMessage` callback.  The callback receives
 * helpers to reply to the sender or broadcast to all peers.
 *
 * @param args.port      - TCP port to listen on.
 * @param args.onMessage - Handler invoked for every inbound message.
 * @returns A {@link MeshServer} handle with `url`, `broadcast`, and `close`.
 */
export function startMeshServer(args: {
  port: number;
  onMessage: (payload: {
    msg: unknown;
    source: WebSocket;
    send: (obj: unknown) => void;
    broadcast: (obj: unknown, exclude?: WebSocket) => void;
  }) => void;
}): MeshServer {
  const sockets = new Set<WebSocket>();
  // Debug logging is gated by env var in util/debug.ts

  const broadcast = (obj: unknown, exclude?: WebSocket) => {
    const data = JSON.stringify(obj);
    for (const s of sockets) {
      if (s === exclude) continue;
      try {
        s.send(data);
      } catch {
        // ignore
      }
    }
  };

  const server = Deno.serve({ port: args.port, onListen: () => {} }, (req) => {
    debugLog("ws:incoming request");
    const upgrade = Deno.upgradeWebSocket(req);
    const socket = upgrade.socket;

    socket.onopen = () => {
      debugLog("ws:open");
      sockets.add(socket);
    };

    socket.onmessage = (event: MessageEvent<string>) => {
      try {
        const msg = JSON.parse(event.data);
        debugLog("ws:message", { type: (msg as { type?: string }).type });
        args.onMessage({
          msg,
          source: socket,
          send: (obj: unknown) => {
            try {
              socket.send(JSON.stringify(obj));
            } catch {
              /* ignore */
            }
          },
          broadcast,
        });
      } catch {
        // ignore malformed message
      }
    };

    const cleanup = () => sockets.delete(socket);
    socket.onclose = cleanup;
    socket.onerror = cleanup;

    return upgrade.response;
  });

  return {
    url: `ws://localhost:${args.port}`,
    broadcast,
    close: () => {
      try {
        server.shutdown();
      } catch {
        /* ignore */
      }
      for (const s of sockets) {
        try {
          s.close();
        } catch {
          /* ignore */
        }
      }
      sockets.clear();
    },
  };
}

/**
 * Open a WebSocket connection to a remote mesh server.
 *
 * Automatically parses inbound JSON frames and dispatches them to the
 * provided handlers.  Connection errors are silently swallowed to prevent
 * unhandled rejections when the remote peer is unavailable.
 *
 * @param url      - WebSocket URL of the remote server.
 * @param handlers - Optional lifecycle callbacks.
 * @returns The underlying {@link WebSocket} instance.
 */
export function connectToPeer(
  url: string,
  handlers: {
    onOpen?: (socket: WebSocket) => void;
    onMessage?: (msg: unknown, socket: WebSocket) => void;
    onClose?: (socket: WebSocket) => void;
  },
): WebSocket {
  const socket = new WebSocket(url);
  if (handlers.onOpen) socket.onopen = () => handlers.onOpen?.(socket);
  if (handlers.onMessage) {
    socket.onmessage = (e) => {
      try {
        handlers.onMessage?.(JSON.parse(e.data), socket);
      } catch {
        /* ignore */
      }
    };
  }
  if (handlers.onClose) socket.onclose = () => handlers.onClose?.(socket);
  socket.onerror = () => {
    /* ignore */
  };
  return socket;
}
