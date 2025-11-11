import * as vscode from "vscode";
import { RustyGunClient } from "../client/RustyGunClient";
import { Buffer } from "node:buffer";

export class GraphExporter {
  constructor(private client: RustyGunClient) {}

  async exportGraph(): Promise<void> {
    if (!this.client.isConnected()) {
      vscode.window.showErrorMessage("Not connected to Rusty Gun server");
      return;
    }

    try {
      const data = await this.client.exportGraph();

      const uri = await vscode.window.showSaveDialog({
        defaultUri: vscode.Uri.file("rusty-gun-export.json"),
        filters: {
          "JSON Files": ["json"],
          "All Files": ["*"],
        },
      });

      if (uri) {
        const jsonData = JSON.stringify(data, null, 2);
        await vscode.workspace.fs.writeFile(uri, Buffer.from(jsonData, "utf8"));
        vscode.window.showInformationMessage(
          `Graph exported successfully to ${uri.fsPath}`,
        );
      }
    } catch (error) {
      vscode.window.showErrorMessage(`Failed to export graph: ${error}`);
    }
  }
}
