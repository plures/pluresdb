//! AI-specific procedures for PluresDB.
//!
//! These procedures are purpose-built for AI agent operations running on
//! Chronos chronicle data — graph nodes that represent state transitions and
//! decisions connected by causal edges.
//!
//! ## Chronicle node schema
//!
//! All procedures operate on nodes that follow this layout (fields marked
//! with `*` are optional):
//!
//! ```json
//! {
//!   "_type":          "chronos:decision",
//!   "route":          "analytical | creative | quick",
//!   "outcome":        "accepted | corrected | abandoned",  // *
//!   "input_context":  { "token_count": 42, "message_type": "...", "keywords": [...] },  // *
//!   "causal_parent":  "<node-id>",   // * link to prior decision in chain
//!   "session_id":     "...",         // *
//!   "reward":         0.5,           // * written by chronos_reward_signal
//!   "original_output":"...",         // * for correction nodes
//!   "corrected_output":"...",        // * for correction nodes
//!   "weight_suggestions": {...}      // * written by cerebellum_tune
//! }
//! ```
//!
//! ## Procedures
//!
//! | Function                        | Trigger             | Description                               |
//! |---------------------------------|---------------------|-------------------------------------------|
//! | [`chronos_decision_audit`]      | cron / manual       | Decision accuracy report for last N hours |
//! | [`chronos_extract_trajectories`]| manual              | (state, action, outcome) JSONL for RL     |
//! | [`chronos_preference_pairs`]    | after_store         | DPO preference pairs from corrections     |
//! | [`chronos_reward_signal`]       | on_cue("session-end")| Heuristic reward assignment               |
//! | [`cerebellum_tune`]             | cron / manual       | Router accuracy analysis + weight hints   |
//! | [`memory_relevance_tune`]       | cron                | Memory recall relevance scoring           |
//! | [`chronos_replay`]              | manual              | Dry-run causal chain with new parameters  |

use std::collections::HashMap;

use uuid::Uuid;

use chrono::{Duration, Utc};

use crate::CrdtStore;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Default look-back window for the decision audit (in hours).
const DEFAULT_AUDIT_HOURS: i64 = 24;

/// Namespace UUID for deterministic ID derivation in AI procedures.
///
/// This is a randomly chosen, project-specific UUID in the RFC 4122 namespace
/// format.  It must remain stable across versions to ensure that deterministic
/// IDs produced by [`derive_id`] (and therefore trajectory / preference-pair
/// node IDs stored in the CRDT store) are reproducible and idempotent.  Do not
/// change this value once data has been written to production stores.
const AI_PROC_NS: Uuid = Uuid::from_bytes([
    0xae, 0x7f, 0x1c, 0x3b, 0x5d, 0x92, 0x4e, 0x11, 0x9a, 0x7d, 0x02, 0x42, 0xac, 0x14, 0x00,
    0x07,
]);

// ---------------------------------------------------------------------------
// Public result types
// ---------------------------------------------------------------------------

/// Accuracy report produced by [`chronos_decision_audit`].
#[derive(Debug, Clone, PartialEq)]
pub struct DecisionAuditReport {
    /// Total number of decision nodes examined.
    pub total_decisions: usize,
    /// Number of decisions with `outcome == "accepted"`.
    pub accepted: usize,
    /// Number of decisions with `outcome == "corrected"`.
    pub corrected: usize,
    /// Number of decisions with `outcome == "abandoned"`.
    pub abandoned: usize,
    /// Accuracy rate: `accepted / total_decisions` (0.0 when total is 0).
    pub accuracy: f64,
    /// Most common failure patterns: `route` → count.
    pub failure_patterns: HashMap<String, usize>,
}

/// A single (state, action, outcome) tuple for RL training.
#[derive(Debug, Clone, PartialEq)]
pub struct Trajectory {
    /// Unique identifier for this trajectory.
    pub id: String,
    /// Serialised context at the decision node.
    pub state: serde_json::Value,
    /// The branch taken (route / model choice / parameter).
    pub action: String,
    /// Terminal outcome: `"success"` or `"failure"`.
    pub outcome: String,
    /// Source decision node ID.
    pub decision_id: String,
}

/// A DPO preference pair produced by [`chronos_preference_pairs`].
#[derive(Debug, Clone, PartialEq)]
pub struct PreferencePair {
    /// Unique identifier for this pair.
    pub id: String,
    /// Original prompt / context that led to both outputs.
    pub prompt: String,
    /// The preferred (correct / accepted) output.
    pub chosen: String,
    /// The original (non-preferred) output that was corrected or rejected by the user.
    pub rejected: String,
    /// Source correction node ID.
    pub correction_id: String,
}

/// Summary produced by [`cerebellum_tune`].
#[derive(Debug, Clone, PartialEq)]
pub struct CerebellumTuneReport {
    /// Per-route accuracy: `route` → accuracy (0.0–1.0).
    pub route_accuracy: HashMap<String, f64>,
    /// Routes that are underperforming (accuracy below threshold).
    pub underperforming_routes: Vec<String>,
    /// Suggested weight adjustments: `route` → adjustment factor.
    pub weight_suggestions: HashMap<String, f64>,
    /// Whether the suggestions were auto-applied (requires human approval gate).
    pub auto_applied: bool,
}

/// Summary produced by [`memory_relevance_tune`].
#[derive(Debug, Clone, PartialEq)]
pub struct MemoryRelevanceTuneReport {
    /// Per-category relevance score: `category` → 0.0–1.0.
    pub category_scores: HashMap<String, f64>,
    /// Categories with below-threshold relevance that may be pruned.
    pub low_relevance_categories: Vec<String>,
    /// Whether relevance weights were updated in the store.
    pub weights_updated: bool,
}

/// Replay report produced by [`chronos_replay`].
#[derive(Debug, Clone, PartialEq)]
pub struct ReplayReport {
    /// Number of nodes in the original causal chain.
    pub original_length: usize,
    /// Number of nodes visited during the replay.
    pub replayed_length: usize,
    /// Divergence points: node IDs where the replay outcome differed.
    pub divergence_points: Vec<String>,
    /// Per-node comparison summary: `node_id` → diff description.
    pub diffs: HashMap<String, String>,
}

// ---------------------------------------------------------------------------
// 1. chronos_decision_audit
// ---------------------------------------------------------------------------

