import type { NodeRecord } from "../types/index.ts";

/**
 * Deno KV-backed storage layer for PluresDB.
 *
 * Provides simple CRUD operations and history tracking for {@link NodeRecord}
 * objects.  Each write also persists a timestamped snapshot under a `history`
 * prefix, enabling full version history retrieval.
 *
 * Call {@link open} before using any other method, and {@link close} when done.
 */
export class KvStorage {
  private kv: Deno.Kv | null = null;

  /**
   * Open the Deno KV store.
   *
   * @param path - Optional file-system path for the KV store.
   *   If omitted an in-memory (ephemeral) store is used.
   */
  async open(path?: string): Promise<void> {
    this.kv = await Deno.openKv(path);
  }

  /**
   * Close the underlying KV store and release resources.
   *
   * Subsequent calls to storage methods will throw until {@link open} is
   * called again.
   */
  async close(): Promise<void> {
    if (this.kv) {
      try {
        this.kv.close();
      } catch {
        /* ignore */
      }
      this.kv = null;
    }
    // Allow microtasks to flush for callers awaiting close()
    await Promise.resolve();
  }

  private ensureKv(): Deno.Kv {
    if (!this.kv) {
      throw new Error("KvStorage is not opened. Call open() first.");
    }
    return this.kv;
  }

  /**
   * Retrieve a single node by its identifier.
   *
   * @param id - Node identifier to look up.
   * @returns The stored {@link NodeRecord}, or `null` if not found.
   */
  async getNode(id: string): Promise<NodeRecord | null> {
    const kv = this.ensureKv();
    const res = await kv.get<NodeRecord>(["node", id]);
    return res.value ?? null;
  }

  /**
   * Persist a node and append a versioned snapshot to its history.
   *
   * @param node - Node to store.
   */
  async setNode(node: NodeRecord): Promise<void> {
    const kv = this.ensureKv();
    await kv.set(["node", node.id], node);

    // Store version history
    const historyKey = ["history", node.id, node.timestamp];
    await kv.set(historyKey, node);
  }

  /**
   * Delete a node from storage.
   *
   * History entries are **not** removed, so the node's past versions remain
   * accessible via {@link getNodeHistory}.
   *
   * @param id - Identifier of the node to delete.
   */
  async deleteNode(id: string): Promise<void> {
    const kv = this.ensureKv();
    await kv.delete(["node", id]);
  }

  /**
   * Async iterator over every current node in the store.
   *
   * @yields Each stored {@link NodeRecord}.
   */
  async *listNodes(): AsyncIterable<NodeRecord> {
    const kv = this.ensureKv();
    for await (const entry of kv.list<NodeRecord>({ prefix: ["node"] })) {
      if (entry.value) yield entry.value;
    }
  }

  /**
   * Async iterator over all historical snapshots of a node.
   *
   * Snapshots are yielded in storage order (ascending timestamp).
   * Use {@link getNodeHistory} for a sorted array.
   *
   * @param id - Node identifier.
   * @yields Each historical {@link NodeRecord} snapshot.
   */
  async *listNodeHistory(id: string): AsyncIterable<NodeRecord> {
    const kv = this.ensureKv();
    for await (
      const entry of kv.list<NodeRecord>({ prefix: ["history", id] })
    ) {
      if (entry.value) yield entry.value;
    }
  }

  /**
   * Return all historical snapshots of a node, sorted most-recent first.
   *
   * @param id - Node identifier.
   * @returns Array of snapshots in descending timestamp order.
   */
  async getNodeHistory(id: string): Promise<NodeRecord[]> {
    const history: NodeRecord[] = [];
    for await (const version of this.listNodeHistory(id)) {
      history.push(version);
    }
    return history.sort((a, b) => b.timestamp - a.timestamp); // Most recent first
  }
}
