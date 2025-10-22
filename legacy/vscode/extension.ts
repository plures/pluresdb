import * as path from "node:path";
import { PluresNode, SQLiteCompatibleAPI } from "../node-index";
import type { PluresDBConfig } from "../types/node-types";

type DisposableLike = { dispose(): void };

type InputBoxOptions = {
  prompt: string;
  placeHolder?: string;
};

type TextDocumentInit = {
  content: string;
  language: string;
};

type UriLike = { toString(): string } | string;

export type VSCodeWindow = {
  showInformationMessage(message: string): void | Promise<unknown>;
  showErrorMessage(message: string): void | Promise<unknown>;
  showInputBox(options: InputBoxOptions): Promise<string | undefined>;
  showTextDocument(document: unknown): Promise<unknown>;
};

export type VSCodeCommands = {
  registerCommand(command: string, callback: (...args: unknown[]) => unknown): DisposableLike;
};

export type VSCodeWorkspace = {
  openTextDocument(init: TextDocumentInit): Promise<unknown>;
};

export type VSCodeEnv = {
  openExternal(target: UriLike): Promise<unknown> | unknown;
};

export type VSCodeUri = {
  parse(target: string): UriLike;
};

export type VSCodeAPI = {
  window: VSCodeWindow;
  commands: VSCodeCommands;
  workspace: VSCodeWorkspace;
  env: VSCodeEnv;
  Uri: VSCodeUri;
};

export type ExtensionContextLike = {
  subscriptions: DisposableLike[];
  globalStorageUri: { fsPath: string };
};

export type ExtensionOptions = {
  config?: PluresDBConfig;
  commandPrefix?: string;
  pluresInstance?: PluresNode;
  sqliteInstance?: SQLiteCompatibleAPI;
};

type CommandFactory = () => Promise<void> | void;

const DEFAULT_CONFIG: PluresDBConfig = {
  port: 34567,
  host: "localhost",
  webPort: 34568,
  logLevel: "info",
};

export class PluresVSCodeExtension {
  private readonly vscode: VSCodeAPI;
  private readonly context: ExtensionContextLike;
  private readonly plures: PluresNode;
  private readonly sqlite: SQLiteCompatibleAPI;
  private readonly commandPrefix: string;
  private readonly disposables: DisposableLike[] = [];
  private activated = false;

  constructor(vscodeApi: VSCodeAPI, context: ExtensionContextLike, options: ExtensionOptions = {}) {
    this.vscode = vscodeApi;
    this.context = context;
    this.commandPrefix = options.commandPrefix ?? "pluresdb";

    const mergedConfig: PluresDBConfig = {
      ...DEFAULT_CONFIG,
      dataDir: path.join(context.globalStorageUri.fsPath, "pluresdb"),
      ...options.config,
    };

    this.plures =
      options.pluresInstance ?? new PluresNode({ config: mergedConfig, autoStart: false });
    this.sqlite =
      options.sqliteInstance ?? new SQLiteCompatibleAPI({ config: mergedConfig, autoStart: false });

    this.setupEventHandlers();
  }

  async activate(): Promise<void> {
    if (this.activated) {
      return;
    }

    try {
      await this.plures.start();
      await this.sqlite.start();
      this.registerCommands();
      await this.setupDatabase();
      this.activated = true;
      await this.safeInfo("PluresDB extension activated");
    } catch (error) {
      await this.safeError(`Failed to activate PluresDB: ${this.errorMessage(error)}`);
      throw error;
    }
  }

  async deactivate(): Promise<void> {
    if (!this.activated) {
      return;
    }

    try {
      await this.sqlite.stop();
      await this.plures.stop();
    } finally {
      this.disposeAll();
      this.activated = false;
    }
  }

  getWebUrl(): string {
    return this.plures.getWebUrl();
  }

  async storeSetting(key: string, value: unknown) {
    return this.sqlite.put(`settings:${key}`, value);
  }

  async getSetting(key: string) {
    return this.sqlite.getValue(`settings:${key}`);
  }

  async storeDocument(id: string, content: string, language: string, filePath: string) {
    return this.sqlite.put(`documents:${id}`, {
      content,
      language,
      filePath,
      updatedAt: new Date().toISOString(),
    });
  }

  async searchDocuments(query: string, limit = 20) {
    return this.sqlite.vectorSearch(query, limit);
  }

  async executeSQL(sql: string, params: unknown[] = []) {
    return this.sqlite.all(sql, params);
  }

  private setupEventHandlers() {
    this.plures.on("started", () => {
      this.safeInfo("PluresDB database started");
    });

    this.plures.on("stopped", () => {
      this.safeInfo("PluresDB database stopped");
    });

    this.plures.on("error", (error: unknown) => {
      this.safeError(`PluresDB error: ${this.errorMessage(error)}`);
    });

    this.plures.on("stderr", (output: string) => {
      const trimmed = output.trim();
      if (trimmed.length > 0) {
        this.safeError(trimmed);
      }
    });

    this.plures.on("stdout", (output: string) => {
      const trimmed = output.trim();
      if (trimmed.length > 0) {
        this.safeInfo(trimmed);
      }
    });
  }

