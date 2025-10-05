"use strict";
/**
 * VSCode Extension Integration Example
 * This shows how to integrate PluresDB into a VSCode extension
 */
var __createBinding =
  (this && this.__createBinding) ||
  (Object.create
    ? function (o, m, k, k2) {
        if (k2 === undefined) k2 = k;
        var desc = Object.getOwnPropertyDescriptor(m, k);
        if (!desc || ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)) {
          desc = {
            enumerable: true,
            get: function () {
              return m[k];
            },
          };
        }
        Object.defineProperty(o, k2, desc);
      }
    : function (o, m, k, k2) {
        if (k2 === undefined) k2 = k;
        o[k2] = m[k];
      });
var __setModuleDefault =
  (this && this.__setModuleDefault) ||
  (Object.create
    ? function (o, v) {
        Object.defineProperty(o, "default", { enumerable: true, value: v });
      }
    : function (o, v) {
        o["default"] = v;
      });
var __importStar =
  (this && this.__importStar) ||
  (function () {
    var ownKeys = function (o) {
      ownKeys =
        Object.getOwnPropertyNames ||
        function (o) {
          var ar = [];
          for (var k in o) if (Object.prototype.hasOwnProperty.call(o, k)) ar[ar.length] = k;
          return ar;
        };
      return ownKeys(o);
    };
    return function (mod) {
      if (mod && mod.__esModule) return mod;
      var result = {};
      if (mod != null)
        for (var k = ownKeys(mod), i = 0; i < k.length; i++)
          if (k[i] !== "default") __createBinding(result, mod, k[i]);
      __setModuleDefault(result, mod);
      return result;
    };
  })();
