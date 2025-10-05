import * as vscode from 'vscode';
import { RustyGunClient, Node } from '../client/RustyGunClient';

export class NodeCreator {
    constructor(private client: RustyGunClient) {}

    async createNode(context?: any): Promise<void> {
        if (!this.client.isConnected()) {
            vscode.window.showErrorMessage('Not connected to Rusty Gun server');
            return;
        }

        const nodeId = await vscode.window.showInputBox({
            prompt: 'Enter node ID',
            placeHolder: 'Leave empty for auto-generated ID',
            validateInput: (value) => {
                if (value && !/^[a-zA-Z0-9_-]+$/.test(value)) {
                    return 'Node ID must contain only letters, numbers, underscores, and hyphens';
                }
                return null;
            }
        });

        if (nodeId === undefined) return;

        const nodeName = await vscode.window.showInputBox({
            prompt: 'Enter node name',
            placeHolder: 'Node name or title'
        });

        if (nodeName === undefined) return;

        const nodeType = await vscode.window.showInputBox({
            prompt: 'Enter node type',
            placeHolder: 'e.g., person, document, project',
            value: 'node'
        });

        if (nodeType === undefined) return;

        const tags = await vscode.window.showInputBox({
            prompt: 'Enter tags (comma-separated)',
            placeHolder: 'tag1, tag2, tag3'
        });

        const nodeData: Partial<Node> = {
            id: nodeId || undefined,
            data: {
                name: nodeName,
                type: nodeType,
                created_by: 'vscode-extension'
            },
            metadata: {
                created_via: 'vscode-extension',
                created_at: new Date().toISOString()
            },
            tags: tags ? tags.split(',').map(tag => tag.trim()) : []
        };

        try {
            const createdNode = await this.client.createNode(nodeData);
            vscode.window.showInformationMessage(`Node created successfully: ${createdNode.id}`);
            vscode.commands.executeCommand('rusty-gun.refreshExplorer');
        } catch (error) {
            vscode.window.showErrorMessage(`Failed to create node: ${error}`);
        }
    }

    async editNode(node: Node): Promise<void> {
        if (!this.client.isConnected()) {
            vscode.window.showErrorMessage('Not connected to Rusty Gun server');
            return;
        }

        const newName = await vscode.window.showInputBox({
            prompt: 'Enter new node name',
            value: node.data?.name || node.data?.title || node.id
        });

        if (newName === undefined) return;

        const newType = await vscode.window.showInputBox({
            prompt: 'Enter new node type',
            value: node.data?.type || 'node'
        });

        if (newType === undefined) return;

        const updatedData = {
            ...node.data,
            name: newName,
            type: newType,
            updated_at: new Date().toISOString()
        };

        try {
            await this.client.updateNode(node.id, { data: updatedData });
            vscode.window.showInformationMessage(`Node updated successfully: ${node.id}`);
            vscode.commands.executeCommand('rusty-gun.refreshExplorer');
        } catch (error) {
            vscode.window.showErrorMessage(`Failed to update node: ${error}`);
        }
    }

    async deleteNode(node: Node): Promise<void> {
        if (!this.client.isConnected()) {
            vscode.window.showErrorMessage('Not connected to Rusty Gun server');
            return;
        }

        const confirm = await vscode.window.showWarningMessage(
            `Are you sure you want to delete node "${node.id}"?`,
            { modal: true },
            'Delete'
        );

        if (confirm !== 'Delete') return;

        try {
            await this.client.deleteNode(node.id);
            vscode.window.showInformationMessage(`Node deleted successfully: ${node.id}`);
            vscode.commands.executeCommand('rusty-gun.refreshExplorer');
        } catch (error) {
            vscode.window.showErrorMessage(`Failed to delete node: ${error}`);
        }
    }

    async viewNode(node: Node): Promise<void> {
        const panel = vscode.window.createWebviewPanel(
            'rusty-gun-node-details',
            `Node: ${node.id}`,
            vscode.ViewColumn.Beside,
            {
                enableScripts: true,
                retainContextWhenHidden: true
            }
        );

        const html = this.generateNodeDetailsHtml(node);
        panel.webview.html = html;
    }

    private generateNodeDetailsHtml(node: Node): string {
        return `
            <!DOCTYPE html>
            <html>
            <head>
                <meta charset="UTF-8">
                <meta name="viewport" content="width=device-width, initial-scale=1.0">
                <title>Node Details</title>
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
                    .tags {
                        display: flex;
                        flex-wrap: wrap;
                        gap: 5px;
                    }
                    .tag {
                        background-color: var(--vscode-badge-background);
                        color: var(--vscode-badge-foreground);
                        padding: 2px 8px;
                        border-radius: 12px;
                        font-size: 12px;
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
                    <h1>Node Details</h1>
                    <div class="metadata">
                        <div class="metadata-item">
                            <div class="metadata-label">ID</div>
                            <div class="metadata-value">${node.id}</div>
                        </div>
                        <div class="metadata-item">
                            <div class="metadata-label">Type</div>
                            <div class="metadata-value">${node.data?.type || 'Unknown'}</div>
                        </div>
                        <div class="metadata-item">
                            <div class="metadata-label">Created</div>
                            <div class="metadata-value">${new Date(node.created_at).toLocaleString()}</div>
                        </div>
                        <div class="metadata-item">
                            <div class="metadata-label">Updated</div>
                            <div class="metadata-value">${new Date(node.updated_at).toLocaleString()}</div>
                        </div>
                    </div>
                </div>

                <div class="section">
                    <h3>Data</h3>
                    <div class="json">${JSON.stringify(node.data, null, 2)}</div>
                </div>

                <div class="section">
                    <h3>Metadata</h3>
                    <div class="json">${JSON.stringify(node.metadata, null, 2)}</div>
                </div>

                <div class="section">
                    <h3>Tags</h3>
                    <div class="tags">
                        ${node.tags.map(tag => `<span class="tag">${tag}</span>`).join('')}
                    </div>
                </div>
            </body>
            </html>
        `;
    }
}