  private registerCommands() {
    const register = (name: string, factory: CommandFactory) => {
      const disposable = this.vscode.commands.registerCommand(`${this.commandPrefix}.${name}`, () =>
        factory(),
      );
      this.context.subscriptions.push(disposable);
      this.disposables.push(disposable);
    };

    register("openWebUI", async () => {
      const webUrl = this.getWebUrl();
      await this.vscode.env.openExternal(this.vscode.Uri.parse(webUrl));
    });

    register("executeQuery", async () => {
      const sql = await this.vscode.window.showInputBox({
        prompt: "Enter SQL query",
        placeHolder: "SELECT * FROM users",
      });

      if (!sql) return;

      try {
        const result = await this.sqlite.all(sql);
        const doc = await this.vscode.workspace.openTextDocument({
          content: JSON.stringify(result, null, 2),
          language: "json",
        });
        await this.vscode.window.showTextDocument(doc);
      } catch (error) {
        await this.safeError(`Query failed: ${this.errorMessage(error)}`);
      }
    });

    register("vectorSearch", async () => {
      const query = await this.vscode.window.showInputBox({
        prompt: "Enter search query",
        placeHolder: "machine learning",
      });

      if (!query) return;

      try {
        const results = await this.sqlite.vectorSearch(query, 10);
        const doc = await this.vscode.workspace.openTextDocument({
          content: JSON.stringify(results, null, 2),
          language: "json",
        });
        await this.vscode.window.showTextDocument(doc);
      } catch (error) {
        await this.safeError(`Vector search failed: ${this.errorMessage(error)}`);
      }
    });

    register("storeData", async () => {
      const key = await this.vscode.window.showInputBox({
        prompt: "Enter key",
        placeHolder: "user:123",
      });

      if (!key) return;

      const json = await this.vscode.window.showInputBox({
        prompt: "Enter value (JSON)",
        placeHolder: '{"name": "Ada", "email": "ada@example.com"}',
      });

      if (!json) return;

      try {
        const value = JSON.parse(json);
        await this.sqlite.put(key, value);
        await this.safeInfo(`Stored data for key: ${key}`);
      } catch (error) {
        await this.safeError(`Failed to store data: ${this.errorMessage(error)}`);
      }
    });

    register("retrieveData", async () => {
      const key = await this.vscode.window.showInputBox({
        prompt: "Enter key to retrieve",
        placeHolder: "user:123",
      });

      if (!key) return;

      try {
        const value = await this.sqlite.getValue(key);
        if (value) {
          const doc = await this.vscode.workspace.openTextDocument({
            content: JSON.stringify(value, null, 2),
            language: "json",
          });
          await this.vscode.window.showTextDocument(doc);
        } else {
          await this.safeInfo("Key not found");
        }
      } catch (error) {
        await this.safeError(`Failed to retrieve data: ${this.errorMessage(error)}`);
      }
    });
  }

  private async setupDatabase() {
    const statements = [
      `CREATE TABLE IF NOT EXISTS settings (
        key TEXT PRIMARY KEY,
        value TEXT,
        created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
        updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
      )`,
      `CREATE TABLE IF NOT EXISTS documents (
        id TEXT PRIMARY KEY,
        content TEXT,
        language TEXT,
        file_path TEXT,
        created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
        updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
      )`,
      `CREATE TABLE IF NOT EXISTS search_history (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        query TEXT,
        results_count INTEGER,
        created_at DATETIME DEFAULT CURRENT_TIMESTAMP
      )`,
    ];

    for (const sql of statements) {
      try {
        await this.sqlite.exec(sql);
      } catch (error) {
        await this.safeError(`Failed to initialize database: ${this.errorMessage(error)}`);
      }
    }
  }

  private disposeAll() {
    for (const disposable of this.disposables.splice(0)) {
      try {
        disposable.dispose();
      } catch (_error) {
        // ignore
      }
    }
  }

  private async safeInfo(message: string) {
    try {
      await this.vscode.window.showInformationMessage(message);
    } catch (_error) {
      // ignore message failures in headless tests
    }
  }

  private async safeError(message: string) {
    try {
      await this.vscode.window.showErrorMessage(message);
    } catch (_error) {
      // ignore message failures in headless tests
    }
  }

  private errorMessage(error: unknown): string {
    if (error instanceof Error) {
      return error.message;
    }
    return String(error);
  }
}

export function createPluresExtension(
  vscodeApi: VSCodeAPI,
  context: ExtensionContextLike,
  options: ExtensionOptions = {},
) {
  return new PluresVSCodeExtension(vscodeApi, context, options);
}
