export interface VectorSearchItem {
  id: string;
  data: unknown;
  score: number;
  timestamp: string;
}

export declare class PluresDatabase {
  constructor(actorId?: string | null, dbPath?: string | null);
  put(id: string, data: unknown): string;
  putWithEmbedding(id: string, data: unknown, embedding: number[]): string;
  get(id: string): unknown | null;
  getWithMetadata(id: string): unknown | null;
  delete(id: string): void;
  list(): unknown[];
  listByType(nodeType: string): unknown[];
  query(sql: string, params?: unknown[] | null): unknown;
  exec(sql: string): unknown;
  search(query: string, limit?: number | null): unknown[];
  vectorSearch(embedding: number[], limit?: number | null, threshold?: number | null): VectorSearchItem[];
  subscribe(): string;
  getActorId(): string;
  stats(): unknown;
}

export declare function init(): void;
