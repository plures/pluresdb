/**
 * Node.js wrapper for PluresDB.
 *
 * Spawns a Deno-based PluresDB server subprocess and exposes a Promise-based
 * client API that communicates with it over HTTP.  Designed for use in
 * VSCode extensions and other Node.js environments that cannot run Deno code
 * directly.
 *
 * @example
 * ```typescript
 * import { PluresNode } from "pluresdb/node-wrapper";
 *
 * const db = new PluresNode({ config: { port: 34567 } });
 * await db.start();
 * await db.put("user:1", { name: "Alice" });
 * const user = await db.get("user:1");
 * await db.stop();
 * ```
 */

import { ChildProcess, spawn } from "node:child_process";
import { EventEmitter } from "node:events";
import * as path from "node:path";
import * as fs from "node:fs";
import * as os from "node:os";

/**
 * Configuration options for the embedded PluresDB server process.
 */
export interface PluresDBConfig {
  /** TCP port the REST API server listens on. Defaults to `34567`. */
  port?: number;
  /** Hostname the server binds to. Defaults to `"localhost"`. */
  host?: string;
  /** File-system path where PluresDB persists its data. Defaults to `~/.pluresdb`. */
  dataDir?: string;
  /** TCP port for the optional web UI. Defaults to `34568`. */
  webPort?: number;
  /** Minimum log level emitted by the server subprocess. Defaults to `"info"`. */
  logLevel?: "debug" | "info" | "warn" | "error";
}

/**
 * Options for constructing a {@link PluresNode} instance.
 */
export interface PluresDBOptions {
  /** Server configuration overrides. */
  config?: PluresDBConfig;
  /**
   * Whether to start the server automatically on construction.
   * Defaults to `true`.  Set to `false` to defer startup until
   * {@link PluresNode.start} is called explicitly.
   */
  autoStart?: boolean;
  /** Explicit path to the `deno` executable.  Auto-detected when omitted. */
  denoPath?: string;
}

/**
 * Node.js wrapper class that manages a PluresDB Deno subprocess.
 *
 * Extends `EventEmitter` and emits the following events:
 * - `"started"` — server is ready to accept requests.
 * - `"stopped"` — server process has exited cleanly.
 * - `"exit"` — server process exited with the given exit code.
 * - `"error"` — an error was emitted by the child process.
 * - `"stdout"` / `"stderr"` — raw output lines from the subprocess.
 */
export class PluresNode extends EventEmitter {
  private process: ChildProcess | null = null;
  private config: PluresDBConfig;
  private denoPath: string;
  private isRunning = false;
  private apiUrl: string = "";

  /**
   * Create a new `PluresNode` instance.
   *
   * @param options - Optional configuration overrides.
   */
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

