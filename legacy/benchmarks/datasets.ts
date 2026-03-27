/**
 * Shared dataset definitions for PluresDB benchmark suites.
 *
 * Provides small / medium / large pre-built payloads so every benchmark
 * uses consistent, reproducible test data.
 */

export interface UserRecord {
  id: number;
  name: string;
  email: string;
  age: number;
  bio: string;
  createdAt: string;
  tags: string[];
  metadata: {
    role: string;
    score: number;
    active: boolean;
  };
}

export interface ProductRecord {
  id: number;
  sku: string;
  name: string;
  description: string;
  price: number;
  category: string;
  stock: number;
  attributes: Record<string, string | number | boolean>;
}

export interface VectorDocument {
  id: number;
  title: string;
  body: string;
  /** Pre-generated 128-dim float vector for benchmarks that bypass the embedder. */
  vector: number[];
  tags: string[];
}

// ---------------------------------------------------------------------------
// Factories
// ---------------------------------------------------------------------------

function makeUser(i: number): UserRecord {
  return {
    id: i,
    name: `User ${i}`,
    email: `user${i}@example.com`,
    age: 20 + (i % 50),
    bio: `Biography for user number ${i}. They enjoy contributing to open-source projects.`,
    createdAt: new Date(Date.now() - i * 86_400_000).toISOString(),
    tags: [`tag-${i % 5}`, `tag-${i % 7}`, `role-${i % 3}`],
    metadata: {
      role: ["admin", "editor", "viewer"][i % 3],
      score: Math.round((i * 13.7) % 100),
      active: i % 4 !== 0,
    },
  };
}

function makeProduct(i: number): ProductRecord {
  const categories = ["electronics", "books", "clothing", "food", "sports"];
  return {
    id: i,
    sku: `SKU-${i.toString().padStart(6, "0")}`,
    name: `Product ${i}`,
    description: `Detailed description for product ${i}. High quality item.`,
    price: parseFloat(((i % 100) * 9.99 + 0.99).toFixed(2)),
    category: categories[i % categories.length],
    stock: (i * 7) % 500,
    attributes: {
      weight: `${(i % 10) + 0.5}kg`,
      color: ["red", "blue", "green", "black", "white"][i % 5],
      inStock: i % 3 !== 0,
    },
  };
}

/** Generate a deterministic 128-dim unit vector for a given index. */
function makeVector(i: number): number[] {
  const vec: number[] = [];
  let mag = 0;
  for (let j = 0; j < 128; j++) {
    const v = Math.sin(i * 0.1 + j * 0.37);
    vec.push(v);
    mag += v * v;
  }
  const norm = Math.sqrt(mag) || 1;
  return vec.map((v) => v / norm);
}

function makeVectorDocument(i: number): VectorDocument {
  const topics = [
    "machine learning and artificial intelligence",
    "distributed systems and consensus protocols",
    "graph databases and CRDT data structures",
    "vector search and similarity algorithms",
    "local-first software and offline-first design",
  ];
  return {
    id: i,
    title: `Document ${i}: ${topics[i % topics.length]}`,
    body: `This document covers ${topics[i % topics.length]}. ` +
      `It includes practical examples, theoretical background, and performance considerations. ` +
      `Document index: ${i}.`,
    vector: makeVector(i),
    tags: [topics[i % topics.length].split(" ")[0], `doc-${i % 10}`],
  };
}

// ---------------------------------------------------------------------------
// Dataset sizes
// ---------------------------------------------------------------------------

export const DATASET_SIZES = {
  /** 100 records — fast smoke-test level. */
  small: 100,
  /** 1 000 records — typical unit-benchmark level. */
  medium: 1_000,
  /** 10 000 records — stress / regression level. */
  large: 10_000,
} as const;

export type DatasetSize = keyof typeof DATASET_SIZES;

// ---------------------------------------------------------------------------
// Dataset generators (lazy, to avoid allocating everything up-front)
// ---------------------------------------------------------------------------

export function generateUsers(count: number): UserRecord[] {
  return Array.from({ length: count }, (_, i) => makeUser(i));
}

export function generateProducts(count: number): ProductRecord[] {
  return Array.from({ length: count }, (_, i) => makeProduct(i));
}

export function generateVectorDocuments(count: number): VectorDocument[] {
  return Array.from({ length: count }, (_, i) => makeVectorDocument(i));
}

// Pre-built named datasets for convenience
export const DATASETS = {
  small: {
    users: generateUsers(DATASET_SIZES.small),
    products: generateProducts(DATASET_SIZES.small),
    vectorDocs: generateVectorDocuments(DATASET_SIZES.small),
  },
  medium: {
    users: generateUsers(DATASET_SIZES.medium),
    products: generateProducts(DATASET_SIZES.medium),
    vectorDocs: generateVectorDocuments(DATASET_SIZES.medium),
  },
  large: {
    users: generateUsers(DATASET_SIZES.large),
    products: generateProducts(DATASET_SIZES.large),
    vectorDocs: generateVectorDocuments(DATASET_SIZES.large),
  },
} as const;
