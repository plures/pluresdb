/**
 * VSCode Extension Integration Example
 * This shows how to integrate PluresDB into a VSCode extension
 */
import * as vscode from 'vscode';
export declare class PluresExtension {
    private plures;
    private sqliteAPI;
    private context;
    constructor(context: vscode.ExtensionContext);
    private setupEventHandlers;
    activate(): Promise<void>;
    deactivate(): Promise<void>;
    private registerCommands;
    private setupDatabase;
    storeSetting(key: string, value: any): Promise<any>;
    getSetting(key: string): Promise<any>;
    storeDocument(id: string, content: string, language: string, filePath: string): Promise<any>;
    searchDocuments(query: string): Promise<any>;
    executeSQL(sql: string, params?: any[]): Promise<any>;
}
export declare function activate(context: vscode.ExtensionContext): PluresExtension;
export declare function deactivate(extension: PluresExtension): void;
//# sourceMappingURL=vscode-extension-integration.d.ts.map