/**
 * Local-First Integration Example
 * 
 * This example demonstrates how to use PluresDB with the new local-first
 * integration API that automatically selects the best integration method
 * based on the runtime environment.
 */

import { PluresDBLocalFirst } from "../legacy/local-first/unified-api.ts";

async function main() {
  console.log("=== PluresDB Local-First Integration Example ===\n");

  // Initialize database with auto-detection
  // In browser: Uses WASM
  // In Tauri: Uses native integration
  // In Node/Deno with PLURESDB_IPC=true: Uses IPC
  // Otherwise: Uses network (HTTP REST)
  const db = new PluresDBLocalFirst({ mode: "auto" });
  
  console.log(`✅ Initialized in ${db.getMode()} mode\n`);

  // Note: Currently defaults to network mode since WASM and IPC
  // are not yet implemented. This demonstrates the API surface.
  
  if (db.getMode() === "network") {
    console.log("⚠️  Running in network mode (fallback)");
    console.log("   To use this example, start PluresDB server:");
    console.log("   npm start\n");
    console.log("   Or set PLURESDB_IPC=true for IPC mode (when available)\n");
  }

  try {
    // Example 1: Basic CRUD operations
    console.log("Example 1: Basic CRUD Operations");
    console.log("-----------------------------------");
    
    // Create
    await db.put("user:alice", {
      type: "User",
      name: "Alice",
      email: "alice@example.com",
      role: "admin",
      createdAt: new Date().toISOString(),
    });
    console.log("✅ Created user:alice");

    await db.put("user:bob", {
      type: "User", 
      name: "Bob",
      email: "bob@example.com",
      role: "user",
      createdAt: new Date().toISOString(),
    });
    console.log("✅ Created user:bob");

    // Read
    const alice = await db.get("user:alice");
    console.log(`✅ Retrieved user:alice - Name: ${alice?.name}`);

    // Update
    await db.put("user:alice", {
      ...alice,
      role: "superadmin",
      updatedAt: new Date().toISOString(),
    });
    console.log("✅ Updated user:alice role to superadmin");

    // List all
    const allUsers = await db.list();
    console.log(`✅ Listed ${allUsers.length} total nodes\n`);

    // Example 2: Vector Search (semantic similarity)
    console.log("Example 2: Vector Search");
    console.log("------------------------");
    
    await db.put("note:1", {
      type: "Note",
      text: "I love visiting museums in London",
      tags: ["travel", "culture"],
    });
    
    await db.put("note:2", {
      type: "Note",
      text: "Best pizza places in New York",
      tags: ["food", "travel"],
    });
    
    await db.put("note:3", {
      type: "Note",
      text: "Art galleries and parks in London are amazing",
      tags: ["culture", "nature"],
    });
    
    console.log("✅ Created 3 notes with different content");
    
    // Semantic search for London-related content
    const results = await db.vectorSearch("Things about London", 5);
    console.log(`\n🔍 Vector search: "Things about London"`);
    console.log(`   Found ${results.length} results`);
    results.forEach((result: any, i: number) => {
      console.log(`   ${i + 1}. ${result.data?.text || result.id}`);
    });

    // Example 3: Cleanup
    console.log("\nExample 3: Cleanup");
    console.log("------------------");
    
    await db.delete("user:bob");
    console.log("✅ Deleted user:bob");
    
    const afterDelete = await db.list();
    console.log(`✅ ${afterDelete.length} nodes remaining after deletion\n`);

  } catch (error) {
    if (error instanceof Error && error.message.includes("Failed")) {
      console.error("\n❌ Error:", error.message);
      console.log("\nMake sure PluresDB server is running:");
      console.log("  npm start");
      console.log("\nOr check the documentation for other integration modes:");
      console.log("  docs/LOCAL_FIRST_INTEGRATION.md");
    } else {
      throw error;
    }
  } finally {
    // Close connection
    await db.close();
    console.log("✅ Database connection closed");
  }
}

// Run the example
if (import.meta.main) {
  main().catch((error) => {
    console.error("Fatal error:", error);
    Deno.exit(1);
  });
}
