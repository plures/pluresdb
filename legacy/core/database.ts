import { KvStorage } from "../storage/kv-storage.ts";
import type { MeshMessage, NodeRecord } from "../types/index.ts";
import { mergeNodes } from "./crdt.ts";
import {
  connectToPeer,
  type MeshServer,
  startMeshServer,
} from "../network/websocket-server.ts";
import { debugLog } from "../util/debug.ts";
import { type Rule, type RuleContext, RuleEngine } from "../logic/rules.ts";
import { BruteForceVectorIndex } from "../vector/index.ts";
import {
  HyperswarmSync,
  generateSyncKey,
  type SyncKeyOptions,
  type PeerInfo,
  type SyncStats,
} from "../network/hyperswarm-sync.ts";

const FUNCTION_PLACEHOLDER = "[sanitized function]";

function isPlainObject(value: unknown): value is Record<string, unknown> {
  if (value === null || typeof value !== "object") return false;
  const proto = Object.getPrototypeOf(value);
  return proto === Object.prototype || proto === null;
}

function sanitizeValue(value: unknown, seen: WeakSet<object>): unknown {
  if (typeof value === "function") return FUNCTION_PLACEHOLDER;
  if (value === null || typeof value !== "object") return value;
  if (seen.has(value as object)) return "[circular]";
  if (Array.isArray(value)) {
    seen.add(value);
    return value.map((item) => sanitizeValue(item, seen));
  }
  if (!isPlainObject(value)) return value;
  seen.add(value as object);
  const clean: Record<string, unknown> = Object.create(null);
  for (const [key, entry] of Object.entries(value as Record<string, unknown>)) {
    if (key === "__proto__" || key === "constructor") continue;
    clean[key] = sanitizeValue(entry, seen);
  }
  return clean;
}

function sanitizeRecord(
  data: Record<string, unknown>,
): Record<string, unknown> {
  const result = sanitizeValue(data, new WeakSet()) as
    | Record<string, unknown>
    | string;
  if (typeof result === "string" || result === undefined) {
    return Object.create(null);
  }
  return result;
}