/// Walk routing decisions from the last `hours` hours and produce an accuracy
/// and failure-pattern report.
///
/// For each `chronos:decision` node within the time window the procedure:
/// - Extracts the `route` (analytical / creative / quick).
/// - Examines `outcome` (accepted / corrected / abandoned).
/// - Counts outcomes per route type to surface failure patterns.
///
/// # Arguments
///
/// * `store` — the CRDT store to read from.
/// * `actor` — logical actor / author identifier.
/// * `hours` — look-back window in hours (`None` defaults to 24 hours).
///
/// # Returns
///
/// A [`DecisionAuditReport`] with counts, accuracy, and failure patterns.
///
/// # Errors
///
/// Returns an error when `actor` is empty.
///
/// # Examples
///
/// ```rust
/// use pluresdb_core::{CrdtStore, procedures::ai_procedures::chronos_decision_audit};
///
/// let store = CrdtStore::default();
/// store.put("d1", "actor", serde_json::json!({
///     "_type": "chronos:decision",
///     "route": "analytical",
///     "outcome": "accepted",
/// }));
/// let report = chronos_decision_audit(&store, "actor", None).unwrap();
/// assert_eq!(report.total_decisions, 1);
/// assert_eq!(report.accepted, 1);
/// ```
pub fn chronos_decision_audit(
    store: &CrdtStore,
    actor: &str,
    hours: Option<i64>,
) -> anyhow::Result<DecisionAuditReport> {
    anyhow::ensure!(!actor.is_empty(), "chronos_decision_audit: actor must not be empty");
    let hours = hours.unwrap_or(DEFAULT_AUDIT_HOURS);
    let cutoff = Utc::now() - Duration::hours(hours);

    let decisions: Vec<_> = store
        .list()
        .into_iter()
        .filter(|n| {
            n.timestamp >= cutoff
                && n.data.get("_type").and_then(|v| v.as_str()) == Some("chronos:decision")
        })
        .collect();

    let mut total = 0usize;
    let mut accepted = 0usize;
    let mut corrected = 0usize;
    let mut abandoned = 0usize;
    // failure_patterns: route → count of non-accepted outcomes
    let mut failure_patterns: HashMap<String, usize> = HashMap::new();

    for node in &decisions {
        total += 1;
        let outcome = node.data.get("outcome").and_then(|v| v.as_str()).unwrap_or("unknown");
        let route = node
            .data
            .get("route")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_owned();

        match outcome {
            "accepted" => accepted += 1,
            "corrected" => {
                corrected += 1;
                *failure_patterns.entry(route).or_insert(0) += 1;
            }
            "abandoned" => {
                abandoned += 1;
                *failure_patterns.entry(route).or_insert(0) += 1;
            }
            _ => {}
        }
    }

    let accuracy = if total > 0 { accepted as f64 / total as f64 } else { 0.0 };

    Ok(DecisionAuditReport { total_decisions: total, accepted, corrected, abandoned, accuracy, failure_patterns })
}

// ---------------------------------------------------------------------------
// 2. chronos_extract_trajectories
// ---------------------------------------------------------------------------

/// Extract (state, action, outcome) tuples from causal chains for RL training.
///
/// For each `chronos:decision` node the procedure constructs a [`Trajectory`]
/// where:
/// - **state** = the `input_context` field (serialised context).
/// - **action** = the `route` field (model / branch choice).
/// - **outcome** = `"success"` when `outcome == "accepted"`, `"failure"` otherwise.
///
/// Each trajectory is written to the store as a `chronos:trajectory` node and
/// also returned in the result vector.  The procedure is idempotent: trajectory
/// IDs are derived deterministically from the source decision node ID.
///
/// # Arguments
///
/// * `store` — the CRDT store to read from and write into.
/// * `actor` — logical actor / author identifier.
///
/// # Returns
///
/// A `Vec<Trajectory>` of all generated trajectories.
///
/// # Errors
///
/// Returns an error when `actor` is empty.
///
/// # Examples
///
/// ```rust
/// use pluresdb_core::{CrdtStore, procedures::ai_procedures::chronos_extract_trajectories};
///
/// let store = CrdtStore::default();
/// store.put("d1", "actor", serde_json::json!({
///     "_type": "chronos:decision",
///     "route": "analytical",
///     "outcome": "accepted",
///     "input_context": {"token_count": 42},
/// }));
/// let trajectories = chronos_extract_trajectories(&store, "actor").unwrap();
/// assert_eq!(trajectories.len(), 1);
/// assert_eq!(trajectories[0].action, "analytical");
/// assert_eq!(trajectories[0].outcome, "success");
/// ```
pub fn chronos_extract_trajectories(
    store: &CrdtStore,
    actor: &str,
) -> anyhow::Result<Vec<Trajectory>> {
    anyhow::ensure!(!actor.is_empty(), "chronos_extract_trajectories: actor must not be empty");

    let decisions: Vec<_> = store
        .list()
        .into_iter()
        .filter(|n| n.data.get("_type").and_then(|v| v.as_str()) == Some("chronos:decision"))
        .collect();

    let mut trajectories = Vec::new();

    for node in &decisions {
        let action = node
            .data
            .get("route")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_owned();

        let raw_outcome = node.data.get("outcome").and_then(|v| v.as_str()).unwrap_or("unknown");
        let outcome = if raw_outcome == "accepted" { "success" } else { "failure" }.to_owned();

        let state = node
            .data
            .get("input_context")
            .cloned()
            .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));

        let traj_id = derive_id(&["trajectory", node.id.as_str()]);

        let traj = Trajectory {
            id: traj_id.clone(),
            state: state.clone(),
            action: action.clone(),
            outcome: outcome.clone(),
            decision_id: node.id.clone(),
        };

        // Persist as a store node (idempotent via deterministic ID).
        store.put(
            traj_id,
            actor,
            serde_json::json!({
                "_type":       "chronos:trajectory",
                "state":       state,
                "action":      action,
                "outcome":     outcome,
                "decision_id": &traj.decision_id,
            }),
        );

        trajectories.push(traj);
    }

    Ok(trajectories)
}

// ---------------------------------------------------------------------------
// 3. chronos_preference_pairs
// ---------------------------------------------------------------------------

