/** Svelte 5 runes-based shared application state. Replaces legacy writable/derived stores. */

export type NodeItem = { id: string; data: Record<string, unknown> };

export type AppSettings = {
  kvPath?: string;
  port?: number;
  apiPortOffset?: number;
  peers?: string[];
  dark?: boolean;
};

class DbState {
  nodes = $state<Record<string, NodeItem>>({});
  selectedId = $state<string | null>(null);
  settings = $state<AppSettings>({});

  readonly selected = $derived.by((): NodeItem | null =>
    this.selectedId ? (this.nodes[this.selectedId] ?? null) : null,
  );

  upsertNode(item: NodeItem): void {
    this.nodes[item.id] = item;
  }

  removeNode(id: string): void {
    delete this.nodes[id];
  }

  setAll(items: NodeItem[]): void {
    const map: Record<string, NodeItem> = {};
    for (const it of items) map[it.id] = it;
    this.nodes = map;
  }
}

/** Singleton reactive state for the entire PluresDB UI. */
export const db = new DbState();
