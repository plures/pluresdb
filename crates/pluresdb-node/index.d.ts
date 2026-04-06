/* Auto-generated TypeScript declarations for pluresdb-node NAPI bindings. */

export interface AgensEventJson {
  event_type: 'message' | 'timer' | 'state_change' | 'model_response' | 'tool_result' | 'praxis_analysis_ready' | 'praxis_analysis_failed' | 'praxis_guidance_updated';
  id: string;
  [key: string]: unknown;
}

export interface AgensTimerEntry {
  id: string;
  name: string;
  intervalSecs: number;
  nextFireAt: string;
  payload: unknown;
}

export interface AgensStateEntry {
  key: string;
  value: unknown;
}

export class PluresDatabase {
  constructor(actorId?: string, dbPath?: string);
  static newWithEmbeddings(model: string, actorId?: string, dbPath?: string): PluresDatabase;

  // CRUD
  put(id: string, data: unknown): string;
  get(id: string): unknown | null;
  getWithMetadata(id: string): unknown | null;
  delete(id: string): void;
  list(): Array<{ id: string; data: unknown; timestamp: string }>;
  listByType(nodeType: string): Array<{ id: string; data: unknown; timestamp: string }>;

  // Query
  query(sql: string, params?: unknown[]): unknown;
  exec(sql: string): unknown;
  search(query: string, limit?: number): Array<{ id: string; data: unknown; score: number; timestamp: string }>;
  vectorSearch(embedding: number[], limit?: number, threshold?: number): Array<{ id: string; data: unknown; score: number; timestamp: string }>;
  putWithEmbedding(id: string, data: unknown, embedding: number[]): string;
  embed(texts: string[]): number[][];
  embeddingDimension(): number | null;

  // Procedures
  execDsl(query: string): unknown;
  execIr(steps: unknown): unknown;

  // Utilities
  subscribe(): string;
  getActorId(): string;
  buildVectorIndex(): number;
  stats(): { totalNodes: number; typeCounts: Record<string, number> };

  // Agens Runtime — reactive event system
  agensEmit(event: AgensEventJson): string;
  agensEmitPraxis(event: AgensEventJson): string;
  agensListEvents(sinceIso: string): AgensEventJson[];
  agensStateGet(key: string): unknown;
  agensStateSet(key: string, value: unknown): void;
  agensStateWatch(sinceIso: string): AgensStateEntry[];
  agensTimerSchedule(name: string, intervalSecs: number, payload: unknown): string;
  agensTimerCancel(timerId: string): boolean;
  agensTimerList(): AgensTimerEntry[];
  agensTimerDue(): AgensTimerEntry[];
  agensTimerReschedule(timerId: string): boolean;
}

export function init(): void;
