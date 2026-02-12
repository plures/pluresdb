/**
 * Example: P2P Sync with Hyperswarm
 *
 * This example demonstrates how to use PluresDB's P2P sync feature
 * with Hyperswarm for DHT-based peer discovery and NAT traversal.
 *
 * Features:
 * - Zero-configuration peer discovery via DHT
 * - Automatic NAT traversal (UDP holepunching)
 * - Encrypted P2P connections
 * - CRDT-based conflict resolution
 * - Real-time data synchronization
 */

// Import PluresDB (Node.js)
const { GunDB } = require("@plures/pluresdb");

// For Deno:
// import { GunDB } from "https://deno.land/x/pluresdb/mod.ts";

async function example1_basicP2PSync() {
  console.log("\n=== Example 1: Basic P2P Sync ===\n");

  // Create two database instances (simulating two devices)
  const dbA = new GunDB({ peerId: "device-A" });
  const dbB = new GunDB({ peerId: "device-B" });

  // Initialize databases with temporary storage
  await dbA.ready("/tmp/device-a.db");
  await dbB.ready("/tmp/device-b.db");

  // Generate a shared sync key
  // This key should be kept secret and shared only between trusted devices
  const syncKey = GunDB.generateSyncKey();
  console.log("Generated sync key:", syncKey.slice(0, 16) + "...");

  // Enable P2P sync on both databases with the same key
  console.log("Enabling sync on both devices...");
  await dbA.enableSync({ key: syncKey });
  await dbB.enableSync({ key: syncKey });

  // Wait for peers to discover each other
  await new Promise((resolve) => setTimeout(resolve, 2000));

  // Check peer connections
  const peersA = dbA.getSyncPeers();
  const peersB = dbB.getSyncPeers();
  console.log(`Device A connected to ${peersA.length} peer(s)`);
  console.log(`Device B connected to ${peersB.length} peer(s)`);

  // Put data on device A
  await dbA.put("user:alice", {
    name: "Alice",
    email: "alice@example.com",
    deviceId: "device-A",
  });

  // Wait for sync
  await new Promise((resolve) => setTimeout(resolve, 1000));

  // Retrieve data on device B (should be synced automatically)
  const userData = await dbB.get("user:alice");
  console.log("Data synced to device B:", userData);

  // Get sync statistics
  const statsA = dbA.getSyncStats();
  console.log("Device A stats:", statsA);

  // Cleanup
  await dbA.disableSync();
  await dbB.disableSync();
  await dbA.close();
  await dbB.close();
}

async function example2_syncEvents() {
  console.log("\n=== Example 2: Sync Events ===\n");

  const db = new GunDB();
  await db.ready("/tmp/events-example.db");

  // Listen for peer connection events
  db.on("peer:connected", (info) => {
    console.log("Peer connected:", {
      peerId: info.peerId.slice(0, 16) + "...",
      remotePublicKey: info.remotePublicKey?.slice(0, 16) + "...",
    });
  });

  db.on("peer:disconnected", (info) => {
    console.log("Peer disconnected:", {
      peerId: info.peerId.slice(0, 16) + "...",
    });
  });

  db.on("sync:complete", (stats) => {
    console.log("Sync completed:", stats);
  });

  // Enable sync with a generated key
  const key = GunDB.generateSyncKey();
  await db.enableSync({ key });

  console.log("Waiting for peer connections...");
  await new Promise((resolve) => setTimeout(resolve, 5000));

  await db.disableSync();
  await db.close();
}

async function example3_multipleDevices() {
  console.log("\n=== Example 3: Multi-Device Sync Network ===\n");

  // Create three devices
  const devices = [
    new GunDB({ peerId: "laptop" }),
    new GunDB({ peerId: "phone" }),
    new GunDB({ peerId: "desktop" }),
  ];

  // Initialize all devices
  await Promise.all(
    devices.map((db, i) => db.ready(`/tmp/device-${i}.db`)),
  );

  // Use the same sync key for all devices
  const syncKey = GunDB.generateSyncKey();

  // Enable sync on all devices
  console.log("Connecting all devices to the mesh network...");
  await Promise.all(devices.map((db) => db.enableSync({ key: syncKey })));

  // Wait for mesh to form
  await new Promise((resolve) => setTimeout(resolve, 3000));

  // Each device should be connected to the others
  devices.forEach((db, i) => {
    const peers = db.getSyncPeers();
    console.log(`Device ${i} connected to ${peers.length} peer(s)`);
  });

  // Put data on one device
  await devices[0].put("shared:document", {
    title: "Team Document",
    content: "This is shared across all devices",
    updatedBy: "laptop",
  });

  // Wait for propagation
  await new Promise((resolve) => setTimeout(resolve, 2000));

  // All devices should have the data
  for (let i = 0; i < devices.length; i++) {
    const data = await devices[i].get("shared:document");
    console.log(`Device ${i} has:`, data?.title);
  }

  // Cleanup
  await Promise.all(devices.map((db) => db.disableSync()));
  await Promise.all(devices.map((db) => db.close()));
}

