export interface VectorIndexResult {
  id: string;
  score: number;
}

export interface VectorIndex {
  upsert(id: string, vector: number[]): void;
  remove(id: string): void;
  search(vector: number[], k: number): VectorIndexResult[];
  clear(): void;
}

export class BruteForceVectorIndex implements VectorIndex {
  private readonly idToVector = new Map<string, Float32Array>();

  upsert(id: string, vector: number[]): void {
    const normed = normalizeVector(vector);
    this.idToVector.set(id, normed);
  }

  remove(id: string): void {
    this.idToVector.delete(id);
  }

  clear(): void {
    this.idToVector.clear();
  }

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
