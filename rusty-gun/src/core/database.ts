import { KvStorage } from "../storage/kv-storage.ts";
import type { MeshMessage, NodeRecord } from "../types/index.ts";
import { mergeNodes } from "./crdt.ts";
import {
  connectToPeer,
  type MeshServer,
  startMeshServer,
} from "../network/websocket-server.ts";
import { debugLog } from "../util/debug.ts";
import { RuleEngine, type Rule, type RuleContext } from "../logic/rules.ts";
import { BruteForceVectorIndex } from "../vector/index.ts";

export interface ServeOptions {
  port?: number;
}
export interface DatabaseOptions {
  kvPath?: string;
  peerId?: string;
}

export class GunDB {
  private readonly storage: KvStorage;
  private readonly listeners: Map<
    string,
    Set<(node: NodeRecord | null) => void>
  > = new Map();
  private readonly anyListeners: Set<(event: { id: string; node: NodeRecord | null }) => void> = new Set();
  private readonly peerId: string;
  private meshServer: MeshServer | null = null;
  private readonly peerSockets: Set<WebSocket> = new Set();
  private closed = false;
  private readonly rules = new RuleEngine();
  private readonly vectorIndex = new BruteForceVectorIndex();

  constructor(options?: DatabaseOptions) {
    this.storage = new KvStorage();
    this.peerId = options?.peerId ?? crypto.randomUUID();
    // Open storage synchronously-ish
    // Caller should await ready()
  }

  async ready(kvPath?: string): Promise<void> {
    await this.storage.open(kvPath);
    // Rebuild in-memory vector index from storage
    for await (const node of this.storage.listNodes()) {
      if (node.vector && node.vector.length > 0) {
        this.vectorIndex.upsert(node.id, node.vector);
      }
    }
  }

  // Basic CRUD
  async put(id: string, data: Record<string, unknown>): Promise<void> {
    await this.applyPut(id, data, false);
  }

  private async applyPut(id: string, data: Record<string, unknown>, suppressRules: boolean): Promise<void> {
    if (this.closed) return;
    debugLog("put()", { id, keys: Object.keys(data ?? {}) });
    const existing = await this.storage.getNode(id);
    const now = Date.now();
    const existingClock = existing?.vectorClock ?? {};
    const newClock = {
      ...existingClock,
      [this.peerId]: (existingClock[this.peerId] ?? 0) + 1,
    };

    let vector: number[] | undefined = undefined;
    const record = data as Record<string, unknown>;
    const maybeVector = record.vector as unknown;
    if (
      Array.isArray(maybeVector) &&
      maybeVector.every((v) => typeof v === "number" && Number.isFinite(v))
    ) {
      vector = maybeVector as number[];
    } else if (typeof record.text === "string") {
      vector = embedTextToVector(record.text);
    } else if (typeof record.content === "string") {
      vector = embedTextToVector(record.content);
    } else vector = existing?.vector ?? undefined;

    const newState: Record<string, number> = { ...(existing?.state ?? {}) };
    for (const key of Object.keys(data ?? {})) newState[key] = now;

    const updated: NodeRecord = {
      id,
      data,
      vector,
      type: typeof record.type === "string"
        ? (record.type as string)
        : (existing?.type ?? undefined),
      timestamp: now,
      state: newState,
      vectorClock: newClock,
    };

    const merged = mergeNodes(existing, updated);
    await this.storage.setNode(merged);
    debugLog("put() merged", { id, timestamp: merged.timestamp });
    this.emit(id, merged);
    if (merged.vector && merged.vector.length > 0) this.vectorIndex.upsert(id, merged.vector);
    else this.vectorIndex.remove(id);
    if (!suppressRules) {
      await this.evaluateRules(merged);
    }
    this.broadcast({ type: "put", originId: this.peerId, node: merged });
  }

  async get<T = Record<string, unknown>>(
    id: string,
  ): Promise<(T & { id: string }) | null> {
    const node = await this.storage.getNode(id);
    if (!node) return null;
    return { id: node.id, ...(node.data as T) };
  }

  async delete(id: string): Promise<void> {
    if (this.closed) return;
    debugLog("delete()", { id });
    await this.storage.deleteNode(id);
    this.emit(id, null);
    this.vectorIndex.remove(id);
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
    for (const cb of this.anyListeners) {
      queueMicrotask(() => cb({ id, node }));
    }
  }

  // Vector search
  async vectorSearch(
    query: string | number[],
    limit: number,
  ): Promise<NodeRecord[]> {
    const queryVector = Array.isArray(query) ? query : embedTextToVector(query);
    const results = this.vectorIndex.search(queryVector, limit);
    if (results.length > 0) {
      const nodes: NodeRecord[] = [];
      for (const r of results) {
        const n = await this.storage.getNode(r.id);
        if (n) nodes.push(n);
      }
      return nodes;
    }
    // Fallback: scan storage when index is empty
    const scored: Array<{ score: number; node: NodeRecord }> = [];
    for await (const node of this.storage.listNodes()) {
      if (!node.vector || node.vector.length === 0) continue;
      const score = cosineSimilarity(queryVector, node.vector);
      if (Number.isFinite(score)) scored.push({ score, node });
    }
    scored.sort((a, b) => b.score - a.score);
    return scored.slice(0, limit).map((s) => s.node);
  }

