export interface AppConfig {
  kvPath?: string;
  port?: number;
  peers?: string[];
  apiPortOffset?: number; // default 1
}

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

export async function saveConfig(cfg: AppConfig): Promise<void> {
  const path = getConfigPath();
  await ensureDirForFile(path);
  await Deno.writeTextFile(path, JSON.stringify(cfg, null, 2));
}

export function getConfigPath(): string {
  const appName = "PluresDB";
  try {
    const os = Deno.build.os;
    if (os === "windows") {
      const appData = Deno.env.get("APPDATA") || Deno.env.get("LOCALAPPDATA") || ".";
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
