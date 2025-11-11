import * as vscode from "vscode";

export class NotificationManager {
  private outputChannel: vscode.OutputChannel;

  constructor() {
    this.outputChannel = vscode.window.createOutputChannel("Rusty Gun");
  }

  showInfo(message: string): void {
    vscode.window.showInformationMessage(`Rusty Gun: ${message}`);
    this.log("INFO", message);
  }

  showWarning(message: string): void {
    vscode.window.showWarningMessage(`Rusty Gun: ${message}`);
    this.log("WARN", message);
  }

  showError(message: string): void {
    vscode.window.showErrorMessage(`Rusty Gun: ${message}`);
    this.log("ERROR", message);
  }

  showSuccess(message: string): void {
    vscode.window.showInformationMessage(
      `Rusty Gun: ${message}`,
      "View Details",
    ).then((selection) => {
      if (selection === "View Details") {
        this.outputChannel.show();
      }
    });
    this.log("SUCCESS", message);
  }

  log(level: string, message: string): void {
    const timestamp = new Date().toISOString();
    this.outputChannel.appendLine(`[${timestamp}] ${level}: ${message}`);
  }

  showOutput(): void {
    this.outputChannel.show();
  }

  dispose(): void {
    this.outputChannel.dispose();
  }
}
