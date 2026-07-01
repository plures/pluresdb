//! Compiler - transforms a parsed `px_ast::PxDocument` into PluresDB records.
//!
//! Each `.px` primitive becomes a JSON record stored in PluresDB under a
//! namespaced key. The runtime engine (`executor.rs`, `async_executor.rs`,
//! `scenario_runner.rs`) reads these records to evaluate rules, check
//! constraints, and execute procedures.
//!
//! **M6 (praxis-lang epic):** the AST this consumes is now the single-source-of-truth
//! `px_ast` from `praxis-lang` (a statement-list of tagged `Statement`s), NOT the old
//! flat `Px*` AST. The `CompiledRecord` *shape* is unchanged - it is the pluresdb-unique
//! storage/runtime contract the executor reads - so the record JSON stays byte-compatible
//! with what the (untouched) executor already parses. This file lowers the richer typed
//! AST back onto that string-typed record format via the `expr_to_string` /
//! `value_to_json` / `TypeExpr::to_string` render helpers below.

use serde_json::{json, Value as Json};

use px_ast::{
    ActionStmt, CaptureEntry, ConstraintDecl, ContractDecl, DataflowProcedureDecl, Expr,
    FactDecl, FunctionDecl, FunctionMode, ImportDecl, LegacyProcedureDecl, LoopSource, LoopStep,
    MatchArm, ParallelStep, ProcedureBody, ProcedureTrigger, RetryOpt, RuleDecl, ScenarioDecl,
    Severity, Statement, Step, StepCall, StepCallArgs, TriggerDecl, TriggerEvent, TryStep,
    Value as AstValue, VarRef,
};

use px_ast::PxDocument;

/// A compiled PluresDB record ready for storage.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CompiledRecord {
    /// PluresDB key (e.g. "px:rule/auto_merge").
    pub key: String,
    /// JSON data to store.
    pub data: Json,
    /// Whether this record should be embedded for vector search.
    pub embed: bool,
}

/// Compile a `PxDocument` into PluresDB records.
///
/// Iterates the statement list and dispatches each `Statement` variant to its
/// per-construct lowering. The old per-construct-vec walk is replaced by a match
/// over the tagged statement enum (the px_ast shape).
pub fn compile(doc: &PxDocument) -> Vec<CompiledRecord> {
    let mut records = Vec::new();

    for stmt in &doc.statements {
        match stmt {
            Statement::Import(i) => records.push(compile_import(i)),
            Statement::Fact(f) => records.push(compile_fact(f)),
            Statement::Rule(r) => records.push(compile_rule(r)),
            Statement::Constraint(c) => records.push(compile_constraint(c)),
            Statement::Contract(c) => records.push(compile_contract(c)),
            Statement::Function(f) => records.push(compile_function(f)),
            Statement::Trigger(t) => records.push(compile_trigger(t)),
            Statement::LegacyProcedure(p) => records.push(compile_legacy_procedure(p)),
            Statement::DataflowProcedure(p) => records.push(compile_dataflow_procedure(p)),
            Statement::Scenario(s) => records.push(compile_scenario(s)),
            // Entity/Config are schema/static-config declarations the record
            // runtime does not (yet) persist as executable records. They are not
            // dropped silently: the previous compiler likewise emitted no record
            // for them (they had no `compile_*`), and the executor reads neither.
            // Kept as an explicit, documented no-op rather than a fake record.
            Statement::Entity(_) | Statement::Config(_) => {}
        }
    }

    records
}

// ─────────────────────────────────────────────────────────────────────────────
// Render helpers: px_ast typed AST → the string-typed record JSON the executor
// already consumes. These reproduce the source-equivalent strings so the record
// bytes stay compatible with the untouched executor (which parses conditions as
// `&str` and resolves `$var` references inside JSON values).
// ─────────────────────────────────────────────────────────────────────────────

/// Render a `VarRef` back to its `$name.field["key"]` source form.
fn var_ref_to_string(v: &VarRef) -> String {
    let mut s = String::with_capacity(v.name.name.len() + 1);
    s.push('$');
    s.push_str(&v.name.name);
    for acc in &v.accessors {
        match acc {
            px_ast::Accessor::Dot(id) => {
                s.push('.');
                s.push_str(&id.name);
            }
            px_ast::Accessor::Bracket(key) => {
                s.push('[');
                s.push_str(key);
                s.push(']');
            }
        }
    }
    s
}

