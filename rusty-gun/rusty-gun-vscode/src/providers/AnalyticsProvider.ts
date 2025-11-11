import * as vscode from "vscode";
import {
  GraphStats,
  RustyGunClient,
  ServerStatus,
  VectorStats,
} from "../client/RustyGunClient";

export class AnalyticsProvider
  implements vscode.TreeDataProvider<AnalyticsItem> {
  private _onDidChangeTreeData: vscode.EventEmitter<
    AnalyticsItem | undefined | null | void
  > = new vscode.EventEmitter<AnalyticsItem | undefined | null | void>();
  readonly onDidChangeTreeData: vscode.Event<
    AnalyticsItem | undefined | null | void
  > = this._onDidChangeTreeData.event;

  private graphStats: GraphStats | null = null;
  private vectorStats: VectorStats | null = null;
  private serverStatus: ServerStatus | null = null;
  private isLoading = false;

  constructor(private client: RustyGunClient) {
    this.loadData();
  }

  refresh(): void {
    this.loadData();
  }

  private async loadData(): Promise<void> {
    if (!this.client.isConnected()) {
      this.graphStats = null;
      this.vectorStats = null;
      this.serverStatus = null;
      this._onDidChangeTreeData.fire();
      return;
    }

    this.isLoading = true;
    this._onDidChangeTreeData.fire();

    try {
      const [graphStats, vectorStats, serverStatus] = await Promise.all([
        this.client.getGraphStats(),
        this.client.getVectorStats(),
        this.client.getServerStatus(),
      ]);

      this.graphStats = graphStats;
      this.vectorStats = vectorStats;
      this.serverStatus = serverStatus;
    } catch (error) {
      console.error("Failed to load analytics data:", error);
    } finally {
      this.isLoading = false;
      this._onDidChangeTreeData.fire();
    }
  }

  getTreeItem(element: AnalyticsItem): vscode.TreeItem {
    return element;
  }

  getChildren(element?: AnalyticsItem): Thenable<AnalyticsItem[]> {
    if (!this.client.isConnected()) {
      return Promise.resolve([
        new AnalyticsItem(
          "Connect to Rusty Gun",
          "Connect to start viewing analytics",
          vscode.TreeItemCollapsibleState.None,
          "connect",
          { command: "rusty-gun.connect", title: "Connect to Rusty Gun" },
        ),
      ]);
    }

    if (this.isLoading) {
      return Promise.resolve([
        new AnalyticsItem(
          "Loading...",
          "Loading analytics data",
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
        new AnalyticsItem(
          "Server Status",
          "Server health and status",
          vscode.TreeItemCollapsibleState.Expanded,
          "server-status",
          undefined,
          this.serverStatus,
        ),
        new AnalyticsItem(
          "Graph Statistics",
          "Graph database statistics",
          vscode.TreeItemCollapsibleState.Expanded,
          "graph-stats",
          undefined,
          this.graphStats,
        ),
        new AnalyticsItem(
          "Vector Statistics",
          "Vector search statistics",
          vscode.TreeItemCollapsibleState.Expanded,
          "vector-stats",
          undefined,
          this.vectorStats,
        ),
      ]);
    }

    if (element.type === "server-status") {
      // Server status details
      const status = element.data as ServerStatus;
      if (!status) return Promise.resolve([]);

      return Promise.resolve([
        new AnalyticsItem(
          "Status",
          status.status,
          vscode.TreeItemCollapsibleState.None,
          "status",
          undefined,
          status,
        ),
        new AnalyticsItem(
          "Version",
          status.version,
          vscode.TreeItemCollapsibleState.None,
          "version",
          undefined,
          status,
        ),
        new AnalyticsItem(
          "Uptime",
          `${Math.floor(status.uptime / 3600)}h ${
            Math.floor((status.uptime % 3600) / 60)
          }m`,
          vscode.TreeItemCollapsibleState.None,
          "uptime",
          undefined,
          status,
        ),
        new AnalyticsItem(
          "Services",
          "Service health status",
          vscode.TreeItemCollapsibleState.Expanded,
          "services",
          undefined,
          status.services,
        ),
      ]);
    }

    if (element.type === "services") {
      // Service health details
      const services = element.data as any;
      if (!services) return Promise.resolve([]);

      return Promise.resolve([
        new AnalyticsItem(
          "Storage",
          services.storage,
          vscode.TreeItemCollapsibleState.None,
          "service",
          undefined,
          { name: "Storage", status: services.storage },
        ),
        new AnalyticsItem(
          "Vector Search",
          services.vector_search,
          vscode.TreeItemCollapsibleState.None,
          "service",
          undefined,
          { name: "Vector Search", status: services.vector_search },
        ),
        new AnalyticsItem(
          "Network",
          services.network,
          vscode.TreeItemCollapsibleState.None,
          "service",
          undefined,
          { name: "Network", status: services.network },
        ),
        new AnalyticsItem(
          "API",
          services.api,
          vscode.TreeItemCollapsibleState.None,
          "service",
          undefined,
          { name: "API", status: services.api },
        ),
      ]);
    }

    if (element.type === "graph-stats") {
      // Graph statistics details
      const stats = element.data as GraphStats;
      if (!stats) return Promise.resolve([]);

      return Promise.resolve([
        new AnalyticsItem(
          "Node Count",
          stats.node_count.toLocaleString(),
          vscode.TreeItemCollapsibleState.None,
          "stat",
          undefined,
          stats,
        ),
        new AnalyticsItem(
          "Relationship Count",
          stats.relationship_count.toLocaleString(),
          vscode.TreeItemCollapsibleState.None,
          "stat",
          undefined,
          stats,
        ),
        new AnalyticsItem(
          "Storage Size",
          `${(stats.storage_size / 1024 / 1024).toFixed(2)} MB`,
          vscode.TreeItemCollapsibleState.None,
          "stat",
          undefined,
          stats,
        ),
        new AnalyticsItem(
          "Index Count",
          stats.index_count.toLocaleString(),
          vscode.TreeItemCollapsibleState.None,
          "stat",
          undefined,
          stats,
        ),
        new AnalyticsItem(
          "Last Updated",
          new Date(stats.last_updated).toLocaleString(),
          vscode.TreeItemCollapsibleState.None,
          "stat",
          undefined,
          stats,
        ),
      ]);
    }

    if (element.type === "vector-stats") {
      // Vector statistics details
      const stats = element.data as VectorStats;
      if (!stats) return Promise.resolve([]);

      return Promise.resolve([
        new AnalyticsItem(
          "Vector Count",
          stats.vector_count.toLocaleString(),
          vscode.TreeItemCollapsibleState.None,
          "stat",
          undefined,
          stats,
        ),
        new AnalyticsItem(
          "Dimensions",
          stats.dimensions.toString(),
          vscode.TreeItemCollapsibleState.None,
          "stat",
          undefined,
          stats,
        ),
        new AnalyticsItem(
          "Index Size",
          `${(stats.index_size / 1024 / 1024).toFixed(2)} MB`,
          vscode.TreeItemCollapsibleState.None,
          "stat",
          undefined,
          stats,
        ),
        new AnalyticsItem(
          "Cache Size",
          stats.cache_size.toLocaleString(),
          vscode.TreeItemCollapsibleState.None,
          "stat",
          undefined,
          stats,
        ),
        new AnalyticsItem(
          "Cache Hits",
          stats.cache_hits.toLocaleString(),
          vscode.TreeItemCollapsibleState.None,
          "stat",
          undefined,
          stats,
        ),
        new AnalyticsItem(
          "Cache Misses",
          stats.cache_misses.toLocaleString(),
          vscode.TreeItemCollapsibleState.None,
          "stat",
          undefined,
          stats,
        ),
        new AnalyticsItem(
          "Last Updated",
          new Date(stats.last_updated).toLocaleString(),
          vscode.TreeItemCollapsibleState.None,
          "stat",
          undefined,
          stats,
        ),
      ]);
    }

    return Promise.resolve([]);
  }
}

export class AnalyticsItem extends vscode.TreeItem {
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
      case "server-status":
        return new vscode.ThemeIcon("server");
      case "graph-stats":
        return new vscode.ThemeIcon("graph");
      case "vector-stats":
        return new vscode.ThemeIcon("search");
      case "status":
        return new vscode.ThemeIcon("check");
      case "version":
        return new vscode.ThemeIcon("tag");
      case "uptime":
        return new vscode.ThemeIcon("clock");
      case "services":
        return new vscode.ThemeIcon("gear");
      case "service":
        return new vscode.ThemeIcon("circle");
      case "stat":
        return new vscode.ThemeIcon("bar-chart");
      default:
        return new vscode.ThemeIcon("file");
    }
  }
}
