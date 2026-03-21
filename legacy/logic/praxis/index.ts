/**
 * PluresDB Praxis — Logic Modules Index
 *
 * Exports all four PluresDB Praxis modules and provides a convenience
 * `buildPluresDBPraxisRegistry()` factory that registers every module in a
 * single `PraxisRegistry` ready for use with `createPraxisEngine`.
 *
 * ## Modules
 * | Module               | Responsibility                                              |
 * |----------------------|-------------------------------------------------------------|
 * | `graph-validation`   | Schema enforcement + field constraints on node mutations   |
 * | `replication-policy` | Sync eligibility, encryption gates, peer trust             |
 * | `access-control`     | Permission gates for read / write / traverse / delete      |
 * | `data-integrity`     | Referential integrity, orphan detection, cycle prevention  |
 *
 * ## Usage
 * ```ts
 * import { buildPluresDBPraxisRegistry, ValidateMutation } from "./praxis/index.ts";
 * import { createPraxisEngine } from "@plures/praxis";
 *
 * const registry = buildPluresDBPraxisRegistry();
 * const engine = createPraxisEngine({ initialContext: {}, registry });
 *
 * const result = engine.step([
 *   ValidateMutation.create({ id: "node:1", data: { name: "Alice" }, operation: "put" }),
 * ]);
 * ```
 */

import { PraxisRegistry } from "@plures/praxis";

import { graphValidationModule } from "./graph-validation.ts";
import { replicationPolicyModule } from "./replication-policy.ts";
import { accessControlModule } from "./access-control.ts";
import { dataIntegrityModule } from "./data-integrity.ts";

// Re-export modules
export { graphValidationModule } from "./graph-validation.ts";
export { replicationPolicyModule } from "./replication-policy.ts";
export { accessControlModule } from "./access-control.ts";
export { dataIntegrityModule } from "./data-integrity.ts";

// Re-export context types
export type { GraphValidationContext } from "./graph-validation.ts";
export type { ReplicationPolicyContext } from "./replication-policy.ts";
export type { AccessControlContext } from "./access-control.ts";
export type { DataIntegrityContext } from "./data-integrity.ts";

// Re-export all events and facts so callers have a single import point
export {
  // Graph-Validation
  ValidateMutation,
  NodeMutationValid,
  NodeMutationInvalid,
  // Replication-Policy
  CheckReplicationEligibility,
  NodeEligibleForSync,
  NodeExcludedFromSync,
  // Access-Control
  CheckAccess,
  AccessGranted,
  AccessDenied,
  // Data-Integrity
  CheckIntegrity,
  IntegrityPassed,
  IntegrityFailed,
} from "./events.ts";

// ---------------------------------------------------------------------------
// Combined context type
// ---------------------------------------------------------------------------

export interface PluresDBPraxisContext {
  pendingMutation: {
    id: string;
    data: Record<string, unknown>;
    operation: "put" | "delete";
  } | null;
  replicationCandidate: {
    id: string;
    data: Record<string, unknown>;
  } | null;
  accessRequest: {
    operation: "read" | "write" | "traverse" | "delete";
    nodeId: string;
    actor: string;
    acl?: string[];
  } | null;
  integrityCheck: {
    nodeId: string;
    data: Record<string, unknown>;
    existingIds: string[];
  } | null;
}

// ---------------------------------------------------------------------------
// Registry factory
// ---------------------------------------------------------------------------

/**
 * Build a `PraxisRegistry` pre-loaded with all four PluresDB logic modules.
 *
 * Each module's rules and constraints are registered individually so that
 * callers can inspect them via `registry.getRuleIds()` and
 * `registry.getConstraintIds()`.
 *
 * @returns A ready-to-use `PraxisRegistry` instance.
 */
export function buildPluresDBPraxisRegistry(): PraxisRegistry {
  const registry = new PraxisRegistry();

  for (const mod of [
    graphValidationModule,
    replicationPolicyModule,
    accessControlModule,
    dataIntegrityModule,
  ]) {
    registry.registerModule(mod);
  }

  return registry;
}