/// Extract DPO preference pairs from user corrections in chronicle data.
///
/// For each `chronos:correction` node the procedure:
/// - Takes `original_output` as the **rejected** response.
/// - Takes `corrected_output` as the **chosen** response.
/// - Resolves the linked decision node to obtain the **prompt** context.
/// - Emits a `{ prompt, chosen, rejected }` pair.
///
/// Pairs are written as `chronos:preference_pair` nodes and returned.
/// The procedure is idempotent: pair IDs are derived deterministically.
///
/// # Arguments
///
/// * `store`        — the CRDT store to read from and write into.
/// * `actor`        — logical actor / author identifier.
/// * `correction_id`— ID of the `chronos:correction` node to process, or
///                    `None` to process all correction nodes in the store.
///
/// # Returns
///
/// A `Vec<PreferencePair>` of all generated pairs.
///
/// # Errors
///
/// Returns an error when `actor` is empty, or when `correction_id` is
/// `Some("")` (empty string).
///
/// # Examples
///
/// ```rust
/// use pluresdb_core::{CrdtStore, procedures::ai_procedures::chronos_preference_pairs};
///
/// let store = CrdtStore::default();
/// store.put("d1", "actor", serde_json::json!({
///     "_type": "chronos:decision",
///     "route": "analytical",
///     "input_context": {"token_count": 10},
/// }));
/// store.put("c1", "actor", serde_json::json!({
///     "_type": "chronos:correction",
///     "decision_id": "d1",
///     "original_output": "wrong answer",
///     "corrected_output": "right answer",
/// }));
/// let pairs = chronos_preference_pairs(&store, "actor", None).unwrap();
/// assert_eq!(pairs.len(), 1);
/// assert_eq!(pairs[0].chosen, "right answer");
/// assert_eq!(pairs[0].rejected, "wrong answer");
/// ```
pub fn chronos_preference_pairs(
    store: &CrdtStore,
    actor: &str,
    correction_id: Option<&str>,
) -> anyhow::Result<Vec<PreferencePair>> {
    anyhow::ensure!(!actor.is_empty(), "chronos_preference_pairs: actor must not be empty");
    if let Some(id) = correction_id {
        anyhow::ensure!(
            !id.is_empty(),
            "chronos_preference_pairs: correction_id must not be empty when provided"
        );
    }

    let corrections: Vec<_> = if let Some(id) = correction_id {
        store.get(id).map(|n| vec![n]).unwrap_or_default()
    } else {
        store
            .list()
            .into_iter()
            .filter(|n| {
                n.data.get("_type").and_then(|v| v.as_str()) == Some("chronos:correction")
            })
            .collect()
    };

    let mut pairs = Vec::new();

    for corr in &corrections {
        // Ensure we are processing a correction node when a specific ID was given.
        if corr.data.get("_type").and_then(|v| v.as_str()) != Some("chronos:correction") {
            continue;
        }

        let original = corr
            .data
            .get("original_output")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_owned();
        let corrected = corr
            .data
            .get("corrected_output")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_owned();

        if original.is_empty() || corrected.is_empty() {
            continue;
        }

        // Resolve the decision node to extract the prompt context.
        let decision_id =
            corr.data.get("decision_id").and_then(|v| v.as_str()).unwrap_or("").to_owned();
        let prompt = if !decision_id.is_empty() {
            store
                .get(&decision_id)
                .and_then(|d| {
                    // Use input_context as the prompt if available; else the route.
                    d.data
                        .get("input_context")
                        .map(|v| v.to_string())
                        .or_else(|| {
                            d.data.get("route").and_then(|v| v.as_str()).map(str::to_owned)
                        })
                })
                .unwrap_or_default()
        } else {
            String::new()
        };

        let pair_id = derive_id(&["preference_pair", corr.id.as_str()]);

        let pair = PreferencePair {
            id: pair_id.clone(),
            prompt: prompt.clone(),
            chosen: corrected.clone(),
            rejected: original.clone(),
            correction_id: corr.id.clone(),
        };

        store.put(
            pair_id,
            actor,
            serde_json::json!({
                "_type":         "chronos:preference_pair",
                "prompt":        prompt,
                "chosen":        corrected,
                "rejected":      original,
                "correction_id": corr.id.clone(),
            }),
        );

        pairs.push(pair);
    }

    Ok(pairs)
}

// ---------------------------------------------------------------------------
// 4. chronos_reward_signal
// ---------------------------------------------------------------------------

/// Compute heuristic reward signals for a session's causal chains.
///
/// Reward values assigned per decision node:
///
/// | Condition                                     | Reward |
/// |-----------------------------------------------|--------|
/// | User explicitly approved / continued naturally| +1.0   |
/// | No correction, session continued              | +0.5   |
/// | Unknown / missing outcome                     | +0.5   |
/// | User corrected a response                     | −0.5   |
/// | User abandoned or expressed frustration       | −1.0   |
///
/// The reward is written back into each decision node's `reward` field.
/// Credit is assigned along the causal path via a simple decay: nodes further
/// from the terminal event receive 90% of the prior node's reward.
///
/// # Arguments
///
/// * `store`      — the CRDT store to read from and write into.
/// * `actor`      — logical actor / author identifier.
/// * `session_id` — ID of the session whose chain(s) to process.
///
/// # Returns
///
/// A JSON object mapping each updated decision node ID to its assigned reward.
///
/// # Errors
///
/// Returns an error when `actor` or `session_id` is empty.
///
/// # Examples
///
/// ```rust
/// use pluresdb_core::{CrdtStore, procedures::ai_procedures::chronos_reward_signal};
///
/// let store = CrdtStore::default();
/// store.put("d1", "actor", serde_json::json!({
///     "_type": "chronos:decision",
///     "session_id": "sess-1",
///     "outcome": "accepted",
/// }));
/// let rewards = chronos_reward_signal(&store, "actor", "sess-1").unwrap();
/// let reward = rewards["d1"].as_f64().unwrap();
/// assert!(reward > 0.0);
/// ```
pub fn chronos_reward_signal(
    store: &CrdtStore,
    actor: &str,
    session_id: &str,
) -> anyhow::Result<serde_json::Value> {
    anyhow::ensure!(!actor.is_empty(), "chronos_reward_signal: actor must not be empty");
    anyhow::ensure!(!session_id.is_empty(), "chronos_reward_signal: session_id must not be empty");

    // Collect all decision nodes for this session.
    let session_decisions: Vec<_> = store
        .list()
        .into_iter()
        .filter(|n| {
            n.data.get("_type").and_then(|v| v.as_str()) == Some("chronos:decision")
                && n.data.get("session_id").and_then(|v| v.as_str()) == Some(session_id)
        })
        .collect();

    let mut reward_map: HashMap<String, f64> = HashMap::new();

    // Assign terminal rewards based on outcome and then propagate backward.
    for node in &session_decisions {
        let outcome = node.data.get("outcome").and_then(|v| v.as_str()).unwrap_or("unknown");
        let terminal_reward = outcome_to_reward(outcome);
        reward_map.insert(node.id.clone(), terminal_reward);
    }

    // Credit assignment: decay reward backward through causal_parent links.
    // We iterate until stable (bounded by chain length).
    const CREDIT_DECAY: f64 = 0.9;
    const MAX_ITERS: usize = 200;
    for _ in 0..MAX_ITERS {
        let mut changed = false;
        // Collect updates separately to avoid borrow conflicts.
        let updates: Vec<(String, f64)> = session_decisions
            .iter()
            .filter_map(|node| {
                let parent_id =
                    node.data.get("causal_parent").and_then(|v| v.as_str())?;
                // Only propagate if the parent is also in this session.
                let child_reward = *reward_map.get(&node.id)?;
                let parent_reward = reward_map.get(parent_id).copied().unwrap_or(0.0);
                let propagated = child_reward * CREDIT_DECAY;
                // Only update parent if propagated reward is stronger.
                if propagated.abs() > parent_reward.abs() {
                    Some((parent_id.to_owned(), propagated))
                } else {
                    None
                }
            })
            .collect();

        for (id, reward) in updates {
            let old = reward_map.insert(id, reward);
            let significantly_changed = match old {
                Some(prev) => (prev - reward).abs() > 1e-9,
                None => true,
            };
            if significantly_changed {
                changed = true;
            }
        }

        if !changed {
            break;
        }
    }

    // Write rewards back into each decision node.
    let mut result = serde_json::Map::new();
    for node in &session_decisions {
        if let Some(&reward) = reward_map.get(&node.id) {
            let mut updated = node.data.clone();
            if let Some(obj) = updated.as_object_mut() {
                obj.insert("reward".to_owned(), serde_json::json!(reward));
            }
            store.put(node.id.clone(), actor, updated);
            result.insert(node.id.clone(), serde_json::json!(reward));
        }
    }

    Ok(serde_json::Value::Object(result))
}

