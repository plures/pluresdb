import * as vscode from 'vscode';
import { RustyGunClient, Relationship } from '../client/RustyGunClient';

export class RelationshipCreator {
    constructor(private client: RustyGunClient) {}

    async createRelationship(context?: any): Promise<void> {
        if (!this.client.isConnected()) {
            vscode.window.showErrorMessage('Not connected to Rusty Gun server');
            return;
        }

        const fromNodeId = await vscode.window.showInputBox({
            prompt: 'Enter source node ID',
            placeHolder: 'Source node ID'
        });

        if (fromNodeId === undefined) return;

        const toNodeId = await vscode.window.showInputBox({
            prompt: 'Enter target node ID',
            placeHolder: 'Target node ID'
        });

        if (toNodeId === undefined) return;

        const relationType = await vscode.window.showInputBox({
            prompt: 'Enter relationship type',
            placeHolder: 'e.g., related_to, works_with, contains'
        });

        if (relationType === undefined) return;

        const relationshipData: Partial<Relationship> = {
            from: fromNodeId,
            to: toNodeId,
            relation_type: relationType,
            metadata: {
                created_via: 'vscode-extension',
                created_at: new Date().toISOString()
            }
        };

        try {
            const createdRelationship = await this.client.createRelationship(relationshipData);
            vscode.window.showInformationMessage(`Relationship created successfully: ${createdRelationship.id}`);
            vscode.commands.executeCommand('rusty-gun.refreshExplorer');
        } catch (error) {
            vscode.window.showErrorMessage(`Failed to create relationship: ${error}`);
        }
    }

    async editRelationship(relationship: Relationship): Promise<void> {
        if (!this.client.isConnected()) {
            vscode.window.showErrorMessage('Not connected to Rusty Gun server');
            return;
        }

        const newType = await vscode.window.showInputBox({
            prompt: 'Enter new relationship type',
            value: relationship.relation_type
        });

        if (newType === undefined) return;

        try {
            await this.client.updateRelationship(relationship.id, { relation_type: newType });
            vscode.window.showInformationMessage(`Relationship updated successfully: ${relationship.id}`);
            vscode.commands.executeCommand('rusty-gun.refreshExplorer');
        } catch (error) {
            vscode.window.showErrorMessage(`Failed to update relationship: ${error}`);
        }
    }

    async deleteRelationship(relationship: Relationship): Promise<void> {
        if (!this.client.isConnected()) {
            vscode.window.showErrorMessage('Not connected to Rusty Gun server');
            return;
        }

        const confirm = await vscode.window.showWarningMessage(
            `Are you sure you want to delete relationship "${relationship.from} → ${relationship.to}"?`,
            { modal: true },
            'Delete'
        );

        if (confirm !== 'Delete') return;

        try {
            await this.client.deleteRelationship(relationship.id);
            vscode.window.showInformationMessage(`Relationship deleted successfully: ${relationship.id}`);
            vscode.commands.executeCommand('rusty-gun.refreshExplorer');
        } catch (error) {
            vscode.window.showErrorMessage(`Failed to delete relationship: ${error}`);
        }
    }

    async viewRelationship(relationship: Relationship): Promise<void> {
        const panel = vscode.window.createWebviewPanel(
            'rusty-gun-relationship-details',
            `Relationship: ${relationship.from} → ${relationship.to}`,
            vscode.ViewColumn.Beside,
            {
                enableScripts: true,
                retainContextWhenHidden: true
            }
        );

        const html = this.generateRelationshipDetailsHtml(relationship);
        panel.webview.html = html;
    }

    private generateRelationshipDetailsHtml(relationship: Relationship): string {
        return `
            <!DOCTYPE html>
            <html>
            <head>
                <meta charset="UTF-8">
                <meta name="viewport" content="width=device-width, initial-scale=1.0">
                <title>Relationship Details</title>
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
                        padding: 20px;
                        background-color: var(--vscode-panel-background);
                        border: 1px solid var(--vscode-panel-border);
                        border-radius: 4px;
                    }
                    .section {
                        margin-bottom: 20px;
                        padding: 15px;
                        background-color: var(--vscode-panel-background);
                        border: 1px solid var(--vscode-panel-border);
                        border-radius: 4px;
                    }
                    .section h3 {
                        margin-top: 0;
                        color: var(--vscode-foreground);
                    }
                    .json {
                        background-color: var(--vscode-editor-background);
                        border: 1px solid var(--vscode-panel-border);
                        border-radius: 4px;
                        padding: 10px;
                        font-family: var(--vscode-editor-font-family);
                        font-size: var(--vscode-editor-font-size);
                        white-space: pre-wrap;
                        overflow-x: auto;
                    }
                    .metadata {
                        display: grid;
                        grid-template-columns: 1fr 1fr;
                        gap: 10px;
                    }
                    .metadata-item {
                        display: flex;
                        flex-direction: column;
                    }
                    .metadata-label {
                        font-size: 12px;
                        color: var(--vscode-descriptionForeground);
                        margin-bottom: 2px;
                    }
                    .metadata-value {
                        font-weight: bold;
                    }
                </style>
            </head>
            <body>
                <div class="header">
                    <h1>Relationship Details</h1>
                    <div class="metadata">
                        <div class="metadata-item">
                            <div class="metadata-label">ID</div>
                            <div class="metadata-value">${relationship.id}</div>
                        </div>
                        <div class="metadata-item">
                            <div class="metadata-label">Type</div>
                            <div class="metadata-value">${relationship.relation_type}</div>
                        </div>
                        <div class="metadata-item">
                            <div class="metadata-label">From</div>
                            <div class="metadata-value">${relationship.from}</div>
                        </div>
                        <div class="metadata-item">
                            <div class="metadata-label">To</div>
                            <div class="metadata-value">${relationship.to}</div>
                        </div>
                        <div class="metadata-item">
                            <div class="metadata-label">Created</div>
                            <div class="metadata-value">${new Date(relationship.created_at).toLocaleString()}</div>
                        </div>
                    </div>
                </div>

                <div class="section">
                    <h3>Metadata</h3>
                    <div class="json">${JSON.stringify(relationship.metadata, null, 2)}</div>
                </div>
            </body>
            </html>
        `;
    }
}


