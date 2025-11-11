#!/usr/bin/env node

/**
 * CLI wrapper for PluresDB in Node.js environment
 * This allows VSCode extensions to use pluresdb as a regular npm package
 */

import { PluresNode } from "./node-wrapper";
import * as path from "path";
import * as fs from "fs";
import process from "node:process";

// Parse command line arguments
const args = process.argv.slice(2);
const command = args[0];

if (!command) {
  console.log(`
PluresDB - P2P Graph Database with SQLite Compatibility

Usage: pluresdb <command> [options]

Commands:
  serve                    Start the PluresDB server
  put <key> <value>        Store a key-value pair
  get <key>                Retrieve a value by key
  delete <key>             Delete a key-value pair
  query <sql>              Execute SQL query
  vsearch <query>          Perform vector search
  list [prefix]            List all keys (optionally with prefix)
  config                   Show configuration
  config set <key> <value> Set configuration value
  --help                   Show this help message
  --version                Show version

Examples:
  pluresdb serve --port 8080
  pluresdb put "user:123" '{"name": "John"}'
  pluresdb get "user:123"
  pluresdb query "SELECT * FROM users"
  pluresdb vsearch "machine learning"
`);
  process.exit(0);
}

if (command === "--version") {
  const packageJson = JSON.parse(
    fs.readFileSync(path.join(__dirname, "../package.json"), "utf8"),
  );
  console.log(packageJson.version);
  process.exit(0);
}

if (command === "--help") {
  console.log(`
PluresDB - P2P Graph Database with SQLite Compatibility

Usage: pluresdb <command> [options]

Commands:
  serve                    Start the PluresDB server
  put <key> <value>        Store a key-value pair
  get <key>                Retrieve a value by key
  delete <key>             Delete a key-value pair
  query <sql>              Execute SQL query
  vsearch <query>          Perform vector search
  list [prefix]            List all keys (optionally with prefix)
  config                   Show configuration
  config set <key> <value> Set configuration value
  --help                   Show this help message
  --version                Show version

Examples:
  pluresdb serve --port 8080
  pluresdb put "user:123" '{"name": "John"}'
  pluresdb get "user:123"
  pluresdb query "SELECT * FROM users"
  pluresdb vsearch "machine learning"
`);
  process.exit(0);
}

// Parse options
const options: any = {};
let i = 1;
while (i < args.length) {
  const arg = args[i];
  if (arg.startsWith("--")) {
    const key = arg.substring(2);
    const value = args[i + 1];
    if (value && !value.startsWith("--")) {
      options[key] = value;
      i += 2;
    } else {
      options[key] = true;
      i += 1;
    }
  } else {
    i += 1;
  }
}

async function main() {
  try {
    if (command === "serve") {
      const config = {
        port: options.port ? parseInt(options.port) : 34567,
        host: options.host || "localhost",
        dataDir: options["data-dir"] ||
          path.join(require("os").homedir(), ".pluresdb"),
        webPort: options["web-port"] ? parseInt(options["web-port"]) : 34568,
        logLevel: options["log-level"] || "info",
      };

      const plures = new PluresNode({ config, autoStart: true });

      console.log(`🚀 PluresDB server starting...`);
      console.log(`📊 API: http://${config.host}:${config.port}`);
      console.log(`🌐 Web UI: http://${config.host}:${config.webPort}`);
      console.log(`📁 Data: ${config.dataDir}`);
      console.log(`\nPress Ctrl+C to stop the server`);

      // Handle graceful shutdown
      process.on("SIGINT", async () => {
        console.log("\n🛑 Shutting down PluresDB...");
        await plures.stop();
        process.exit(0);
      });

      // Keep the process alive
      await new Promise(() => {});
    } else {
      // For other commands, we need to start the server first
      const plures = new PluresNode({ autoStart: true });

      try {
        switch (command) {
          case "put":
            if (args.length < 3) {
              console.error("Error: put command requires key and value");
              process.exit(1);
            }
            const key = args[1];
            const value = JSON.parse(args[2]);
            await plures.put(key, value);
            console.log(`✅ Stored: ${key}`);
            break;

          case "get":
            if (args.length < 2) {
              console.error("Error: get command requires key");
              process.exit(1);
            }
            const getKey = args[1];
            const result = await plures.get(getKey);
            if (result === null) {
              console.log("Key not found");
            } else {
              console.log(JSON.stringify(result, null, 2));
            }
            break;

          case "delete":
            if (args.length < 2) {
              console.error("Error: delete command requires key");
              process.exit(1);
            }
            const deleteKey = args[1];
            await plures.delete(deleteKey);
            console.log(`✅ Deleted: ${deleteKey}`);
            break;

          case "query":
            if (args.length < 2) {
              console.error("Error: query command requires SQL");
              process.exit(1);
            }
            const sql = args[1];
            const queryResult = await plures.query(sql);
            console.log(JSON.stringify(queryResult, null, 2));
            break;

          case "vsearch":
            if (args.length < 2) {
              console.error("Error: vsearch command requires query");
              process.exit(1);
            }
            const searchQuery = args[1];
            const limit = options.limit ? parseInt(options.limit) : 10;
            const searchResult = await plures.vectorSearch(searchQuery, limit);
            console.log(JSON.stringify(searchResult, null, 2));
            break;

          case "list":
            const prefix = args[1];
            const listResult = await plures.list(prefix);
            console.log(JSON.stringify(listResult, null, 2));
            break;

          case "config":
            if (args[1] === "set") {
              if (args.length < 4) {
                console.error("Error: config set requires key and value");
                process.exit(1);
              }
              const configKey = args[2];
              const configValue = args[3];
              await plures.setConfig({ [configKey]: configValue });
              console.log(`✅ Set config: ${configKey} = ${configValue}`);
            } else {
              const config = await plures.getConfig();
              console.log(JSON.stringify(config, null, 2));
            }
            break;

          default:
            console.error(`Unknown command: ${command}`);
            console.log('Run "pluresdb --help" for usage information');
            process.exit(1);
        }
      } finally {
        await plures.stop();
      }
    }
  } catch (error) {
    console.error(
      "Error:",
      error instanceof Error ? error.message : String(error),
    );
    process.exit(1);
  }
}

// Run the main function
main().catch((error) => {
  console.error(
    "Fatal error:",
    error instanceof Error ? error.message : String(error),
  );
  process.exit(1);
});
