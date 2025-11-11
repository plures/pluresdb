import * as vscode from "vscode";
import { Node, Relationship, RustyGunClient } from "../client/RustyGunClient";

export class RustyGunProvider implements vscode.TreeDataProvider<RustyGunItem> {
  private _onDidChangeTreeData: vscode.EventEmitter<
    RustyGunItem | undefined | null | void
  > = new vscode.EventEmitter<RustyGunItem | undefined | null | void>();
  readonly onDidChangeTreeData: vscode.Event<
    RustyGunItem | undefined | null | void
  > = this._onDidChangeTreeData.event;

  private nodes: Node[] = [];
  private relationships: Relationship[] = [];
  private isLoading = false;

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
        this.client.getRelationships(),
      ]);

      this.nodes = nodes;
      this.relationships = relationships;
    } catch (error) {
      console.error("Failed to load data:", error);
    } finally {
      this.isLoading = false;
      this._onDidChangeTreeData.fire();
    }
  }

  getTreeItem(element: RustyGunItem): vscode.TreeItem {
    return element;
  }

  getChildren(element?: RustyGunItem): Thenable<RustyGunItem[]> {
    if (!this.client.isConnected()) {
      return Promise.resolve([
        new RustyGunItem(
          "Connect to Rusty Gun",
          "Connect to start using the database",
          vscode.TreeItemCollapsibleState.None,
          "connect",
          { command: "rusty-gun.connect", title: "Connect to Rusty Gun" },
        ),
      ]);
    }

    if (this.isLoading) {
      return Promise.resolve([
        new RustyGunItem(
          "Loading...",
          "Loading database data",
          vscode.TreeItemCollapsibleState.None,
          "loading",
          undefined,
          undefined,
          true,
        ),
      ]);
    }

    if (!element) {
      // Root level items
      return Promise.resolve([
        new RustyGunItem(
          "Nodes",
          `${this.nodes.length} nodes`,
          vscode.TreeItemCollapsibleState.Expanded,
          "nodes",
          undefined,
          this.nodes,
        ),
        new RustyGunItem(
          "Relationships",
          `${this.relationships.length} relationships`,
          vscode.TreeItemCollapsibleState.Expanded,
          "relationships",
          undefined,
          this.relationships,
        ),
        new RustyGunItem(
          "Graph Stats",
          "View graph statistics",
          vscode.TreeItemCollapsibleState.None,
          "stats",
          { command: "rusty-gun.openDashboard", title: "Open Dashboard" },
        ),
      ]);
    }

    if (element.type === "nodes") {
      // Node items
      return Promise.resolve(
        this.nodes.map((node) =>
          new RustyGunItem(
            node.data?.name || node.data?.title || node.id,
            `${node.data?.type || "Unknown"} • ${
              new Date(node.created_at).toLocaleDateString()
            }`,
            vscode.TreeItemCollapsibleState.Collapsed,
            "node",
            undefined,
            node,
          )
        ),
      );
    }

    if (element.type === "relationships") {
      // Relationship items
      return Promise.resolve(
        this.relationships.map((rel) =>
          new RustyGunItem(
            `${rel.from} → ${rel.to}`,
            `${rel.relation_type} • ${
              new Date(rel.created_at).toLocaleDateString()
            }`,
            vscode.TreeItemCollapsibleState.None,
            "relationship",
            undefined,
            rel,
          )
        ),
      );
    }

    if (element.type === "node") {
      // Node details
      const node = element.data as Node;
      return Promise.resolve([
        new RustyGunItem(
          "Data",
          JSON.stringify(node.data, null, 2),
          vscode.TreeItemCollapsibleState.None,
          "data",
          { command: "rusty-gun.viewNode", title: "View Node" },
          node,
        ),
        new RustyGunItem(
          "Metadata",
          JSON.stringify(node.metadata, null, 2),
          vscode.TreeItemCollapsibleState.None,
          "metadata",
          { command: "rusty-gun.viewNode", title: "View Node" },
          node,
        ),
        new RustyGunItem(
          "Tags",
          node.tags.join(", "),
          vscode.TreeItemCollapsibleState.None,
          "tags",
          { command: "rusty-gun.viewNode", title: "View Node" },
          node,
        ),
        new RustyGunItem(
          "Created",
          new Date(node.created_at).toLocaleString(),
          vscode.TreeItemCollapsibleState.None,
          "created",
          { command: "rusty-gun.viewNode", title: "View Node" },
          node,
        ),
        new RustyGunItem(
          "Updated",
          new Date(node.updated_at).toLocaleString(),
          vscode.TreeItemCollapsibleState.None,
          "updated",
          { command: "rusty-gun.viewNode", title: "View Node" },
          node,
        ),
      ]);
    }

    return Promise.resolve([]);
  }
}

export class RustyGunItem extends vscode.TreeItem {
  constructor(
    public readonly label: string,
    public readonly description: string,
    public readonly collapsibleState: vscode.TreeItemCollapsibleState,
    public readonly type: string,
    public readonly command?: vscode.Command,
    public readonly data?: any,
    public readonly isLoading = false,
  ) {
    super(label, collapsibleState);

    this.tooltip = description;
    this.contextValue = type;

    if (isLoading) {
      this.iconPath = new vscode.ThemeIcon("loading~spin");
    } else {
      this.iconPath = this.getIconForType(type);
    }

    if (command) {
      this.command = command;
    }
  }

  private getIconForType(type: string): vscode.ThemeIcon {
    switch (type) {
      case "connect":
        return new vscode.ThemeIcon("plug");
      case "loading":
        return new vscode.ThemeIcon("loading~spin");
      case "nodes":
        return new vscode.ThemeIcon("database");
      case "relationships":
        return new vscode.ThemeIcon("git-branch");
      case "stats":
        return new vscode.ThemeIcon("graph");
      case "node":
        return new vscode.ThemeIcon("circle");
      case "relationship":
        return new vscode.ThemeIcon("arrow-right");
      case "data":
        return new vscode.ThemeIcon("json");
      case "metadata":
        return new vscode.ThemeIcon("info");
      case "tags":
        return new vscode.ThemeIcon("tag");
      case "created":
      case "updated":
        return new vscode.ThemeIcon("calendar");
      default:
        return new vscode.ThemeIcon("file");
    }
  }
}