// ---------------------------------------------------------------------------
// 5. cerebellum_tune
// ---------------------------------------------------------------------------

/// Analyse cerebellum router accuracy from chronicle data and optionally
/// suggest or apply weight adjustments.
///
/// The procedure:
/// 1. Collects all `chronos:decision` nodes that have a `route` field.
/// 2. Computes accuracy per route type (`accepted / total`).
/// 3. Identifies routes that consistently underperform (below `threshold`).
/// 4. Suggests a weight reduction factor proportional to the error rate.
/// 5. If `auto_apply` is `true` **and** human approval has been explicitly
///    granted (i.e. a `chronos:approval` node exists with `approved: true`
///    for this tuning run), writes the weight suggestions back to a
///    `cerebellum:config` node in the store.
///
/// The approval gate prevents inadvertent auto-application: callers that want
/// automated tuning must first write a `chronos:approval` node with
/// `{ "_type": "chronos:approval", "procedure": "cerebellum_tune", "approved": true }`.
///
/// # Arguments
///
/// * `store`     — the CRDT store to read from and write into.
/// * `actor`     — logical actor / author identifier.
/// * `threshold` — minimum acceptable accuracy (default 0.7).  Routes below
///                 this value are flagged as underperforming.
/// * `auto_apply`— when `true`, apply weight suggestions if human approval
///                 has been granted in the store.
///
/// # Returns
///
/// A [`CerebellumTuneReport`].
///
/// # Errors
///
/// Returns an error when `actor` is empty.
///
/// # Examples
///
/// ```rust
/// use pluresdb_core::{CrdtStore, procedures::ai_procedures::cerebellum_tune};
///
/// let store = CrdtStore::default();
/// store.put("d1", "actor", serde_json::json!({
///     "_type": "chronos:decision", "route": "creative", "outcome": "accepted",
/// }));
/// let report = cerebellum_tune(&store, "actor", None, false).unwrap();
/// assert!(!report.route_accuracy.is_empty());
/// ```
pub fn cerebellum_tune(
    store: &CrdtStore,
    actor: &str,
    threshold: Option<f64>,
    auto_apply: bool,
) -> anyhow::Result<CerebellumTuneReport> {
    anyhow::ensure!(!actor.is_empty(), "cerebellum_tune: actor must not be empty");
    let min_acc = threshold.unwrap_or(0.7).clamp(0.0, 1.0);

    let decisions: Vec<_> = store
        .list()
        .into_iter()
        .filter(|n| n.data.get("_type").and_then(|v| v.as_str()) == Some("chronos:decision"))
        .collect();

    // Per-route counters.
    let mut total_per_route: HashMap<String, usize> = HashMap::new();
    let mut accepted_per_route: HashMap<String, usize> = HashMap::new();

    for node in &decisions {
        let route = node
            .data
            .get("route")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_owned();
        let outcome = node.data.get("outcome").and_then(|v| v.as_str()).unwrap_or("unknown");

        *total_per_route.entry(route.clone()).or_insert(0) += 1;
        if outcome == "accepted" {
            *accepted_per_route.entry(route).or_insert(0) += 1;
        }
    }

    let mut route_accuracy: HashMap<String, f64> = HashMap::new();
    let mut underperforming: Vec<String> = Vec::new();
    let mut weight_suggestions: HashMap<String, f64> = HashMap::new();

    for (route, total) in &total_per_route {
        let acc = if *total > 0 {
            *accepted_per_route.get(route).unwrap_or(&0) as f64 / *total as f64
        } else {
            0.0
        };
        route_accuracy.insert(route.clone(), acc);

        if acc < min_acc {
            underperforming.push(route.clone());
            // Suggest reducing this route's weight proportionally to the deficit.
            let deficit = min_acc - acc;
            let factor = (1.0 - deficit).clamp(0.1, 0.99);
            weight_suggestions.insert(route.clone(), factor);
        }
    }
    // Sort for deterministic output across runs and peers.
    underperforming.sort_unstable();

    // Human approval gate: only auto-apply when an approval node exists.
    // Use deterministic approval node ID instead of scanning the entire store.
    let approval_node_id = "approval:cerebellum_tune";
    let approval_granted = store
        .get(approval_node_id)
        .map(|n| {
            n.data.get("_type").and_then(|v| v.as_str()) == Some("chronos:approval")
                && n.data.get("procedure").and_then(|v| v.as_str()) == Some("cerebellum_tune")
                && n.data.get("approved").and_then(|v| v.as_bool()).unwrap_or(false)
        })
        .unwrap_or(false);

    let auto_applied = auto_apply && approval_granted;

    if auto_applied {
        // Write weight suggestions into a cerebellum:config node.
        let config_id = "cerebellum:config";
        let existing = store.get(config_id).map(|n| n.data).unwrap_or_default();
        let mut config = existing.as_object().cloned().unwrap_or_default();
        config.insert("_type".to_owned(), serde_json::json!("cerebellum:config"));
        config.insert(
            "route_weights".to_owned(),
            serde_json::to_value(&weight_suggestions).unwrap_or_default(),
        );
        store.put(config_id, actor, serde_json::Value::Object(config));
    }

    Ok(CerebellumTuneReport {
        route_accuracy,
        underperforming_routes: underperforming,
        weight_suggestions,
        auto_applied,
    })
}

// ---------------------------------------------------------------------------
// 6. memory_relevance_tune
// ---------------------------------------------------------------------------