/// Render a v1 `Value` to the JSON the executor expects.
///
/// - String/Integer/Float/Boolean/Null → native JSON scalars.
/// - List → JSON array; Map/object → JSON object (keys are idents).
/// - Var → `"$name.field"` string so `resolve_vars` substitutes it at runtime.
/// - Path/Ident → bare string (dotted path / enum-variant-like bare name), matching
///   the old flat-AST behavior where these arrived as plain strings.
/// - Call/Arithmetic/Paren → rendered to their source-form string (the record format
///   is string-typed for these; the executor re-parses where it needs to).
fn value_to_json(v: &AstValue) -> Json {
    match v {
        AstValue::String(s) => json!(s),
        AstValue::Integer(i) => json!(i),
        AstValue::Float(f) => json!(f),
        AstValue::Boolean(b) => json!(b),
        AstValue::Null => Json::Null,
        AstValue::List(items) => Json::Array(items.iter().map(value_to_json).collect()),
        AstValue::Map(entries) => {
            let mut obj = serde_json::Map::with_capacity(entries.len());
            for (k, val) in entries {
                obj.insert(k.name.clone(), value_to_json(val));
            }
            Json::Object(obj)
        }
        AstValue::Var(vr) => json!(var_ref_to_string(vr)),
        AstValue::Path(dotted) => json!(dotted_to_string(dotted)),
        AstValue::Ident(id) => json!(id.name),
        AstValue::Call { name, args } => json!(call_to_string(&name.name, args)),
        AstValue::Arithmetic { left, op, right } => {
            json!(format!(
                "{} {} {}",
                value_to_source(left),
                arith_op_str(*op),
                value_to_source(right)
            ))
        }
        AstValue::Paren(inner) => json!(format!("({})", expr_to_string(inner))),
    }
}

/// Render a dotted identifier path `a.b.c`.
fn dotted_to_string(d: &px_ast::DottedIdent) -> String {
    d.segments
        .iter()
        .map(|s| s.name.as_str())
        .collect::<Vec<_>>()
        .join(".")
}

/// Render a `Value` in bare *source* form (no JSON quoting) - used when a value
/// appears inside a larger expression string (arithmetic operands, call args).
fn value_to_source(v: &AstValue) -> String {
    match v {
        AstValue::String(s) => format!("\"{}\"", s),
        AstValue::Integer(i) => i.to_string(),
        AstValue::Float(f) => f.to_string(),
        AstValue::Boolean(b) => b.to_string(),
        AstValue::Null => "null".to_string(),
        AstValue::List(items) => {
            let inner = items.iter().map(value_to_source).collect::<Vec<_>>().join(", ");
            format!("[{}]", inner)
        }
        AstValue::Map(entries) => {
            let inner = entries
                .iter()
                .map(|(k, val)| format!("{}: {}", k.name, value_to_source(val)))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{{{}}}", inner)
        }
        AstValue::Var(vr) => var_ref_to_string(vr),
        AstValue::Path(dotted) => dotted_to_string(dotted),
        AstValue::Ident(id) => id.name.clone(),
        AstValue::Call { name, args } => call_to_string(&name.name, args),
        AstValue::Arithmetic { left, op, right } => format!(
            "{} {} {}",
            value_to_source(left),
            arith_op_str(*op),
            value_to_source(right)
        ),
        AstValue::Paren(inner) => format!("({})", expr_to_string(inner)),
    }
}

fn arith_op_str(op: px_ast::ArithOp) -> &'static str {
    match op {
        px_ast::ArithOp::Add => "+",
        px_ast::ArithOp::Sub => "-",
        px_ast::ArithOp::Mul => "*",
        px_ast::ArithOp::Div => "/",
        px_ast::ArithOp::Mod => "%",
    }
}

fn bin_op_str(op: px_ast::BinOp) -> &'static str {
    use px_ast::BinOp::*;
    match op {
        And => "&&",
        Or => "||",
        Eq => "==",
        Neq => "!=",
        Gt => ">",
        Lt => "<",
        Gte => ">=",
        Lte => "<=",
        Add => "+",
        Sub => "-",
        Mul => "*",
        Div => "/",
        Mod => "%",
        Pow => "^",
    }
}

fn call_to_string(name: &str, args: &[Expr]) -> String {
    let rendered = args.iter().map(expr_to_string).collect::<Vec<_>>().join(", ");
    format!("{}({})", name, rendered)
}

