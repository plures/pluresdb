/**
 * Unified Local-First API for PluresDB
 * 
 * Automatically selects the best integration method based on runtime environment:
 * - Browser (WASM): Direct in-process WebAssembly
 * - Tauri: Direct Rust crate linking via Tauri commands
 * - Native IPC: Shared memory inter-process communication
 * - Network: HTTP REST API (fallback for backward compatibility)
 * 
 * This provides a consistent API across all platforms while maximizing performance
 * and minimizing network overhead.
 */

import { debugLog } from "../util/debug.ts";

export interface LocalFirstOptions {
  /**
   * Integration mode. If not specified, auto-detects the best mode.
   * - "auto": Auto-detect runtime environment
   * - "wasm": WebAssembly (browser)
   * - "tauri": Tauri native integration
   * - "ipc": Shared memory IPC
   * - "network": HTTP REST API (backward compatibility)
   */
  mode?: "auto" | "wasm" | "tauri" | "ipc" | "network";

  /**
   * Database name (for WASM mode)
   */
  dbName?: string;

  /**
   * IPC channel name (for IPC mode)
   */
  channelName?: string;

  /**
   * Network URL (for network mode)
   */
  networkUrl?: string;

  /**
   * Network port (for network mode)
   */
  port?: number;
}

export interface LocalFirstBackend {
  put(id: string, data: unknown): Promise<string>;
  get(id: string): Promise<unknown>;
  delete(id: string): Promise<void>;
  list(): Promise<unknown[]>;
  vectorSearch?(query: string, limit: number): Promise<unknown[]>;
  close?(): Promise<void>;
}

/**
 * Runtime environment detection
 */
class RuntimeDetector {
  static isBrowser(): boolean {
    return typeof globalThis !== "undefined" &&
      typeof (globalThis as any).window !== "undefined" &&
      typeof (globalThis as any).document !== "undefined" &&
      typeof (globalThis as any).WebAssembly !== "undefined";
  }

  static isTauri(): boolean {
    return typeof globalThis !== "undefined" &&
      typeof (globalThis as any).window !== "undefined" &&
      (globalThis as any).window?.__TAURI__ !== undefined;
  }

  static isNode(): boolean {
    return typeof process !== "undefined" &&
      process.versions != null &&
      process.versions.node != null;
  }

  static isDeno(): boolean {
    return typeof globalThis !== "undefined" &&
      (globalThis as any).Deno !== undefined;
  }

  static hasIPCEnvironment(): boolean {
    if (this.isNode()) {
      return process.env.PLURESDB_IPC === "true";
    }
    if (this.isDeno()) {
      const Deno = (globalThis as any).Deno;
      return Deno?.env?.get?.("PLURESDB_IPC") === "true";
    }
    return false;
  }

  static detectBestMode(): "wasm" | "tauri" | "ipc" | "network" {
    if (this.isTauri()) {
      debugLog("Detected Tauri environment - using native integration");
      return "tauri";
    }

    if (this.isBrowser()) {
      debugLog("Detected browser environment - using WASM");
      return "wasm";
    }

    if (this.hasIPCEnvironment()) {
      debugLog("Detected IPC environment - using shared memory");
      return "ipc";
    }

    debugLog("Using network mode (fallback)");
    return "network";
  }
}

/**
 * WASM Backend (Browser)
 * 
 * Uses WebAssembly for direct in-process database access.
 * Data is persisted in IndexedDB.
 */
class WasmBackend implements LocalFirstBackend {
  private db: unknown = null;
  private dbName: string;

  constructor(dbName: string = "pluresdb") {
    this.dbName = dbName;
  }

  async initialize(): Promise<void> {
    // Note: This is a placeholder. The actual WASM module will be implemented
    // in Phase 1 of the roadmap (pluresdb-wasm crate).
    throw new Error(
      "WASM backend not yet implemented. Please see docs/LOCAL_FIRST_INTEGRATION.md for implementation status.",
    );
  }

