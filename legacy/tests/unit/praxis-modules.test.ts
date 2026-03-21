// @ts-nocheck — sloppy-imports mode; Deno resolves npm: specifiers at runtime
import { assertEquals, assertExists } from "@std/assert";
import { createPraxisEngine } from "@plures/praxis";

import {
  buildPluresDBPraxisRegistry,
  ValidateMutation,
  NodeMutationValid,
  NodeMutationInvalid,
  CheckReplicationEligibility,
  NodeEligibleForSync,
  NodeExcludedFromSync,
  CheckAccess,
  AccessGranted,
  AccessDenied,
  CheckIntegrity,
  IntegrityPassed,
  IntegrityFailed,
} from "../../logic/praxis/index.ts";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makeEngine() {
  const registry = buildPluresDBPraxisRegistry();
  const engine = createPraxisEngine({ initialContext: {}, registry });
  return { engine, registry };
}

function factsOfTag(facts: unknown[], tag: string) {
  return (facts as Array<{ tag: string }>).filter((f) => f.tag === tag);
}

// ---------------------------------------------------------------------------
// buildPluresDBPraxisRegistry
// ---------------------------------------------------------------------------

Deno.test("buildPluresDBPraxisRegistry — registers all four modules", () => {
  const registry = buildPluresDBPraxisRegistry();
  const ruleIds = registry.getRuleIds();
  const constraintIds = registry.getConstraintIds();

  // Graph-Validation rules
  assertExists(ruleIds.find((id) => id === "graph-validation.validateId"));
  assertExists(ruleIds.find((id) => id === "graph-validation.validateType"));
  assertExists(ruleIds.find((id) => id === "graph-validation.validatePayload"));

  // Replication-Policy rules
  assertExists(ruleIds.find((id) => id === "replication-policy.checkPrivate"));
  assertExists(
    ruleIds.find((id) => id === "replication-policy.checkEncryption"),
  );
  assertExists(ruleIds.find((id) => id === "replication-policy.allowSync"));

  // Access-Control rules
  assertExists(
    ruleIds.find((id) => id === "access-control.checkWritePermission"),
  );
  assertExists(ruleIds.find((id) => id === "access-control.checkAcl"));
  assertExists(ruleIds.find((id) => id === "access-control.allowRead"));

  // Data-Integrity rules
  assertExists(
    ruleIds.find((id) => id === "data-integrity.checkOrphanRefs"),
  );
  assertExists(ruleIds.find((id) => id === "data-integrity.checkSelfRef"));
  assertExists(ruleIds.find((id) => id === "data-integrity.passIntegrity"));

  // Constraints
  assertExists(
    constraintIds.find((id) => id === "graph-validation.noNullPayload"),
  );
  assertExists(
    constraintIds.find(
      (id) => id === "replication-policy.encryptionMetadataRequired",
    ),
  );
  assertExists(
    constraintIds.find(
      (id) => id === "access-control.actorRequiredForWrite",
    ),
  );
  assertExists(
    constraintIds.find((id) => id === "data-integrity.noSelfReference"),
  );
});

// ---------------------------------------------------------------------------
// Graph-Validation Module
// ---------------------------------------------------------------------------

Deno.test("graph-validation — valid mutation emits NodeMutationValid", () => {
  const { engine } = makeEngine();
  const result = engine.step([
    ValidateMutation.create({
      id: "node:1",
      data: { name: "Alice", _type: "Person" },
      operation: "put",
    }),
  ]);
  const valid = factsOfTag(result.state.facts, NodeMutationValid.tag);
  assertEquals(valid.length, 1);
  assertEquals((valid[0] as any).payload.id, "node:1");
});

Deno.test("graph-validation — empty ID emits NodeMutationInvalid", () => {
  const { engine } = makeEngine();
  const result = engine.step([
    ValidateMutation.create({
      id: "  ",
      data: { name: "Alice" },
      operation: "put",
    }),
  ]);
  const invalid = factsOfTag(result.state.facts, NodeMutationInvalid.tag);
  assertEquals(invalid.length >= 1, true);
  const reasons: string[] = (invalid[0] as any).payload.reasons;
  assertExists(reasons.find((r) => r.includes("non-empty")));
});

Deno.test("graph-validation — invalid _type field emits NodeMutationInvalid", () => {
  const { engine } = makeEngine();
  const result = engine.step([
    ValidateMutation.create({
      id: "node:2",
      data: { _type: 42 as unknown as string },
      operation: "put",
    }),
  ]);
  const invalid = factsOfTag(result.state.facts, NodeMutationInvalid.tag);
  assertEquals(invalid.length >= 1, true);
  const reasons: string[] = (invalid[0] as any).payload.reasons;
  assertExists(reasons.find((r) => r.includes("_type")));
});

Deno.test("graph-validation — delete operation with empty ID emits NodeMutationInvalid", () => {
  const { engine } = makeEngine();
  const result = engine.step([
    ValidateMutation.create({
      id: "",
      data: {},
      operation: "delete",
    }),
  ]);
  const invalid = factsOfTag(result.state.facts, NodeMutationInvalid.tag);
  assertEquals(invalid.length >= 1, true);
});

// ---------------------------------------------------------------------------
// Replication-Policy Module
// ---------------------------------------------------------------------------

Deno.test("replication-policy — public node is eligible for sync", () => {
  const { engine } = makeEngine();
  const result = engine.step([
    CheckReplicationEligibility.create({
      id: "node:pub",
      data: { name: "Public Node" },
    }),
  ]);
  const eligible = factsOfTag(result.state.facts, NodeEligibleForSync.tag);
  assertEquals(eligible.length, 1);
  assertEquals((eligible[0] as any).payload.id, "node:pub");
});