async function example4_conflictResolution() {
  console.log("\n=== Example 4: CRDT Conflict Resolution ===\n");

  const dbA = new GunDB({ peerId: "device-A" });
  const dbB = new GunDB({ peerId: "device-B" });

  await dbA.ready("/tmp/conflict-a.db");
  await dbB.ready("/tmp/conflict-b.db");

  const syncKey = GunDB.generateSyncKey();

  // Both devices modify the same data before syncing
  await dbA.put("doc:1", { title: "Document", version: 1, editedBy: "A" });
  await dbB.put("doc:1", { title: "Document", version: 2, editedBy: "B" });

  console.log("Before sync:");
  console.log("Device A:", await dbA.get("doc:1"));
  console.log("Device B:", await dbB.get("doc:1"));

  // Enable sync - CRDTs will merge the conflicts
  await dbA.enableSync({ key: syncKey });
  await dbB.enableSync({ key: syncKey });

  // Wait for sync and conflict resolution
  await new Promise((resolve) => setTimeout(resolve, 2000));

  console.log("\nAfter sync (CRDT merge):");
  const finalA = await dbA.get("doc:1");
  const finalB = await dbB.get("doc:1");
  console.log("Device A:", finalA);
  console.log("Device B:", finalB);

  // Both should converge to the same state
  console.log(
    "\nConverged:",
    JSON.stringify(finalA) === JSON.stringify(finalB),
  );

  await dbA.disableSync();
  await dbB.disableSync();
  await dbA.close();
  await dbB.close();
}

async function example5_selectiveSync() {
  console.log("\n=== Example 5: Using Sync Key for Private Channels ===\n");

  // Create different sync keys for different groups
  const teamAKey = GunDB.generateSyncKey();
  const teamBKey = GunDB.generateSyncKey();

  const teamA1 = new GunDB({ peerId: "team-a-member-1" });
  const teamA2 = new GunDB({ peerId: "team-a-member-2" });
  const teamB1 = new GunDB({ peerId: "team-b-member-1" });

  await teamA1.ready("/tmp/team-a-1.db");
  await teamA2.ready("/tmp/team-a-2.db");
  await teamB1.ready("/tmp/team-b-1.db");

  // Team A members sync with teamAKey
  await teamA1.enableSync({ key: teamAKey });
  await teamA2.enableSync({ key: teamAKey });

  // Team B uses different key (won't see Team A's data)
  await teamB1.enableSync({ key: teamBKey });

  await new Promise((resolve) => setTimeout(resolve, 2000));

  // Team A members should be connected
  console.log("Team A member 1 peers:", teamA1.getSyncPeers().length);
  console.log("Team A member 2 peers:", teamA2.getSyncPeers().length);
  console.log("Team B member 1 peers:", teamB1.getSyncPeers().length);

  // Team A data
  await teamA1.put("team-a:project", { name: "Secret Project A" });

  await new Promise((resolve) => setTimeout(resolve, 1000));

  // Team A member 2 should have it
  const projectA = await teamA2.get("team-a:project");
  console.log("Team A member 2 has:", projectA);

  // Team B should NOT have it (different sync key)
  const projectB = await teamB1.get("team-a:project");
  console.log("Team B member 1 has:", projectB); // Should be null

  await teamA1.disableSync();
  await teamA2.disableSync();
  await teamB1.disableSync();
  await teamA1.close();
  await teamA2.close();
  await teamB1.close();
}

// Run examples
if (require.main === module) {
  (async () => {
    try {
      await example1_basicP2PSync();
      await example2_syncEvents();
      await example3_multipleDevices();
      await example4_conflictResolution();
      await example5_selectiveSync();

      console.log("\n✅ All examples completed!\n");
    } catch (error) {
      console.error("Error:", error);
      process.exit(1);
    }
  })();
}

module.exports = {
  example1_basicP2PSync,
  example2_syncEvents,
  example3_multipleDevices,
  example4_conflictResolution,
  example5_selectiveSync,
};
