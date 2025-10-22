/**
 * Node.js Entry Point for PluresDB
 * This provides a clean API for VSCode extensions and other Node.js applications
 */

import { EventEmitter } from "node:events";
import { spawn, ChildProcess } from "node:child_process";
import * as path from "node:path";
import * as fs from "node:fs";
import * as os from "node:os";
import process from "node:process";
import { MessageChannel, Worker, receiveMessageOnPort } from "node:worker_threads";
import {
  BetterSQLite3Options,
  BetterSQLite3RunResult,
  PluresDBConfig,
  PluresDBOptions,
  QueryResult,
} from "./types/node-types";
import {
  isPlainObject,
  normalizeParameterInput,
  normalizeQueryResult,
  sanitizeDataDirName,
  shapeRow,
  splitSqlStatements,
} from "./better-sqlite3-shared";

const packageRoot =
  typeof __dirname !== "undefined" ? path.resolve(__dirname, "..") : process.cwd();


export class PluresNode extends EventEmitter {
  private process: ChildProcess | null = null;
  private config: PluresDBConfig;
  private denoPath: string;
  private isRunning = false;
  private apiUrl: string = "";

  constructor(options: PluresDBOptions = {}) {
    super();

    this.config = {
      port: 34567,
      host: "localhost",
      dataDir: path.join(os.homedir(), ".pluresdb"),
      webPort: 34568,
      logLevel: "info",
      ...options.config,
    };

    this.denoPath = options.denoPath || this.findDenoPath();

    if (options.autoStart !== false) {
      this.start();
    }
  }

  private findDenoPath(): string {
    // Try to find Deno in common locations
    const possiblePaths = [
      "deno", // In PATH
      path.join(os.homedir(), ".deno", "bin", "deno"),
      path.join(os.homedir(), ".local", "bin", "deno"),
      "/usr/local/bin/deno",
      "/opt/homebrew/bin/deno",
      "C:\\Users\\" + os.userInfo().username + "\\.deno\\bin\\deno.exe",
      "C:\\Program Files\\deno\\deno.exe",
    ];

    for (const denoPath of possiblePaths) {
      try {
        if (fs.existsSync(denoPath) || this.isCommandAvailable(denoPath)) {
          return denoPath;
        }
      } catch {
        // Continue to next path
      }
    }

    throw new Error("Deno not found. Please install Deno from https://deno.land/");
  }

  private isCommandAvailable(command: string): boolean {
    try {
      require("child_process").execSync(`"${command}" --version`, { stdio: "ignore" });
      return true;
    } catch {
      return false;
    }
  }

  async start(): Promise<void> {
    if (this.isRunning) {
      return;
    }

    return new Promise((resolve, reject) => {
      try {
        // Ensure data directory exists
        if (!fs.existsSync(this.config.dataDir!)) {
          fs.mkdirSync(this.config.dataDir!, { recursive: true });
        }

        const kvPath = path.join(this.config.dataDir!, "pluresdb.kv");

        // Find the main.ts file
        const mainTsPath = path.join(packageRoot, "src", "main.ts");
        if (!fs.existsSync(mainTsPath)) {
          throw new Error(
            "PluresDB main.ts not found. Please ensure the package is properly installed.",
          );
        }

        // Start the Deno process
        const args = [
          "run",
          "-A",
          "--unstable-kv",
          "--no-lock",
          mainTsPath,
          "serve",
          "--port",
          this.config.port!.toString(),
          "--host",
          this.config.host!,
          "--kv",
          kvPath,
        ];

        this.process = spawn(this.denoPath, args, {
          stdio: ["pipe", "pipe", "pipe"],
          cwd: packageRoot,
        });

        this.apiUrl = `http://${this.config.host}:${this.config.port}`;

        // Handle process events
        this.process.on("error", (error) => {
          this.emit("error", error);
          reject(error);
        });

        this.process.on("exit", (code) => {
          this.isRunning = false;
          this.emit("exit", code);
        });

        // Wait for server to start
        this.waitForServer()
          .then(() => {
            this.isRunning = true;
            this.emit("started");
            resolve();
          })
          .catch(reject);

        // Handle stdout/stderr
        this.process.stdout?.on("data", (data) => {
          const output = data.toString();
          this.emit("stdout", output);
        });

        this.process.stderr?.on("data", (data) => {
          const output = data.toString();
          this.emit("stderr", output);
        });
      } catch (error) {
        reject(error);
      }
    });
  }

