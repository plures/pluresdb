import * as vscode from "vscode";

export class ConfigurationManager {
  private configuration: vscode.WorkspaceConfiguration;

  constructor() {
    this.configuration = vscode.workspace.getConfiguration("rusty-gun");
  }

  reload(): void {
    this.configuration = vscode.workspace.getConfiguration("rusty-gun");
  }

  get<T>(key: string, defaultValue: T): T {
    return this.configuration.get<T>(key, defaultValue);
  }

  update(
    key: string,
    value: any,
    target?: vscode.ConfigurationTarget,
  ): Thenable<void> {
    return this.configuration.update(key, value, target);
  }

  // Convenience methods for common configurations
  getServerUrl(): string {
    return this.get("serverUrl", "http://localhost:34569");
  }

  getAutoConnect(): boolean {
    return this.get("autoConnect", true);
  }

  getVectorSearchThreshold(): number {
    return this.get("vectorSearchThreshold", 0.3);
  }

  getMaxResults(): number {
    return this.get("maxResults", 100);
  }

  getEnableNotifications(): boolean {
    return this.get("enableNotifications", true);
  }

  getTheme(): string {
    return this.get("theme", "auto");
  }
}
