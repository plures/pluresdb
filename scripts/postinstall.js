/**
 * Post-install script for PluresDB npm package
 * This script ensures Deno is available and sets up the environment
 */

const { spawn, exec } = require("child_process");
const fs = require("fs");
const path = require("path");
const os = require("os");

const DENO_VERSION = "1.40.0";

function log(message) {
  console.log(`[pluresdb] ${message}`);
}

function logError(message) {
  console.error(`[pluresdb] ERROR: ${message}`);
}

function isDenoInstalled() {
  return new Promise((resolve) => {
    exec("deno --version", (error) => {
      resolve(!error);
    });
  });
}

function installDeno() {
  return new Promise((resolve, reject) => {
    const platform = os.platform();
    const arch = os.arch();

    log("Installing Deno...");

    let installCommand;

    if (platform === "win32") {
      // Windows - use PowerShell
      installCommand = `powershell -c "iwr https://deno.land/install.ps1 -useb | iex"`;
    } else if (platform === "darwin") {
      // macOS - use Homebrew or curl
      installCommand = "curl -fsSL https://deno.land/install.sh | sh";
    } else {
      // Linux - use curl
      installCommand = "curl -fsSL https://deno.land/install.sh | sh";
    }

    exec(installCommand, (error, stdout, stderr) => {
      if (error) {
        logError(`Failed to install Deno: ${error.message}`);
        logError("Please install Deno manually from https://deno.land/");
        reject(error);
      } else {
        log("Deno installed successfully");
        resolve();
      }
    });
  });
}

function createStartScript() {
  const scriptContent = `#!/bin/bash
# PluresDB start script
export DENO_INSTALL="$HOME/.deno"
export PATH="$DENO_INSTALL/bin:$PATH"

# Start PluresDB
deno run -A "${path.join(__dirname, "../src/main.ts")}" serve "$@"
`;

  const scriptPath = path.join(__dirname, "../bin/pluresdb.sh");
  const scriptDir = path.dirname(scriptPath);

  if (!fs.existsSync(scriptDir)) {
    fs.mkdirSync(scriptDir, { recursive: true });
  }

  fs.writeFileSync(scriptPath, scriptContent);
  fs.chmodSync(scriptPath, "755");

  log("Created start script");
}

function createWindowsStartScript() {
  const scriptContent = `@echo off
REM PluresDB start script for Windows
set DENO_INSTALL=%USERPROFILE%\\.deno
set PATH=%DENO_INSTALL%\\bin;%PATH%

REM Start PluresDB
deno run -A "${path.join(__dirname, "../src/main.ts")}" serve %*
`;

  const scriptPath = path.join(__dirname, "../bin/pluresdb.bat");
  const scriptDir = path.dirname(scriptPath);

  if (!fs.existsSync(scriptDir)) {
    fs.mkdirSync(scriptDir, { recursive: true });
  }

  fs.writeFileSync(scriptPath, scriptContent);

  log("Created Windows start script");
}

async function main() {
  try {
    log("Setting up PluresDB...");

    // Check if Deno is installed
    const denoInstalled = await isDenoInstalled();

    if (!denoInstalled) {
      log("Deno not found, attempting to install...");
      try {
        await installDeno();
      } catch {
        logError("Failed to install Deno automatically");
        logError("Please install Deno manually:");
        logError("  Windows: iwr https://deno.land/install.ps1 -useb | iex");
        logError("  macOS/Linux: curl -fsSL https://deno.land/install.sh | sh");
        logError("  Or visit: https://deno.land/");
        return;
      }
    } else {
      log("Deno is already installed");
    }

    // Create start scripts
    createStartScript();
    if (os.platform() === "win32") {
      createWindowsStartScript();
    }

    // Create data directory
    const dataDir = path.join(os.homedir(), ".pluresdb");
    if (!fs.existsSync(dataDir)) {
      fs.mkdirSync(dataDir, { recursive: true });
      log(`Created data directory: ${dataDir}`);
    }

    log("Setup complete!");
    log("Usage:");
    log("  npx pluresdb serve");
    log("  or");
    log("  node node_modules/pluresdb/dist/cli.js serve");
  } catch (error) {
    logError(`Setup failed: ${error.message}`);
    Deno.exit(1);
  }
}

// Run the setup
main();
