import * as vscode from 'vscode';
import { RustyGunClient, VectorSearchResult } from '../client/RustyGunClient';

export class VectorSearchProvider implements vscode.TreeDataProvider<VectorSearchItem> {
    private _onDidChangeTreeData: vscode.EventEmitter<VectorSearchItem | undefined | null | void> = new vscode.EventEmitter<VectorSearchItem | undefined | null | void>();
    readonly onDidChangeTreeData: vscode.Event<VectorSearchItem | undefined | null | void> = this._onDidChangeTreeData.event;

    private searchResults: VectorSearchResult[] = [];
    private searchQuery = '';
    private isLoading = false;

    constructor(private client: RustyGunClient) {}

    refresh(): void {
        this._onDidChangeTreeData.fire();
    }

    async showSearchDialog(): Promise<void> {
        const query = await vscode.window.showInputBox({
            prompt: 'Enter search query',
            placeHolder: 'Search for similar content...',
            value: this.searchQuery
        });

        if (query) {
            await this.performSearch(query);
        }
    }

    async searchSimilar(item: any): Promise<void> {
        if (item.text) {
            await this.performSearch(item.text);
        } else if (item.data?.name || item.data?.title) {
            await this.performSearch(item.data.name || item.data.title);
        } else {
            vscode.window.showWarningMessage('No text content found to search');
        }
    }

    private async performSearch(query: string): Promise<void> {
        if (!this.client.isConnected()) {
            vscode.window.showErrorMessage('Not connected to Rusty Gun server');
            return;
        }

        this.searchQuery = query;
        this.isLoading = true;
        this._onDidChangeTreeData.fire();

        try {
            const threshold = vscode.workspace.getConfiguration('rusty-gun').get('vectorSearchThreshold', 0.3);
            const limit = vscode.workspace.getConfiguration('rusty-gun').get('maxResults', 100);
            
            this.searchResults = await this.client.searchVectors(query, limit, threshold);
        } catch (error) {
            console.error('Vector search failed:', error);
            vscode.window.showErrorMessage(`Vector search failed: ${error}`);
            this.searchResults = [];
        } finally {
            this.isLoading = false;
            this._onDidChangeTreeData.fire();
        }
    }

    getTreeItem(element: VectorSearchItem): vscode.TreeItem {
        return element;
    }

    getChildren(element?: VectorSearchItem): Thenable<VectorSearchItem[]> {
        if (!this.client.isConnected()) {
            return Promise.resolve([
                new VectorSearchItem(
                    'Connect to Rusty Gun',
                    'Connect to start using vector search',
                    vscode.TreeItemCollapsibleState.None,
                    'connect',
                    { command: 'rusty-gun.connect', title: 'Connect to Rusty Gun' }
                )
            ]);
        }

        if (this.isLoading) {
            return Promise.resolve([
                new VectorSearchItem(
                    'Searching...',
                    'Performing vector search',
                    vscode.TreeItemCollapsibleState.None,
                    'loading',
                    undefined,
                    undefined,
                    true
                )
            ]);
        }

        if (!element) {
            // Root level items
            const items: VectorSearchItem[] = [
                new VectorSearchItem(
                    'Search',
                    'Perform vector search',
                    vscode.TreeItemCollapsibleState.None,
                    'search',
                    { command: 'rusty-gun.vectorSearch', title: 'Vector Search' }
                )
            ];

            if (this.searchQuery) {
                items.push(
                    new VectorSearchItem(
                        `Query: "${this.searchQuery}"`,
                        `${this.searchResults.length} results`,
                        vscode.TreeItemCollapsibleState.Expanded,
                        'query',
                        undefined,
                        this.searchResults
                    )
                );
            }

            return Promise.resolve(items);
        }

        if (element.type === 'query') {
            // Search results
            if (this.searchResults.length === 0) {
                return Promise.resolve([
                    new VectorSearchItem(
                        'No results found',
                        'Try adjusting your search query or threshold',
                        vscode.TreeItemCollapsibleState.None,
                        'no-results'
                    )
                ]);
            }

            return Promise.resolve(
                this.searchResults.map((result, index) => new VectorSearchItem(
                    `${index + 1}. ${result.id}`,
                    `${Math.round(result.score * 100)}% similar`,
                    vscode.TreeItemCollapsibleState.Collapsed,
                    'result',
                    undefined,
                    result
                ))
            );
        }

        if (element.type === 'result') {
            // Result details
            const result = element.data as VectorSearchResult;
            return Promise.resolve([
                new VectorSearchItem(
                    'Score',
                    `${Math.round(result.score * 100)}%`,
                    vscode.TreeItemCollapsibleState.None,
                    'score',
                    undefined,
                    result
                ),
                new VectorSearchItem(
                    'Metadata',
                    JSON.stringify(result.metadata, null, 2),
                    vscode.TreeItemCollapsibleState.None,
                    'metadata',
                    undefined,
                    result
                ),
                new VectorSearchItem(
                    'Text Hash',
                    result.text_hash,
                    vscode.TreeItemCollapsibleState.None,
                    'hash',
                    undefined,
                    result
                )
            ]);
        }

        return Promise.resolve([]);
    }
}

export class VectorSearchItem extends vscode.TreeItem {
    constructor(
        public readonly label: string,
        public readonly description: string,
        public readonly collapsibleState: vscode.TreeItemCollapsibleState,
        public readonly type: string,
        public readonly command?: vscode.Command,
        public readonly data?: any,
        public readonly isLoading = false
    ) {
        super(label, collapsibleState);

        this.tooltip = description;
        this.contextValue = type;

        if (isLoading) {
            this.iconPath = new vscode.ThemeIcon('loading~spin');
        } else {
            this.iconPath = this.getIconForType(type);
        }

        if (command) {
            this.command = command;
        }
    }

    private getIconForType(type: string): vscode.ThemeIcon {
        switch (type) {
            case 'connect':
                return new vscode.ThemeIcon('plug');
            case 'loading':
                return new vscode.ThemeIcon('loading~spin');
            case 'search':
                return new vscode.ThemeIcon('search');
            case 'query':
                return new vscode.ThemeIcon('quote');
            case 'result':
                return new vscode.ThemeIcon('file-text');
            case 'no-results':
                return new vscode.ThemeIcon('search-stop');
            case 'score':
                return new vscode.ThemeIcon('percentage');
            case 'metadata':
                return new vscode.ThemeIcon('info');
            case 'hash':
                return new vscode.ThemeIcon('key');
            default:
                return new vscode.ThemeIcon('file');
        }
    }
}

