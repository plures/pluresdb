/**
 * PluresDB WASM bindings.
 * For use in browsers and Deno.
 */

export { NodeId, ActorId, VectorClock, NodeRecord, VectorSearchResult, PluresDBOptions, CrdtOperation } from './index';

/** Initialize the WASM module. Must be called before creating any database instances. */
export default function init(): Promise<void>;

/** PluresDB browser instance backed by IndexedDB for persistence. */
export declare class PluresDBBrowser {
  constructor(dbName: string, actorId?: string);
  
  put(id: string, data: unknown): string;
  putWithEmbedding(id: string, data: unknown, embedding: number[]): string;
  get(id: string): NodeRecord | null;
  delete(id: string): void;
  list(): NodeRecord[];
  vectorSearch(queryEmbedding: number[], limit: number, minScore?: number): VectorSearchResult[];
}
