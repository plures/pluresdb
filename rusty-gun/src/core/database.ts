import { KvStorage } from "../storage/kv-storage.ts";
import type { NodeRecord, MeshMessage } from "../types/index.ts";
import { mergeNodes } from "./crdt.ts";
import { connectToPeer, startMeshServer, type MeshServer } from "../network/websocket-server.ts";

export interface ServeOptions { port?: number }
export interface DatabaseOptions { kvPath?: string; peerId?: string }

export class GunDB {
  private readonly storage: KvStorage;
  private readonly listeners: Map<string, Set<(node: NodeRecord | null) => void>> = new Map();
  private readonly peerId: string;
  private meshServer: MeshServer | null = null;
  private readonly peerSockets: Set<WebSocket> = new Set();
  private closed = false;

  constructor(options?: DatabaseOptions) {
    this.storage = new KvStorage();
    this.peerId = options?.peerId ?? crypto.randomUUID();
    // Open storage synchronously-ish
    // Caller should await ready()
  }

  async ready(kvPath?: string): Promise<void> {
    await this.storage.open(kvPath);
  }

  // Basic CRUD
  async put(id: string, data: Record<string, unknown>): Promise<void> {
    if (this.closed) return;
    const existing = await this.storage.getNode(id);
    const now = Date.now();
    const existingClock = existing?.vectorClock ?? {};
    const newClock = { ...existingClock, [this.peerId]: (existingClock[this.peerId] ?? 0) + 1 };

    let vector: number[] | undefined = undefined;
    if (Array.isArray((data as any).vector)) vector = (data as any).vector as number[];
    else if (typeof (data as any).text === "string") vector = embedTextToVector((data as any).text);
    else if (typeof (data as any).content === "string") vector = embedTextToVector((data as any).content);
    else vector = existing?.vector ?? undefined;

    const updated: NodeRecord = {
      id,
      data,
      vector,
      type: typeof (data as any).type === "string" ? (data as any).type : (existing?.type ?? undefined),
      timestamp: now,
      vectorClock: newClock,
    };

    const merged = mergeNodes(existing, updated);
    await this.storage.setNode(merged);
    this.emit(id, merged);
    this.broadcast({ type: "put", originId: this.peerId, node: merged });
  }

  async get<T = Record<string, unknown>>(id: string): Promise<(T & { id: string }) | null> {
    const node = await this.storage.getNode(id);
    if (!node) return null;
    return { id: node.id, ...(node.data as T) };
  }

  async delete(id: string): Promise<void> {
    if (this.closed) return;
    await this.storage.deleteNode(id);
    this.emit(id, null);
    this.broadcast({ type: "delete", originId: this.peerId, id });
  }

  // Subscriptions
  on(id: string, callback: (node: NodeRecord | null) => void): void {
    const set = this.listeners.get(id) ?? new Set();
    set.add(callback);
    this.listeners.set(id, set);
  }

  off(id: string, callback?: (node: NodeRecord | null) => void): void {
    const set = this.listeners.get(id);
    if (!set) return;
    if (callback) set.delete(callback);
    else set.clear();
  }

  private emit(id: string, node: NodeRecord | null): void {
    const set = this.listeners.get(id);
    if (!set) return;
    for (const cb of set) {
      queueMicrotask(() => cb(node));
    }
  }

  // Vector search
  async vectorSearch(query: string | number[], limit: number): Promise<NodeRecord[]> {
    const queryVector = Array.isArray(query) ? query : embedTextToVector(query);
    const scored: Array<{ score: number; node: NodeRecord }> = [];
    for await (const node of this.storage.listNodes()) {
      if (!node.vector || node.vector.length === 0) continue;
      const score = cosineSimilarity(queryVector, node.vector);
      if (Number.isFinite(score)) scored.push({ score, node });
    }
    scored.sort((a, b) => b.score - a.score);
    return scored.slice(0, limit).map((s) => s.node);
  }