  // Type system convenience
  async instancesOf(typeName: string): Promise<NodeRecord[]> {
    const results: NodeRecord[] = [];
    for await (const node of this.storage.listNodes()) {
      if (node.type === typeName) results.push(node);
    }
    return results;
  }

  async getNodeHistory(id: string): Promise<NodeRecord[]> {
    return await this.storage.getNodeHistory(id);
  }

  async restoreNodeVersion(id: string, timestamp: number): Promise<void> {
    const history = await this.getNodeHistory(id);
    const version = history.find(v => v.timestamp === timestamp);
    if (!version) throw new Error(`Version not found for node ${id} at timestamp ${timestamp}`);
    
    // Restore by putting the historical version
    await this.put(id, version.data);
  }

  async setType(id: string, typeName: string): Promise<void> {
    const existing = await this.storage.getNode(id);
    const data: Record<string, unknown> = existing ? existing.data : {};
    data.type = typeName;
    await this.put(id, data);
  }

  // Any-change subscription (internal use for API streaming)
  onAny(callback: (event: { id: string; node: NodeRecord | null }) => void): void {
    this.anyListeners.add(callback);
  }
  offAny(callback: (event: { id: string; node: NodeRecord | null }) => void): void {
    this.anyListeners.delete(callback);
  }

  async *list(): AsyncIterable<NodeRecord> {
    for await (const node of this.storage.listNodes()) {
      yield node;
    }
  }

  // Mesh networking
  serve(options?: ServeOptions): void {
    const port = options?.port ?? 8080;
    if (!this.meshServer) {
      debugLog("serve() starting", { port });
      this.meshServer = startMeshServer({
        port,
        onMessage: ({ msg, source, send, broadcast }) => {
          this.handleInboundMessage(msg as MeshMessage, {
            send,
            broadcast,
            source,
          });
        },
      });
    }
  }

  connect(url: string): void {
    const socket = connectToPeer(url, {
      onOpen: (s) => {
        // Request a snapshot
        try {
          s.send(
            JSON.stringify({ type: "sync_request", originId: this.peerId }),
          );
        } catch { /* ignore */ }
      },
      onMessage: (msg) =>
        this.handleInboundMessage(msg as MeshMessage, {
          send: (obj) => {
            try {
              socket.send(JSON.stringify(obj));
            } catch { /* ignore */ }
          },
          broadcast: (_obj) => {/* do not rebroadcast from clients */},
          source: socket,
        }),
    });
    this.peerSockets.add(socket);
    socket.onclose = () => this.peerSockets.delete(socket);
  }

  async close(): Promise<void> {
    this.closed = true;
    for (const s of this.peerSockets) {
      try {
        s.onmessage = null;
        s.close();
      } catch { /* ignore */ }
    }
    this.peerSockets.clear();
    if (this.meshServer) {
      try {
        this.meshServer.close();
      } catch { /* ignore */ }
      this.meshServer = null;
    }
    await this.storage.close();
  }

  private async handleInboundMessage(
    msg: MeshMessage,
    ctx: {
      send: (obj: unknown) => void;
      broadcast: (obj: unknown, exclude?: WebSocket) => void;
      source: WebSocket;
    },
  ): Promise<void> {
    if (this.closed) return;
    if (!msg || typeof msg !== "object") return;
    const originId = (msg as Partial<{ originId: string }>).originId;
    debugLog("inbound", { type: (msg as { type: string }).type, originId });
    if (originId === this.peerId) return; // ignore our own

    switch (msg.type) {
      case "put": {
        const { node } = msg;
        debugLog("apply put", { id: node.id });
        const existing = await this.storage.getNode(node.id);
        const merged = mergeNodes(existing, node);
        await this.storage.setNode(merged);
        this.emit(node.id, merged);
        if (merged.vector && merged.vector.length > 0) this.vectorIndex.upsert(node.id, merged.vector);
        else this.vectorIndex.remove(node.id);
        await this.evaluateRules(merged);
        // Rebroadcast to other peers if we're acting as server
        try {
          ctx.broadcast(msg, ctx.source);
        } catch { /* ignore */ }
        break;
      }
      case "delete": {
        debugLog("apply delete", { id: msg.id });
        await this.storage.deleteNode(msg.id);
        this.emit(msg.id, null);
        try {
          ctx.broadcast(msg, ctx.source);
        } catch { /* ignore */ }
        break;
      }
      case "sync_request": {
        debugLog("sync_request sending snapshot");
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
      try {
        this.meshServer.broadcast(obj);
      } catch { /* ignore */ }
    }
    // Also forward to directly connected peers (client mode)
    for (const s of this.peerSockets) {
      try {
        s.send(JSON.stringify(obj));
      } catch { /* ignore */ }
    }
  }

  // --- rules ---
  addRule(rule: Rule): void {
    this.rules.addRule(rule);
  }
  removeRule(name: string): void {
    this.rules.removeRule(name);
  }
  private async evaluateRules(node: NodeRecord): Promise<void> {
    const ctx: RuleContext = {
      db: {
        put: (id, data) => this.applyPut(id, data, true),
        get: (id) => this.get(id),
      },
    };
    await this.rules.evaluateNode(node, ctx);
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
    dot += av * bv;
    na += av * av;
    nb += bv * bv;
  }
  const denom = Math.sqrt(na) * Math.sqrt(nb) || 1;
  return dot / denom;
}
