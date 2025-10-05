import * as vscode from 'vscode';
import { RustyGunClient, Node, Relationship } from '../client/RustyGunClient';

export class GraphViewProvider implements vscode.TreeDataProvider<GraphViewItem> {
    private _onDidChangeTreeData: vscode.EventEmitter<GraphViewItem | undefined | null | void> = new vscode.EventEmitter<GraphViewItem | undefined | null | void>();
    readonly onDidChangeTreeData: vscode.Event<GraphViewItem | undefined | null | void> = this._onDidChangeTreeData.event;

    private nodes: Node[] = [];
    private relationships: Relationship[] = [];
    private isLoading = false;
    private currentLayout = 'cose-bilkent';

    constructor(private client: RustyGunClient) {
        this.loadData();
    }

    refresh(): void {
        this.loadData();
    }

    private async loadData(): Promise<void> {
        if (!this.client.isConnected()) {
            this.nodes = [];
            this.relationships = [];
            this._onDidChangeTreeData.fire();
            return;
        }

        this.isLoading = true;
        this._onDidChangeTreeData.fire();

        try {
            const [nodes, relationships] = await Promise.all([
                this.client.getNodes(100, 0),
                this.client.getRelationships()
            ]);

            this.nodes = nodes;
            this.relationships = relationships;
        } catch (error) {
            console.error('Failed to load graph data:', error);
        } finally {
            this.isLoading = false;
            this._onDidChangeTreeData.fire();
        }
    }

    zoomToFit(): void {
        // This would be implemented in the webview
        vscode.commands.executeCommand('rusty-gun.refreshExplorer');
    }

    centerGraph(): void {
        // This would be implemented in the webview
        vscode.commands.executeCommand('rusty-gun.refreshExplorer');
    }

    toggleLayout(): void {
        this.currentLayout = this.currentLayout === 'cose-bilkent' ? 'dagre' : 'cose-bilkent';
        vscode.commands.executeCommand('rusty-gun.refreshExplorer');
    }

    getTreeItem(element: GraphViewItem): vscode.TreeItem {
        return element;
    }

    getChildren(element?: GraphViewItem): Thenable<GraphViewItem[]> {
        if (!this.client.isConnected()) {
            return Promise.resolve([
                new GraphViewItem(
                    'Connect to Rusty Gun',
                    'Connect to start using graph view',
                    vscode.TreeItemCollapsibleState.None,
                    'connect',
                    { command: 'rusty-gun.connect', title: 'Connect to Rusty Gun' }
                )
            ]);
        }

        if (this.isLoading) {
            return Promise.resolve([
                new GraphViewItem(
                    'Loading...',
                    'Loading graph data',
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
            return Promise.resolve([
                new GraphViewItem(
                    'Graph Controls',
                    'Graph visualization controls',
                    vscode.TreeItemCollapsibleState.Expanded,
                    'controls',
                    undefined,
                    undefined
                ),
                new GraphViewItem(
                    'Nodes',
                    `${this.nodes.length} nodes`,
                    vscode.TreeItemCollapsibleState.Expanded,
                    'nodes',
                    undefined,
                    this.nodes
                ),
                new GraphViewItem(
                    'Relationships',
                    `${this.relationships.length} relationships`,
                    vscode.TreeItemCollapsibleState.Expanded,
                    'relationships',
                    undefined,
                    this.relationships
                )
            ]);
        }

        if (element.type === 'controls') {
            // Graph controls
            return Promise.resolve([
                new GraphViewItem(
                    'Zoom to Fit',
                    'Fit all nodes in view',
                    vscode.TreeItemCollapsibleState.None,
                    'zoom-fit',
                    { command: 'rusty-gun.zoomToFit', title: 'Zoom to Fit' }
                ),
                new GraphViewItem(
                    'Center Graph',
                    'Center the graph view',
                    vscode.TreeItemCollapsibleState.None,
                    'center',
                    { command: 'rusty-gun.centerGraph', title: 'Center Graph' }
                ),
                new GraphViewItem(
                    'Toggle Layout',
                    `Current: ${this.currentLayout}`,
                    vscode.TreeItemCollapsibleState.None,
                    'layout',
                    { command: 'rusty-gun.toggleLayout', title: 'Toggle Layout' }
                )
            ]);
        }

        if (element.type === 'nodes') {
            // Node items
            return Promise.resolve(
                this.nodes.map(node => new GraphViewItem(
                    node.data?.name || node.data?.title || node.id,
                    `${node.data?.type || 'Unknown'} • ${this.getNodeDegree(node.id)} connections`,
                    vscode.TreeItemCollapsibleState.Collapsed,
                    'node',
                    undefined,
                    node
                ))
            );
        }

        if (element.type === 'relationships') {
            // Relationship items
            return Promise.resolve(
                this.relationships.map(rel => new GraphViewItem(
                    `${rel.from} → ${rel.to}`,
                    `${rel.relation_type} • ${new Date(rel.created_at).toLocaleDateString()}`,
                    vscode.TreeItemCollapsibleState.None,
                    'relationship',
                    undefined,
                    rel
                ))
            );
        }

        if (element.type === 'node') {
            // Node details
            const node = element.data as Node;
            const degree = this.getNodeDegree(node.id);
            return Promise.resolve([
                new GraphViewItem(
                    'ID',
                    node.id,
                    vscode.TreeItemCollapsibleState.None,
                    'id',
                    { command: 'rusty-gun.copyNodeId', title: 'Copy Node ID' },
                    node
                ),
                new GraphViewItem(
                    'Type',
                    node.data?.type || 'Unknown',
                    vscode.TreeItemCollapsibleState.None,
                    'type',
                    undefined,
                    node
                ),
                new GraphViewItem(
                    'Connections',
                    `${degree} connections`,
                    vscode.TreeItemCollapsibleState.None,
                    'connections',
                    undefined,
                    node
                ),
                new GraphViewItem(
                    'Created',
                    new Date(node.created_at).toLocaleString(),
                    vscode.TreeItemCollapsibleState.None,
                    'created',
                    undefined,
                    node
                )
            ]);
        }

        return Promise.resolve([]);
    }

    private getNodeDegree(nodeId: string): number {
        return this.relationships.filter(rel => 
            rel.from === nodeId || rel.to === nodeId
        ).length;
    }
}

export class GraphViewItem extends vscode.TreeItem {
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
            case 'controls':
                return new vscode.ThemeIcon('settings-gear');
            case 'nodes':
                return new vscode.ThemeIcon('database');
            case 'relationships':
                return new vscode.ThemeIcon('git-branch');
            case 'node':
                return new vscode.ThemeIcon('circle');
            case 'relationship':
                return new vscode.ThemeIcon('arrow-right');
            case 'zoom-fit':
                return new vscode.ThemeIcon('zoom-in');
            case 'center':
                return new vscode.ThemeIcon('center-focus-strong');
            case 'layout':
                return new vscode.ThemeIcon('layout');
            case 'id':
                return new vscode.ThemeIcon('key');
            case 'type':
                return new vscode.ThemeIcon('tag');
            case 'connections':
                return new vscode.ThemeIcon('link');
            case 'created':
                return new vscode.ThemeIcon('calendar');
            default:
                return new vscode.ThemeIcon('file');
        }
    }
}

