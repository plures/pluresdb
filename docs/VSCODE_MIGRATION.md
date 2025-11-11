# VSCode Extension Migration Guide

This guide helps you migrate your VSCode extension from SQLite to PluresDB, gaining P2P capabilities while maintaining SQLite compatibility.

## 🎯 Why Migrate?

- **P2P Sync**: Share data across devices and team members
- **Offline-First**: Work without internet connection
- **Vector Search**: Semantic search across your data
- **Encrypted Sharing**: Secure data sharing between peers
- **SQLite Compatibility**: **Zero code changes required!** 🎉

## 📋 Migration Steps

### 1. Install PluresDB

```bash
npm install pluresdb
# or
yarn add pluresdb
# or
pnpm add pluresdb
```

### 2. Update Package.json

```json
{
  "dependencies": {
    "pluresdb": "^1.0.0"
  },
  "peerDependencies": {
    "deno": ">=1.40.0"
  }
}
```

### 3. Replace SQLite Imports

**Before (SQLite):**

```typescript
import sqlite3 from "sqlite3";
import { open } from "sqlite";
```

**After (PluresDB):**

```typescript
import sqlite3 from "pluresdb";
import { open } from "pluresdb";
```

### 4. That's It! 🎉

**No other changes needed!** Your existing SQLite code will work exactly the same:

```typescript
// This works exactly the same as before!
const db = await open({
  filename: path.join(context.globalStorageUri.fsPath, "database.db"),
  driver: sqlite3.Database,
});

// All your existing SQLite operations work unchanged
await db.exec("CREATE TABLE IF NOT EXISTS settings (key TEXT, value TEXT)");
await db.run("INSERT INTO settings VALUES (?, ?)", ["theme", "dark"]);
const settings = await db.all("SELECT * FROM settings");
```

### 5. Database Operations (No Changes Required!)

**All SQLite operations work exactly the same:**

```typescript
// Create tables
await db.exec(`
  CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value TEXT
  )
`);

// Insert data
await db.run("INSERT INTO settings VALUES (?, ?)", ["theme", "dark"]);

// Query data
const settings = await db.all("SELECT * FROM settings");
const setting = await db.get("SELECT * FROM settings WHERE key = ?", ["theme"]);

// Update data
await db.run("UPDATE settings SET value = ? WHERE key = ?", ["light", "theme"]);

// Delete data
await db.run("DELETE FROM settings WHERE key = ?", ["theme"]);

// Transactions
await db.transaction(async (db) => {
  await db.run("INSERT INTO settings VALUES (?, ?)", ["key1", "value1"]);
  await db.run("INSERT INTO settings VALUES (?, ?)", ["key2", "value2"]);
});

// Prepared statements
const stmt = db.prepare("INSERT INTO settings VALUES (?, ?)");
await stmt.run(["key3", "value3"]);
stmt.finalize();
```

**All existing SQLite code works without any changes!** ✨

### 6. Add P2P Features (Optional)

```typescript
import { PluresDBNode } from "pluresdb";

// Initialize P2P capabilities (uses same data directory as SQLite)
const p2p = new PluresDBNode({
  config: {
    dataDir: path.join(context.globalStorageUri.fsPath, "pluresdb"),
    port: 34567,
    host: "localhost",
  },
});

await p2p.start();

// Create identity
await p2p.createIdentity({
  name: "My Extension",
  email: "user@example.com",
});

// Search for peers
const peers = await p2p.searchPeers("developer");

// Share data with peers
await p2p.shareNode("settings:theme", peerId, {
  accessLevel: "read-only",
});
```

## 🔧 Complete Example

Here's a complete example of migrating a VSCode extension:

```typescript
import * as vscode from "vscode";
import sqlite3 from "pluresdb";
import { open } from "pluresdb";
import { PluresDBNode } from "pluresdb";
import * as path from "path";

export class MyExtension {
  private db: any; // SQLite-compatible database
  private p2p: PluresDBNode;
  private context: vscode.ExtensionContext;

  constructor(context: vscode.ExtensionContext) {
    this.context = context;

    // Initialize database (exactly like SQLite!)
    this.db = null; // Will be initialized in activate()

    // Initialize P2P (optional)
    this.p2p = new PluresDBNode({
      config: {
        dataDir: path.join(context.globalStorageUri.fsPath, "pluresdb"),
        port: 34567,
        host: "localhost",
      },
    });
  }

  async activate() {
    // Start database (exactly like SQLite!)
    this.db = await open({
      filename: path.join(this.context.globalStorageUri.fsPath, "database.db"),
      driver: sqlite3.Database,
    });

    await this.p2p.start();

    // Set up database schema
    await this.setupDatabase();

    // Register commands
    this.registerCommands();
  }

  async deactivate() {
    await this.db.close();
    await this.p2p.stop();
  }

  private async setupDatabase() {
    // Create tables
    await this.db.exec(`
      CREATE TABLE IF NOT EXISTS settings (
        key TEXT PRIMARY KEY,
        value TEXT,
        created_at DATETIME DEFAULT CURRENT_TIMESTAMP
      )
    `);

    await this.db.exec(`
      CREATE TABLE IF NOT EXISTS documents (
        id TEXT PRIMARY KEY,
        content TEXT,
        language TEXT,
        file_path TEXT,
        created_at DATETIME DEFAULT CURRENT_TIMESTAMP
      )
    `);
  }

  private registerCommands() {
    // Command to store setting
    const storeSetting = vscode.commands.registerCommand(
      "myExtension.storeSetting",
      async () => {
        const key = await vscode.window.showInputBox({ prompt: "Setting key" });
        const value = await vscode.window.showInputBox({
          prompt: "Setting value",
        });

        if (key && value) {
          await this.db.run("INSERT OR REPLACE INTO settings VALUES (?, ?)", [
            key,
            value,
          ]);
          vscode.window.showInformationMessage(`Setting stored: ${key}`);
        }
      },
    );

    // Command to get setting
    const getSetting = vscode.commands.registerCommand(
      "myExtension.getSetting",
      async () => {
        const key = await vscode.window.showInputBox({ prompt: "Setting key" });

        if (key) {
          const setting = await this.db.get(
            "SELECT * FROM settings WHERE key = ?",
            [key],
          );
          if (setting) {
            vscode.window.showInformationMessage(
              `Setting ${key}: ${setting.value}`,
            );
          } else {
            vscode.window.showInformationMessage("Setting not found");
          }
        }
      },
    );

    // Command to search documents
    const searchDocuments = vscode.commands.registerCommand(
      "myExtension.searchDocuments",
      async () => {
        const query = await vscode.window.showInputBox({
          prompt: "Search query",
        });

        if (query) {
          // Use SQL LIKE for text search (same as SQLite)
          const results = await this.db.all(
            "SELECT * FROM documents WHERE content LIKE ?",
            [
              `%${query}%`,
            ],
          );

          // Display results
          const doc = await vscode.workspace.openTextDocument({
            content: JSON.stringify(results, null, 2),
            language: "json",
          });
          await vscode.window.showTextDocument(doc);
        }
      },
    );

    // Command to share data
    const shareData = vscode.commands.registerCommand(
      "myExtension.shareData",
      async () => {
        const key = await vscode.window.showInputBox({
          prompt: "Data key to share",
        });

        if (key) {
          // Search for peers
          const peers = await this.p2p.searchPeers("developer");

          if (peers.length > 0) {
            const peer = await vscode.window.showQuickPick(
              peers.map((p) => ({
                label: p.name,
                description: p.email,
                peer: p,
              })),
              { placeHolder: "Select peer to share with" },
            );

            if (peer) {
              await this.p2p.shareNode(key, peer.peer.id, {
                accessLevel: "read-only",
              });
              vscode.window.showInformationMessage(
                `Shared ${key} with ${peer.label}`,
              );
            }
          } else {
            vscode.window.showInformationMessage("No peers found");
          }
        }
      },
    );

    this.context.subscriptions.push(
      storeSetting,
      getSetting,
      searchDocuments,
      shareData,
    );
  }
}

// Extension activation
export function activate(context: vscode.ExtensionContext) {
  const extension = new MyExtension(context);
  extension.activate();
  return extension;
}

// Extension deactivation
export function deactivate(extension: MyExtension) {
  extension.deactivate();
}
```

## 🚀 Advanced Features

### Vector Search

```typescript
// Search for semantically similar content
const results = await db.vectorSearch("machine learning", 10);

// Results include similarity scores
results.forEach((result) => {
  console.log(`${result.content} (similarity: ${result.score})`);
});
```

### P2P Sync

```typescript
// Enable automatic sync
await p2p.enableAutoSync();

// Sync with specific peer
await p2p.syncWithPeer(peerId);

// Handle sync events
p2p.on("sync", (data) => {
  console.log("Data synced:", data);
});
```

### Encrypted Sharing

```typescript
// Share data with encryption
await p2p.shareNode("sensitive-data", peerId, {
  accessLevel: "read-only",
  encryption: true,
  expiration: "1 week",
});

// Accept shared data
await p2p.acceptSharedNode(sharedNodeId);
```

## 🔍 Troubleshooting

### Common Issues

1. **Deno not found**: Install Deno from https://deno.land/
2. **Port conflicts**: Change the port in configuration
3. **Permission errors**: Check file system permissions
4. **Sync issues**: Verify network connectivity

### Debug Mode

```typescript
const db = new SQLiteCompatibleAPI({
  config: {
    logLevel: "debug",
  },
});
```

### Performance Optimization

```typescript
// Use transactions for bulk operations
await db.exec("BEGIN TRANSACTION");
for (const item of items) {
  await db.run("INSERT INTO items VALUES (?, ?)", [item.id, item.value]);
}
await db.exec("COMMIT");
```

## 📚 Additional Resources

- [API Reference](API.md)
- [P2P Guide](P2P.md)
- [Performance Tips](PERFORMANCE.md)
- [Examples](examples/)

## 🆘 Support

- **Issues**: [GitHub Issues](https://github.com/plures/pluresdb/issues)
- **Discussions**: [GitHub Discussions](https://github.com/plures/pluresdb/discussions)
- **Discord**: [Join our Discord](https://discord.gg/pluresdb)

---

**Ready to add P2P capabilities to your VSCode extension?** 🚀