  async put(id: string, data: unknown): Promise<string> {
    if (!this.db) await this.initialize();
    return (this.db as any).put(id, data);
  }

  async get(id: string): Promise<unknown> {
    if (!this.db) await this.initialize();
    return (this.db as any).get(id);
  }

  async delete(id: string): Promise<void> {
    if (!this.db) await this.initialize();
    return (this.db as any).delete(id);
  }

  async list(): Promise<unknown[]> {
    if (!this.db) await this.initialize();
    return (this.db as any).list();
  }

  async vectorSearch(query: string, limit: number): Promise<unknown[]> {
    if (!this.db) await this.initialize();
    return (this.db as any).vectorSearch(query, limit);
  }

  async close(): Promise<void> {
    if (this.db) {
      await (this.db as any).close();
      this.db = null;
    }
  }
}

/**
 * Tauri Backend
 * 
 * Uses Tauri commands to invoke Rust functions directly.
 * Provides native performance with no network overhead.
 */
class TauriBackend implements LocalFirstBackend {
  private invoke: any;

  constructor() {
    const win = typeof globalThis !== "undefined" ? (globalThis as any).window : undefined;
    if (!win || !win.__TAURI__) {
      throw new Error("Tauri backend requires Tauri environment");
    }
    this.invoke = win.__TAURI__.invoke;
  }

  async put(id: string, data: unknown): Promise<string> {
    return await this.invoke("pluresdb_put", { id, data });
  }

  async get(id: string): Promise<unknown> {
    return await this.invoke("pluresdb_get", { id });
  }

  async delete(id: string): Promise<void> {
    await this.invoke("pluresdb_delete", { id });
  }

  async list(): Promise<unknown[]> {
    return await this.invoke("pluresdb_list");
  }

  async vectorSearch(query: string, limit: number): Promise<unknown[]> {
    return await this.invoke("pluresdb_vector_search", { query, limit });
  }
}

/**
 * IPC Backend (Native Apps)
 * 
 * Uses shared memory and message passing for inter-process communication.
 * Provides low-latency access without network overhead.
 */
class IPCBackend implements LocalFirstBackend {
  private channelName: string;

  constructor(channelName: string = "pluresdb") {
    this.channelName = channelName;
  }

  async initialize(): Promise<void> {
    // Note: This is a placeholder. The actual IPC implementation will be
    // in Phase 3 of the roadmap (pluresdb-ipc crate).
    throw new Error(
      "IPC backend not yet implemented. Please see docs/LOCAL_FIRST_INTEGRATION.md for implementation status.",
    );
  }

  async put(id: string, data: unknown): Promise<string> {
    throw new Error("IPC backend not yet implemented");
  }

  async get(id: string): Promise<unknown> {
    throw new Error("IPC backend not yet implemented");
  }

  async delete(id: string): Promise<void> {
    throw new Error("IPC backend not yet implemented");
  }

  async list(): Promise<unknown[]> {
    throw new Error("IPC backend not yet implemented");
  }

  async vectorSearch(query: string, limit: number): Promise<unknown[]> {
    throw new Error("IPC backend not yet implemented");
  }
}

/**
 * Network Backend (HTTP REST API)
 * 
 * Uses HTTP requests to communicate with a PluresDB server.
 * This is the fallback mode for backward compatibility.
 */
class NetworkBackend implements LocalFirstBackend {
  private baseUrl: string;

  constructor(url?: string, port?: number) {
    this.baseUrl = url || `http://localhost:${port || 34567}`;
  }

