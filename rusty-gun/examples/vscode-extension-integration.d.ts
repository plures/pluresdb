/**
 * VSCode Extension Integration Example
 * This shows how to integrate Rusty Gun into a VSCode extension
 */
import * as vscode from 'vscode';
export declare class RustyGunExtension {
    private rustyGun;
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
export declare function activate(context: vscode.ExtensionContext): RustyGunExtension;
export declare function deactivate(extension: RustyGunExtension): void;
//# sourceMappingURL=vscode-extension-integration.d.ts.map