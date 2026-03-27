/**
 * Sync Configuration Examples
 *
 * Demonstrates how to configure PluresDB P2P synchronisation across a range
 * of network environments — from direct peer discovery via Hyperswarm to
 * relay-assisted transport for corporate firewalls.
 *
 * Run (Deno):
 *   deno run -A examples/sync-configuration.ts
 *
 * Run (Node.js, after build):
 *   node dist/examples/sync-configuration.js
 */

import { PluresDB } from "../legacy/core/database.ts";
import type { SyncStats } from "../legacy/network/hyperswarm-sync.ts";

// ---------------------------------------------------------------------------
// Example 1 — Hyperswarm P2P sync (LAN / open internet)
// ---------------------------------------------------------------------------

async function example1_hyperswarmSync(): Promise<void> {
  console.log("── Example 1: Hyperswarm P2P Sync ─────────────────────────\n");

  const alice = new PluresDB();
  const bob = new PluresDB();
  await alice.ready();
  await bob.ready();

  // Generate a shared sync key (share out-of-band — QR code, invite link, etc.)
  const syncKey = PluresDB.generateSyncKey();
  console.log(`Sync key: ${syncKey.slice(0, 16)}…`);

  // Attach event listeners before enabling sync
  alice.on("peer:connected", (node) => {
    if (node) {
      // Peer events carry PeerInfo at runtime, typed as NodeRecord for the generic emitter
      const info = node as unknown as { peerId: string };
      console.log(`  Alice ← peer connected: ${info.peerId.slice(0, 8)}…`);
    }
  });
  bob.on("peer:connected", (node) => {
    if (node) {
      const info = node as unknown as { peerId: string };
      console.log(`  Bob   ← peer connected: ${info.peerId.slice(0, 8)}…`);
    }
  });

  // Enable sync on both sides using the same key
  await alice.enableSync({ key: syncKey });
  await bob.enableSync({ key: syncKey });

  console.log("  Sync enabled on both nodes.");

  // Write data on Alice — Bob will receive it once connected
  await alice.put("doc:1", {
    type: "document",
    title: "Shared note",
    content: "Hello from Alice",
  });
  console.log("  Alice wrote doc:1");

  // Inspect live stats
  const stats: SyncStats | null = alice.getSyncStats();
  if (stats) {
    console.log(
      `  Stats — peers: ${stats.peersConnected}, sent: ${stats.messagesSent}`,
    );
  }

  const peers = alice.getSyncPeers();
  console.log(`  Alice has ${peers.length} connected peer(s).`);

  await alice.disableSync();
  await bob.disableSync();
  console.log("  Sync disabled.\n");

  await alice.close();
  await bob.close();
}

// ---------------------------------------------------------------------------
// Example 2 — Relay transport (NAT / corporate firewall)
// ---------------------------------------------------------------------------
// When peers cannot reach each other directly (e.g. both behind NAT or a
// corporate firewall blocks UDP), use the built-in Gun relay transport.
//
// Start your own relay:
//   pluresdb serve --relay --port 8765
//
// Or use the TypeScript relay helper:
//   import { startApiServer } from "pluresdb";
//   startApiServer({ port: 8765, relay: true });
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Example 2 — Relay transport reference (NAT / corporate firewall)
// ---------------------------------------------------------------------------
// NOTE: `relayUrl` is not yet part of the TypeScript `SyncKeyOptions`
// interface.  This example documents the *Rust-native* TransportConfig API
// and the corresponding CLI flags.  For TypeScript-only applications the
// Hyperswarm transport (example 1) is used automatically, which handles most
// NAT traversal cases via UDP hole-punching.
//
// Rust TransportConfig:
//
//   let config = TransportConfig {
//       mode:      TransportMode::Relay,
//       relay_url: Some("wss://relay.example.com:8765".into()),
//       timeout_ms: 30_000,
//       encryption: true,
//   };
//
// CLI (serve your own relay):
//   pluresdb serve --relay --port 8765
//
// See docs/SYNC_TRANSPORT.md for full relay configuration reference.
// ---------------------------------------------------------------------------

async function example2_relayTransportReference(): Promise<void> {
  console.log("── Example 2: Relay Transport (reference) ──────────────────\n");
  console.log(
    "  The TypeScript API uses Hyperswarm by default and handles most NAT",
  );
  console.log(
    "  traversal automatically via UDP hole-punching.",
  );
  console.log(
    "  For strict corporate firewalls, run a Gun relay server and configure",
  );
  console.log(
    "  the Rust SyncBroadcaster with TransportMode::Relay.",
  );
  console.log(
    "  See docs/SYNC_TRANSPORT.md and docs/API.md#transport-trait for details.\n",
  );

  // TypeScript sync using Hyperswarm (works without a relay server)
  const db = new PluresDB();
  await db.ready();

  const syncKey = PluresDB.generateSyncKey();
  await db.enableSync({ key: syncKey });

  const stats = db.getSyncStats();
  console.log(`  syncKey prefix: ${stats?.syncKey?.slice(0, 8) ?? syncKey.slice(0, 8)}…`);

  await db.disableSync();
  await db.close();
  console.log("  Done.\n");
}

// ---------------------------------------------------------------------------
// Example 3 — Selective sync with separate topic keys
// ---------------------------------------------------------------------------
// Use distinct sync keys to create isolated sync groups (e.g. per-team,
// per-environment, or per-document-collection).

async function example3_selectiveSync(): Promise<void> {
  console.log("── Example 3: Selective Sync with Topic Keys ───────────────\n");

  const teamAKey = PluresDB.generateSyncKey();
  const teamBKey = PluresDB.generateSyncKey();

  const alice = new PluresDB();
  const carol = new PluresDB();
  await alice.ready();
  await carol.ready();

  // Alice joins Team A
  await alice.enableSync({ key: teamAKey });

  // Carol joins Team B — they won't receive each other's data
  await carol.enableSync({ key: teamBKey });

  await alice.put("project:1", { type: "Project", name: "Alpha", team: "A" });
  await carol.put("project:2", { type: "Project", name: "Beta", team: "B" });

  console.log("  Alice and Carol are syncing to separate topic namespaces.");
  console.log(`  Team A key: ${teamAKey.slice(0, 16)}…`);
  console.log(`  Team B key: ${teamBKey.slice(0, 16)}…`);

  await alice.disableSync();
  await carol.disableSync();
  await alice.close();
  await carol.close();
  console.log("  Done.\n");
}

// ---------------------------------------------------------------------------
// Example 4 — Listening to sync lifecycle events
// ---------------------------------------------------------------------------

async function example4_syncEvents(): Promise<void> {
  console.log("── Example 4: Sync Lifecycle Events ───────────────────────\n");

  const db = new PluresDB();
  await db.ready();

  db.on("peer:connected", (node) => {
    if (node) {
      const info = node as unknown as { peerId: string };
      console.log(`  ✅ Peer connected:    ${info.peerId.slice(0, 12)}…`);
    }
  });

  db.on("peer:disconnected", (node) => {
    if (node) {
      const info = node as unknown as { peerId: string };
      console.log(`  ⚠️  Peer disconnected: ${info.peerId.slice(0, 12)}…`);
    }
  });

  const key = PluresDB.generateSyncKey();
  await db.enableSync({ key });

  console.log("  Listening for sync events… (no peers in this demo)\n");

  await db.disableSync();
  await db.close();
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

if (import.meta.main) {
  await example1_hyperswarmSync();
  await example2_relayTransportReference();
  await example3_selectiveSync();
  await example4_syncEvents();

  console.log("All sync-configuration examples complete.");
}
