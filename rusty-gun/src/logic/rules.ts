import type { NodeRecord } from "../types/index.ts";

// Minimal DB interface to avoid circular imports
export interface DatabaseLike {
  put(id: string, data: Record<string, unknown>): Promise<void>;
  get<T = Record<string, unknown>>(id: string): Promise<(T & { id: string }) | null>;
}

export interface RuleContext {
  db: DatabaseLike;
}

export type RulePredicate = (node: NodeRecord) => boolean | Promise<boolean>;
export type RuleAction = (ctx: RuleContext, node: NodeRecord) => Promise<void>;

export interface Rule {
  name: string;
  whenType?: string;
  predicate?: RulePredicate;
  action: RuleAction;
}

export class RuleEngine {
  private readonly rules: Map<string, Rule> = new Map();

  addRule(rule: Rule): void {
    this.rules.set(rule.name, rule);
  }

  removeRule(name: string): void {
    this.rules.delete(name);
  }

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


