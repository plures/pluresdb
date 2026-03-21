/**
 * PluresDB Praxis — Replication-Policy Module
 *
 * Determines which nodes are eligible for P2P synchronisation and enforces
 * encryption requirements for nodes that opt-in to encrypted replication.
 *
 * Rules
 * ─────
 * • replication-policy.checkPrivate    — `_private: true` nodes are excluded
 * • replication-policy.checkEncryption — `_encrypt: true` requires `_encrypted_key`
 * • replication-policy.allowSync       — nodes that pass all gates are eligible
 *
 * Constraints
 * ───────────
 * • replication-policy.encryptionMetadataRequired
 *     Nodes that request encryption must carry the `_encrypted_key` field.
 */

import {
  defineConstraint,
  defineModule,
  defineRule,
  RuleResult,
} from "@plures/praxis";
import {
  CheckReplicationEligibility,
  NodeEligibleForSync,
  NodeExcludedFromSync,
} from "./events.ts";

// ---------------------------------------------------------------------------
// Context
// ---------------------------------------------------------------------------

export interface ReplicationPolicyContext {
  replicationCandidate: {
    id: string;
    data: Record<string, unknown>;
  } | null;
}

// ---------------------------------------------------------------------------
// Rules
// ---------------------------------------------------------------------------

const checkPrivate = defineRule<ReplicationPolicyContext>({
  id: "replication-policy.checkPrivate",
  description:
    "Nodes marked with `_private: true` must not be replicated to peers.",
  eventTypes: [CheckReplicationEligibility.tag],
  impl: (_state, events) => {
    const evt = events.find(CheckReplicationEligibility.is);
    if (!evt) return RuleResult.noop();

    const { id, data } = evt.payload;
    if (data["_private"] === true) {
      return RuleResult.emit([
        NodeExcludedFromSync.create({
          id,
          reason: "Node is marked private and must not be replicated",
        }),
      ]);
    }
    return RuleResult.noop();
  },
});

const checkEncryption = defineRule<ReplicationPolicyContext>({
  id: "replication-policy.checkEncryption",
  description:
    "Nodes that request encryption (`_encrypt: true`) must carry `_encrypted_key` before replication.",
  eventTypes: [CheckReplicationEligibility.tag],
  impl: (_state, events) => {
    const evt = events.find(CheckReplicationEligibility.is);
    if (!evt) return RuleResult.noop();

    const { id, data } = evt.payload;
    if (data["_encrypt"] === true && !data["_encrypted_key"]) {
      return RuleResult.emit([
        NodeExcludedFromSync.create({
          id,
          reason:
            "Node requests encryption but is missing `_encrypted_key`; replication blocked",
        }),
      ]);
    }
    return RuleResult.noop();
  },
});

const allowSync = defineRule<ReplicationPolicyContext>({
  id: "replication-policy.allowSync",
  description:
    "Nodes that are not private and satisfy encryption requirements are eligible for sync.",
  eventTypes: [CheckReplicationEligibility.tag],
  impl: (_state, events) => {
    const evt = events.find(CheckReplicationEligibility.is);
    if (!evt) return RuleResult.noop();

    const { id, data } = evt.payload;
    const isPrivate = data["_private"] === true;
    const needsEncryption = data["_encrypt"] === true;
    const hasEncryptionKey = Boolean(data["_encrypted_key"]);

    if (isPrivate) return RuleResult.noop();
    if (needsEncryption && !hasEncryptionKey) return RuleResult.noop();

    return RuleResult.emit([NodeEligibleForSync.create({ id })]);
  },
});

// ---------------------------------------------------------------------------
// Constraints
// ---------------------------------------------------------------------------

const encryptionMetadataRequired = defineConstraint<ReplicationPolicyContext>({
  id: "replication-policy.encryptionMetadataRequired",
  description:
    "A replication candidate that declares `_encrypt: true` must also carry `_encrypted_key`.",
  impl: (state) => {
    const c = state?.context?.replicationCandidate;
    if (!c) return true;
    if (c.data["_encrypt"] !== true) return true;
    return (
      Boolean(c.data["_encrypted_key"])
    ) || "Encrypted replication requires `_encrypted_key` to be present";
  },
});

// ---------------------------------------------------------------------------
// Module
// ---------------------------------------------------------------------------

export const replicationPolicyModule = defineModule<ReplicationPolicyContext>({
  rules: [checkPrivate, checkEncryption, allowSync],
  constraints: [encryptionMetadataRequired],
  meta: {
    name: "replication-policy",
    description:
      "Sync eligibility, encryption requirements, and peer trust gates",
  },
});
