import type { TransportConfig } from "./sync/transport.ts";

/**
 * Top-level application configuration for PluresDB.
 *
 * Persisted to disk as JSON via {@link saveConfig} and loaded via
 * {@link loadConfig}.
 */
export interface AppConfig {
  /** File-system path for the Deno KV store. */
  kvPath?: string;
  /** TCP port for the WebSocket mesh server. */
  port?: number;
  /** List of remote peer WebSocket URLs to connect to on startup. */
  peers?: string[];
  /**
   * Port offset added to `port` to derive the REST API port.
   * Defaults to `1` (so `port=8080` → API port `8081`).
   */
  apiPortOffset?: number; // default 1
  
  // Sync transport configuration
  syncTransport?: TransportConfig;
}

/**
 * Load application configuration from the platform-appropriate config file.
 *
 * Returns an empty object when the file does not exist or cannot be parsed.
 *
 * @returns Parsed {@link AppConfig}.
 */
export async function loadConfig(): Promise<AppConfig> {
  const path = getConfigPath();
  try {
    const text = await Deno.readTextFile(path);
    const cfg = JSON.parse(text) as AppConfig;
    return cfg;
  } catch {
    return {};
  }
}

/**
 * Persist application configuration to disk.
 *
 * Creates intermediate directories if they do not yet exist.
 *
 * @param cfg - Configuration to save.
 */
export async function saveConfig(cfg: AppConfig): Promise<void> {
  const path = getConfigPath();
  await ensureDirForFile(path);
  await Deno.writeTextFile(path, JSON.stringify(cfg, null, 2));
}

/**
 * Return the platform-appropriate path to the PluresDB config file.
 *
 * - **Windows**: `%APPDATA%\PluresDB\config.json`
 * - **macOS/Linux**: `~/.pluresdb/config.json`
 * - **Fallback**: `./config.json` (when environment variables are unavailable)
 *
 * @returns Absolute path string.
 */
export function getConfigPath(): string {
  const appName = "PluresDB";
  try {
    const os = Deno.build.os;
    if (os === "windows") {
      const appData = Deno.env.get("APPDATA") || Deno.env.get("LOCALAPPDATA") ||
        ".";
      return `${appData}\\${appName}\\config.json`;
    }
    const home = Deno.env.get("HOME") || ".";
    return `${home}/.${appName.toLowerCase()}/config.json`;
  } catch {
    return `./config.json`;
  }
}

async function ensureDirForFile(filePath: string): Promise<void> {
  const sep = filePath.includes("\\") ? "\\" : "/";
  const dir = filePath.split(sep).slice(0, -1).join(sep);
  if (!dir) return;
  try {
    await Deno.mkdir(dir, { recursive: true });
  } catch {
    /* ignore */
  }
}
