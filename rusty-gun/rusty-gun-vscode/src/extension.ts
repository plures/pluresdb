import * as vscode from "vscode";
import { RustyGunProvider } from "./providers/RustyGunProvider";
import { VectorSearchProvider } from "./providers/VectorSearchProvider";
import { GraphViewProvider } from "./providers/GraphViewProvider";
import { AnalyticsProvider } from "./providers/AnalyticsProvider";
import { RustyGunClient } from "./client/RustyGunClient";
import { DashboardWebview } from "./webview/DashboardWebview";
import { QueryExecutor } from "./executors/QueryExecutor";
import { NodeCreator } from "./creators/NodeCreator";
import { RelationshipCreator } from "./creators/RelationshipCreator";
import { GraphExporter } from "./exporters/GraphExporter";
import { GraphImporter } from "./importers/GraphImporter";
import { StatusBarManager } from "./ui/StatusBarManager";
import { NotificationManager } from "./ui/NotificationManager";
import { ConfigurationManager } from "./config/ConfigurationManager";

let rustyGunClient: RustyGunClient;
let statusBarManager: StatusBarManager;
let notificationManager: NotificationManager;
let configManager: ConfigurationManager;

export async function activate(context: vscode.ExtensionContext) {
  console.log("Rusty Gun extension is now active!");

  // Initialize managers
  configManager = new ConfigurationManager();
  notificationManager = new NotificationManager();
  statusBarManager = new StatusBarManager();

  // Initialize Rusty Gun client
  rustyGunClient = new RustyGunClient(configManager, notificationManager);

  // Initialize providers
  const rustyGunProvider = new RustyGunProvider(rustyGunClient);
  const vectorSearchProvider = new VectorSearchProvider(rustyGunClient);
  const graphViewProvider = new GraphViewProvider(rustyGunClient);
  const analyticsProvider = new AnalyticsProvider(rustyGunClient);

  // Initialize webview
  const dashboardWebview = new DashboardWebview(
    context.extensionUri,
    rustyGunClient,
  );

  // Initialize executors and creators
  const queryExecutor = new QueryExecutor(rustyGunClient);
  const nodeCreator = new NodeCreator(rustyGunClient);
  const relationshipCreator = new RelationshipCreator(rustyGunClient);
  const graphExporter = new GraphExporter(rustyGunClient);
  const graphImporter = new GraphImporter(rustyGunClient);

  // Register tree data providers
  vscode.window.registerTreeDataProvider(
    "rusty-gun.explorer",
    rustyGunProvider,
  );
  vscode.window.registerTreeDataProvider(
    "rusty-gun.vectorSearch",
    vectorSearchProvider,
  );
  vscode.window.registerTreeDataProvider(
    "rusty-gun.graphView",
    graphViewProvider,
  );
  vscode.window.registerTreeDataProvider(
    "rusty-gun.analytics",
    analyticsProvider,
  );

  // Register commands
  const commands = [
    // Connection commands
    vscode.commands.registerCommand("rusty-gun.connect", async () => {
      await rustyGunClient.connect();
      statusBarManager.updateConnectionStatus(true);
      vscode.commands.executeCommand("rusty-gun.refreshExplorer");
    }),

    vscode.commands.registerCommand("rusty-gun.disconnect", async () => {
      await rustyGunClient.disconnect();
      statusBarManager.updateConnectionStatus(false);
      vscode.commands.executeCommand("rusty-gun.refreshExplorer");
    }),

    // Dashboard command
    vscode.commands.registerCommand("rusty-gun.openDashboard", () => {
      dashboardWebview.show();
    }),

    // Query execution
    vscode.commands.registerCommand("rusty-gun.executeQuery", async () => {
      await queryExecutor.executeQuery();
    }),

    // Vector search
    vscode.commands.registerCommand("rusty-gun.vectorSearch", async () => {
      await vectorSearchProvider.showSearchDialog();
    }),

    // Node operations
    vscode.commands.registerCommand("rusty-gun.createNode", async (context) => {
      await nodeCreator.createNode(context);
    }),

    vscode.commands.registerCommand("rusty-gun.editNode", async (node) => {
      await nodeCreator.editNode(node);
    }),

    vscode.commands.registerCommand("rusty-gun.deleteNode", async (node) => {
      await nodeCreator.deleteNode(node);
    }),

    // Relationship operations
    vscode.commands.registerCommand(
      "rusty-gun.createRelationship",
      async (context) => {
        await relationshipCreator.createRelationship(context);
      },
    ),

    vscode.commands.registerCommand(
      "rusty-gun.editRelationship",
      async (relationship) => {
        await relationshipCreator.editRelationship(relationship);
      },
    ),

    vscode.commands.registerCommand(
      "rusty-gun.deleteRelationship",
      async (relationship) => {
        await relationshipCreator.deleteRelationship(relationship);
      },
    ),

    // Explorer refresh
    vscode.commands.registerCommand("rusty-gun.refreshExplorer", () => {
      rustyGunProvider.refresh();
      vectorSearchProvider.refresh();
      graphViewProvider.refresh();
      analyticsProvider.refresh();
    }),

    // Export/Import
    vscode.commands.registerCommand("rusty-gun.exportGraph", async () => {
      await graphExporter.exportGraph();
    }),

    vscode.commands.registerCommand("rusty-gun.importGraph", async () => {
      await graphImporter.importGraph();
    }),

    // Node context menu commands
    vscode.commands.registerCommand("rusty-gun.viewNode", async (node) => {
      await nodeCreator.viewNode(node);
    }),

    vscode.commands.registerCommand("rusty-gun.copyNodeId", async (node) => {
      await vscode.env.clipboard.writeText(node.id);
      notificationManager.showInfo("Node ID copied to clipboard");
    }),

    // Relationship context menu commands
    vscode.commands.registerCommand(
      "rusty-gun.viewRelationship",
      async (relationship) => {
        await relationshipCreator.viewRelationship(relationship);
      },
    ),

    // Vector search context menu
    vscode.commands.registerCommand("rusty-gun.searchSimilar", async (item) => {
      await vectorSearchProvider.searchSimilar(item);
    }),

    // Graph view commands
    vscode.commands.registerCommand("rusty-gun.zoomToFit", () => {
      graphViewProvider.zoomToFit();
    }),

    vscode.commands.registerCommand("rusty-gun.centerGraph", () => {
      graphViewProvider.centerGraph();
    }),

    vscode.commands.registerCommand("rusty-gun.toggleLayout", () => {
      graphViewProvider.toggleLayout();
    }),
  ];

  // Register all commands
  context.subscriptions.push(...commands);

  // Register webview provider
  context.subscriptions.push(
    vscode.window.registerWebviewViewProvider(
      "rusty-gun.dashboard",
      dashboardWebview,
    ),
  );

  // Register configuration change listener
  context.subscriptions.push(
    vscode.workspace.onDidChangeConfiguration((e) => {
      if (e.affectsConfiguration("rusty-gun")) {
        configManager.reload();
        rustyGunClient.updateConfiguration();
      }
    }),
  );

  // Auto-connect if enabled
  if (configManager.get("autoConnect", true)) {
    try {
      await rustyGunClient.connect();
      statusBarManager.updateConnectionStatus(true);
    } catch (error) {
      console.warn("Failed to auto-connect to Rusty Gun:", error);
    }
  }

  // Initialize status bar
  statusBarManager.initialize();
  statusBarManager.updateConnectionStatus(rustyGunClient.isConnected());

  // Show welcome message
  notificationManager.showInfo(
    "Rusty Gun extension activated! Use the command palette to get started.",
  );
}

export function deactivate() {
  if (rustyGunClient) {
    rustyGunClient.disconnect();
  }
  if (statusBarManager) {
    statusBarManager.dispose();
  }
  if (notificationManager) {
    notificationManager.dispose();
  }
}