    throw new Error(
      "Deno not found. Please install Deno from https://deno.land/",
    );
  }

  private isCommandAvailable(command: string): boolean {
    try {
      require("child_process").execSync(`"${command}" --version`, {
        stdio: "ignore",
      });
      return true;
    } catch {
      return false;
    }
  }

  /**
   * Start the PluresDB server subprocess.
   *
   * Resolves once the server is ready to accept HTTP requests.
   * Rejects if the process fails to start or if the server does not become
   * healthy within the default 10-second timeout.
   *
   * Calling `start()` while the server is already running is a no-op.
   */
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

        // Find the main.ts file
        const mainTsPath = path.join(__dirname, "main.ts");
        if (!fs.existsSync(mainTsPath)) {
          throw new Error(
            "PluresDB main.ts not found. Please ensure the package is properly installed.",
          );
        }

        // Start the Deno process
        const args = [
          "run",
          "-A",
          mainTsPath,
          "serve",
          "--port",
          this.config.port!.toString(),
          "--host",
          this.config.host!,
          "--data-dir",
          this.config.dataDir!,
        ];

        this.process = spawn(this.denoPath, args, {
          stdio: ["pipe", "pipe", "pipe"],
          cwd: path.dirname(__dirname),
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

  private async waitForServer(timeout = 10000): Promise<void> {
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

  /**
   * Stop the PluresDB server subprocess.
   *
   * Sends `SIGTERM` and waits for the process to exit.  If the process has
   * not exited after 5 seconds a `SIGKILL` is sent.
   *
   * Calling `stop()` when the server is not running is a no-op.
   */
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

  /**
   * Return the base URL of the REST API server (e.g. `"http://localhost:34567"`).
   */
  getApiUrl(): string {
    return this.apiUrl;
  }

  /**
   * Return the base URL of the optional web UI server.
   */
  getWebUrl(): string {
    return `http://${this.config.host}:${this.config.webPort}`;
  }

  /**
   * Return `true` when the server subprocess is running and healthy.
   */
  isServerRunning(): boolean {
    return this.isRunning;
  }

  // SQLite-compatible API methods
  /**
   * Execute a SQL SELECT statement and return the result rows.
   *
   * @param sql    - SQL query string.
   * @param params - Positional bind parameters.
   * @returns Array of result rows as plain objects.
   */
  async query(sql: string, params: unknown[] = []): Promise<unknown> {
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

  /**
   * Insert or update a node by key.
   *
   * @param key   - Stable node identifier.
   * @param value - Arbitrary JSON payload to store.
   */
  async put(key: string, value: unknown): Promise<void> {
    const response = await fetch(`${this.apiUrl}/api/data`, {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ key, value }),
    });

    if (!response.ok) {
      throw new Error(`Put failed: ${response.statusText}`);
    }
  }

  /**
   * Retrieve a node by key.
   *
   * @param key - Node identifier.
   * @returns The stored JSON payload, or `null` if not found.
   */
  async get(key: string): Promise<unknown> {
    const response = await fetch(
      `${this.apiUrl}/api/data/${encodeURIComponent(key)}`,
    );

    if (!response.ok) {
      if (response.status === 404) {
        return null;
      }
      throw new Error(`Get failed: ${response.statusText}`);
    }

    return response.json();
  }

  /**
   * Delete a node by key.
   *
   * @param key - Node identifier to delete.
   */
  async delete(key: string): Promise<void> {
    const response = await fetch(
      `${this.apiUrl}/api/data/${encodeURIComponent(key)}`,
      {
        method: "DELETE",
      },
    );

    if (!response.ok) {
      throw new Error(`Delete failed: ${response.statusText}`);
    }
  }

  /**
   * Perform a vector similarity search using a natural-language query string.
   *
   * The server embeds the query text and returns the closest nodes by cosine
   * similarity.
   *
   * @param query - Natural-language search query.
   * @param limit - Maximum number of results to return. Defaults to `10`.
   * @returns Array of matching node objects ordered by similarity score.
   */
  async vectorSearch(query: string, limit = 10): Promise<unknown[]> {
    const response = await fetch(`${this.apiUrl}/api/vsearch`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ query, limit }),
    });

    if (!response.ok) {
      throw new Error(`Vector search failed: ${response.statusText}`);
    }

    return response.json() as Promise<unknown[]>;
  }

  /**
   * List all node keys, optionally filtered by prefix.
   *
   * @param prefix - Optional key prefix filter.
   * @returns Array of matching node key strings.
   */
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

  /**
   * Retrieve the current server configuration.
   *
   * @returns The active {@link PluresDBConfig} as reported by the server.
   */
  async getConfig(): Promise<unknown> {
    const response = await fetch(`${this.apiUrl}/api/config`);

    if (!response.ok) {
      throw new Error(`Get config failed: ${response.statusText}`);
    }

    return response.json();
  }

  /**
   * Update the server configuration at runtime.
   *
   * @param config - Partial configuration overrides to apply.
   */
  async setConfig(config: unknown): Promise<void> {
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

// Export the main class and types
export { PluresNode as default };
export * from "./types/index";
