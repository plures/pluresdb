export interface QueryResult {
  columns: string[];
  rows: Array<Record<string, any>>;
  changes: number;
  lastInsertRowid: number;
}

export interface ExecutionResult {
  changes: number;
  lastInsertRowid: number;
}

export interface NodeWithMetadata {
  id: string;
  data: any;
  clock: Record<string, number>;
  timestamp: string;
}

export interface SearchResult {
  id: string;
  data: any;
  score: number;
  timestamp: string;
}

export interface DatabaseStats {
  totalNodes: number;
  typeCounts: Record<string, number>;
}

export class PluresDatabase {
  constructor(actorId?: string, dbPath?: string);
  put(id: string, data: any): string;
  get(id: string): any | null;
  getWithMetadata(id: string): NodeWithMetadata | null;
  delete(id: string): void;
  list(): Array<{ id: string; data: any; timestamp: string }>;
  listByType(nodeType: string): Array<{ id: string; data: any; timestamp: string }>;
  query(sql: string, params?: any[]): QueryResult;
  exec(sql: string): ExecutionResult;
  search(query: string, limit?: number): SearchResult[];
  vectorSearch(query: string, limit?: number, threshold?: number): SearchResult[];
  subscribe(): string;
  getActorId(): string;
  stats(): DatabaseStats;
}

export function init(): void;