Object.defineProperty(exports, "__esModule", { value: true });
exports.PluresExtension = void 0;
exports.activate = activate;
exports.deactivate = deactivate;
const vscode = __importStar(require("vscode"));
const pluresdb_1 = require("pluresdb");
class PluresExtension {
  plures;
  sqliteAPI;
  context;
  constructor(context) {
    this.context = context;
    // Initialize PluresDB with VSCode-specific configuration
    this.plures = new pluresdb_1.PluresNode({
      config: {
        port: 34567,
        host: "localhost",
        dataDir: path.join(context.globalStorageUri.fsPath, "pluresdb"),
        webPort: 34568,
        logLevel: "info",
      },
      autoStart: false, // We'll start it manually
    });
    // Create SQLite-compatible API
    this.sqliteAPI = new pluresdb_1.SQLiteCompatibleAPI({
      config: {
        port: 34567,
        host: "localhost",
        dataDir: path.join(context.globalStorageUri.fsPath, "pluresdb"),
        webPort: 34568,
        logLevel: "info",
      },
      autoStart: false,
    });
    // Set up event handlers
    this.setupEventHandlers();
  }
  setupEventHandlers() {
    this.plures.on("started", () => {
      vscode.window.showInformationMessage("PluresDB database started");
    });
    this.plures.on("stopped", () => {
      vscode.window.showInformationMessage("PluresDB database stopped");
    });
    this.plures.on("error", (error) => {
      vscode.window.showErrorMessage(`PluresDB error: ${error.message}`);
    });
  }
  async activate() {
    try {
      // Start PluresDB
      await this.plures.start();
      await this.sqliteAPI.start();
      // Register commands
      this.registerCommands();
      // Set up database schema
      await this.setupDatabase();
      vscode.window.showInformationMessage("PluresDB extension activated");
    } catch (error) {
      vscode.window.showErrorMessage(`Failed to activate PluresDB: ${error.message}`);
    }
  }
  async deactivate() {
    try {
      await this.plures.stop();
      await this.sqliteAPI.stop();
    } catch (error) {
      console.error("Error stopping PluresDB:", error);
    }
  }
  registerCommands() {
    // Command to open PluresDB web UI
    const openWebUI = vscode.commands.registerCommand("pluresdb.openWebUI", () => {
      const webUrl = this.plures.getWebUrl();
      vscode.env.openExternal(vscode.Uri.parse(webUrl));
    });
    // Command to execute SQL query
    const executeQuery = vscode.commands.registerCommand("pluresdb.executeQuery", async () => {
      const sql = await vscode.window.showInputBox({
        prompt: "Enter SQL query",
        placeHolder: "SELECT * FROM users",
      });
      if (sql) {
        try {
          const result = await this.sqliteAPI.all(sql);
          const doc = await vscode.workspace.openTextDocument({
            content: JSON.stringify(result, null, 2),
            language: "json",
          });
          await vscode.window.showTextDocument(doc);
        } catch (error) {
          vscode.window.showErrorMessage(`Query failed: ${error.message}`);
        }
      }
    });
    // Command to perform vector search
    const vectorSearch = vscode.commands.registerCommand("pluresdb.vectorSearch", async () => {
      const query = await vscode.window.showInputBox({
        prompt: "Enter search query",
        placeHolder: "machine learning",
      });
      if (query) {
        try {
          const results = await this.sqliteAPI.vectorSearch(query, 10);
          const doc = await vscode.workspace.openTextDocument({
            content: JSON.stringify(results, null, 2),
            language: "json",
          });
          await vscode.window.showTextDocument(doc);
        } catch (error) {
          vscode.window.showErrorMessage(`Vector search failed: ${error.message}`);
        }
      }
    });
    // Command to store data
    const storeData = vscode.commands.registerCommand("pluresdb.storeData", async () => {
      const key = await vscode.window.showInputBox({
        prompt: "Enter key",
        placeHolder: "user:123",
      });
      if (key) {
        const value = await vscode.window.showInputBox({
          prompt: "Enter value (JSON)",
          placeHolder: '{"name": "John", "email": "john@example.com"}',
        });
        if (value) {
          try {
            const parsedValue = JSON.parse(value);
            await this.sqliteAPI.put(key, parsedValue);
            vscode.window.showInformationMessage(`Stored data for key: ${key}`);
          } catch (error) {
            vscode.window.showErrorMessage(`Failed to store data: ${error.message}`);
          }
        }
      }
    });
    // Command to retrieve data
    const retrieveData = vscode.commands.registerCommand("pluresdb.retrieveData", async () => {
      const key = await vscode.window.showInputBox({
        prompt: "Enter key to retrieve",
        placeHolder: "user:123",
      });
      if (key) {
        try {
          const value = await this.sqliteAPI.getValue(key);
          if (value) {
            const doc = await vscode.workspace.openTextDocument({
              content: JSON.stringify(value, null, 2),
              language: "json",
            });
            await vscode.window.showTextDocument(doc);
          } else {
            vscode.window.showInformationMessage("Key not found");
          }
        } catch (error) {
          vscode.window.showErrorMessage(`Failed to retrieve data: ${error.message}`);
        }
      }
    });
    // Register all commands
    this.context.subscriptions.push(openWebUI, executeQuery, vectorSearch, storeData, retrieveData);
  }
  async setupDatabase() {
    // Create tables for common VSCode extension use cases
    const tables = [
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
    for (const sql of tables) {
      try {
        await this.sqliteAPI.exec(sql);
      } catch (error) {
        console.error("Error creating table:", error);
      }
    }
  }
  // Public API methods for other parts of the extension
  async storeSetting(key, value) {
    return this.sqliteAPI.put(`settings:${key}`, value);
  }
  async getSetting(key) {
    return this.sqliteAPI.getValue(`settings:${key}`);
  }
  async storeDocument(id, content, language, filePath) {
    return this.sqliteAPI.put(`documents:${id}`, {
      content,
      language,
      filePath,
      updatedAt: new Date().toISOString(),
    });
  }
  async searchDocuments(query) {
    return this.sqliteAPI.vectorSearch(query, 20);
  }
  async executeSQL(sql, params = []) {
    return this.sqliteAPI.all(sql, params);
  }
}
exports.PluresExtension = PluresExtension;
// Extension activation function
function activate(context) {
  const extension = new PluresExtension(context);
  extension.activate();
  return extension;
}
// Extension deactivation function
function deactivate(extension) {
  extension.deactivate();
}
//# sourceMappingURL=vscode-extension-integration.js.map
