#!/usr/bin/env node

/**
 * Pre-publish Validation Script for NPM
 * 
 * This script validates that the package is ready to be published to npm.
 * It checks:
 * 1. TypeScript compilation succeeds
 * 2. Deno type checking passes
 * 3. All tests pass
 * 4. Required files exist in dist/
 * 5. package.json is valid
 */

const { execSync } = require("node:child_process");
const fs = require("node:fs");
const path = require("node:path");

const RED = "\x1b[31m";
const GREEN = "\x1b[32m";
const YELLOW = "\x1b[33m";
const RESET = "\x1b[0m";
const BOLD = "\x1b[1m";

// Configuration
const MAX_PACKAGE_SIZE_MB = 10;

function log(message, color = RESET) {
  console.log(`${color}${message}${RESET}`);
}

function error(message) {
  log(`✗ ${message}`, RED);
}

function success(message) {
  log(`✓ ${message}`, GREEN);
}

function info(message) {
  log(`ℹ ${message}`, YELLOW);
}

function title(message) {
  log(`\n${BOLD}${message}${RESET}`);
}

function runCommand(command, description) {
  try {
    info(`Running: ${description}...`);
    execSync(command, { stdio: "inherit", cwd: process.cwd() });
    success(description);
    return true;
  } catch (err) {
    error(`${description} failed`);
    return false;
  }
}

function checkFileExists(filePath, description) {
  const fullPath = path.join(process.cwd(), filePath);
  if (fs.existsSync(fullPath)) {
    success(`${description}: ${filePath}`);
    return true;
  } else {
    error(`${description} missing: ${filePath}`);
    return false;
  }
}

async function main() {
  title("🚀 NPM Publish Validation");

  let allChecksPassed = true;

  // 1. Check package.json is valid
  title("📦 Validating package.json...");
  try {
    const packageJson = JSON.parse(
      fs.readFileSync(path.join(process.cwd(), "package.json"), "utf-8"),
    );
    if (!packageJson.name || !packageJson.version) {
      error("package.json missing required fields (name or version)");
      allChecksPassed = false;
    } else {
      success(
        `Package: ${packageJson.name}@${packageJson.version}`,
      );
    }
  } catch (err) {
    error(`Invalid package.json: ${err.message}`);
    allChecksPassed = false;
  }

  // 2. TypeScript compilation
  title("🔨 Building TypeScript...");
  if (!runCommand("npm run build:lib", "TypeScript compilation")) {
    allChecksPassed = false;
  }

  // 3. Check required dist files exist
  title("📁 Checking required files...");
  const requiredFiles = [
    "dist/node-index.js",
    "dist/node-index.d.ts",
    "dist/better-sqlite3.js",
    "dist/better-sqlite3.d.ts",
    "dist/cli.js",
    "dist/cli.d.ts",
    "dist/local-first/unified-api.js",
    "dist/local-first/unified-api.d.ts",
  ];

  for (const file of requiredFiles) {
    if (!checkFileExists(file, "Required file")) {
      allChecksPassed = false;
    }
  }

  // 4. Deno type checking
  title("🦕 Deno type checking...");
  const denoPath = process.env.DENO_PATH || "deno";
  const denoCheckFiles = [
    "legacy/local-first/unified-api.ts",
    "legacy/node-index.ts",
    "legacy/better-sqlite3.ts",
  ];

  // Check if Deno is available
  let denoAvailable = false;
  try {
    execSync(`${denoPath} --version`, { stdio: "pipe" });
    denoAvailable = true;
  } catch (err) {
    info("Deno not available - skipping Deno type checks");
  }

  if (denoAvailable) {
    let denoChecksFailed = false;
    for (const file of denoCheckFiles) {
      if (
        !runCommand(
          `${denoPath} check --sloppy-imports ${file}`,
          `Deno type check: ${file}`,
        )
      ) {
        error(`Deno type check failed for ${file}`);
        denoChecksFailed = true;
        allChecksPassed = false;
        // Continue checking other files to show all failures
      }
    }
    if (!denoChecksFailed) {
      success("All Deno type checks passed");
    }
  }

  // 5. Run tests (if Deno is available)
  title("🧪 Running tests...");
  if (denoAvailable) {
    // Set DENO_PATH environment variable so npm test can find deno
    const testEnv = { ...process.env };
    const denoPathEnv = process.env.DENO_PATH;
    if (denoPathEnv && denoPathEnv.includes(path.sep)) {
      // If DENO_PATH was provided as a path, make sure its directory is in PATH for npm test
      const denoBinDir = path.dirname(denoPathEnv);
      // Use path.delimiter for cross-platform compatibility (: on Unix, ; on Windows)
      testEnv.PATH = `${denoBinDir}${path.delimiter}${process.env.PATH}`;
    }
    
    try {
      execSync("npm test", { stdio: "inherit", cwd: process.cwd(), env: testEnv });
      success("Deno tests");
    } catch (err) {
      error("Tests failed");
      allChecksPassed = false;
    }
  } else {
    info("Deno tests skipped (Deno not available)");
  }

  // 6. Check package size
  title("📊 Package size check...");
  try {
    const output = execSync("npm pack --dry-run 2>&1", { encoding: "utf-8" });
    const sizeMatch = output.match(/package size:\s+(\d+\.?\d*)\s*(\w+)/i);
    if (sizeMatch) {
      const size = parseFloat(sizeMatch[1]);
      const unit = sizeMatch[2];
      success(`Package size: ${size} ${unit}`);

      // Warn if package is larger than configured threshold
      if (unit.toLowerCase() === "mb" && size > MAX_PACKAGE_SIZE_MB) {
        info(
          `Warning: Package size is quite large (${size} ${unit}). Consider excluding unnecessary files.`,
        );
      }
    }
  } catch (err) {
    info("Could not determine package size");
  }

  // Summary
  title("📋 Validation Summary");
  if (allChecksPassed) {
    success("All critical checks passed! ✨");
    log(
      "\nThe package is ready to be published to npm.",
      GREEN,
    );
    process.exit(0);
  } else {
    error("Some checks failed. Please fix the issues before publishing.");
    process.exit(1);
  }
}

main().catch((err) => {
  error(`Validation failed with error: ${err.message}`);
  console.error(err);
  process.exit(1);
});
