/**
 * Node.js Wrapper for PluresDB
 * This module provides a Node.js-compatible API for VSCode extensions
 */

import { spawn, ChildProcess } from "node:child_process";
import { EventEmitter } from "node:events";
import * as path from "node:path";
import * as fs from "node:fs";
import * as os from "node:os";

export interface PluresDBConfig {
  port?: number;
  host?: string;
  dataDir?: string;
  webPort?: number;
  logLevel?: "debug" | "info" | "warn" | "error";
}

export interface PluresDBOptions {
  config?: PluresDBConfig;
  autoStart?: boolean;
  denoPath?: string;
}

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

// Export the main class and types
export { PluresNode as default };
export * from "./types/index";
