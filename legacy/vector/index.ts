/**
 * A single result returned by a vector index search.
 */
export interface VectorIndexResult {
  /** Identifier of the matching node. */
  id: string;
  /** Cosine similarity score in the range `[-1, 1]`; higher is more similar. */
  score: number;
}

/**
 * Generic interface for a vector index supporting upsert, removal, and k-NN search.
 */
export interface VectorIndex {
  /**
   * Insert or update the embedding vector for a node.
   *
   * @param id     - Node identifier.
   * @param vector - Raw (unnormalized) embedding vector.
   */
  upsert(id: string, vector: number[]): void;
  /**
   * Remove a node's vector from the index.
   *
   * @param id - Node identifier to remove.
   */
  remove(id: string): void;
  /**
   * Find the `k` most similar vectors to `vector` using cosine similarity.
   *
   * @param vector - Query embedding vector.
   * @param k      - Maximum number of results to return.
   * @returns Array of results sorted by descending similarity.
   */
  search(vector: number[], k: number): VectorIndexResult[];
  /** Remove all vectors from the index. */
  clear(): void;
}

/**
 * In-memory brute-force vector index using cosine similarity.
 *
 * All stored vectors are L2-normalised on insertion.  Search computes the dot
 * product of the normalised query vector against every stored vector (which is
 * equivalent to cosine similarity for unit vectors) and returns the top `k`
 * matches.
 *
 * Suitable for datasets up to tens of thousands of nodes.  For larger datasets
 * consider replacing with an HNSW-based index.
 */
export class BruteForceVectorIndex implements VectorIndex {
  private readonly idToVector = new Map<string, Float32Array>();

  /**
   * Insert or update the embedding vector for a node.
   *
   * The vector is L2-normalised before storage.
   *
   * @param id     - Node identifier.
   * @param vector - Raw embedding vector (will be normalised).
   */
  upsert(id: string, vector: number[]): void {
    const normed = normalizeVector(vector);
    this.idToVector.set(id, normed);
  }

  /**
   * Remove a node's vector from the index.
   *
   * @param id - Node identifier to remove.
   */
  remove(id: string): void {
    this.idToVector.delete(id);
  }

  /** Remove all vectors from the index. */
  clear(): void {
    this.idToVector.clear();
  }

  /**
   * Return the `k` nearest neighbours to `vector` by cosine similarity.
   *
   * The query vector is normalised before comparison.  Results are sorted by
   * descending score.
   *
   * @param vector - Query embedding vector.
   * @param k      - Maximum number of results to return.
   * @returns Array of {@link VectorIndexResult} sorted by descending score.
   */
  search(vector: number[], k: number): VectorIndexResult[] {
    const q = normalizeVector(vector);
    const results: VectorIndexResult[] = [];
    for (const [id, v] of this.idToVector) {
      const score = cosine(q, v);
      if (Number.isFinite(score)) results.push({ id, score });
    }
    results.sort((a, b) => b.score - a.score);
    return results.slice(0, k);
  }
}

function normalizeVector(vector: number[]): Float32Array {
  const arr = new Float32Array(vector.length);
  let n = 0;
  for (let i = 0; i < vector.length; i++) {
    const v = vector[i] ?? 0;
    arr[i] = v;
    n += v * v;
  }
  const norm = Math.sqrt(n) || 1;
  for (let i = 0; i < arr.length; i++) arr[i] /= norm;
  return arr;
}

function cosine(a: Float32Array, b: Float32Array): number {
  const len = Math.min(a.length, b.length);
  let dot = 0;
  for (let i = 0; i < len; i++) dot += a[i] * b[i];
  return dot;
}
