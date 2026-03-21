/**
 * PluresDB Praxis — Data-Integrity Module
 *
 * Referential-integrity checks, orphan detection, and cycle prevention for
 * graph mutations.  These rules run after a mutation has been structurally
 * validated (graph-validation module) but before it is committed.
 *
 * Rules
 * ─────
 * • data-integrity.checkOrphanRefs  — values that look like node IDs (strings
 *                                     containing ":") must resolve to existing
 *                                     nodes; unresolved refs are orphans
 * • data-integrity.checkSelfRef     — a node must not reference its own ID
 * • data-integrity.passIntegrity    — emits IntegrityPassed when no violations
 *                                     were found
 *
 * Constraints
 * ───────────
 * • data-integrity.noSelfReference  — the integrity-check payload must not
 *                                     carry a reference to its own node ID
 */

import {
  defineConstraint,
  defineModule,
  defineRule,
  RuleResult,
} from "@plures/praxis";
import {
  CheckIntegrity,
  IntegrityFailed,
  IntegrityPassed,
} from "./events.ts";

// ---------------------------------------------------------------------------
// Context
// ---------------------------------------------------------------------------

export interface DataIntegrityContext {
  integrityCheck: {
    nodeId: string;
    data: Record<string, unknown>;
    existingIds: string[];
  } | null;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Recursively collect string values that look like graph node IDs (`*:*`). */
function collectRefs(data: unknown, seen = new Set<string>()): string[] {
  if (typeof data === "string") {
    if (data.includes(":")) seen.add(data);
  } else if (Array.isArray(data)) {
    for (const item of data) collectRefs(item, seen);
  } else if (data !== null && typeof data === "object") {
    for (const val of Object.values(data as Record<string, unknown>)) {
      collectRefs(val, seen);
    }
  }
  return [...seen];
}

// ---------------------------------------------------------------------------
// Rules
// ---------------------------------------------------------------------------

const checkOrphanRefs = defineRule<DataIntegrityContext>({
  id: "data-integrity.checkOrphanRefs",
  description:
    "String values that look like node IDs (contain ':') must resolve to existing nodes.",
  eventTypes: [CheckIntegrity.tag],
  impl: (_state, events) => {
    const evt = events.find(CheckIntegrity.is);
    if (!evt) return RuleResult.noop();

    const { nodeId, data, existingIds } = evt.payload;
    const existingSet = new Set(existingIds);
    const refs = collectRefs(data);
    const orphans = refs.filter((ref) => !existingSet.has(ref));

    if (orphans.length > 0) {
      return RuleResult.emit([
        IntegrityFailed.create({
          nodeId,
          violations: orphans.map(
            (ref) => `Reference "${ref}" does not resolve to an existing node`,
          ),
        }),
      ]);
    }
    return RuleResult.noop();
  },
});

const checkSelfRef = defineRule<DataIntegrityContext>({
  id: "data-integrity.checkSelfRef",
  description: "A node must not contain a reference to its own ID.",
  eventTypes: [CheckIntegrity.tag],
  impl: (_state, events) => {
    const evt = events.find(CheckIntegrity.is);
    if (!evt) return RuleResult.noop();

    const { nodeId, data } = evt.payload;
    const refs = collectRefs(data);

    if (refs.includes(nodeId)) {
      return RuleResult.emit([
        IntegrityFailed.create({
          nodeId,
          violations: [`Node "${nodeId}" contains a self-reference`],
        }),
      ]);
    }
    return RuleResult.noop();
  },
});

const passIntegrity = defineRule<DataIntegrityContext>({
  id: "data-integrity.passIntegrity",
  description:
    "Emits IntegrityPassed when the mutation contains no orphan references and no self-reference.",
  eventTypes: [CheckIntegrity.tag],
  impl: (_state, events) => {
    const evt = events.find(CheckIntegrity.is);
    if (!evt) return RuleResult.noop();

    const { nodeId, data, existingIds } = evt.payload;
    const existingSet = new Set(existingIds);
    const refs = collectRefs(data);
    const hasSelfRef = refs.includes(nodeId);
    const hasOrphans = refs.some((ref) => !existingSet.has(ref));

    if (hasSelfRef || hasOrphans) return RuleResult.noop();
    return RuleResult.emit([IntegrityPassed.create({ nodeId })]);
  },
});

// ---------------------------------------------------------------------------
// Constraints
// ---------------------------------------------------------------------------

const noSelfReference = defineConstraint<DataIntegrityContext>({
  id: "data-integrity.noSelfReference",
  description:
    "The pending integrity-check payload must not carry a direct reference to its own node ID.",
  impl: (state) => {
    const check = state?.context?.integrityCheck;
    if (!check) return true;
    const refs = collectRefs(check.data);
    return (
      !refs.includes(check.nodeId)
    ) || `Node "${check.nodeId}" must not reference itself`;
  },
});

// ---------------------------------------------------------------------------
// Module
// ---------------------------------------------------------------------------

export const dataIntegrityModule = defineModule<DataIntegrityContext>({
  rules: [checkOrphanRefs, checkSelfRef, passIntegrity],
  constraints: [noSelfReference],
  meta: {
    name: "data-integrity",
    description:
      "Referential integrity, orphan detection, and graph cycle prevention",
  },
});