function sanitizeForOutput(
  data: Record<string, unknown>,
): Record<string, unknown> {
  const clean = sanitizeRecord(data);
  if (typeof clean["toString"] !== "string") {
    clean["toString"] = Object.prototype.toString.call(clean);
  }
  return clean;
}

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
  private readonly anyListeners: Set<
    (event: { id: string; node: NodeRecord | null }) => void
  > = new Set();
  private readonly peerId: string;
  private meshServer: MeshServer | null = null;
  private readonly peerSockets: Set<WebSocket> = new Set();
  private closed = false;
  private readyState = false;
  private readonly rules = new RuleEngine();
  private readonly vectorIndex = new BruteForceVectorIndex();
  private hyperswarmSync: HyperswarmSync | null = null;

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
    this.closed = false;
    this.readyState = true;
  }

  // Basic CRUD
  async put(id: string, data: Record<string, unknown>): Promise<void> {
    this.ensureReady();
    await this.applyPut(id, data, false);
  }

  private async applyPut(
    id: string,
    data: Record<string, unknown>,
    suppressRules: boolean,
  ): Promise<void> {
    if (this.closed) return;
    debugLog("put()", { id, keys: Object.keys(data ?? {}) });
    const existing = await this.storage.getNode(id);
    const now = Date.now();
    const existingClock = existing?.vectorClock ?? {};
    const newClock = {
      ...existingClock,
      [this.peerId]: (existingClock[this.peerId] ?? 0) + 1,
    };

    const sanitizedData = sanitizeRecord(data ?? {});
    let vector: number[] | undefined = undefined;
    const record = sanitizedData as Record<string, unknown>;
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
    for (const key of Object.keys(record ?? {})) newState[key] = now;

    const updated: NodeRecord = {
      id,
      data: record,
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
    if (merged.vector && merged.vector.length > 0) {
      this.vectorIndex.upsert(id, merged.vector);
    } else this.vectorIndex.remove(id);
    if (!suppressRules) {
      await this.evaluateRules(merged);
    }
    this.broadcast({ type: "put", originId: this.peerId, node: merged });
  }

  async get<T = Record<string, unknown>>(
    id: string,
  ): Promise<(T & { id: string }) | null> {
    this.ensureReady();
    const node = await this.storage.getNode(id);
    if (!node) return null;
    const sanitized = sanitizeForOutput(
      (node.data ?? {}) as Record<string, unknown>,
    );
    return { id: node.id, ...(sanitized as T) };
  }

  async delete(id: string): Promise<void> {
    this.ensureReady();
    if (this.closed) return;
    debugLog("delete()", { id });
    await this.storage.deleteNode(id);
    this.emit(id, null);
    this.vectorIndex.remove(id);
    this.broadcast({ type: "delete", originId: this.peerId, id });
  }

  // Subscriptions
  on(id: string, callback: (node: NodeRecord | null) => void): () => void {
    this.ensureReady();
    const set = this.listeners.get(id) ?? new Set();
    set.add(callback);
    this.listeners.set(id, set);
    return () => this.off(id, callback);
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
  ): Promise<Array<NodeRecord & { similarity?: number }>> {
    this.ensureReady();
    const queryVector = Array.isArray(query) ? query : embedTextToVector(query);
    const results = this.vectorIndex.search(queryVector, limit);
    if (results.length > 0) {
      const nodes: Array<NodeRecord & { similarity?: number }> = [];
      for (const r of results) {
        const n = await this.storage.getNode(r.id);
        if (n) nodes.push({ ...n, similarity: r.score });
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
    return scored.slice(0, limit).map((s) => ({
      ...s.node,
      similarity: s.score,
    }));
  }

  // Type system convenience
  async instancesOf(typeName: string): Promise<NodeRecord[]> {
    this.ensureReady();
    const results: NodeRecord[] = [];
    for await (const node of this.storage.listNodes()) {
      if (node.type === typeName) results.push(node);
    }
    return results;
  }

  async getNodeHistory(id: string): Promise<NodeRecord[]> {
    this.ensureReady();
    return await this.storage.getNodeHistory(id);
  }

  async restoreNodeVersion(id: string, timestamp: number): Promise<void> {
    this.ensureReady();
    const history = await this.getNodeHistory(id);
    const version = history.find((v) => v.timestamp === timestamp);
    if (!version) {
      throw new Error(
        `Version not found for node ${id} at timestamp ${timestamp}`,
      );
    }

    // Restore by putting the historical version
    await this.put(id, version.data);
  }

  async setType(id: string, typeName: string): Promise<void> {
    this.ensureReady();
    const existing = await this.storage.getNode(id);
    const data: Record<string, unknown> = existing ? existing.data : {};
    data.type = typeName;
    await this.put(id, data);
  }

  // Any-change subscription (internal use for API streaming)
  onAny(
    callback: (event: { id: string; node: NodeRecord | null }) => void,
  ): () => void {
    this.ensureReady();
    this.anyListeners.add(callback);
    return () => this.offAny(callback);
  }
  offAny(
    callback: (event: { id: string; node: NodeRecord | null }) => void,
  ): void {
    this.anyListeners.delete(callback);
  }

  async *list(): AsyncIterable<NodeRecord> {
    this.ensureReady();
    for await (const node of this.storage.listNodes()) {
      yield node;
    }
  }

  async getAll(): Promise<NodeRecord[]> {
    this.ensureReady();
    const out: NodeRecord[] = [];
    for await (const node of this.storage.listNodes()) out.push(node);
    return out;
  }

  // Mesh networking
  serve(options?: ServeOptions): void {
    this.ensureReady();
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
    this.ensureReady();
    const socket = connectToPeer(url, {
      onOpen: (s) => {
        // Request a snapshot
        try {
          s.send(
            JSON.stringify({ type: "sync_request", originId: this.peerId }),
          );
        } catch {
          /* ignore */
        }
      },
      onMessage: (msg) =>
        this.handleInboundMessage(msg as MeshMessage, {
          send: (obj) => {
            try {
              socket.send(JSON.stringify(obj));
            } catch {
              /* ignore */
            }
          },
          broadcast: (_obj) => {
            /* do not rebroadcast from clients */
          },
          source: socket,
        }),
    });
    this.peerSockets.add(socket);
    socket.onclose = () => this.peerSockets.delete(socket);
  }

  async close(): Promise<void> {
    this.closed = true;
    this.readyState = false;
    for (const s of this.peerSockets) {
      try {
        s.onmessage = null;
        s.close();
      } catch {
        /* ignore */
      }
    }
    this.peerSockets.clear();
    if (this.meshServer) {
      try {
        this.meshServer.close();
      } catch {
        /* ignore */
      }
      this.meshServer = null;
    }
    // Close Hyperswarm sync if enabled
    if (this.hyperswarmSync) {
      try {
        await this.disableSync();
      } catch {
        /* ignore */
      }
    }
    await this.storage.close();
  }

  // --- P2P Sync via Hyperswarm ---

  /**
   * Generate a new sync key for P2P synchronization
   * @returns 32-byte hex string
   */
  static generateSyncKey(): string {
    return generateSyncKey();
  }

  /**
   * Enable P2P sync via Hyperswarm (DHT discovery + NAT traversal)
   * @param options Sync configuration with key
   */
  async enableSync(options: SyncKeyOptions): Promise<void> {
    this.ensureReady();

    if (this.hyperswarmSync?.isEnabled()) {
      throw new Error("Sync already enabled. Call disableSync() first.");
    }

    debugLog("enableSync", { keyProvided: !!options.key });

    // Create HyperswarmSync instance if not exists
    if (!this.hyperswarmSync) {
      this.hyperswarmSync = new HyperswarmSync({
        onPeerConnected: (info: PeerInfo) => {
          debugLog("peer:connected", { peerId: info.peerId.slice(0, 16) });
          this.emit("peer:connected", info);
        },
        onPeerDisconnected: (info: PeerInfo) => {
          debugLog("peer:disconnected", { peerId: info.peerId.slice(0, 16) });
          this.emit("peer:disconnected", info);
        },
        onSyncComplete: (stats: SyncStats) => {
          debugLog("sync:complete", stats);
          this.emit("sync:complete", stats);
        },
        onMessage: async ({ msg, peerId, send }) => {
          await this.handleInboundMessage(msg, {
            send,
            broadcast: (obj) => {
              // Broadcast to all Hyperswarm peers except the sender
              this.hyperswarmSync?.broadcast(obj, peerId);
            },
            source: null as any, // Not needed for Hyperswarm
          });
        },
      });
    }

    // Enable sync with the provided key
    await this.hyperswarmSync.enableSync(options);

    // Request sync from all peers
    this.hyperswarmSync.broadcast({
      type: "sync_request",
      originId: this.peerId,
    });
  }

  /**
   * Disable P2P sync and disconnect from all peers
   */
  async disableSync(): Promise<void> {
    if (this.hyperswarmSync) {
      await this.hyperswarmSync.disableSync();
    }
  }

  /**
   * Get sync statistics (peer count, messages, bandwidth)
   */
  getSyncStats(): SyncStats | null {
    return this.hyperswarmSync?.getStats() || null;
  }

  /**
   * Get list of connected P2P peers
   */
  getSyncPeers(): PeerInfo[] {
    return this.hyperswarmSync?.getPeers() || [];
  }

  /**
   * Check if P2P sync is enabled
   */
  isSyncEnabled(): boolean {
    return this.hyperswarmSync?.isEnabled() || false;
  }

  /**
   * Get the current sync key (if sync is enabled)
   */
  getSyncKey(): string | null {
    return this.hyperswarmSync?.getSyncKey() || null;
  }

  // ---

  private ensureReady(): void {
    if (!this.readyState || this.closed) {
      throw new Error("Database not ready");
    }
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
        const compatPayload = msg as Partial<{
          id: string;
          data: Record<string, unknown>;
        }>;
        if (!("node" in msg) && compatPayload.id && compatPayload.data) {
          debugLog("apply put (compat)", { id: compatPayload.id });
          await this.put(compatPayload.id, compatPayload.data);
          break;
        }
        const { node } = msg;
        debugLog("apply put", { id: node.id });
        const existing = await this.storage.getNode(node.id);
        const merged = mergeNodes(existing, node);
        await this.storage.setNode(merged);
        this.emit(node.id, merged);
        if (merged.vector && merged.vector.length > 0) {
          this.vectorIndex.upsert(node.id, merged.vector);
        } else this.vectorIndex.remove(node.id);
        await this.evaluateRules(merged);
        try {
          ctx.broadcast(msg, ctx.source);
        } catch {
          /* ignore */
        }
        break;
      }
      case "delete": {
        debugLog("apply delete", { id: msg.id });
        await this.storage.deleteNode(msg.id);
        this.emit(msg.id, null);
        try {
          ctx.broadcast(msg, ctx.source);
        } catch {
          /* ignore */
        }
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
      } catch {
        /* ignore */
      }
    }
    // Also forward to directly connected peers (client mode)
    for (const s of this.peerSockets) {
      try {
        s.send(JSON.stringify(obj));
      } catch {
        /* ignore */
      }
    }
    // Broadcast to Hyperswarm peers if sync is enabled
    if (this.hyperswarmSync?.isEnabled()) {
      try {
        this.hyperswarmSync.broadcast(obj);
      } catch {
        /* ignore */
      }
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
  let dot = 0,
    na = 0,
    nb = 0;
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
