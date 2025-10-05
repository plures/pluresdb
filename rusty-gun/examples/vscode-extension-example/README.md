# Rusty Gun VSCode Extension Example

This is an example VSCode extension that demonstrates how to integrate Rusty Gun into your VSCode extension.

## Features

- **Store Data**: Store key-value pairs in Rusty Gun
- **Retrieve Data**: Retrieve data by key
- **Vector Search**: Perform semantic search across your data
- **SQLite Compatibility**: Use familiar SQLite API

## Installation

1. Install dependencies:
   ```bash
   npm install
   ```

2. Compile the extension:
   ```bash
   npm run compile
   ```

3. Press F5 to run the extension in a new Extension Development Host window

## Usage

1. Open the Command Palette (Ctrl+Shift+P)
2. Run one of these commands:
   - `Rusty Gun Example: Hello World` - Shows a hello world message
   - `Rusty Gun Example: Store Data` - Store data in Rusty Gun
   - `Rusty Gun Example: Retrieve Data` - Retrieve data from Rusty Gun
   - `Rusty Gun Example: Search Data` - Search data with vector search

## Code Example

```typescript
import { SQLiteCompatibleAPI } from 'rusty-gun';

// Initialize database
const db = new SQLiteCompatibleAPI({
    config: {
        dataDir: path.join(context.globalStorageUri.fsPath, 'rusty-gun'),
        port: 34567,
        host: 'localhost'
    }
});

// Start database
await db.start();

// Store data
await db.put('user:123', { name: 'John', email: 'john@example.com' });

// Retrieve data
const user = await db.getValue('user:123');

// Vector search
const results = await db.vectorSearch('machine learning', 10);

// SQL queries
await db.exec('CREATE TABLE users (id TEXT, name TEXT)');
const users = await db.all('SELECT * FROM users');
```

## Migration from SQLite

If you're migrating from SQLite, the API is nearly identical:

```typescript
// Before (SQLite)
import sqlite3 from 'sqlite3';
const db = new sqlite3.Database('./data.db');

// After (Rusty Gun)
import { SQLiteCompatibleAPI } from 'rusty-gun';
const db = new SQLiteCompatibleAPI();
await db.start();
```

## Benefits of Rusty Gun

- **P2P Sync**: Share data across devices
- **Offline-First**: Work without internet
- **Vector Search**: Semantic search capabilities
- **Encrypted Sharing**: Secure data sharing
- **SQLite Compatibility**: Easy migration

## Learn More

- [Rusty Gun Documentation](../../README.md)
- [VSCode Extension API](https://code.visualstudio.com/api)
- [Migration Guide](../../docs/VSCODE_MIGRATION.md)