  private async waitForServer(timeout = 20000): Promise<void> {
    const startTime = Date.now();

    while (Date.now() - startTime < timeout) {
      try {
        const response = await fetch(`${this.apiUrl}/api/config`);
        if (response.ok) {
          return;
        }
      } catch {
        // Server not ready yet
      }

      await new Promise((resolve) => setTimeout(resolve, 100));
    }

    throw new Error("Server failed to start within timeout");
  }

  async stop(): Promise<void> {
    if (!this.isRunning || !this.process) {
      return;
    }

    return new Promise((resolve) => {
      this.process!.kill("SIGTERM");

      this.process!.on("exit", () => {
        this.isRunning = false;
        this.emit("stopped");
        resolve();
      });

      // Force kill after 5 seconds
      setTimeout(() => {
        if (this.process && this.isRunning) {
          this.process.kill("SIGKILL");
        }
        resolve();
      }, 5000);
    });
  }

  getApiUrl(): string {
    return this.apiUrl;
  }

  getWebUrl(): string {
    return `http://${this.config.host}:${this.config.webPort}`;
  }

  isServerRunning(): boolean {
    return this.isRunning;
  }

  // SQLite-compatible API methods
  async query(sql: string, params: any[] = []): Promise<any> {
    const response = await fetch(`${this.apiUrl}/api/query`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ sql, params }),
    });

    if (!response.ok) {
      throw new Error(`Query failed: ${response.statusText}`);
    }

    return response.json();
  }

  async put(key: string, value: any): Promise<void> {
    const response = await fetch(`${this.apiUrl}/api/data`, {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ key, value }),
    });

    if (!response.ok) {
      throw new Error(`Put failed: ${response.statusText}`);
    }
  }

  async get(key: string): Promise<any> {
    const response = await fetch(`${this.apiUrl}/api/data/${encodeURIComponent(key)}`);

    if (!response.ok) {
      if (response.status === 404) {
        return null;
      }
      throw new Error(`Get failed: ${response.statusText}`);
    }

    return response.json();
  }

  async delete(key: string): Promise<void> {
    const response = await fetch(`${this.apiUrl}/api/data/${encodeURIComponent(key)}`, {
      method: "DELETE",
    });

    if (!response.ok) {
      throw new Error(`Delete failed: ${response.statusText}`);
    }
  }

  async vectorSearch(query: string, limit = 10): Promise<any[]> {
    const response = await fetch(`${this.apiUrl}/api/vsearch`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ query, limit }),
    });

    if (!response.ok) {
      throw new Error(`Vector search failed: ${response.statusText}`);
    }

    return response.json() as Promise<any[]>;
  }

  async list(prefix?: string): Promise<string[]> {
    const url = prefix
      ? `${this.apiUrl}/api/list?prefix=${encodeURIComponent(prefix)}`
      : `${this.apiUrl}/api/list`;
    const response = await fetch(url);

    if (!response.ok) {
      throw new Error(`List failed: ${response.statusText}`);
    }

    return response.json() as Promise<string[]>;
  }

  async getConfig(): Promise<any> {
    const response = await fetch(`${this.apiUrl}/api/config`);

    if (!response.ok) {
      throw new Error(`Get config failed: ${response.statusText}`);
    }

    return response.json();
  }

  async setConfig(config: any): Promise<void> {
    const response = await fetch(`${this.apiUrl}/api/config`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(config),
    });

    if (!response.ok) {
      throw new Error(`Set config failed: ${response.statusText}`);
    }
  }
}

// SQLite-compatible API for easy migration
export class SQLiteCompatibleAPI {
  private plures: PluresNode;

  constructor(options?: PluresDBOptions) {
    this.plures = new PluresNode(options);
  }

  async start() {
    await this.plures.start();
  }

