import type { NodeRecord } from "../types/index.ts";

export class KvStorage {
  private kv: Deno.Kv | null = null;

  async open(path?: string): Promise<void> {
    this.kv = await Deno.openKv(path);
  }

  async close(): Promise<void> {
    if (this.kv) {
      try {
        this.kv.close();
      } catch { /* ignore */ }
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

  async getNode(id: string): Promise<NodeRecord | null> {
    const kv = this.ensureKv();
    const res = await kv.get<NodeRecord>(["node", id]);
    return res.value ?? null;
  }

  async setNode(node: NodeRecord): Promise<void> {
    const kv = this.ensureKv();
    await kv.set(["node", node.id], node);
  }

  async deleteNode(id: string): Promise<void> {
    const kv = this.ensureKv();
    await kv.delete(["node", id]);
  }

  async *listNodes(): AsyncIterable<NodeRecord> {
    const kv = this.ensureKv();
    for await (const entry of kv.list<NodeRecord>({ prefix: ["node"] })) {
      if (entry.value) yield entry.value;
    }
  }
}
