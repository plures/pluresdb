#!/usr/bin/env -S deno run -A

/**
 * Health check script for Docker containers
 * Verifies that Rusty Gun is running and responding correctly
 */

const API_PORT = Deno.env.get("RUSTY_GUN_PORT") || "34567";
const WEB_PORT = Deno.env.get("RUSTY_GUN_WEB_PORT") || "34568";
const HOST = Deno.env.get("RUSTY_GUN_HOST") || "localhost";

interface HealthStatus {
  status: "healthy" | "unhealthy";
  checks: {
    api: boolean;
    web: boolean;
    database: boolean;
  };
  timestamp: string;
  uptime: number;
}

async function checkApiHealth(): Promise<boolean> {
  try {
    const response = await fetch(`http://${HOST}:${API_PORT}/api/health`, {
      method: "GET",
      headers: {
        "Accept": "application/json",
        "User-Agent": "rusty-gun-healthcheck/1.0.0"
      },
      signal: AbortSignal.timeout(5000) // 5 second timeout
    });
    
    if (!response.ok) {
      console.error(`API health check failed: ${response.status} ${response.statusText}`);
      return false;
    }
    
    const data = await response.json();
    return data.status === "healthy" || data.status === "ok";
  } catch (error) {
    console.error(`API health check error: ${error.message}`);
    return false;
  }
}

async function checkWebHealth(): Promise<boolean> {
  try {
    const response = await fetch(`http://${HOST}:${WEB_PORT}/`, {
      method: "GET",
      headers: {
        "Accept": "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
        "User-Agent": "rusty-gun-healthcheck/1.0.0"
      },
      signal: AbortSignal.timeout(5000) // 5 second timeout
    });
    
    if (!response.ok) {
      console.error(`Web health check failed: ${response.status} ${response.statusText}`);
      return false;
    }
    
    const contentType = response.headers.get("content-type");
    return contentType?.includes("text/html") || contentType?.includes("application/json");
  } catch (error) {
    console.error(`Web health check error: ${error.message}`);
    return false;
  }
}

async function checkDatabaseHealth(): Promise<boolean> {
  try {
    // Check if data directory exists and is writable
    const dataDir = Deno.env.get("RUSTY_GUN_DATA_DIR") || "./data";
    
    try {
      await Deno.stat(dataDir);
    } catch {
      // Data directory doesn't exist, try to create it
      await Deno.mkdir(dataDir, { recursive: true });
    }
    
    // Test write permissions
    const testFile = `${dataDir}/.healthcheck-test`;
    await Deno.writeTextFile(testFile, "healthcheck");
    await Deno.remove(testFile);
    
    return true;
  } catch (error) {
    console.error(`Database health check error: ${error.message}`);
    return false;
  }
}

async function main(): Promise<void> {
  const startTime = Date.now();
  
  console.log("Starting Rusty Gun health check...");
  console.log(`API: http://${HOST}:${API_PORT}/api/health`);
  console.log(`Web: http://${HOST}:${WEB_PORT}/`);
  
  // Run health checks in parallel
  const [apiHealthy, webHealthy, dbHealthy] = await Promise.all([
    checkApiHealth(),
    checkWebHealth(),
    checkDatabaseHealth()
  ]);
  
  const uptime = Date.now() - startTime;
  const allHealthy = apiHealthy && webHealthy && dbHealthy;
  
  const healthStatus: HealthStatus = {
    status: allHealthy ? "healthy" : "unhealthy",
    checks: {
      api: apiHealthy,
      web: webHealthy,
      database: dbHealthy
    },
    timestamp: new Date().toISOString(),
    uptime
  };
  
  console.log("Health check results:");
  console.log(`  API Server: ${apiHealthy ? "✓" : "✗"}`);
  console.log(`  Web UI: ${webHealthy ? "✓" : "✗"}`);
  console.log(`  Database: ${dbHealthy ? "✓" : "✗"}`);
  console.log(`  Overall: ${allHealthy ? "✓ Healthy" : "✗ Unhealthy"}`);
  console.log(`  Response time: ${uptime}ms`);
  
  if (allHealthy) {
    console.log("Health check passed");
    Deno.exit(0);
  } else {
    console.error("Health check failed");
    Deno.exit(1);
  }
}

// Handle graceful shutdown
Deno.addSignalListener("SIGTERM", () => {
  console.log("Health check interrupted by SIGTERM");
  Deno.exit(0);
});

Deno.addSignalListener("SIGINT", () => {
  console.log("Health check interrupted by SIGINT");
  Deno.exit(0);
});

// Run the health check
if (import.meta.main) {
  main().catch((error) => {
    console.error("Health check failed with error:", error);
    Deno.exit(1);
  });
}
