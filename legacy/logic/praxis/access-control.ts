/**
 * PluresDB Praxis — Access-Control Module
 *
 * Permission gates for read, write, traverse, and delete operations on the
 * graph.  When no ACL is configured all reads are permitted; write and delete
 * operations always require a non-empty actor identity.
 *
 * Rules
 * ─────
 * • access-control.checkWritePermission — write/delete require a non-empty actor
 * • access-control.checkAcl             — if an ACL list is provided the actor
 *                                         must appear in it
 * • access-control.allowRead            — reads without an ACL are permitted
 *
 * Constraints
 * ───────────
 * • access-control.actorRequiredForWrite
 *     The `actor` field must be a non-empty string for write and delete ops.
 */

import {
  defineConstraint,
  defineModule,
  defineRule,
  RuleResult,
} from "@plures/praxis";

import {
  AccessDenied,
  AccessGranted,
  CheckAccess,
} from "./events.ts";

// ---------------------------------------------------------------------------
// Context
// ---------------------------------------------------------------------------

export interface AccessControlContext {
  accessRequest: {
    operation: "read" | "write" | "traverse" | "delete";
    nodeId: string;
    actor: string;
    acl?: string[];
  } | null;
}

// ---------------------------------------------------------------------------
// Rules
// ---------------------------------------------------------------------------

const checkWritePermission = defineRule<AccessControlContext>({
  id: "access-control.checkWritePermission",
  description:
    "Write and delete operations require a non-empty actor identity.",
  eventTypes: [CheckAccess.tag],
  impl: (_state, events) => {
    const evt = events.find(CheckAccess.is);
    if (!evt) return RuleResult.noop();

    const { operation, nodeId, actor } = evt.payload;
    if (operation !== "write" && operation !== "delete") return RuleResult.noop();

    if (!actor || actor.trim().length === 0) {
      return RuleResult.emit([
        AccessDenied.create({
          operation,
          nodeId,
          actor,
          reasons: [
            `Actor identity is required for ${operation} operations`,
          ],
        }),
      ]);
    }
    return RuleResult.noop();
  },
});

const checkAcl = defineRule<AccessControlContext>({
  id: "access-control.checkAcl",
  description:
    "When an ACL list is present the requesting actor must be included in it.",
  eventTypes: [CheckAccess.tag],
  impl: (_state, events) => {
    const evt = events.find(CheckAccess.is);
    if (!evt) return RuleResult.noop();

    const { operation, nodeId, actor, acl } = evt.payload;
    if (!acl || acl.length === 0) return RuleResult.noop();

    if (!acl.includes(actor)) {
      return RuleResult.emit([
        AccessDenied.create({
          operation,
          nodeId,
          actor,
          reasons: [`Actor "${actor}" is not listed in the node ACL`],
        }),
      ]);
    }
    return RuleResult.emit([AccessGranted.create({ operation, nodeId, actor })]);
  },
});

const allowRead = defineRule<AccessControlContext>({
  id: "access-control.allowRead",
  description:
    "Read and traverse operations without an ACL are permitted by default.",
  eventTypes: [CheckAccess.tag],
  impl: (_state, events) => {
    const evt = events.find(CheckAccess.is);
    if (!evt) return RuleResult.noop();

    const { operation, nodeId, actor, acl } = evt.payload;
    if (operation !== "read" && operation !== "traverse") return RuleResult.noop();
    if (acl && acl.length > 0) return RuleResult.noop();

    return RuleResult.emit([AccessGranted.create({ operation, nodeId, actor })]);
  },
});

// ---------------------------------------------------------------------------
// Constraints
// ---------------------------------------------------------------------------

const actorRequiredForWrite = defineConstraint<AccessControlContext>({
  id: "access-control.actorRequiredForWrite",
  description:
    "The `actor` field must be a non-empty string for write and delete operations.",
  impl: (state) => {
    const req = state?.context?.accessRequest;
    if (!req) return true;
    if (req.operation !== "write" && req.operation !== "delete") return true;
    return (
      typeof req.actor === "string" && req.actor.trim().length > 0
    ) || "Actor identity is required for write and delete operations";
  },
});

// ---------------------------------------------------------------------------
// Module
// ---------------------------------------------------------------------------

export const accessControlModule = defineModule<AccessControlContext>({
  rules: [checkWritePermission, checkAcl, allowRead],
  constraints: [actorRequiredForWrite],
  meta: {
    name: "access-control",
    description:
      "Permission gates for read, write, traverse, and delete operations on the graph",
  },
});