Deno.test("replication-policy — private node is excluded from sync", () => {
  const { engine } = makeEngine();
  const result = engine.step([
    CheckReplicationEligibility.create({
      id: "node:priv",
      data: { name: "Private Node", _private: true },
    }),
  ]);
  const excluded = factsOfTag(result.state.facts, NodeExcludedFromSync.tag);
  assertEquals(excluded.length, 1);
  assertExists(
    (excluded[0] as any).payload.reason.toLowerCase().includes("private"),
  );
});

Deno.test(
  "replication-policy — encrypted node without key is excluded from sync",
  () => {
    const { engine } = makeEngine();
    const result = engine.step([
      CheckReplicationEligibility.create({
        id: "node:enc",
        data: { _encrypt: true },
      }),
    ]);
    const excluded = factsOfTag(result.state.facts, NodeExcludedFromSync.tag);
    assertEquals(excluded.length, 1);
    assertExists(
      (excluded[0] as any).payload.reason.toLowerCase().includes(
        "encryption",
      ),
    );
  },
);

Deno.test(
  "replication-policy — encrypted node with key is eligible for sync",
  () => {
    const { engine } = makeEngine();
    const result = engine.step([
      CheckReplicationEligibility.create({
        id: "node:enc-ok",
        data: { _encrypt: true, _encrypted_key: "abc123" },
      }),
    ]);
    const eligible = factsOfTag(result.state.facts, NodeEligibleForSync.tag);
    assertEquals(eligible.length, 1);
  },
);

// ---------------------------------------------------------------------------
// Access-Control Module
// ---------------------------------------------------------------------------

Deno.test("access-control — read without ACL is granted", () => {
  const { engine } = makeEngine();
  const result = engine.step([
    CheckAccess.create({
      operation: "read",
      nodeId: "node:1",
      actor: "alice",
    }),
  ]);
  const granted = factsOfTag(result.state.facts, AccessGranted.tag);
  assertEquals(granted.length, 1);
  assertEquals((granted[0] as any).payload.actor, "alice");
});

Deno.test("access-control — write with empty actor is denied", () => {
  const { engine } = makeEngine();
  const result = engine.step([
    CheckAccess.create({
      operation: "write",
      nodeId: "node:1",
      actor: "",
    }),
  ]);
  const denied = factsOfTag(result.state.facts, AccessDenied.tag);
  assertEquals(denied.length, 1);
  assertExists(
    (denied[0] as any).payload.reasons.find((r: string) =>
      r.includes("Actor")
    ),
  );
});

Deno.test("access-control — actor in ACL is granted", () => {
  const { engine } = makeEngine();
  const result = engine.step([
    CheckAccess.create({
      operation: "write",
      nodeId: "node:1",
      actor: "bob",
      acl: ["alice", "bob"],
    }),
  ]);
  const granted = factsOfTag(result.state.facts, AccessGranted.tag);
  assertEquals(granted.length, 1);
});

Deno.test("access-control — actor not in ACL is denied", () => {
  const { engine } = makeEngine();
  const result = engine.step([
    CheckAccess.create({
      operation: "write",
      nodeId: "node:1",
      actor: "carol",
      acl: ["alice", "bob"],
    }),
  ]);
  const denied = factsOfTag(result.state.facts, AccessDenied.tag);
  assertEquals(denied.length, 1);
  assertExists(
    (denied[0] as any).payload.reasons.find((r: string) =>
      r.includes("carol")
    ),
  );
});

// ---------------------------------------------------------------------------
// Data-Integrity Module
// ---------------------------------------------------------------------------

Deno.test("data-integrity — node with valid refs passes integrity", () => {
  const { engine } = makeEngine();
  const result = engine.step([
    CheckIntegrity.create({
      nodeId: "node:1",
      data: { parent: "node:2", label: "child" },
      existingIds: ["node:2"],
    }),
  ]);
  const passed = factsOfTag(result.state.facts, IntegrityPassed.tag);
  assertEquals(passed.length, 1);
});

Deno.test("data-integrity — orphan reference emits IntegrityFailed", () => {
  const { engine } = makeEngine();
  const result = engine.step([
    CheckIntegrity.create({
      nodeId: "node:1",
      data: { parent: "node:missing" },
      existingIds: [],
    }),
  ]);
  const failed = factsOfTag(result.state.facts, IntegrityFailed.tag);
  assertEquals(failed.length >= 1, true);
  assertExists(
    (failed[0] as any).payload.violations.find((v: string) =>
      v.includes("node:missing")
    ),
  );
});

Deno.test("data-integrity — self-reference emits IntegrityFailed", () => {
  const { engine } = makeEngine();
  const result = engine.step([
    CheckIntegrity.create({
      nodeId: "node:1",
      data: { self: "node:1" },
      existingIds: ["node:1"],
    }),
  ]);
  const failed = factsOfTag(result.state.facts, IntegrityFailed.tag);
  assertEquals(failed.length >= 1, true);
  assertExists(
    (failed[0] as any).payload.violations.find((v: string) =>
      v.includes("self-reference")
    ),
  );
});

Deno.test(
  "data-integrity — node with no refs and no self-ref passes integrity",
  () => {
    const { engine } = makeEngine();
    const result = engine.step([
      CheckIntegrity.create({
        nodeId: "node:standalone",
        data: { label: "standalone", count: 42 },
        existingIds: [],
      }),
    ]);
    const passed = factsOfTag(result.state.facts, IntegrityPassed.tag);
    assertEquals(passed.length, 1);
  },
);
