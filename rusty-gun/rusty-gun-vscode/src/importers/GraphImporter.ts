import * as vscode from "vscode";
import { RustyGunClient } from "../client/RustyGunClient";

export class GraphImporter {
  constructor(private client: RustyGunClient) {}

  async importGraph(): Promise<void> {
    if (!this.client.isConnected()) {
      vscode.window.showErrorMessage("Not connected to Rusty Gun server");
      return;
    }

    const uri = await vscode.window.showOpenDialog({
      canSelectFiles: true,
      canSelectFolders: false,
      canSelectMany: false,
      filters: {
        "JSON Files": ["json"],
        "All Files": ["*"],
      },
    });

    if (!uri || uri.length === 0) return;

    try {
      const fileData = await vscode.workspace.fs.readFile(uri[0]);
      const jsonData = JSON.parse(fileData.toString());

      await this.client.importGraph(jsonData);
      vscode.window.showInformationMessage(
        `Graph imported successfully from ${uri[0].fsPath}`,
      );
      vscode.commands.executeCommand("rusty-gun.refreshExplorer");
    } catch (error) {
      vscode.window.showErrorMessage(`Failed to import graph: ${error}`);
    }
  }
}
