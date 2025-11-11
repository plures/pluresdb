import * as vscode from "vscode";

export class StatusBarManager {
  private statusBarItem: vscode.StatusBarItem;
  private connectionStatusItem: vscode.StatusBarItem;

  constructor() {
    this.statusBarItem = vscode.window.createStatusBarItem(
      vscode.StatusBarAlignment.Left,
      100,
    );
    this.connectionStatusItem = vscode.window.createStatusBarItem(
      vscode.StatusBarAlignment.Left,
      99,
    );
  }

  initialize(): void {
    this.statusBarItem.text = "Rusty Gun";
    this.statusBarItem.command = "rusty-gun.openDashboard";
    this.statusBarItem.show();

    this.updateConnectionStatus(false);
  }

  updateConnectionStatus(connected: boolean): void {
    if (connected) {
      this.connectionStatusItem.text = "$(plug) Connected";
      this.connectionStatusItem.color = "#4CAF50";
      this.connectionStatusItem.tooltip = "Connected to Rusty Gun server";
      this.connectionStatusItem.command = "rusty-gun.disconnect";
    } else {
      this.connectionStatusItem.text = "$(plug) Disconnected";
      this.connectionStatusItem.color = "#F44336";
      this.connectionStatusItem.tooltip = "Disconnected from Rusty Gun server";
      this.connectionStatusItem.command = "rusty-gun.connect";
    }
    this.connectionStatusItem.show();
  }

  updateStatus(message: string, icon?: string): void {
    this.statusBarItem.text = `${icon ? icon + " " : ""}${message}`;
  }

  dispose(): void {
    this.statusBarItem.dispose();
    this.connectionStatusItem.dispose();
  }
}
