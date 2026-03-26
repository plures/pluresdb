import type { NodeRecord } from "../types/index.ts";

/**
 * Minimal database interface exposed to rule actions.
 *
 * Kept intentionally narrow to avoid circular imports and to prevent rules
 * from calling methods unrelated to data mutation.
 */
export interface DatabaseLike {
  /**
   * Insert or update a node.
   *
   * @param id   - Node identifier.
   * @param data - Arbitrary JSON payload to store.
   */
  put(id: string, data: Record<string, unknown>): Promise<void>;
  /**
   * Retrieve a node by its identifier.
   *
   * @param id - Node identifier to look up.
   * @returns The node data merged with its `id` field, or `null` if not found.
   */
  get<T = Record<string, unknown>>(
    id: string,
  ): Promise<(T & { id: string }) | null>;
}

/**
 * Context object passed to a {@link RuleAction}.
 *
 * Provides the rule with a scoped database handle that can be used to read or
 * write nodes as part of the rule's side-effect.
 */
export interface RuleContext {
  /** Database handle available to rule actions. */
  db: DatabaseLike;
}

/**
 * Predicate function that determines whether a rule should fire for a node.
 *
 * Return `true` (or a Promise resolving to `true`) to allow the rule's action
 * to run; return `false` to skip it.
 */
export type RulePredicate = (node: NodeRecord) => boolean | Promise<boolean>;

/**
 * Action function executed when a rule's conditions are met.
 *
 * May perform arbitrary async operations including reading or writing database
 * nodes via `ctx.db`.
 */
export type RuleAction = (ctx: RuleContext, node: NodeRecord) => Promise<void>;

/**
 * Definition of a reactive rule.
 *
 * Rules are evaluated after every node write.  A rule fires when:
 * 1. `whenType` is undefined **or** the node's `type` matches `whenType`, and
 * 2. `predicate` is undefined **or** resolves to `true`.
 */
export interface Rule {
  /** Unique name for this rule.  Used as the key in the rule registry. */
  name: string;
  /**
   * Optional type filter.  If set, the rule only fires for nodes whose
   * `type` field equals this value.
   */
  whenType?: string;
  /**
   * Optional additional predicate evaluated after the `whenType` filter.
   * Return `true` to allow the action to run.
   */
  predicate?: RulePredicate;
  /** Side-effect to execute when the rule fires. */
  action: RuleAction;
}

/**
 * Registry and evaluator for reactive rules.
 *
 * Rules are stored by name and evaluated against every node write.  Adding a
 * rule with an existing name overwrites the previous rule.
 *
 * @example
 * ```typescript
 * const engine = new RuleEngine();
 * engine.addRule({
 *   name: "auto-tag-users",
 *   whenType: "User",
 *   action: async (ctx, node) => {
 *     if (!node.data.tag) {
 *       await ctx.db.put(node.id, { ...node.data, tag: "new" });
 *     }
 *   },
 * });
 * ```
 */
export class RuleEngine {
  private readonly rules: Map<string, Rule> = new Map();

  /**
   * Register a rule.
   *
   * If a rule with the same `name` already exists it is replaced.
   *
   * @param rule - Rule to add.
   */
  addRule(rule: Rule): void {
    this.rules.set(rule.name, rule);
  }

  /**
   * Remove a rule by name.
   *
   * Silently does nothing if no rule with that name is registered.
   *
   * @param name - Name of the rule to remove.
   */
  removeRule(name: string): void {
    this.rules.delete(name);
  }

  /**
   * Evaluate all registered rules against the given node.
   *
   * Rules are evaluated in insertion order.  Each rule's `whenType` and
   * `predicate` filters are checked before its `action` is invoked.
   *
   * @param node - The node that was just written.
   * @param ctx  - Database context passed to rule actions.
   */
  async evaluateNode(node: NodeRecord, ctx: RuleContext): Promise<void> {
    for (const rule of this.rules.values()) {
      if (rule.whenType && node.type !== rule.whenType) continue;
      if (rule.predicate) {
        const ok = await rule.predicate(node);
        if (!ok) continue;
      }
      await rule.action(ctx, node);
    }
  }
}
