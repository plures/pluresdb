import * as vscode from "vscode";
import { RustyGunClient } from "../client/RustyGunClient";

export class DashboardWebview implements vscode.WebviewViewProvider {
  public static readonly viewType = "rusty-gun.dashboard";

  constructor(
    private readonly extensionUri: vscode.Uri,
    private readonly client: RustyGunClient,
  ) {}

  public resolveWebviewView(
    webviewView: vscode.WebviewView,
    context: vscode.WebviewViewResolveContext,
    _token: vscode.CancellationToken,
  ) {
    webviewView.webview.options = {
      enableScripts: true,
      localResourceRoots: [this.extensionUri],
    };

    webviewView.webview.html = this.getHtmlForWebview(webviewView.webview);

    // Handle messages from the webview
    webviewView.webview.onDidReceiveMessage(
      (message) => {
        switch (message.command) {
          case "refresh":
            this.refreshData(webviewView);
            break;
          case "connect":
            this.client.connect();
            break;
          case "disconnect":
            this.client.disconnect();
            break;
        }
      },
      undefined,
      [],
    );

    // Refresh data when webview becomes visible
    webviewView.onDidChangeVisibility(() => {
      if (webviewView.visible) {
        this.refreshData(webviewView);
      }
    });
  }

  public show(): void {
    vscode.commands.executeCommand("rusty-gun.dashboard.focus");
  }

  private async refreshData(webviewView: vscode.WebviewView): Promise<void> {
    if (!this.client.isConnected()) {
      webviewView.webview.postMessage({
        type: "disconnected",
        data: null,
      });
      return;
    }

    try {
      const [graphStats, vectorStats, serverStatus] = await Promise.all([
        this.client.getGraphStats(),
        this.client.getVectorStats(),
        this.client.getServerStatus(),
      ]);

      webviewView.webview.postMessage({
        type: "data",
        data: {
          graphStats,
          vectorStats,
          serverStatus,
        },
      });
    } catch (error) {
      console.error("Failed to refresh dashboard data:", error);
      webviewView.webview.postMessage({
        type: "error",
        data: { message: "Failed to load dashboard data" },
      });
    }
  }