/// Track recalled memories vs actually used memories and adjust relevance
/// scoring.
///
/// For each `chronos:recall` node the procedure checks whether the recalled
/// memory content appeared in the subsequent agent response node.  A simple
/// substring check is used as a heuristic.  The relevance score per memory
/// category is computed as `used / recalled`.
///
/// If `auto_update` is `true` **and** a `chronos:approval` node exists with
/// `{ "procedure": "memory_relevance_tune", "approved": true }`, the computed
/// scores are written back to a `memory:relevance_config` node in the store.
///
/// # Arguments
///
/// * `store`       — the CRDT store to read from and write into.
/// * `actor`       — logical actor / author identifier.
/// * `threshold`   — minimum relevance score; categories below are flagged
///                   (default 0.3).
/// * `auto_update` — when `true` and approval granted, persist the scores.
///
/// # Returns
///
/// A [`MemoryRelevanceTuneReport`].
///
/// # Errors
///
/// Returns an error when `actor` is empty.
///
/// # Examples
///
/// ```rust
/// use pluresdb_core::{CrdtStore, procedures::ai_procedures::memory_relevance_tune};
///
/// let store = CrdtStore::default();
/// let report = memory_relevance_tune(&store, "actor", None, false).unwrap();
/// assert!(report.category_scores.is_empty()); // no recall nodes
/// ```
pub fn memory_relevance_tune(
    store: &CrdtStore,
    actor: &str,
    threshold: Option<f64>,
    auto_update: bool,
) -> anyhow::Result<MemoryRelevanceTuneReport> {
    anyhow::ensure!(!actor.is_empty(), "memory_relevance_tune: actor must not be empty");
    let min_rel = threshold.unwrap_or(0.3).clamp(0.0, 1.0);

    let all = store.list();

    // Index response nodes by session_id for quick lookup.
    let responses: HashMap<String, Vec<String>> = {
        let mut map: HashMap<String, Vec<String>> = HashMap::new();
        for n in &all {
            if n.data.get("_type").and_then(|v| v.as_str()) == Some("chronos:response") {
                let sid = n
                    .data
                    .get("session_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_owned();
                let text = n
                    .data
                    .get("text")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_owned();
                if !sid.is_empty() {
                    map.entry(sid).or_default().push(text);
                }
            }
        }
        map
    };

    // Per-category counters.
    let mut recalled_per_cat: HashMap<String, usize> = HashMap::new();
    let mut used_per_cat: HashMap<String, usize> = HashMap::new();

    for node in &all {
        if node.data.get("_type").and_then(|v| v.as_str()) != Some("chronos:recall") {
            continue;
        }

        let category = node
            .data
            .get("category")
            .and_then(|v| v.as_str())
            .unwrap_or("general")
            .to_owned();
        let recalled_text = node
            .data
            .get("recalled_text")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let session_id =
            node.data.get("session_id").and_then(|v| v.as_str()).unwrap_or("");

        *recalled_per_cat.entry(category.clone()).or_insert(0) += 1;

        // Check if recalled content appeared in any response for this session.
        if !recalled_text.is_empty() && !session_id.is_empty() {
            let was_used = responses
                .get(session_id)
                .map(|texts| texts.iter().any(|t| t.contains(recalled_text)))
                .unwrap_or(false);

            if was_used {
                *used_per_cat.entry(category).or_insert(0) += 1;
            }
        }
    }

    let mut category_scores: HashMap<String, f64> = HashMap::new();
    let mut low_relevance: Vec<String> = Vec::new();

    for (cat, total) in &recalled_per_cat {
        let used = *used_per_cat.get(cat).unwrap_or(&0);
        let score = if *total > 0 { used as f64 / *total as f64 } else { 0.0 };
        category_scores.insert(cat.clone(), score);
        if score < min_rel {
            low_relevance.push(cat.clone());
        }
    }
    // Sort for deterministic output across runs and peers.
    low_relevance.sort_unstable();

    // Human approval gate.
    let approval_granted = all.iter().any(|n| {
        n.data.get("_type").and_then(|v| v.as_str()) == Some("chronos:approval")
            && n.data.get("procedure").and_then(|v| v.as_str())
                == Some("memory_relevance_tune")
            && n.data.get("approved").and_then(|v| v.as_bool()).unwrap_or(false)
    });

    let weights_updated = auto_update && approval_granted;

    if weights_updated {
        store.put(
            "memory:relevance_config",
            actor,
            serde_json::json!({
                "_type":             "memory:relevance_config",
                "category_scores":   category_scores,
                "low_relevance":     low_relevance,
            }),
        );
    }

    Ok(MemoryRelevanceTuneReport { category_scores, low_relevance_categories: low_relevance, weights_updated })
}

// ---------------------------------------------------------------------------
// 7. chronos_replay (stretch goal)
// ---------------------------------------------------------------------------

/// Re-execute a causal chain with modified parameters (dry run).
///
/// Given the ID of a chain root node and a map of parameter overrides, the
/// procedure walks the causal chain forward and compares the `outcome` field
/// that would be predicted under the new parameters against the original.
///
/// The "predicted outcome" heuristic uses the overridden `route` parameter:
/// if the override changes the route for a node, the outcome is simulated as
/// `"accepted"` (optimistic assumption); otherwise the original outcome is
/// preserved.
///
/// This procedure does **not** write to the store — it is purely a dry run.
///
/// # Arguments
///
/// * `store`            — the CRDT store to read from.
/// * `actor`            — logical actor / author identifier.
/// * `chain_root_id`    — ID of the root node of the causal chain to replay.
/// * `param_overrides`  — parameter overrides applied during replay,
///                        e.g. `{"route": "analytical"}`.
///
/// # Returns
///
/// A [`ReplayReport`] describing divergence points between the original and
/// replayed chains.
///
/// # Errors
///
/// Returns an error when `actor` or `chain_root_id` is empty.
///
/// # Examples
///
/// ```rust
/// use pluresdb_core::{CrdtStore, procedures::ai_procedures::chronos_replay};
///
/// let store = CrdtStore::default();
/// store.put("root", "actor", serde_json::json!({
///     "_type": "chronos:decision",
///     "route": "analytical",
///     "outcome": "corrected",
/// }));
/// let report = chronos_replay(
///     &store, "actor", "root",
///     &serde_json::json!({"route": "creative"}),
/// ).unwrap();
/// assert_eq!(report.original_length, 1);
/// ```
pub fn chronos_replay(
    store: &CrdtStore,
    actor: &str,
    chain_root_id: &str,
    param_overrides: &serde_json::Value,
) -> anyhow::Result<ReplayReport> {
    anyhow::ensure!(!actor.is_empty(), "chronos_replay: actor must not be empty");
    anyhow::ensure!(
        !chain_root_id.is_empty(),
        "chronos_replay: chain_root_id must not be empty"
    );

    // Collect the causal chain (forward traversal from root).
    let chain: Vec<_> = store
        .list()
        .into_iter()
        .filter(|n| {
            n.data.get("_type").and_then(|v| v.as_str()) == Some("chronos:decision")
        })
        .collect();

    // Build forward adjacency via causal_parent to walk from root.
    let mut forward_map: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
    for n in &chain {
        if let Some(parent_id) = n.data.get("causal_parent").and_then(|v| v.as_str()) {
            if !parent_id.is_empty() {
                forward_map.entry(parent_id.to_owned()).or_default().push(n.id.clone());
            }
        }
    }

    // BFS from root.
    let mut ordered_chain: Vec<_> = Vec::new();
    let mut queue: std::collections::VecDeque<String> = std::collections::VecDeque::new();
    let mut visited: std::collections::HashSet<String> = std::collections::HashSet::new();

    if store.get(chain_root_id).is_some() {
        queue.push_back(chain_root_id.to_owned());
        visited.insert(chain_root_id.to_owned());
    }

    while let Some(id) = queue.pop_front() {
        if let Some(node) = store.get(&id) {
            ordered_chain.push(node);
        }
        if let Some(children) = forward_map.get(&id) {
            for child in children {
                if visited.insert(child.clone()) {
                    queue.push_back(child.clone());
                }
            }
        }
    }

    let original_length = ordered_chain.len();

    // Simulate replay with parameter overrides.
    let override_route = param_overrides.get("route").and_then(|v| v.as_str());

    let mut divergence_points: Vec<String> = Vec::new();
    let mut diffs: std::collections::HashMap<String, String> = std::collections::HashMap::new();

    for node in &ordered_chain {
        let original_route =
            node.data.get("route").and_then(|v| v.as_str()).unwrap_or("unknown");
        let original_outcome =
            node.data.get("outcome").and_then(|v| v.as_str()).unwrap_or("unknown");

        // Apply route override if specified.
        if let Some(new_route) = override_route {
            if new_route != original_route {
                // Simulate: overriding route predicts "accepted" outcome.
                let simulated_outcome = "accepted";
                if simulated_outcome != original_outcome {
                    divergence_points.push(node.id.clone());
                    diffs.insert(
                        node.id.clone(),
                        format!(
                            "route: {} → {}; outcome: {} → {}",
                            original_route, new_route, original_outcome, simulated_outcome
                        ),
                    );
                }
            }
        }
    }

    Ok(ReplayReport {
        original_length,
        replayed_length: original_length, // dry run — same chain length
        divergence_points,
        diffs,
    })
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Map an `outcome` string to a heuristic reward value.
fn outcome_to_reward(outcome: &str) -> f64 {
    match outcome {
        "accepted" => 1.0,
        "continued" => 0.5,
        "corrected" => -0.5,
        "abandoned" | "frustrated" => -1.0,
        _ => 0.5, // unknown / no correction → neutral-positive
    }
}

/// Derive a deterministic ID for AI procedure nodes.
fn derive_id(components: &[&str]) -> String {
    let combined = components.join(":");
    Uuid::new_v5(&AI_PROC_NS, combined.as_bytes()).to_string()
}

// ---------------------------------------------------------------------------
// JSONL export helpers
// ---------------------------------------------------------------------------

/// Export trajectories as a JSONL string compatible with standard RL pipelines.
///
/// Each line is a JSON object with fields: `id`, `state`, `action`, `outcome`,
/// `decision_id` (the source chronicle node ID).
///
/// # Examples
///
/// ```rust
/// use pluresdb_core::procedures::ai_procedures::{Trajectory, export_trajectories_jsonl};
///
/// let t = Trajectory {
///     id: "t1".to_string(),
///     state: serde_json::json!({"token_count": 42}),
///     action: "analytical".to_string(),
///     outcome: "success".to_string(),
///     decision_id: "d1".to_string(),
/// };
/// let jsonl = export_trajectories_jsonl(&[t]).unwrap();
/// assert!(jsonl.contains("\"action\":\"analytical\""));
/// ```
pub fn export_trajectories_jsonl(trajectories: &[Trajectory]) -> anyhow::Result<String> {
    let lines: Vec<String> = trajectories
        .iter()
        .map(|t| {
            serde_json::to_string(&serde_json::json!({
                "id":          &t.id,
                "state":       &t.state,
                "action":      &t.action,
                "outcome":     &t.outcome,
                "decision_id": &t.decision_id,
            }))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(lines.join("\n"))
}

/// Export preference pairs as a JSONL string for DPO training.
///
/// Each line is a JSON object with fields: `id`, `prompt`, `chosen`, `rejected`,
/// `correction_id` (provenance — the source `chronos:correction` node ID).
///
/// # Examples
///
/// ```rust
/// use pluresdb_core::procedures::ai_procedures::{PreferencePair, export_preference_pairs_jsonl};
///
/// let p = PreferencePair {
///     id: "p1".to_string(),
///     prompt: "What is 2+2?".to_string(),
///     chosen: "4".to_string(),
///     rejected: "5".to_string(),
///     correction_id: "c1".to_string(),
/// };
/// let jsonl = export_preference_pairs_jsonl(&[p]).unwrap();
/// assert!(jsonl.contains("\"chosen\":\"4\""));
/// ```
pub fn export_preference_pairs_jsonl(pairs: &[PreferencePair]) -> anyhow::Result<String> {
    let lines: Vec<String> = pairs
        .iter()
        .map(|p| {
            serde_json::to_string(&serde_json::json!({
                "id":            &p.id,
                "prompt":        &p.prompt,
                "chosen":        &p.chosen,
                "rejected":      &p.rejected,
                "correction_id": &p.correction_id,
            }))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(lines.join("\n"))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CrdtStore;

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn put_decision(store: &CrdtStore, id: &str, route: &str, outcome: &str, session: &str) {
        store.put(
            id,
            "actor",
            serde_json::json!({
                "_type":      "chronos:decision",
                "route":      route,
                "outcome":    outcome,
                "session_id": session,
            }),
        );
    }

    fn put_decision_with_context(
        store: &CrdtStore,
        id: &str,
        route: &str,
        outcome: &str,
        session: &str,
        token_count: u64,
    ) {
        store.put(
            id,
            "actor",
            serde_json::json!({
                "_type":         "chronos:decision",
                "route":         route,
                "outcome":       outcome,
                "session_id":    session,
                "input_context": { "token_count": token_count },
            }),
        );
    }

    // -----------------------------------------------------------------------
    // chronos_decision_audit
    // -----------------------------------------------------------------------

    #[test]
    fn audit_empty_store_returns_zero_totals() {
        let store = CrdtStore::default();
        let report = chronos_decision_audit(&store, "actor", None).unwrap();
        assert_eq!(report.total_decisions, 0);
        assert_eq!(report.accuracy, 0.0);
    }

    #[test]
    fn audit_counts_outcomes_correctly() {
        let store = CrdtStore::default();
        put_decision(&store, "d1", "analytical", "accepted", "s1");
        put_decision(&store, "d2", "creative", "corrected", "s1");
        put_decision(&store, "d3", "quick", "abandoned", "s1");
        put_decision(&store, "d4", "analytical", "accepted", "s1");

        let report = chronos_decision_audit(&store, "actor", None).unwrap();
        assert_eq!(report.total_decisions, 4);
        assert_eq!(report.accepted, 2);
        assert_eq!(report.corrected, 1);
        assert_eq!(report.abandoned, 1);
        assert!((report.accuracy - 0.5).abs() < 1e-9);
    }

    #[test]
    fn audit_surfaces_failure_patterns() {
        let store = CrdtStore::default();
        put_decision(&store, "d1", "creative", "corrected", "s1");
        put_decision(&store, "d2", "creative", "abandoned", "s1");
        put_decision(&store, "d3", "analytical", "corrected", "s1");

        let report = chronos_decision_audit(&store, "actor", None).unwrap();
        assert_eq!(*report.failure_patterns.get("creative").unwrap(), 2);
        assert_eq!(*report.failure_patterns.get("analytical").unwrap(), 1);
    }

    #[test]
    fn audit_rejects_empty_actor() {
        let store = CrdtStore::default();
        assert!(chronos_decision_audit(&store, "", None).is_err());
    }

    // -----------------------------------------------------------------------
    // chronos_extract_trajectories
    // -----------------------------------------------------------------------

    #[test]
    fn extract_produces_trajectory_per_decision() {
        let store = CrdtStore::default();
        put_decision_with_context(&store, "d1", "analytical", "accepted", "s1", 42);
        put_decision_with_context(&store, "d2", "creative", "corrected", "s1", 10);

        let trajs = chronos_extract_trajectories(&store, "actor").unwrap();
        assert_eq!(trajs.len(), 2);

        let t1 = trajs.iter().find(|t| t.decision_id == "d1").unwrap();
        assert_eq!(t1.action, "analytical");
        assert_eq!(t1.outcome, "success");

        let t2 = trajs.iter().find(|t| t.decision_id == "d2").unwrap();
        assert_eq!(t2.outcome, "failure");
    }

    #[test]
    fn extract_is_idempotent() {
        let store = CrdtStore::default();
        put_decision_with_context(&store, "d1", "quick", "accepted", "s1", 5);

        let first = chronos_extract_trajectories(&store, "actor").unwrap();
        let second = chronos_extract_trajectories(&store, "actor").unwrap();
        // Second run should produce the same number and IDs (idempotent).
        assert_eq!(first.len(), second.len());
        assert_eq!(first[0].id, second[0].id);
    }

    #[test]
    fn extract_trajectories_jsonl_format() {
        let store = CrdtStore::default();
        put_decision_with_context(&store, "d1", "analytical", "accepted", "s1", 10);

        let trajs = chronos_extract_trajectories(&store, "actor").unwrap();
        let jsonl = export_trajectories_jsonl(&trajs).unwrap();
        assert!(jsonl.contains("\"action\":\"analytical\""));
        assert!(jsonl.contains("\"outcome\":\"success\""));
    }

    // -----------------------------------------------------------------------
    // chronos_preference_pairs
    // -----------------------------------------------------------------------

    #[test]
    fn preference_pairs_empty_store() {
        let store = CrdtStore::default();
        let pairs = chronos_preference_pairs(&store, "actor", None).unwrap();
        assert!(pairs.is_empty());
    }

    #[test]
    fn preference_pairs_generates_dpo_pair() {
        let store = CrdtStore::default();
        store.put(
            "d1",
            "actor",
            serde_json::json!({
                "_type": "chronos:decision",
                "route": "analytical",
                "input_context": {"token_count": 10},
            }),
        );
        store.put(
            "c1",
            "actor",
            serde_json::json!({
                "_type":            "chronos:correction",
                "decision_id":      "d1",
                "original_output":  "wrong answer",
                "corrected_output": "right answer",
            }),
        );

        let pairs = chronos_preference_pairs(&store, "actor", None).unwrap();
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].chosen, "right answer");
        assert_eq!(pairs[0].rejected, "wrong answer");
        assert_eq!(pairs[0].correction_id, "c1");
    }

    #[test]
    fn preference_pairs_skips_incomplete_corrections() {
        let store = CrdtStore::default();
        // Missing corrected_output
        store.put(
            "c1",
            "actor",
            serde_json::json!({
                "_type":           "chronos:correction",
                "original_output": "something",
            }),
        );
        let pairs = chronos_preference_pairs(&store, "actor", None).unwrap();
        assert!(pairs.is_empty());
    }

    #[test]
    fn preference_pairs_jsonl_format() {
        let pairs = vec![PreferencePair {
            id: "p1".to_string(),
            prompt: "What is 2+2?".to_string(),
            chosen: "4".to_string(),
            rejected: "5".to_string(),
            correction_id: "c1".to_string(),
        }];
        let jsonl = export_preference_pairs_jsonl(&pairs).unwrap();
        assert!(jsonl.contains("\"chosen\":\"4\""));
        assert!(jsonl.contains("\"rejected\":\"5\""));
        assert!(jsonl.contains("\"prompt\":\"What is 2+2?\""));
    }

    // -----------------------------------------------------------------------
    // chronos_reward_signal
    // -----------------------------------------------------------------------

    #[test]
    fn reward_assigns_positive_for_accepted() {
        let store = CrdtStore::default();
        put_decision(&store, "d1", "analytical", "accepted", "sess-1");

        let rewards = chronos_reward_signal(&store, "actor", "sess-1").unwrap();
        assert!(rewards["d1"].as_f64().unwrap() > 0.0);
    }

    #[test]
    fn reward_assigns_negative_for_corrected() {
        let store = CrdtStore::default();
        put_decision(&store, "d1", "creative", "corrected", "sess-2");

        let rewards = chronos_reward_signal(&store, "actor", "sess-2").unwrap();
        assert!(rewards["d1"].as_f64().unwrap() < 0.0);
    }

    #[test]
    fn reward_propagates_backward_via_causal_parent() {
        let store = CrdtStore::default();
        // root ← leaf; root has positive outcome, leaf inherits propagated reward
        store.put(
            "root",
            "actor",
            serde_json::json!({
                "_type": "chronos:decision",
                "route": "analytical",
                "outcome": "accepted",
                "session_id": "sess-3",
            }),
        );
        store.put(
            "leaf",
            "actor",
            serde_json::json!({
                "_type": "chronos:decision",
                "route": "quick",
                "outcome": "accepted",
                "session_id": "sess-3",
                "causal_parent": "root",
            }),
        );

        let rewards = chronos_reward_signal(&store, "actor", "sess-3").unwrap();
        // Both should have positive rewards
        assert!(rewards["root"].as_f64().unwrap() > 0.0);
        assert!(rewards["leaf"].as_f64().unwrap() > 0.0);
    }

    #[test]
    fn reward_rejects_empty_session_id() {
        let store = CrdtStore::default();
        assert!(chronos_reward_signal(&store, "actor", "").is_err());
    }

    // -----------------------------------------------------------------------
    // cerebellum_tune
    // -----------------------------------------------------------------------

    #[test]
    fn cerebellum_tune_empty_store() {
        let store = CrdtStore::default();
        let report = cerebellum_tune(&store, "actor", None, false).unwrap();
        assert!(report.route_accuracy.is_empty());
        assert!(!report.auto_applied);
    }

    #[test]
    fn cerebellum_tune_computes_route_accuracy() {
        let store = CrdtStore::default();
        put_decision(&store, "d1", "analytical", "accepted", "s1");
        put_decision(&store, "d2", "analytical", "accepted", "s1");
        put_decision(&store, "d3", "creative", "corrected", "s1");

        let report = cerebellum_tune(&store, "actor", Some(0.9), false).unwrap();
        // analytical: 2/2 = 1.0 → not underperforming
        // creative:   0/1 = 0.0 → underperforming
        assert!((report.route_accuracy["analytical"] - 1.0).abs() < 1e-9);
        assert_eq!(report.route_accuracy["creative"], 0.0);
        assert!(report.underperforming_routes.contains(&"creative".to_string()));
        assert!(!report.underperforming_routes.contains(&"analytical".to_string()));
    }

    #[test]
    fn cerebellum_tune_requires_approval_for_auto_apply() {
        let store = CrdtStore::default();
        put_decision(&store, "d1", "quick", "abandoned", "s1");

        // auto_apply = true but no approval node → should NOT apply
        let report = cerebellum_tune(&store, "actor", None, true).unwrap();
        assert!(!report.auto_applied);
        assert!(store.get("cerebellum:config").is_none());

        // Now grant approval
        store.put(
            "approval:cerebellum_tune",
            "actor",
            serde_json::json!({
                "_type":     "chronos:approval",
                "procedure": "cerebellum_tune",
                "approved":  true,
            }),
        );

        let report2 = cerebellum_tune(&store, "actor", None, true).unwrap();
        assert!(report2.auto_applied);
        assert!(store.get("cerebellum:config").is_some());
    }

    // -----------------------------------------------------------------------
    // memory_relevance_tune
    // -----------------------------------------------------------------------

    #[test]
    fn memory_relevance_tune_empty_store() {
        let store = CrdtStore::default();
        let report = memory_relevance_tune(&store, "actor", None, false).unwrap();
        assert!(report.category_scores.is_empty());
        assert!(!report.weights_updated);
    }

    #[test]
    fn memory_relevance_tune_scores_categories() {
        let store = CrdtStore::default();
        // Recall node for session s1
        store.put(
            "recall1",
            "actor",
            serde_json::json!({
                "_type":         "chronos:recall",
                "category":      "fact",
                "session_id":    "s1",
                "recalled_text": "Rust is fast",
            }),
        );
        // Response that contains the recalled text
        store.put(
            "resp1",
            "actor",
            serde_json::json!({
                "_type":      "chronos:response",
                "session_id": "s1",
                "text":       "Rust is fast and memory-safe.",
            }),
        );

        let report = memory_relevance_tune(&store, "actor", Some(0.5), false).unwrap();
        // "fact" category was recalled once and used once → score 1.0
        assert_eq!(report.category_scores["fact"], 1.0);
        assert!(!report.low_relevance_categories.contains(&"fact".to_string()));
    }

    #[test]
    fn memory_relevance_tune_requires_approval_for_update() {
        let store = CrdtStore::default();
        store.put(
            "recall1",
            "actor",
            serde_json::json!({
                "_type":         "chronos:recall",
                "category":      "preference",
                "session_id":    "s1",
                "recalled_text": "likes dark mode",
            }),
        );

        // auto_update = true without approval → should NOT persist
        let report = memory_relevance_tune(&store, "actor", None, true).unwrap();
        assert!(!report.weights_updated);
        assert!(store.get("memory:relevance_config").is_none());

        // Grant approval
        store.put(
            "approval:mrt",
            "actor",
            serde_json::json!({
                "_type":     "chronos:approval",
                "procedure": "memory_relevance_tune",
                "approved":  true,
            }),
        );

        let report2 = memory_relevance_tune(&store, "actor", None, true).unwrap();
        assert!(report2.weights_updated);
        assert!(store.get("memory:relevance_config").is_some());
    }

    // -----------------------------------------------------------------------
    // chronos_replay
    // -----------------------------------------------------------------------

    #[test]
    fn replay_empty_chain() {
        let store = CrdtStore::default();
        // Root doesn't exist
        let report =
            chronos_replay(&store, "actor", "nonexistent", &serde_json::json!({})).unwrap();
        assert_eq!(report.original_length, 0);
        assert!(report.divergence_points.is_empty());
    }

    #[test]
    fn replay_detects_divergence_with_route_override() {
        let store = CrdtStore::default();
        store.put(
            "root",
            "actor",
            serde_json::json!({
                "_type":   "chronos:decision",
                "route":   "analytical",
                "outcome": "corrected",
            }),
        );

        let report = chronos_replay(
            &store,
            "actor",
            "root",
            &serde_json::json!({"route": "creative"}),
        )
        .unwrap();
        assert_eq!(report.original_length, 1);
        // Route changed → outcome predicted to change → divergence
        assert_eq!(report.divergence_points.len(), 1);
        assert_eq!(report.divergence_points[0], "root");
    }

    #[test]
    fn replay_no_divergence_same_route() {
        let store = CrdtStore::default();
        store.put(
            "root",
            "actor",
            serde_json::json!({
                "_type":   "chronos:decision",
                "route":   "analytical",
                "outcome": "accepted",
            }),
        );

        let report = chronos_replay(
            &store,
            "actor",
            "root",
            &serde_json::json!({"route": "analytical"}), // no change
        )
        .unwrap();
        assert!(report.divergence_points.is_empty());
    }

    #[test]
    fn replay_rejects_empty_chain_root() {
        let store = CrdtStore::default();
        assert!(chronos_replay(&store, "actor", "", &serde_json::json!({})).is_err());
    }

    // -----------------------------------------------------------------------
    // JSONL export
    // -----------------------------------------------------------------------

    #[test]
    fn trajectories_jsonl_multiline() {
        let t1 = Trajectory {
            id: "t1".to_string(),
            state: serde_json::json!({}),
            action: "quick".to_string(),
            outcome: "success".to_string(),
            decision_id: "d1".to_string(),
        };
        let t2 = Trajectory {
            id: "t2".to_string(),
            state: serde_json::json!({}),
            action: "analytical".to_string(),
            outcome: "failure".to_string(),
            decision_id: "d2".to_string(),
        };
        let jsonl = export_trajectories_jsonl(&[t1, t2]).unwrap();
        let lines: Vec<&str> = jsonl.lines().collect();
        assert_eq!(lines.len(), 2);
    }
}