  async put(id: string, data: unknown): Promise<string> {
    const response = await fetch(`${this.baseUrl}/api/put`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ id, data }),
    });

    if (!response.ok) {
      throw new Error(`PUT failed: ${response.statusText}`);
    }

    // API returns { ok: true }, return the id
    await response.json();
    return id;
  }

  async get(id: string): Promise<unknown> {
    const response = await fetch(`${this.baseUrl}/api/get?id=${encodeURIComponent(id)}`);

    if (!response.ok) {
      if (response.status === 404) return null;
      throw new Error(`GET failed: ${response.statusText}`);
    }

    // API returns the node data directly (can be null)
    return await response.json();
  }

  async delete(id: string): Promise<void> {
    const response = await fetch(`${this.baseUrl}/api/delete?id=${encodeURIComponent(id)}`, {
      method: "DELETE",
    });

    if (!response.ok) {
      throw new Error(`DELETE failed: ${response.statusText}`);
    }
  }

  async list(): Promise<unknown[]> {
    const response = await fetch(`${this.baseUrl}/api/list`);

    if (!response.ok) {
      throw new Error(`LIST failed: ${response.statusText}`);
    }

    // API returns array of { id, data }
    const nodes: Array<{ id: string; data: unknown }> = await response.json();
    return nodes;
  }

  async vectorSearch(query: string, limit: number): Promise<unknown[]> {
    const response = await fetch(`${this.baseUrl}/api/search`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ query, limit }),
    });

    if (!response.ok) {
      throw new Error(`VECTOR_SEARCH failed: ${response.statusText}`);
    }

    // API returns array of { id, data }
    const nodes: Array<{ id: string; data: unknown }> = await response.json();
    return nodes;
  }
}

/**
 * PluresDB Local-First API
 * 
 * Unified interface that works across all platforms:
 * - Browser (WASM)
 * - Tauri (native)
 * - Native apps (IPC)
 * - Traditional apps (network)
 * 
 * Automatically selects the best integration method or allows manual override.
 * 
 * @example
 * ```typescript
 * // Auto-detect best mode
 * const db = new PluresDBLocalFirst();
 * 
 * // Manual mode selection
 * const db = new PluresDBLocalFirst({ mode: "wasm", dbName: "my-app" });
 * 
 * // Use the API (same across all modes)
 * await db.put("user:1", { name: "Alice", email: "alice@example.com" });
 * const user = await db.get("user:1");
 * const results = await db.vectorSearch("Find users named Alice", 10);
 * ```
 */
export class PluresDBLocalFirst {
  private backend: LocalFirstBackend;
  private mode: string;

  constructor(options: LocalFirstOptions = {}) {
    const mode = options.mode || "auto";
    const actualMode = mode === "auto" ? RuntimeDetector.detectBestMode() : mode;

    this.mode = actualMode;

    switch (actualMode) {
      case "wasm":
        this.backend = new WasmBackend(options.dbName);
        break;

      case "tauri":
        this.backend = new TauriBackend();
        break;

      case "ipc":
        this.backend = new IPCBackend(options.channelName);
        break;

      case "network":
        this.backend = new NetworkBackend(options.networkUrl, options.port);
        break;

      default:
        throw new Error(`Unknown mode: ${actualMode}`);
    }

    debugLog(`Initialized in ${this.mode} mode`);
  }

  /**
   * Get the current integration mode
   */
  getMode(): string {
    return this.mode;
  }

  /**
   * Insert or update a node
   */
  async put(id: string, data: unknown): Promise<string> {
    return this.backend.put(id, data);
  }

  /**
   * Retrieve a node by ID
   */
  async get(id: string): Promise<unknown> {
    return this.backend.get(id);
  }

  /**
   * Delete a node by ID
   */
  async delete(id: string): Promise<void> {
    return this.backend.delete(id);
  }

  /**
   * List all nodes
   */
  async list(): Promise<unknown[]> {
    return this.backend.list();
  }

  /**
   * Vector search (semantic similarity)
   */
  async vectorSearch(query: string, limit: number = 10): Promise<unknown[]> {
    if (!this.backend.vectorSearch) {
      throw new Error("Vector search not supported in this mode");
    }
    return this.backend.vectorSearch(query, limit);
  }

  /**
   * Close the database connection
   */
  async close(): Promise<void> {
    if (this.backend.close) {
      await this.backend.close();
    }
  }
}

/**
 * Legacy export for backward compatibility
 */
export default PluresDBLocalFirst;