  private getHtmlForWebview(webview: vscode.Webview): string {
    return `<!DOCTYPE html>
        <html lang="en">
        <head>
            <meta charset="UTF-8">
            <meta name="viewport" content="width=device-width, initial-scale=1.0">
            <title>Rusty Gun Dashboard</title>
            <style>
                body {
                    font-family: var(--vscode-font-family);
                    font-size: var(--vscode-font-size);
                    color: var(--vscode-foreground);
                    background-color: var(--vscode-editor-background);
                    margin: 0;
                    padding: 20px;
                }
                .header {
                    text-align: center;
                    margin-bottom: 20px;
                }
                .status {
                    display: flex;
                    align-items: center;
                    justify-content: center;
                    gap: 10px;
                    margin-bottom: 20px;
                    padding: 10px;
                    border-radius: 4px;
                    background-color: var(--vscode-panel-background);
                    border: 1px solid var(--vscode-panel-border);
                }
                .status-indicator {
                    width: 8px;
                    height: 8px;
                    border-radius: 50%;
                }
                .status-connected {
                    background-color: #4CAF50;
                }
                .status-disconnected {
                    background-color: #F44336;
                }
                .stats-grid {
                    display: grid;
                    grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
                    gap: 15px;
                    margin-bottom: 20px;
                }
                .stat-card {
                    background-color: var(--vscode-panel-background);
                    border: 1px solid var(--vscode-panel-border);
                    border-radius: 4px;
                    padding: 15px;
                    text-align: center;
                }
                .stat-value {
                    font-size: 24px;
                    font-weight: bold;
                    color: var(--vscode-foreground);
                    margin-bottom: 5px;
                }
                .stat-label {
                    font-size: 12px;
                    color: var(--vscode-descriptionForeground);
                }
                .actions {
                    display: flex;
                    gap: 10px;
                    justify-content: center;
                }
                .btn {
                    padding: 8px 16px;
                    border: 1px solid var(--vscode-button-border);
                    background-color: var(--vscode-button-background);
                    color: var(--vscode-button-foreground);
                    border-radius: 4px;
                    cursor: pointer;
                    font-size: 12px;
                }
                .btn:hover {
                    background-color: var(--vscode-button-hoverBackground);
                }
                .btn:disabled {
                    opacity: 0.5;
                    cursor: not-allowed;
                }
                .loading {
                    text-align: center;
                    padding: 20px;
                    color: var(--vscode-descriptionForeground);
                }
                .error {
                    text-align: center;
                    padding: 20px;
                    color: var(--vscode-errorForeground);
                    background-color: var(--vscode-inputValidation-errorBackground);
                    border: 1px solid var(--vscode-inputValidation-errorBorder);
                    border-radius: 4px;
                }
            </style>
        </head>
        <body>
            <div class="header">
                <h2>Rusty Gun Dashboard</h2>
            </div>
            
            <div id="status" class="status">
                <div class="status-indicator status-disconnected"></div>
                <span>Disconnected</span>
            </div>
            
            <div id="content">
                <div class="loading">Loading dashboard data...</div>
            </div>
            
            <div class="actions">
                <button class="btn" onclick="connect()">Connect</button>
                <button class="btn" onclick="refresh()">Refresh</button>
            </div>

            <script>
                const vscode = acquireVsCodeApi();
                
                function connect() {
                    vscode.postMessage({ command: 'connect' });
                }
                
                function refresh() {
                    vscode.postMessage({ command: 'refresh' });
                }
                
                function updateStatus(connected) {
                    const statusEl = document.getElementById('status');
                    const indicator = statusEl.querySelector('.status-indicator');
                    const text = statusEl.querySelector('span');
                    
                    if (connected) {
                        indicator.className = 'status-indicator status-connected';
                        text.textContent = 'Connected';
                    } else {
                        indicator.className = 'status-indicator status-disconnected';
                        text.textContent = 'Disconnected';
                    }
                }
                
                function updateContent(data) {
                    const contentEl = document.getElementById('content');
                    
                    if (!data) {
                        contentEl.innerHTML = '<div class="loading">Not connected to server</div>';
                        return;
                    }
                    
                    const { graphStats, vectorStats, serverStatus } = data;
                    
                    contentEl.innerHTML = \`
                        <div class="stats-grid">
                            <div class="stat-card">
                                <div class="stat-value">\${graphStats.node_count.toLocaleString()}</div>
                                <div class="stat-label">Nodes</div>
                            </div>
                            <div class="stat-card">
                                <div class="stat-value">\${graphStats.relationship_count.toLocaleString()}</div>
                                <div class="stat-label">Relationships</div>
                            </div>
                            <div class="stat-card">
                                <div class="stat-value">\${vectorStats.vector_count.toLocaleString()}</div>
                                <div class="stat-label">Vectors</div>
                            </div>
                            <div class="stat-card">
                                <div class="stat-value">\${(graphStats.storage_size / 1024 / 1024).toFixed(1)} MB</div>
                                <div class="stat-label">Storage</div>
                            </div>
                        </div>
                    \`;
                }
                
                function showError(message) {
                    const contentEl = document.getElementById('content');
                    contentEl.innerHTML = \`<div class="error">\${message}</div>\`;
                }
                
                // Listen for messages from the extension
                window.addEventListener('message', event => {
                    const message = event.data;
                    
                    switch (message.type) {
                        case 'data':
                            updateStatus(true);
                            updateContent(message.data);
                            break;
                        case 'disconnected':
                            updateStatus(false);
                            updateContent(null);
                            break;
                        case 'error':
                            updateStatus(false);
                            showError(message.data.message);
                            break;
                    }
                });
                
                // Initial load
                refresh();
            </script>
        </body>
        </html>`;
  }
}