  // Mesh networking
  async serve(options?: ServeOptions): Promise<void> {
    const port = options?.port ?? 8080;
    if (!this.meshServer) {
      this.meshServer = startMeshServer({
        port,
        onMessage: ({ msg, source, send, broadcast }) => {
          this.handleInboundMessage(msg as MeshMessage, { send, broadcast, source });
        },
      });
    }
  }

  connect(url: string): void {
    const socket = connectToPeer(url, {
      onOpen: (s) => {
        // Request a snapshot
        try { s.send(JSON.stringify({ type: "sync_request", originId: this.peerId })); } catch { /* ignore */ }
      },
      onMessage: (msg) => this.handleInboundMessage(msg as MeshMessage, {
        send: (obj) => { try { socket.send(JSON.stringify(obj)); } catch { /* ignore */ } },
        broadcast: (obj) => {/* do not rebroadcast from clients */},
        source: socket,
      }),
    });
    this.peerSockets.add(socket);
    socket.onclose = () => this.peerSockets.delete(socket);
  }

  async close(): Promise<void> {
    this.closed = true;
    for (const s of this.peerSockets) {
      try { s.onmessage = null as any; s.close(); } catch { /* ignore */ }
    }
    this.peerSockets.clear();
    if (this.meshServer) {
      try { this.meshServer.close(); } catch { /* ignore */ }
      this.meshServer = null;
    }
    await this.storage.close();
  }

  private async handleInboundMessage(msg: MeshMessage, ctx: { send: (obj: unknown) => void; broadcast: (obj: unknown, exclude?: WebSocket) => void; source: WebSocket }): Promise<void> {
    if (this.closed) return;
    if (!msg || typeof msg !== "object") return;
    if ((msg as any).originId === this.peerId) return; // ignore our own

    switch (msg.type) {
      case "put": {
        const { node } = msg;
        const existing = await this.storage.getNode(node.id);
        const merged = mergeNodes(existing, node);
        await this.storage.setNode(merged);
        this.emit(node.id, merged);
        // Rebroadcast to other peers if we're acting as server
        try { ctx.broadcast(msg, ctx.source); } catch { /* ignore */ }
        break;
      }
      case "delete": {
        await this.storage.deleteNode(msg.id);
        this.emit(msg.id, null);
        try { ctx.broadcast(msg, ctx.source); } catch { /* ignore */ }
        break;
      }
      case "sync_request": {
        // send snapshot to requester
        for await (const node of this.storage.listNodes()) {
          ctx.send({ type: "put", originId: this.peerId, node });
        }
        break;
      }
    }
  }

  private broadcast(obj: unknown): void {
    if (this.meshServer) {
      try { this.meshServer.broadcast(obj); } catch { /* ignore */ }
    }
    // Also forward to directly connected peers (client mode)
    for (const s of this.peerSockets) {
      try { s.send(JSON.stringify(obj)); } catch { /* ignore */ }
    }
  }
}

// --- utilities ---
function embedTextToVector(text: string, dims = 64): number[] {
  const vec = new Float32Array(dims);
  let h = 2166136261 >>> 0; // FNV-1a baseline
  for (let i = 0; i < text.length; i++) {
    h ^= text.charCodeAt(i);
    h = Math.imul(h, 16777619);
    const idx = h % dims;
    vec[idx] += 1;
  }
  // L2 normalize
  let norm = 0;
  for (let i = 0; i < dims; i++) norm += vec[i] * vec[i];
  norm = Math.sqrt(norm) || 1;
  for (let i = 0; i < dims; i++) vec[i] /= norm;
  return Array.from(vec);
}

function cosineSimilarity(a: number[], b: number[]): number {
  if (a.length !== b.length) {
    const dims = Math.min(a.length, b.length);
    a = a.slice(0, dims);
    b = b.slice(0, dims);
  }
  let dot = 0, na = 0, nb = 0;
  for (let i = 0; i < a.length; i++) {
    const av = a[i] ?? 0;
    const bv = b[i] ?? 0;
    dot += av * bv; na += av * av; nb += bv * bv;
  }
  const denom = Math.sqrt(na) * Math.sqrt(nb) || 1;
  return dot / denom;
}
