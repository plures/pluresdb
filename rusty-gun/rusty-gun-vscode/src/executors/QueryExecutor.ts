import * as vscode from 'vscode';
import { RustyGunClient } from '../client/RustyGunClient';

export class QueryExecutor {
    constructor(private client: RustyGunClient) {}

    async executeQuery(): Promise<void> {
        const editor = vscode.window.activeTextEditor;
        if (!editor) {
            vscode.window.showErrorMessage('No active editor found');
            return;
        }

        const selection = editor.selection;
        const text = editor.document.getText(selection.isEmpty ? undefined : selection);

        if (!text.trim()) {
            vscode.window.showErrorMessage('No SQL query selected or found');
            return;
        }

        try {
            const result = await this.client.executeSQL(text);
            await this.showQueryResult(result);
        } catch (error) {
            vscode.window.showErrorMessage(`Query execution failed: ${error}`);
        }
    }

    private async showQueryResult(result: any): Promise<void> {
        const panel = vscode.window.createWebviewPanel(
            'rusty-gun-query-result',
            'Query Result',
            vscode.ViewColumn.Beside,
            {
                enableScripts: true,
                retainContextWhenHidden: true
            }
        );

        const html = this.generateResultHtml(result);
        panel.webview.html = html;
    }

    private generateResultHtml(result: any): string {
        const rows = result.rows || [];
        const columns = result.columns || [];
        
        let tableHtml = '';
        if (rows.length > 0) {
            tableHtml = `
                <table style="width: 100%; border-collapse: collapse; margin: 20px 0;">
                    <thead>
                        <tr style="background-color: #f5f5f5;">
                            ${columns.map((col: string) => `<th style="border: 1px solid #ddd; padding: 8px; text-align: left;">${col}</th>`).join('')}
                        </tr>
                    </thead>
                    <tbody>
                        ${rows.map((row: any[]) => `
                            <tr>
                                ${row.map((cell: any) => `<td style="border: 1px solid #ddd; padding: 8px;">${this.formatCell(cell)}</td>`).join('')}
                            </tr>
                        `).join('')}
                    </tbody>
                </table>
            `;
        }

        return `
            <!DOCTYPE html>
            <html>
            <head>
                <meta charset="UTF-8">
                <meta name="viewport" content="width=device-width, initial-scale=1.0">
                <title>Query Result</title>
                <style>
                    body {
                        font-family: var(--vscode-font-family);
                        font-size: var(--vscode-font-size);
                        color: var(--vscode-foreground);
                        background-color: var(--vscode-editor-background);
                        margin: 20px;
                    }
                    .header {
                        margin-bottom: 20px;
                        padding: 10px;
                        background-color: var(--vscode-editor-background);
                        border: 1px solid var(--vscode-panel-border);
                        border-radius: 4px;
                    }
                    .stats {
                        display: flex;
                        gap: 20px;
                        margin-bottom: 20px;
                    }
                    .stat {
                        padding: 10px;
                        background-color: var(--vscode-editor-background);
                        border: 1px solid var(--vscode-panel-border);
                        border-radius: 4px;
                    }
                    .stat-label {
                        font-size: 12px;
                        color: var(--vscode-descriptionForeground);
                    }
                    .stat-value {
                        font-size: 18px;
                        font-weight: bold;
                        color: var(--vscode-foreground);
                    }
                    table {
                        width: 100%;
                        border-collapse: collapse;
                        margin: 20px 0;
                    }
                    th, td {
                        border: 1px solid var(--vscode-panel-border);
                        padding: 8px;
                        text-align: left;
                    }
                    th {
                        background-color: var(--vscode-panel-background);
                        font-weight: bold;
                    }
                    tr:nth-child(even) {
                        background-color: var(--vscode-panel-background);
                    }
                    .no-results {
                        text-align: center;
                        padding: 40px;
                        color: var(--vscode-descriptionForeground);
                    }
                </style>
            </head>
            <body>
                <div class="header">
                    <h2>Query Result</h2>
                    <p>Query executed successfully</p>
                </div>
                
                <div class="stats">
                    <div class="stat">
                        <div class="stat-label">Rows</div>
                        <div class="stat-value">${rows.length}</div>
                    </div>
                    <div class="stat">
                        <div class="stat-label">Columns</div>
                        <div class="stat-value">${columns.length}</div>
                    </div>
                    <div class="stat">
                        <div class="stat-label">Changes</div>
                        <div class="stat-value">${result.changes || 0}</div>
                    </div>
                </div>

                ${rows.length > 0 ? tableHtml : '<div class="no-results">No results returned</div>'}
            </body>
            </html>
        `;
    }

    private formatCell(cell: any): string {
        if (cell === null || cell === undefined) {
            return '<em>NULL</em>';
        }
        if (typeof cell === 'object') {
            return JSON.stringify(cell, null, 2);
        }
        return String(cell);
    }
}


