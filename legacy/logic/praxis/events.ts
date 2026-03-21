/**
 * PluresDB Praxis — Shared Event and Fact Definitions
 *
 * Typed vocabulary (events + facts) shared across all PluresDB Praxis modules.
 * Events trigger rules; facts are the typed conclusions emitted by rules.
 */

import { defineEvent, defineFact } from "@plures/praxis";

// ---------------------------------------------------------------------------
// Graph-Validation vocabulary
// ---------------------------------------------------------------------------

/** Trigger: validate a pending node mutation before it is applied. */
export const ValidateMutation = defineEvent<
  "graph.VALIDATE_MUTATION",
  { id: string; data: Record<string, unknown>; operation: "put" | "delete" }
>("graph.VALIDATE_MUTATION");

/** Conclusion: node mutation passed all validation checks. */
export const NodeMutationValid = defineFact<
  "graph.NodeMutationValid",
  { id: string }
>("graph.NodeMutationValid");

/** Conclusion: node mutation failed one or more validation checks. */
export const NodeMutationInvalid = defineFact<
  "graph.NodeMutationInvalid",
  { id: string; reasons: string[] }
>("graph.NodeMutationInvalid");

// ---------------------------------------------------------------------------
// Replication-Policy vocabulary
// ---------------------------------------------------------------------------

/** Trigger: check whether a node is eligible for P2P replication. */
export const CheckReplicationEligibility = defineEvent<
  "replication.CHECK_ELIGIBILITY",
  { id: string; data: Record<string, unknown> }
>("replication.CHECK_ELIGIBILITY");

/** Conclusion: node is eligible for P2P sync. */
export const NodeEligibleForSync = defineFact<
  "replication.NodeEligibleForSync",
  { id: string }
>("replication.NodeEligibleForSync");

/** Conclusion: node must not be replicated (private / encryption gate). */
export const NodeExcludedFromSync = defineFact<
  "replication.NodeExcludedFromSync",
  { id: string; reason: string }
>("replication.NodeExcludedFromSync");

// ---------------------------------------------------------------------------
// Access-Control vocabulary
// ---------------------------------------------------------------------------

/** Trigger: check whether an actor may perform an operation on a node. */
export const CheckAccess = defineEvent<
  "access.CHECK_REQUEST",
  {
    operation: "read" | "write" | "traverse" | "delete";
    nodeId: string;
    actor: string;
    acl?: string[];
  }
>("access.CHECK_REQUEST");

/** Conclusion: the operation is permitted. */
export const AccessGranted = defineFact<
  "access.AccessGranted",
  { operation: "read" | "write" | "traverse" | "delete"; nodeId: string; actor: string }
>("access.AccessGranted");

/** Conclusion: the operation is denied. */
export const AccessDenied = defineFact<
  "access.AccessDenied",
  {
    operation: "read" | "write" | "traverse" | "delete";
    nodeId: string;
    actor: string;
    reasons: string[];
  }
>("access.AccessDenied");

// ---------------------------------------------------------------------------
// Data-Integrity vocabulary
// ---------------------------------------------------------------------------

/** Trigger: check referential integrity and cycle prevention for a mutation. */
export const CheckIntegrity = defineEvent<
  "integrity.CHECK_MUTATION",
  {
    nodeId: string;
    data: Record<string, unknown>;
    existingIds: string[];
  }
>("integrity.CHECK_MUTATION");

/** Conclusion: all integrity constraints passed. */
export const IntegrityPassed = defineFact<
  "integrity.IntegrityPassed",
  { nodeId: string }
>("integrity.IntegrityPassed");

/** Conclusion: one or more integrity constraints failed. */
export const IntegrityFailed = defineFact<
  "integrity.IntegrityFailed",
  { nodeId: string; violations: string[] }
>("integrity.IntegrityFailed");