  async stop() {
    await this.plures.stop();
  }

  // SQLite-compatible methods
  async run(sql: string, params: any[] = []) {
    return this.plures.query(sql, params);
  }

  async get(sql: string, params: any[] = []) {
    const result = await this.plures.query(sql, params);
    return result.rows?.[0] || null;
  }

  async all(sql: string, params: any[] = []) {
    const result = await this.plures.query(sql, params);
    return result.rows || [];
  }

  async exec(sql: string) {
    return this.plures.query(sql);
  }

  // Additional PluresDB specific methods
  async put(key: string, value: any) {
    return this.plures.put(key, value);
  }

  async getValue(key: string) {
    return this.plures.get(key);
  }

  async delete(key: string) {
    return this.plures.delete(key);
  }

  async vectorSearch(query: string, limit = 10) {
    return this.plures.vectorSearch(query, limit);
  }

  async list(prefix?: string) {
    return this.plures.list(prefix);
  }

  getApiUrl() {
    return this.plures.getApiUrl();
  }

  getWebUrl() {
    return this.plures.getWebUrl();
  }

  isRunning() {
    return this.plures.isServerRunning();
  }
}

export class BetterSQLite3Statement {
  private boundParams: unknown[] | undefined;
  private rawMode = false;
  private pluckMode = false;
  private expandMode = false;
  readonly reader: boolean;

  constructor(private readonly database: BetterSQLite3Database, private readonly sql: string) {
    this.reader = /^\s*select/i.test(sql);
  }

  get databaseInstance(): BetterSQLite3Database {
    return this.database;
  }

  bind(...params: unknown[]): this {
    this.boundParams = normalizeParameterInput(params);
    return this;
  }

  raw(toggle = true): this {
    this.rawMode = toggle;
    if (toggle) {
      this.pluckMode = false;
    }
    return this;
  }

  pluck(toggle = true): this {
    this.pluckMode = toggle;
    if (toggle) {
      this.expandMode = false;
      this.rawMode = false;
    }
    return this;
  }

  expand(toggle = true): this {
    this.expandMode = toggle;
    if (toggle) {
      this.pluckMode = false;
    }
    return this;
  }

  safeIntegers(): this {
    // No-op for compatibility with better-sqlite3 API
    return this;
  }

  run(...params: unknown[]): BetterSQLite3RunResult {
    const result = this.database.executeStatement(this.sql, this.resolveParams(params));
    return {
      changes: typeof result.changes === "number" ? result.changes : 0,
      lastInsertRowid:
        typeof result.lastInsertRowId === "number" ? result.lastInsertRowId : null,
      columns: result.columns,
    };
  }

  get(...params: unknown[]): unknown {
    const rows = this.fetchRows(params);
    return rows.length > 0 ? rows[0] : undefined;
  }

  all(...params: unknown[]): unknown[] {
    return this.fetchRows(params);
  }

  iterate(...params: unknown[]): IterableIterator<unknown> {
    const rows = this.fetchRows(params);
    function* generator(): IterableIterator<unknown> {
      for (const row of rows) {
        yield row;
      }
    }
    return generator();
  }

  columns(): string[] {
    const result = this.database.executeStatement(this.sql, this.boundParams ?? []);
    return result.columns ?? [];
  }

  private resolveParams(params: unknown[]): unknown[] {
    const normalized = normalizeParameterInput(params);
    if (normalized.length > 0) {
      return normalized;
    }
    return this.boundParams ? [...this.boundParams] : [];
  }

  private fetchRows(params: unknown[]): unknown[] {
    const result = this.database.executeStatement(this.sql, this.resolveParams(params));
    return this.transformRows(result);
  }

  private transformRows(result: QueryResult): unknown[] {
    return result.rows.map((row) =>
      shapeRow(row, result.columns, {
        raw: this.rawMode,
        pluck: this.pluckMode,
        expand: this.expandMode,
      }),
    );
  }
}

export class BetterSQLite3Database {
  private readonly options: BetterSQLite3Options;
  private readonly plures: PluresNode;
  private readonly filename: string;
  private readonly verbose?: (...args: unknown[]) => void;
  private openPromise: Promise<void> | null = null;
  private opened = false;

