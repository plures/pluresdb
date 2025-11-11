/**
 * VSCode Extension Integration Example
 * This shows how to integrate PluresDB into a VSCode extension
 */

import * as vscode from "vscode";
import {
  createPluresExtension,
  ExtensionContextLike,
  PluresVSCodeExtension,
  VSCodeAPI,
} from "../src/vscode/extension.ts";

let extensionInstance: PluresVSCodeExtension | undefined;

export async function activate(context: vscode.ExtensionContext) {
  const mappedContext: ExtensionContextLike = {
    subscriptions: context.subscriptions,
    globalStorageUri: { fsPath: context.globalStorageUri.fsPath },
  };

  const vscodeApi = vscode as unknown as VSCodeAPI;
  extensionInstance = createPluresExtension(vscodeApi, mappedContext);

  await extensionInstance.activate();

  context.subscriptions.push({
    dispose: () => {
      extensionInstance?.deactivate().catch((error) => {
        console.error("Failed to deactivate PluresDB extension", error);
      });
    },
  });

  return extensionInstance;
}

export async function deactivate() {
  await extensionInstance?.deactivate();
  extensionInstance = undefined;
}