/// Render a v1 `Expr` back to its source-equivalent string. This is what the
/// executor's `evaluate_condition(&str, ...)` parses, so it MUST reproduce the
/// operator/spacing shape the executor's condition parser understands
/// (`a == b`, `a && b`, `!a`, `a.b.c`, `$var`, calls, match).
///
/// `pub` so downstream consumers of this crate's public API (e.g.
/// `pluresdb-node`'s `.px` loader) render a px-ast `Expr` back to the SAME
/// canonical source form the executor understands, instead of duplicating the
/// renderer (ADR-0010 anti-duplication). Re-exported at `crate::px::expr_to_string`.
pub fn expr_to_string(e: &Expr) -> String {
    match e {
        Expr::Literal(v) => value_to_source(v),
        Expr::Var(vr) => var_ref_to_string(vr),
        Expr::Path(d) => dotted_to_string(d),
        Expr::Paren(inner) => format!("({})", expr_to_string(inner)),
        Expr::Unary { op, operand } => {
            let sym = match op {
                px_ast::UnaryOp::Not => "!",
                px_ast::UnaryOp::Neg => "-",
            };
            format!("{}{}", sym, expr_to_string(operand))
        }
        Expr::Binary { left, op, right } => {
            format!(
                "{} {} {}",
                expr_to_string(left),
                bin_op_str(*op),
                expr_to_string(right)
            )
        }
        Expr::Call { name, args } => call_to_string(&name.name, args),
        Expr::InlineIf {
            condition,
            then_val,
            else_val,
        } => format!(
            "if {}: {} else: {}",
            expr_to_string(condition),
            expr_to_string(then_val),
            expr_to_string(else_val)
        ),
        Expr::Match { subject, arms } => {
            let arms_s = arms
                .iter()
                .map(|a| {
                    let pat = match &a.pattern {
                        px_ast::ExprMatchPattern::Wildcard => "_".to_string(),
                        px_ast::ExprMatchPattern::Values(vs) => vs
                            .iter()
                            .map(value_to_source)
                            .collect::<Vec<_>>()
                            .join(" | "),
                    };
                    format!("{} => {}", pat, expr_to_string(&a.result))
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!("match {} {{ {} }}", expr_to_string(subject), arms_s)
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Per-construct lowering
// ─────────────────────────────────────────────────────────────────────────────

fn compile_import(import: &ImportDecl) -> CompiledRecord {
    // px_ast ImportDecl.path is Vec<Ident> (a::b::c); the record format stored a
    // single string path. Join with "::" to reproduce the source path form.
    let path = import
        .path
        .iter()
        .map(|s| s.name.as_str())
        .collect::<Vec<_>>()
        .join("::");
    let alias = import.alias.as_ref().map(|a| a.name.clone());
    CompiledRecord {
        key: format!("px:import/{}", alias.as_deref().unwrap_or(&path)),
        data: json!({
            "type": "import",
            "path": path,
            "alias": alias,
        }),
        embed: false,
    }
}

fn compile_fact(fact: &FactDecl) -> CompiledRecord {
    let fields: Vec<Json> = fact
        .fields
        .iter()
        .map(|f| json!({ "name": f.name.name, "type": f.field_type.to_string() }))
        .collect();

    CompiledRecord {
        key: format!("px:fact/{}", fact.name.name),
        data: json!({
            "type": "fact",
            "name": fact.name.name,
            "fields": fields,
        }),
        embed: true, // facts are searchable
    }
}

fn compile_rule(rule: &RuleDecl) -> CompiledRecord {
    let conditions: Vec<String> = rule.conditions.iter().map(expr_to_string).collect();

    let actions: Vec<Json> = rule.actions.iter().map(action_to_json).collect();

    let captures: Vec<Json> = rule.captures.iter().map(capture_to_json).collect();

    let lets: Vec<Json> = rule
        .let_bindings
        .iter()
        .map(|b| json!({ "var": b.name.name, "expr": expr_to_string(&b.value) }))
        .collect();

    CompiledRecord {
        key: format!("px:rule/{}", rule.name.name),
        data: json!({
            "type": "rule",
            "name": rule.name.name,
            "priority": rule.priority.unwrap_or(50),
            "conditions": conditions,
            "lets": lets,
            "actions": actions,
            "captures": captures,
        }),
        embed: true,
    }
}

/// Lower a rule action. Old shape: `{ kind, condition?, <param key>: <value> }`.
fn action_to_json(a: &ActionStmt) -> Json {
    let mut obj = json!({ "kind": a.action_name.name });
    if let Some(cond) = &a.condition {
        obj["condition"] = json!(expr_to_string(cond));
    }
    for pair in &a.params {
        obj[pair.key.name.clone()] = value_to_json(&pair.value);
    }
    obj
}

fn capture_to_json(c: &CaptureEntry) -> Json {
    let tags: Vec<Json> = c
        .tags
        .as_ref()
        .map(|ts| ts.iter().map(value_to_json).collect())
        .unwrap_or_default();
    json!({
        "content": c.fact.value,
        "category": c.category.as_ref().map(|i| i.name.clone()),
        "tags": tags,
    })
}

fn severity_str(s: Severity) -> &'static str {
    match s {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Info => "info",
    }
}

fn compile_constraint(c: &ConstraintDecl) -> CompiledRecord {
    let phases: Vec<String> = c.phase.iter().map(|p| p.name.clone()).collect();
    CompiledRecord {
        key: format!("px:constraint/{}", c.name.name),
        data: json!({
            "type": "constraint",
            "name": c.name.name,
            "scope": c.scope.as_ref().map(|i| i.name.clone()),
            "phases": phases,
            "trait_category": c.trait_name.as_ref().map(|i| i.name.clone()),
            "weight": c.weight,
            "prompt_injection": c.prompt.as_ref().map(|s| s.value.clone()),
            "when": c.when.as_ref().map(expr_to_string),
            "require": c.require.as_ref().map(expr_to_string),
            "severity": severity_str(c.severity),
            "message": c.message.as_ref().map(|s| s.value.clone()),
        }),
        embed: true,
    }
}

fn compile_contract(c: &ContractDecl) -> CompiledRecord {
    let examples: Vec<Json> = c
        .examples
        .iter()
        .map(|e| {
            json!({
                "input": value_to_json(&e.input),
                "expect": value_to_json(&e.expect),
                "threshold": e.threshold,
            })
        })
        .collect();
    CompiledRecord {
        key: format!("px:contract/{}", c.name.name),
        data: json!({
            "type": "contract",
            "name": c.name.name,
            "given": c.given.as_ref().map(|s| s.value.clone()),
            "when": c.when.as_ref().map(|s| s.value.clone()),
            "then": c.then.as_ref().map(|s| s.value.clone()),
            "threshold": c.threshold,
            "examples": examples,
        }),
        embed: true,
    }
}

fn function_mode_str(mode: Option<FunctionMode>) -> &'static str {
    match mode {
        Some(FunctionMode::Deterministic) | None => "deterministic",
        Some(FunctionMode::Probabilistic) => "probabilistic",
        Some(FunctionMode::Hybrid) => "hybrid",
    }
}

fn compile_function(f: &FunctionDecl) -> CompiledRecord {
    let params: Vec<Json> = f
        .params
        .iter()
        .map(|p| json!({ "name": p.name.name, "type": p.field_type.to_string() }))
        .collect();
    CompiledRecord {
        key: format!("px:function/{}", f.name.name),
        data: json!({
            "type": "function",
            "name": f.name.name,
            "mode": function_mode_str(f.mode),
            "params": params,
            "return_type": f.return_type.to_string(),
            "docstring": f.docstring.clone().unwrap_or_default(),
        }),
        embed: true, // functions are searchable by description
    }
}

fn trigger_event_str(ev: &TriggerEvent) -> Json {
    match ev {
        TriggerEvent::AfterStore => json!("after_store"),
        TriggerEvent::BeforeSearch => json!("before_search"),
        TriggerEvent::Timer => json!("timer"),
        TriggerEvent::OnEvent(s) => json!(s.value),
    }
}

fn compile_trigger(t: &TriggerDecl) -> CompiledRecord {
    CompiledRecord {
        key: format!("px:trigger/{}", t.name.name),
        data: json!({
            "type": "trigger",
            "name": t.name.name,
            "on": trigger_event_str(&t.event),
            "schedule": t.schedule.as_ref().map(|s| s.value.clone()),
            "run": t.run.name,
        }),
        embed: false,
    }
}

/// Lower a legacy (v1) procedure. Body may be a step-list OR a v2 code block.
fn compile_legacy_procedure(p: &LegacyProcedureDecl) -> CompiledRecord {
    let trigger = p.trigger.as_ref().map(procedure_trigger_to_json);
    let body = procedure_body_to_json(&p.body);
    CompiledRecord {
        key: format!("px:procedure/{}", p.name.name),
        data: json!({
            "type": "procedure",
            "name": p.name.name,
            "trigger": trigger,
            "given": p.given.as_ref().map(|s| s.value.clone()),
            "params": p.params.iter().map(|i| i.name.clone()).collect::<Vec<_>>(),
            "steps": body.steps,
            "body_kind": body.kind,
            "code": body.code,
        }),
        embed: true,
    }
}

/// Lower a dataflow (v3) procedure. This is the PRIMARY procedure form.
///
/// NOTE the px_ast field renames (§2d): a param's queue is `source_queue`
/// (was `source`) and the return's queue is `dest_queue` (was `destination`).
/// The record keys (`from`/`into`) are the executor-facing names and stay put.
fn compile_dataflow_procedure(p: &DataflowProcedureDecl) -> CompiledRecord {
    let params: Vec<Json> = p
        .params
        .iter()
        .map(|param| {
            json!({
                "name": param.name.name,
                "type": param.param_type.to_string(),
                "from": param.source_queue.as_ref().map(|s| s.value.clone()),
            })
        })
        .collect();

    let ret = p.return_type.as_ref().map(|r| {
        json!({
            "type": r.return_type.to_string(),
            "into": r.dest_queue.as_ref().map(|s| s.value.clone()),
        })
    });

    let body = procedure_body_to_json(&p.body);

    CompiledRecord {
        key: format!("px:procedure/{}", p.name.name),
        data: json!({
            "type": "procedure",
            "name": p.name.name,
            "dataflow": true,
            "params": params,
            "returns": ret,
            "given": p.given.as_ref().map(|s| s.value.clone()),
            "steps": body.steps,
            "body_kind": body.kind,
            "code": body.code,
        }),
        embed: true,
    }
}

fn compile_scenario(s: &ScenarioDecl) -> CompiledRecord {
    let setup: Vec<Json> = s.setup.iter().map(step_to_json).collect();
    let expectations: Vec<Json> = s
        .expectations
        .iter()
        .map(|e| {
            json!({
                "negated": e.negated,
                "name": e.name.name,
                "args": e.args.as_ref().map(value_to_json),
            })
        })
        .collect();
    let run = s.run.as_ref().map(|r| {
        json!({
            "procedure": r.procedure.name,
            "args": r.args.as_ref().map(value_to_json),
        })
    });
    CompiledRecord {
        key: format!("px:scenario/{}", s.name.name),
        data: json!({
            "type": "scenario",
            "name": s.name.name,
            "given": s.given.as_ref().map(|g| g.value.clone()),
            "setup": setup,
            "run": run,
            "expect": expectations,
        }),
        embed: false,
    }
}

// ──────────────────────────────────────────────────────────────────
// Procedure body + trigger + steps
// ──────────────────────────────────────────────────────────────────

/// The lowered form of a procedure body: a step list, plus the body kind and
/// (for v2 code blocks) the serialized code so nothing is silently dropped.
struct LoweredBody {
    kind: &'static str,
    steps: Vec<Json>,
    code: Option<Json>,
}

/// Lower a `ProcedureBody`, handling BOTH forms honestly (C-NOSTUB-001):
/// - `Steps(_)` → the executor-facing step list (`kind = "steps"`, `code = null`).
/// - `Code(_)` → the v2 Rust-style block is NOT dropped: it is serialized into the
///   record under `code` with `body_kind = "code"`. The executor does not yet run
///   v2 code blocks (that runtime is a documented follow-up), but the parsed body
///   is preserved faithfully in the record rather than discarded. Serialization
///   uses `CodeBlock`'s own serde projection (the canonical px_ast JSON shape).
fn procedure_body_to_json(body: &ProcedureBody) -> LoweredBody {
    match body {
        ProcedureBody::Steps(steps) => LoweredBody {
            kind: "steps",
            steps: steps.iter().map(step_to_json).collect(),
            code: None,
        },
        ProcedureBody::Code(block) => LoweredBody {
            kind: "code",
            steps: Vec::new(),
            // Preserve the code block verbatim via its canonical serde shape.
            // `serde_json::to_value` on a plain data struct cannot fail here.
            code: Some(serde_json::to_value(block).unwrap_or(Json::Null)),
        },
    }
}

fn procedure_trigger_to_json(t: &ProcedureTrigger) -> Json {
    match t {
        ProcedureTrigger::Periodic { interval } => json!({
            "kind": "periodic",
            "interval": interval.as_ref().map(value_to_json),
        }),
        ProcedureTrigger::OnWrite { pattern, args } => json!({
            "kind": "on_write",
            "pattern": pattern.as_ref().map(|s| s.value.clone()),
            "args": args.as_ref().map(value_to_json),
        }),
        ProcedureTrigger::OnEvent(s) => json!({ "kind": "on_event", "event": s.value }),
        ProcedureTrigger::Startup => json!({ "kind": "startup" }),
        ProcedureTrigger::BeforeResponse => json!({ "kind": "before_response" }),
        ProcedureTrigger::AfterResponse => json!({ "kind": "after_response" }),
        ProcedureTrigger::Cron { schedule } => json!({
            "kind": "cron",
            "schedule": schedule.as_ref().map(value_to_json),
        }),
        ProcedureTrigger::Manual => json!({ "kind": "manual" }),
    }
}

/// Decode a `TryStep`/`ParallelBranch` retry spec into the executor-facing keys.
fn apply_retry_opts(obj: &mut Json, retries: Option<i64>, opts: &[RetryOpt]) {
    if let Some(n) = retries {
        obj["retry"] = json!(n);
    }
    for opt in opts {
        match opt {
            RetryOpt::Delay(ms) => obj["retry_delay_ms"] = json!(ms),
            RetryOpt::MaxDelay(ms) => obj["retry_max_delay_ms"] = json!(ms),
            RetryOpt::Jitter => obj["retry_jitter"] = json!(true),
            RetryOpt::Backoff(b) => {
                obj["retry_backoff"] = json!(match b {
                    px_ast::BackoffStrategy::Exponential => "exponential",
                    px_ast::BackoffStrategy::Fixed => "fixed",
                })
            }
        }
    }
}

/// Render `StepCallArgs` into the JSON `params` the executor resolves.
///
/// The executor treats `params` as a JSON object/array and runs `resolve_vars`
/// over it. Positional/values forms become a JSON array; the map/params forms
/// become a JSON object; `None` becomes an empty object.
fn step_call_args_to_json(args: &StepCallArgs) -> Json {
    match args {
        StepCallArgs::None => json!({}),
        StepCallArgs::Map(v) => value_to_json(v),
        StepCallArgs::Params(pairs) => {
            let mut obj = serde_json::Map::with_capacity(pairs.len());
            for (k, v) in pairs {
                obj.insert(k.name.clone(), value_to_json(v));
            }
            Json::Object(obj)
        }
        StepCallArgs::Values(vals) => Json::Array(vals.iter().map(value_to_json).collect()),
        StepCallArgs::Positional(exprs) => {
            // Positional exprs render to source-form strings; `resolve_vars`
            // substitutes any `$var` among them at runtime.
            Json::Array(exprs.iter().map(|e| json!(expr_to_string(e))).collect())
        }
    }
}

/// Lower a single v1 `Step` to the executor-facing JSON record.
///
/// The `kind` tags and field names below are the exact strings the (untouched)
/// `executor.rs` matches on (`call`/`match`/`when`/`loop`/`emit`/`try`/`parallel`/
/// `if`/`for`/`return`/`abort`/`assign`/`define`). Conditions/iterables render to
/// source-form strings (`expr_to_string`); values render via `value_to_json`.
pub(crate) fn step_to_json(step: &Step) -> Json {
    match step {
        Step::Define { var, value } => json!({
            "kind": "define",
            "var": var.name,
            "value": value_to_json(value),
        }),
        Step::Return { value } => json!({
            "kind": "return",
            "value": value.as_ref().map(value_to_json),
        }),
        Step::Abort { value } => json!({
            "kind": "abort",
            "value": value.as_ref().map(value_to_json),
        }),
        Step::Call(call) => step_call_to_json(call),
        Step::Assign { target, value } => json!({
            "kind": "assign",
            "var": var_ref_to_string(target),
            "value": value,
        }),
        Step::If {
            condition,
            then_steps,
            else_steps,
        } => json!({
            "kind": "if",
            "condition": expr_to_string(condition),
            "then": then_steps.iter().map(step_to_json).collect::<Vec<_>>(),
            "else": else_steps
                .as_ref()
                .map(|s| s.iter().map(step_to_json).collect::<Vec<_>>()),
        }),
        Step::Match { arms } => json!({
            "kind": "match",
            "arms": arms.iter().map(match_arm_to_json).collect::<Vec<_>>(),
        }),
        Step::When { condition, steps } => json!({
            "kind": "when",
            "condition": expr_to_string(condition),
            "steps": steps.iter().map(step_to_json).collect::<Vec<_>>(),
        }),
        Step::For {
            var,
            collection,
            steps,
        } => json!({
            "kind": "for",
            "var": var_ref_to_string(var),
            "iterable": expr_to_string(collection),
            "steps": steps.iter().map(step_to_json).collect::<Vec<_>>(),
        }),
        Step::Loop(l) => loop_step_to_json(l),
        Step::Emit { params } => {
            let mut ev = serde_json::Map::with_capacity(params.len());
            for (k, v) in params {
                ev.insert(k.name.clone(), value_to_json(v));
            }
            json!({ "kind": "emit", "event": Json::Object(ev) })
        }
        Step::Try(t) => try_step_to_json(t),
        Step::Parallel(p) => parallel_step_to_json(p),
    }
}

fn step_call_to_json(call: &StepCall) -> Json {
    json!({
        "kind": "call",
        "name": call.action.name,
        "params": step_call_args_to_json(&call.args),
        "output_var": call.output.as_ref().map(|o| o.name.clone()),
    })
}

fn match_arm_to_json(a: &MatchArm) -> Json {
    // Old record form: `{ condition: <expr-string>, result: <target> }`. The
    // executor compares the resolved subject against `condition` (or treats
    // "default"/"_" as the catch-all) and runs the `result` action.
    json!({
        "condition": expr_to_string(&a.pattern),
        "result": a.target.name,
    })
}

fn loop_step_to_json(l: &LoopStep) -> Json {
    let mut obj = json!({
        "kind": "loop",
        "as": l.item_name.as_ref().map(|i| i.name.clone()).unwrap_or_else(|| "item".to_string()),
        "key_as": l.key_name.as_ref().map(|i| i.name.clone()),
        "output_var": l.output.as_ref().map(|o| o.name.clone()),
        "steps": l.steps.iter().map(step_to_json).collect::<Vec<_>>(),
    });
    match &l.source {
        LoopSource::Over(id) => obj["over"] = json!(id.name),
        LoopSource::Times(n) => obj["times"] = json!(n),
    }
    obj
}

fn try_step_to_json(t: &TryStep) -> Json {
    let mut obj = json!({
        "kind": "try",
        "steps": t.steps.iter().map(step_to_json).collect::<Vec<_>>(),
        "catch": t
            .catch
            .as_ref()
            .map(|c| c.iter().map(step_to_json).collect::<Vec<_>>()),
    });
    apply_retry_opts(&mut obj, t.retries, &t.retry_opts);
    obj
}

fn parallel_step_to_json(p: &ParallelStep) -> Json {
    let branches: Vec<Json> = p
        .branches
        .iter()
        .map(|b| {
            let mut bo = json!({
                "name": b.name.name,
                "steps": b.steps.iter().map(step_to_json).collect::<Vec<_>>(),
            });
            apply_retry_opts(&mut bo, b.retries, &b.retry_opts);
            bo
        })
        .collect();
    json!({
        "kind": "parallel",
        "branches": branches,
        "output_var": p.output.as_ref().map(|o| o.name.clone()),
    })
}

// ──────────────────────────────────────────────────────────────────
// Compile-with-stats + compile-with-lint (public API preserved)
// ──────────────────────────────────────────────────────────────────

/// Per-construct counts produced by a compile pass.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct CompileStats {
    pub imports: usize,
    pub facts: usize,
    pub rules: usize,
    pub constraints: usize,
    pub contracts: usize,
    pub functions: usize,
    pub triggers: usize,
    pub procedures: usize,
    pub scenarios: usize,
    pub total: usize,
}

/// Records + stats produced by a compile pass. (Public API preserved from v1.)
#[derive(Debug)]
pub struct CompileResult {
    pub records: Vec<CompiledRecord>,
    pub stats: CompileStats,
}

/// Records + stats + lint diagnostics. (Public API preserved from v1.)
#[derive(Debug)]
pub struct CompileWithLintResult {
    pub records: Vec<CompiledRecord>,
    pub stats: CompileStats,
    pub diagnostics: Vec<super::lint::LintDiagnostic>,
}

/// Compile and also return per-construct statistics.
pub fn compile_with_stats(doc: &PxDocument) -> CompileResult {
    let mut stats = CompileStats::default();
    for stmt in &doc.statements {
        match stmt {
            Statement::Import(_) => stats.imports += 1,
            Statement::Fact(_) => stats.facts += 1,
            Statement::Rule(_) => stats.rules += 1,
            Statement::Constraint(_) => stats.constraints += 1,
            Statement::Contract(_) => stats.contracts += 1,
            Statement::Function(_) => stats.functions += 1,
            Statement::Trigger(_) => stats.triggers += 1,
            Statement::LegacyProcedure(_) | Statement::DataflowProcedure(_) => {
                stats.procedures += 1
            }
            Statement::Scenario(_) => stats.scenarios += 1,
            Statement::Entity(_) | Statement::Config(_) => {}
        }
    }
    let records = compile(doc);
    stats.total = records.len();
    CompileResult { records, stats }
}

/// Compile a document and run the lint pass over the same px-ast document.
pub fn compile_with_lint(doc: &PxDocument) -> CompileWithLintResult {
    let result = compile_with_stats(doc);
    let diagnostics = super::lint::lint(doc);
    CompileWithLintResult {
        records: result.records,
        stats: result.stats,
        diagnostics,
    }
}

/// Compile a single px-ast [`Step`] to its executor record JSON.
///
/// Public API preserved from v1 (was `compile_step`). Thin wrapper over the
/// crate-internal `step_to_json` so external callers keep a stable entry point.
pub fn compile_step(step: &Step) -> Json {
    step_to_json(step)
}

#[cfg(test)]
mod tests {
    use super::*;
    use px_compiler::parse;

    fn compile_src(src: &str) -> Vec<CompiledRecord> {
        let doc = parse(src).expect("parse");
        compile(&doc)
    }

    fn find<'a>(records: &'a [CompiledRecord], key: &str) -> &'a CompiledRecord {
        records
            .iter()
            .find(|r| r.key == key)
            .unwrap_or_else(|| panic!("record {key} not found; have: {:?}", records.iter().map(|r| &r.key).collect::<Vec<_>>()))
    }

    #[test]
    fn compiles_fact_to_record() {
        let recs = compile_src("fact MemoryEntry:\n  content: string\n  category: string\n");
        let rec = find(&recs, "px:fact/MemoryEntry");
        assert_eq!(rec.data["type"], "fact");
        assert_eq!(rec.data["name"], "MemoryEntry");
        assert_eq!(rec.data["fields"][0]["name"], "content");
        assert_eq!(rec.data["fields"][0]["type"], "string");
        assert!(rec.embed);
    }

    #[test]
    fn compiles_rule_conditions_to_source_strings() {
        // The executor parses `conditions` as &str - verify we render them back
        // to source-equivalent strings (not structured Expr JSON).
        let recs = compile_src(
            "fact msg_state:\n  level: string\n\nrule detect_urgency:\n  priority: 10\n  when:\n    - msg_state.level == \"urgent\"\n  then:\n    - action: flag_priority level: \"high\"\n",
        );
        let rec = find(&recs, "px:rule/detect_urgency");
        assert_eq!(rec.data["type"], "rule");
        assert_eq!(rec.data["priority"], 10);
        let conds = rec.data["conditions"].as_array().expect("conditions array");
        assert_eq!(conds.len(), 1);
        // rendered back to a source string, not an Expr object
        assert!(conds[0].is_string(), "condition must be a string, got {:?}", conds[0]);
        assert_eq!(conds[0], "msg_state.level == \"urgent\"");
        // action lowered with its param
        assert_eq!(rec.data["actions"][0]["kind"], "flag_priority");
        assert_eq!(rec.data["actions"][0]["level"], "high");
    }

    #[test]
    fn compiles_constraint_require_and_severity() {
        let recs = compile_src(
            "constraint no_empty:\n  scope: response\n  require: response.length > 0\n  severity: error\n  message: \"no empties\"\n",
        );
        let rec = find(&recs, "px:constraint/no_empty");
        assert_eq!(rec.data["type"], "constraint");
        assert_eq!(rec.data["severity"], "error");
        // require rendered to a source-form string the executor can parse
        assert_eq!(rec.data["require"], "response.length > 0");
        assert_eq!(rec.data["message"], "no empties");
    }

    #[test]
    fn compiles_function_with_types() {
        let recs = compile_src(
            "function classify(message: string) -> string:\n  mode: deterministic\n  \"\"\"Classify a message.\"\"\"\n",
        );
        let rec = find(&recs, "px:function/classify");
        assert_eq!(rec.data["type"], "function");
        assert_eq!(rec.data["mode"], "deterministic");
        assert_eq!(rec.data["params"][0]["name"], "message");
        assert_eq!(rec.data["params"][0]["type"], "string");
        assert_eq!(rec.data["return_type"], "string");
    }

    #[test]
    fn compile_with_stats_counts_constructs() {
        let stats = {
            let doc = parse(
                "fact A:\n  x: int\nfact B:\n  y: int\nfunction f(a: int) -> int:\n  mode: deterministic\n  \"\"\"Adds one.\"\"\"\n",
            )
            .expect("parse");
            compile_with_stats(&doc).stats
        };
        assert_eq!(stats.facts, 2);
        assert_eq!(stats.functions, 1);
        assert_eq!(stats.total, 3);
    }

    #[test]
    fn code_block_body_is_preserved_not_dropped() {
        // C-NOSTUB-001: a v2 code-block procedure body must NOT be silently
        // dropped. Whether or not the parser accepts a given code block, the
        // lowering path for `ProcedureBody::Code` serializes it into the record
        // under `code` with body_kind="code". This test exercises the lowering
        // helper directly so it is independent of grammar surface for v2 blocks.
        use px_ast::{CodeBlock, CodeExpr, CodeLiteral, CodeStmt, Ident, ProcedureBody};
        let body = ProcedureBody::Code(CodeBlock {
            statements: vec![CodeStmt::Let {
                name: Ident::new("x"),
                value: CodeExpr::Literal(CodeLiteral::Integer(1)),
            }],
        });
        let lowered = procedure_body_to_json(&body);
        assert_eq!(lowered.kind, "code");
        assert!(lowered.steps.is_empty());
        let code = lowered.code.expect("code block preserved");
        // The serialized code carries the statement - proof it wasn't dropped.
        assert_eq!(code["statements"][0]["kind"], "Let");
    }
}