  constructor(filenameOrOptions?: string | BetterSQLite3Options, maybeOptions?: BetterSQLite3Options) {
    const { filename, options } = this.resolveOptions(filenameOrOptions, maybeOptions);
    this.filename = filename;
    this.options = options;
    this.verbose = options.verbose;

    const config: PluresDBConfig = { ...options.config };
    if (!config.dataDir) {
      const baseDir = options.memory
        ? path.join(os.tmpdir(), "pluresdb", "better-sqlite3-memory")
        : path.join(os.homedir(), ".pluresdb", "better-sqlite3");
      const safeName = sanitizeDataDirName(filename === ":memory:" ? "memory" : filename);
      config.dataDir = path.join(baseDir, safeName);
    }

    this.plures = new PluresNode({
      config,
      denoPath: options.denoPath,
      autoStart: false,
    });

    if (options.autoStart !== false) {
      void this.open();
    }
  }

  get name(): string {
    return this.filename;
  }

  get isOpen(): boolean {
    return this.opened;
  }

  async open(): Promise<this> {
    await this.ensureOpen();
    return this;
  }

  async close(): Promise<void> {
    if (!this.opened && !this.openPromise) {
      return;
    }
    await this.plures.stop();
    this.opened = false;
    this.openPromise = null;
  }

  prepare(sql: string): BetterSQLite3Statement {
    if (!this.opened) {
      throw new Error("Database is not open. Call await db.open() before preparing statements.");
    }
    return new BetterSQLite3Statement(this, sql);
  }

  async exec(sql: string): Promise<this> {
    await this.ensureOpen();
    for (const statement of splitSqlStatements(sql)) {
      await this.executeStatement(statement, []);
    }
    return this;
  }

  transaction<TArgs extends unknown[], TResult>(
    fn: (...args: TArgs) => Promise<TResult> | TResult,
  ): (...args: TArgs) => Promise<TResult> {
    return async (...args: TArgs): Promise<TResult> => {
      await this.ensureOpen();
      await this.executeStatement("BEGIN", []);
      try {
        const result = await fn(...args);
        await this.executeStatement("COMMIT", []);
        return result;
      } catch (error) {
        await this.executeStatement("ROLLBACK", []).catch(() => undefined);
        throw error;
      }
    };
  }

  async pragma(statement: string): Promise<unknown[]> {
    const sql = /^\s*pragma/i.test(statement) ? statement : `PRAGMA ${statement}`;
    const result = await this.executeStatement(sql, []);
    return result.rows;
  }

  defaultSafeIntegers(): this {
    return this;
  }

  unsafeMode(): this {
    return this;
  }

  async executeStatement(sql: string, params: unknown[]): Promise<QueryResult> {
    await this.ensureOpen();
    const normalizedParams = normalizeParameterInput(params);
    const raw = await this.plures.query(sql, normalizedParams);
    return normalizeQueryResult(raw);
  }

  private async ensureOpen(): Promise<void> {
    if (this.opened) {
      return;
    }
    if (!this.openPromise) {
      this.openPromise = (async () => {
        await this.plures.start();
        this.opened = true;
        if (this.verbose) {
          this.verbose(`PluresDB ready for better-sqlite3 compatibility (${this.filename})`);
        }
      })();
    }
    await this.openPromise;
  }

  private resolveOptions(
    filenameOrOptions?: string | BetterSQLite3Options,
    maybeOptions?: BetterSQLite3Options,
  ): { filename: string; options: BetterSQLite3Options } {
    if (typeof filenameOrOptions === "string") {
      return {
        filename: filenameOrOptions,
        options: { ...(maybeOptions ?? {}), filename: filenameOrOptions },
      };
    }

    const options = filenameOrOptions ?? {};
    const filename = options.filename ?? ":memory:";
    return { filename, options: { ...options, filename } };
  }
}

// Export the main class and types
export { PluresNode as default };
export * from "./types/node-types";
export { PluresVSCodeExtension, createPluresExtension } from "./vscode/extension";
