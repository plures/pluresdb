import * as vscode from 'vscode';
import { SQLiteCompatibleAPI } from 'rusty-gun';
import * as path from 'path';

export function activate(context: vscode.ExtensionContext) {
    console.log('Rusty Gun extension is now active!');

    // Initialize Rusty Gun database
    const db = new SQLiteCompatibleAPI({
        config: {
            dataDir: path.join(context.globalStorageUri.fsPath, 'rusty-gun'),
            port: 34567,
            host: 'localhost'
        }
    });

    // Start the database
    db.start().then(() => {
        console.log('Rusty Gun database started');
        
        // Set up database schema
        setupDatabase(db);
    }).catch(error => {
        console.error('Failed to start Rusty Gun:', error);
        vscode.window.showErrorMessage('Failed to start Rusty Gun database');
    });

    // Register commands
    const helloWorldCommand = vscode.commands.registerCommand('rusty-gun-example.helloWorld', () => {
        vscode.window.showInformationMessage('Hello World from Rusty Gun!');
    });

    const storeDataCommand = vscode.commands.registerCommand('rusty-gun-example.storeData', async () => {
        const key = await vscode.window.showInputBox({
            prompt: 'Enter key to store',
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
                    await db.put(key, parsedValue);
                    vscode.window.showInformationMessage(`Stored data for key: ${key}`);
                } catch (error) {
                    vscode.window.showErrorMessage(`Failed to store data: ${error instanceof Error ? error.message : String(error)}`);
                }
            }
        }
    });

    const retrieveDataCommand = vscode.commands.registerCommand('rusty-gun-example.retrieveData', async () => {
        const key = await vscode.window.showInputBox({
            prompt: 'Enter key to retrieve',
            placeHolder: 'user:123'
        });

        if (key) {
            try {
                const value = await db.getValue(key);
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
                vscode.window.showErrorMessage(`Failed to retrieve data: ${error instanceof Error ? error.message : String(error)}`);
            }
        }
    });

    const searchDataCommand = vscode.commands.registerCommand('rusty-gun-example.searchData', async () => {
        const query = await vscode.window.showInputBox({
            prompt: 'Enter search query',
            placeHolder: 'machine learning'
        });

        if (query) {
            try {
                const results = await db.vectorSearch(query, 10);
                const doc = await vscode.workspace.openTextDocument({
                    content: JSON.stringify(results, null, 2),
                    language: 'json'
                });
                await vscode.window.showTextDocument(doc);
            } catch (error) {
                vscode.window.showErrorMessage(`Search failed: ${error instanceof Error ? error.message : String(error)}`);
            }
        }
    });

    // Register all commands
    context.subscriptions.push(
        helloWorldCommand,
        storeDataCommand,
        retrieveDataCommand,
        searchDataCommand
    );

    // Clean up on deactivation
    context.subscriptions.push({
        dispose: () => {
            db.stop().catch(console.error);
        }
    });
}

async function setupDatabase(db: SQLiteCompatibleAPI) {
    try {
        // Create tables for common VSCode extension use cases
        await db.exec(`
            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )
        `);

        await db.exec(`
            CREATE TABLE IF NOT EXISTS documents (
                id TEXT PRIMARY KEY,
                content TEXT,
                language TEXT,
                file_path TEXT,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )
        `);

        console.log('Database schema created successfully');
    } catch (error) {
        console.error('Failed to create database schema:', error);
    }
}

export function deactivate() {
    console.log('Rusty Gun extension deactivated');
}


