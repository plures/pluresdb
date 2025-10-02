/**
 * VSCode Extension Integration Example
 * This shows how to integrate Rusty Gun into a VSCode extension
 */

import * as vscode from 'vscode';
import { RustyGunNode, SQLiteCompatibleAPI } from 'rusty-gun';

export class RustyGunExtension {
  private rustyGun: RustyGunNode;
  private sqliteAPI: SQLiteCompatibleAPI;
  private context: vscode.ExtensionContext;

  constructor(context: vscode.ExtensionContext) {
    this.context = context;
    
    // Initialize Rusty Gun with VSCode-specific configuration
    this.rustyGun = new RustyGunNode({
      config: {
        port: 34567,
        host: 'localhost',
        dataDir: path.join(context.globalStorageUri.fsPath, 'rusty-gun'),
        webPort: 34568,
        logLevel: 'info'
      },
      autoStart: false // We'll start it manually
    });

    // Create SQLite-compatible API
    this.sqliteAPI = new SQLiteCompatibleAPI({
      config: {
        port: 34567,
        host: 'localhost',
        dataDir: path.join(context.globalStorageUri.fsPath, 'rusty-gun'),
        webPort: 34568,
        logLevel: 'info'
      },
      autoStart: false
    });

    // Set up event handlers
    this.setupEventHandlers();
  }

  private setupEventHandlers() {
    this.rustyGun.on('started', () => {
      vscode.window.showInformationMessage('Rusty Gun database started');
    });

    this.rustyGun.on('stopped', () => {
      vscode.window.showInformationMessage('Rusty Gun database stopped');
    });

    this.rustyGun.on('error', (error) => {
      vscode.window.showErrorMessage(`Rusty Gun error: ${error.message}`);
    });
  }

  async activate() {
    try {
      // Start Rusty Gun
      await this.rustyGun.start();
      await this.sqliteAPI.start();

      // Register commands
      this.registerCommands();

      // Set up database schema
      await this.setupDatabase();

      vscode.window.showInformationMessage('Rusty Gun extension activated');
    } catch (error) {
      vscode.window.showErrorMessage(`Failed to activate Rusty Gun: ${error.message}`);
    }
  }

  async deactivate() {
    try {
      await this.rustyGun.stop();
      await this.sqliteAPI.stop();
    } catch (error) {
      console.error('Error stopping Rusty Gun:', error);
    }
  }

  private registerCommands() {
    // Command to open Rusty Gun web UI
    const openWebUI = vscode.commands.registerCommand('rusty-gun.openWebUI', () => {
      const webUrl = this.rustyGun.getWebUrl();
      vscode.env.openExternal(vscode.Uri.parse(webUrl));
    });

    // Command to execute SQL query
    const executeQuery = vscode.commands.registerCommand('rusty-gun.executeQuery', async () => {
      const sql = await vscode.window.showInputBox({
        prompt: 'Enter SQL query',
        placeHolder: 'SELECT * FROM users'
      });

      if (sql) {
        try {
          const result = await this.sqliteAPI.all(sql);
          const doc = await vscode.workspace.openTextDocument({
            content: JSON.stringify(result, null, 2),
            language: 'json'
          });
          await vscode.window.showTextDocument(doc);
        } catch (error) {
          vscode.window.showErrorMessage(`Query failed: ${error.message}`);
        }
      }
    });

    // Command to perform vector search
    const vectorSearch = vscode.commands.registerCommand('rusty-gun.vectorSearch', async () => {
      const query = await vscode.window.showInputBox({
        prompt: 'Enter search query',
        placeHolder: 'machine learning'
      });

      if (query) {
        try {
          const results = await this.sqliteAPI.vectorSearch(query, 10);
          const doc = await vscode.workspace.openTextDocument({
            content: JSON.stringify(results, null, 2),
            language: 'json'
          });
          await vscode.window.showTextDocument(doc);
        } catch (error) {
          vscode.window.showErrorMessage(`Vector search failed: ${error.message}`);
        }
      }
    });

    // Command to store data
    const storeData = vscode.commands.registerCommand('rusty-gun.storeData', async () => {
      const key = await vscode.window.showInputBox({
        prompt: 'Enter key',
        placeHolder: 'user:123'
      });

      if (key) {
        const value = await vscode.window.showInputBox({
          prompt: 'Enter value (JSON)',
          placeHolder: '{"name": "John", "email": "john@example.com"}'
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
    const retrieveData = vscode.commands.registerCommand('rusty-gun.retrieveData', async () => {
      const key = await vscode.window.showInputBox({
        prompt: 'Enter key to retrieve',
        placeHolder: 'user:123'
      });

      if (key) {
        try {
          const value = await this.sqliteAPI.getValue(key);
          if (value) {
            const doc = await vscode.workspace.openTextDocument({
              content: JSON.stringify(value, null, 2),
              language: 'json'
            });
            await vscode.window.showTextDocument(doc);
          } else {
            vscode.window.showInformationMessage('Key not found');
          }
        } catch (error) {
          vscode.window.showErrorMessage(`Failed to retrieve data: ${error.message}`);
        }
      }
    });

    // Register all commands
    this.context.subscriptions.push(
      openWebUI,
      executeQuery,
      vectorSearch,
      storeData,
      retrieveData
    );
  }

  private async setupDatabase() {
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
      )`
    ];

    for (const sql of tables) {
      try {
        await this.sqliteAPI.exec(sql);
      } catch (error) {
        console.error('Error creating table:', error);
      }
    }
  }

  // Public API methods for other parts of the extension
  async storeSetting(key: string, value: any) {
    return this.sqliteAPI.put(`settings:${key}`, value);
  }

  async getSetting(key: string) {
    return this.sqliteAPI.getValue(`settings:${key}`);
  }

  async storeDocument(id: string, content: string, language: string, filePath: string) {
    return this.sqliteAPI.put(`documents:${id}`, {
      content,
      language,
      filePath,
      updatedAt: new Date().toISOString()
    });
  }

  async searchDocuments(query: string) {
    return this.sqliteAPI.vectorSearch(query, 20);
  }

  async executeSQL(sql: string, params: any[] = []) {
    return this.sqliteAPI.all(sql, params);
  }
}

// Extension activation function
export function activate(context: vscode.ExtensionContext) {
  const extension = new RustyGunExtension(context);
  extension.activate();
  return extension;
}

// Extension deactivation function
export function deactivate(extension: RustyGunExtension) {
  extension.deactivate();
}

