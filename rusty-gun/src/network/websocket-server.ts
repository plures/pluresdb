import { debugLog } from "../util/debug.ts";
export interface MeshServer {
  url: string;
  broadcast: (obj: unknown, exclude?: WebSocket) => void;
  close: () => void;
}

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

  const server = Deno.serve({ port: args.port }, (req) => {
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
            } catch { /* ignore */ }
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
      } catch { /* ignore */ }
      for (const s of sockets) {
        try {
          s.close();
        } catch { /* ignore */ }
      }
      sockets.clear();
    },
  };
}

export function connectToPeer(url: string, handlers: {
  onOpen?: (socket: WebSocket) => void;
  onMessage?: (msg: unknown, socket: WebSocket) => void;
  onClose?: (socket: WebSocket) => void;
}): WebSocket {
  const socket = new WebSocket(url);
  if (handlers.onOpen) socket.onopen = () => handlers.onOpen?.(socket);
  if (handlers.onMessage) {
    socket.onmessage = (e) => {
      try {
        handlers.onMessage?.(JSON.parse(e.data), socket);
      } catch { /* ignore */ }
    };
  }
  if (handlers.onClose) socket.onclose = () => handlers.onClose?.(socket);
  socket.onerror = () => {/* ignore */};
  return socket;
}

 
