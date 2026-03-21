/**
 * PluresDB Praxis — Graph-Validation Module
 *
 * Enforces schema constraints and field-level rules on every node mutation
 * before the write reaches the CRDT store.
 *
 * Rules
 * ─────
 * • graph-validation.validateId      — node ID must be a non-empty string
 * • graph-validation.validateType    — `_type` field, when present, must be a string
 * • graph-validation.validatePayload — mutation data must be a plain object
 *
 * Constraints
 * ───────────
 * • graph-validation.noNullPayload   — pending mutation data may not be null
 */

import {
  defineConstraint,
  defineModule,
  defineRule,
  RuleResult,
} from "@plures/praxis";
import {
  NodeMutationInvalid,
  NodeMutationValid,
  ValidateMutation,
} from "./events.ts";

// ---------------------------------------------------------------------------
// Context
// ---------------------------------------------------------------------------

export interface GraphValidationContext {
  pendingMutation: {
    id: string;
    data: Record<string, unknown>;
    operation: "put" | "delete";
  } | null;
}

// ---------------------------------------------------------------------------
// Rules
// ---------------------------------------------------------------------------

const validateId = defineRule<GraphValidationContext>({
  id: "graph-validation.validateId",
  description:
    "Node ID must be a non-empty string containing no whitespace-only characters.",
  eventTypes: [ValidateMutation.tag],
  impl: (_state, events) => {
    const evt = events.find(ValidateMutation.is);
    if (!evt) return RuleResult.noop();

    const { id } = evt.payload;
    if (typeof id !== "string" || id.trim().length === 0) {
      return RuleResult.emit([
        NodeMutationInvalid.create({
          id,
          reasons: ["Node ID must be a non-empty string"],
        }),
      ]);
    }
    return RuleResult.noop();
  },
});

const validateType = defineRule<GraphValidationContext>({
  id: "graph-validation.validateType",
  description:
    "When a `_type` field is present on the mutation payload it must be a non-empty string.",
  eventTypes: [ValidateMutation.tag],
  impl: (_state, events) => {
    const evt = events.find(ValidateMutation.is);
    if (!evt) return RuleResult.noop();

    const { id, data, operation } = evt.payload;
    if (operation === "delete") return RuleResult.noop();

    const typeField = data["_type"];
    if (typeField !== undefined) {
      if (typeof typeField !== "string" || typeField.trim().length === 0) {
        return RuleResult.emit([
          NodeMutationInvalid.create({
            id,
            reasons: ["`_type` field must be a non-empty string when present"],
          }),
        ]);
      }
    }
    return RuleResult.noop();
  },
});

const validatePayload = defineRule<GraphValidationContext>({
  id: "graph-validation.validatePayload",
  description:
    "For put operations the mutation data must be a plain object (not an array or primitive).",
  eventTypes: [ValidateMutation.tag],
  impl: (_state, events) => {
    const evt = events.find(ValidateMutation.is);
    if (!evt) return RuleResult.noop();

    const { id, data, operation } = evt.payload;
    if (operation === "delete") {
      // Deletes skip payload-shape validation but still emit a positive validation fact.
      return RuleResult.emit([NodeMutationValid.create({ id })]);
    }

    const isPlain =
      data !== null &&
      typeof data === "object" &&
      !Array.isArray(data);

    if (!isPlain) {
      return RuleResult.emit([
        NodeMutationInvalid.create({
          id,
          reasons: ["Mutation data must be a plain object"],
        }),
      ]);
    }

    // Avoid emitting a contradictory "valid" fact if an invalid fact
    // has already been produced for this mutation in the current step.
    const hasPriorInvalidForId = events.some(
      (e) => NodeMutationInvalid.is(e) && e.payload.id === id,
    );
    if (hasPriorInvalidForId) {
      return RuleResult.noop();
    }
    return RuleResult.emit([NodeMutationValid.create({ id })]);
  },
});

// ---------------------------------------------------------------------------
// Constraints
// ---------------------------------------------------------------------------

const noNullPayload = defineConstraint<GraphValidationContext>({
  id: "graph-validation.noNullPayload",
  description:
    "A pending put mutation must carry a non-null data object before the engine processes it.",
  impl: (state) => {
    const m = state?.context?.pendingMutation;
    if (!m || m.operation === "delete") return true;
    return (
      m.data !== null && typeof m.data === "object" && !Array.isArray(m.data)
    ) || "Pending mutation data must be a non-null plain object";
  },
});

// ---------------------------------------------------------------------------
// Module
// ---------------------------------------------------------------------------

export const graphValidationModule = defineModule<GraphValidationContext>({
  rules: [validateId, validateType, validatePayload],
  constraints: [noNullPayload],
  meta: {
    name: "graph-validation",
    description:
      "Schema enforcement and field constraints on graph node mutations",
  },
});
