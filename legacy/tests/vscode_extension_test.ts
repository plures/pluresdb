// @ts-nocheck
import { assert, assertEquals, assertMatch } from "jsr:@std/assert@1.0.14";
import {
  createPluresExtension,
  ExtensionContextLike,
  PluresVSCodeExtension,
  VSCodeAPI,
} from "../vscode/extension.ts";

interface RecordedDocument {
  content: string;
  language: string;
}

interface RecordedMessage {
  type: "info" | "error";
  text: string;
}

type CommandHandler = (...args: unknown[]) => unknown;

type TestHarness = {
  vscode: VSCodeAPI;
  context: ExtensionContextLike;
  commands: Map<string, CommandHandler>;
  docs: RecordedDocument[];
  messages: RecordedMessage[];
  shownTargets: string[];
  queueInputs: (...values: (string | undefined)[]) => void;
};

function createHarness(storageDir: string): TestHarness {
  const commands = new Map<string, CommandHandler>();
  const docs: RecordedDocument[] = [];
  const messages: RecordedMessage[] = [];
  const shownTargets: string[] = [];
  const inputQueue: Array<string | undefined> = [];

  const vscode: VSCodeAPI = {
    commands: {
      registerCommand(command, handler) {
        commands.set(command, handler);
        return {
          dispose() {
            commands.delete(command);
          },
        };
      },
    },
    window: {
      async showInformationMessage(message: string) {
        messages.push({ type: "info", text: message });
      },
      async showErrorMessage(message: string) {
        messages.push({ type: "error", text: message });
      },
      async showInputBox() {
        return inputQueue.shift();
      },
      async showTextDocument(doc: unknown) {
        if (
          typeof doc === "object" && doc && "content" in doc &&
          "language" in doc
        ) {
          const record = doc as RecordedDocument;
          docs.push({ content: record.content, language: record.language });
        }
      },
    },
    workspace: {
      async openTextDocument(init) {
        docs.push(init as RecordedDocument);
        return init;
      },
    },
    env: {
      async openExternal(target) {
        shownTargets.push(
          typeof target === "string" ? target : target.toString(),
        );
      },
    },
    Uri: {
      parse(target: string) {
        return target;
      },
    },
  };

  const context: ExtensionContextLike = {
    subscriptions: [],
    globalStorageUri: { fsPath: storageDir },
  };

  return {
    vscode,
    context,
    commands,
    docs,
    messages,
    shownTargets,
    queueInputs: (...values: (string | undefined)[]) => {
      inputQueue.push(...values);
    },
  };
}

async function getFreePort(): Promise<number> {
  const listener = Deno.listen({ hostname: "127.0.0.1", port: 0 });
  const { port } = listener.addr as Deno.NetAddr;
  listener.close();
  return port;
}

async function waitFor(
  predicate: () => Promise<boolean>,
  timeout = 10_000,
  interval = 200,
) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    if (await predicate()) return;
    await new Promise((resolve) => setTimeout(resolve, interval));
  }
  throw new Error("Timed out waiting for condition");
}

async function removeDirWithRetry(target: string) {
  const delays = [0, 200, 400, 800, 1600, 3200];
  for (let i = 0; i < delays.length; i++) {
    if (delays[i] > 0) {
      await new Promise((resolve) => setTimeout(resolve, delays[i]));
    }
    try {
      await Deno.remove(target, { recursive: true });
      return;
    } catch (error) {
      const retryable = error instanceof Error &&
        /used by another process/i.test(error.message);
      if (!retryable || i === delays.length - 1) {
        throw error;
      }
    }
  }
}

Deno.test("VSCode integration dogfood workflow", async () => {
  const storageDir = await Deno.makeTempDir({ prefix: "plures-vscode-" });
  const apiPort = await getFreePort();
  const webPort = await getFreePort();
  const apiUrl = `http://127.0.0.1:${apiPort}`;

  const harness = createHarness(storageDir);
  const extension = createPluresExtension(harness.vscode, harness.context, {
    config: {
      host: "127.0.0.1",
      port: apiPort,
      webPort,
    },
  });

  try {
    try {
      await extension.activate();
    } catch (error) {
      console.error("Activation failed", harness.messages);
      throw error;
    }

    await waitFor(async () => {
      try {
        const res = await fetch(`${apiUrl}/api/list`);
        return res.ok;
      } catch {
        return false;
      }
    });

    const seed = await fetch(`${apiUrl}/api/put`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        id: "extension:vector-doc",
        data: {
          type: "VSCodeDocument",
          title: "Integration Test",
          tags: ["extension", "sqlite"],
          vector: [0.12, 0.24, 0.48, 0.96],
        },
      }),
    });
    assert(seed.ok, "Failed to seed vector test data");

    const openWebUICmd = harness.commands.get("pluresdb.openWebUI");
    if (!openWebUICmd) {
      throw new Error("openWebUI command should be registered");
    }
    await openWebUICmd();
    assertEquals(harness.shownTargets.at(-1), `http://127.0.0.1:${webPort}`);

    harness.queueInputs("SELECT name FROM pragma_table_info('settings')");
    const execQuery = harness.commands.get("pluresdb.executeQuery");
    if (!execQuery) {
      throw new Error("executeQuery command should be registered");
    }
    await execQuery();
    const schemaDoc = harness.docs.at(-1);
    if (!schemaDoc) {
      throw new Error("Expected schema document to open");
    }
    const schemaRows = JSON.parse(schemaDoc.content) as Array<
      Record<string, unknown>
    >;
    const columnNames = schemaRows.map((row) => String(row.name));
    assert(
      columnNames.includes("key"),
      "Settings table should expose key column",
    );

    harness.queueInputs(
      "user:alpha",
      '{"name":"Ada","vector":[0.2,0.1,0.3,0.4],"role":"builder"}',
    );
    const storeCommand = harness.commands.get("pluresdb.storeData");
    if (!storeCommand) {
      throw new Error("storeData command should be registered");
    }
    await storeCommand();
    assertMatch(
      harness.messages.at(-1)?.text ?? "",
      /Stored data for key: user:alpha/,
    );

    harness.queueInputs("user:alpha");
    const retrieveCommand = harness.commands.get("pluresdb.retrieveData");
    if (!retrieveCommand) {
      throw new Error("retrieveData command should be registered");
    }
    await retrieveCommand();
    const retrievedDoc = harness.docs.at(-1);
    if (!retrievedDoc) {
      throw new Error("Expected retrieved document to open");
    }
    const retrieved = JSON.parse(retrievedDoc.content);
    assertEquals(retrieved.name, "Ada");

    harness.queueInputs("sqlite builder search");
    const vectorCommand = harness.commands.get("pluresdb.vectorSearch");
    if (!vectorCommand) {
      throw new Error("vectorSearch command should be registered");
    }
    await vectorCommand();
    const vectorDoc = harness.docs.at(-1);
    if (!vectorDoc) {
      throw new Error("Expected vector search document to open");
    }
    const vectorResults = JSON.parse(vectorDoc.content) as Array<
      Record<string, unknown>
    >;
    assert(
      vectorResults.length > 0,
      "Vector search should return at least one result",
    );

    const settingsRows = await extension.executeSQL(
      "SELECT name FROM pragma_table_info('documents')",
    );
    assert(Array.isArray(settingsRows));
  } finally {
    await extension.deactivate();
    try {
      await removeDirWithRetry(storageDir);
    } catch (error) {
      console.warn(
        `⚠️  Failed to remove temp dir: ${
          error instanceof Error ? error.message : String(error)
        }`,
      );
    }
  }
});
