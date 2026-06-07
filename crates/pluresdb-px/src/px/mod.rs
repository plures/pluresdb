//! Praxis Intent Language (.px) parser.
//!
//! Parses `.px` files into typed AST nodes using the pest PEG grammar.

pub mod builder;
pub mod compiler;
pub mod executor;
pub mod lint;
pub mod resolver;
pub mod scenario_runner;

#[cfg(feature = "async")]
pub mod async_executor;
#[cfg(feature = "async")]
pub mod compose;
#[cfg(feature = "watcher")]
pub mod watcher;

use pest::Parser;
use pest_derive::Parser;
use serde::{Deserialize, Serialize};

#[derive(Parser)]
#[grammar = "px/grammar.pest"]
pub struct PxParser;

/// A parsed .px document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PxDocument {
    pub imports: Vec<PxImport>,
    pub facts: Vec<PxFact>,
    pub rules: Vec<PxRule>,
    pub constraints: Vec<PxConstraint>,
    pub contracts: Vec<PxContract>,
    pub functions: Vec<PxFunction>,
    pub triggers: Vec<PxTrigger>,
    pub procedures: Vec<PxProcedure>,
    pub scenarios: Vec<PxScenario>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PxImport {
    pub path: String,
    pub alias: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PxFact {
    pub name: String,
    pub fields: Vec<PxField>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PxField {
    pub name: String,
    pub type_expr: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PxRule {
    pub name: String,
    pub priority: Option<i32>,
    pub conditions: Vec<String>,
    pub lets: Vec<(String, String)>,
    pub actions: Vec<PxAction>,
    pub captures: Vec<PxCapture>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PxAction {
    pub kind: String,
    pub params: std::collections::HashMap<String, serde_json::Value>,
    pub condition: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PxCapture {
    pub content: String,
    pub category: Option<String>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PxConstraint {
    pub name: String,
    pub scope: Option<String>,
    pub phases: Vec<String>,
    pub trait_category: Option<String>,
    pub weight: Option<f64>,
    pub prompt_injection: Option<String>,
    pub when_expr: Option<String>,
    pub require_expr: Option<String>,
    pub severity: String,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PxContract {
    pub name: String,
    pub given: Option<String>,
    pub when_desc: Option<String>,
    pub then_desc: Option<String>,
    pub threshold: Option<f64>,
    pub examples: Vec<PxExample>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PxExample {
    pub input: serde_json::Value,
    pub expect: serde_json::Value,
    pub threshold: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PxFunction {
    pub name: String,
    pub params: Vec<PxField>,
    pub return_type: String,
    pub mode: FunctionMode,
    pub docstring: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub enum FunctionMode {
    #[default]
    Deterministic,
    Probabilistic,
    Hybrid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PxTrigger {
    pub name: String,
    pub on_event: String,
    pub schedule: Option<String>,
    pub run: String,
}

/// A procedure — a sequence of steps triggered by events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PxProcedure {
    pub name: String,
    pub trigger: Option<PxProcedureTrigger>,
    pub given: Option<String>,
    pub steps: Vec<PxStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PxProcedureTrigger {
    pub kind: String,
    pub params: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PxStep {
    Call {
        name: String,
        params: serde_json::Value,
        output_var: Option<String>,
    },
    Match {
        arms: Vec<PxMatchArm>,
    },
    When {
        condition: String,
        steps: Vec<PxStep>,
    },
    Loop {
        /// Variable name to iterate over (resolved from vars at runtime).
        over: Option<String>,
        /// Fixed number of iterations (alternative to `over`).
        times: Option<u64>,
        /// Variable name for the current item (default: "item").
        item_var: String,
        /// Variable name for the current key during map iteration (default: "key").
        key_var: Option<String>,
        /// Nested steps executed per iteration.
        steps: Vec<PxStep>,
        /// Optional variable to collect results into.
        output_var: Option<String>,
    },
    Emit {
        /// Event data to emit (may contain $variable references).
        event: serde_json::Value,
    },
    Try {
        /// Steps to attempt.
        steps: Vec<PxStep>,
        /// Steps to execute on error.
        catch: Vec<PxStep>,
        /// Retry count (0 = no retry, N = up to N additional attempts).
        retry: Option<u64>,
        /// Delay between retries in milliseconds.
        retry_delay_ms: Option<u64>,
        /// Backoff strategy: "fixed" or "exponential".
        retry_backoff: Option<String>,
        /// Maximum delay cap in milliseconds (for exponential backoff).
        retry_max_delay_ms: Option<u64>,
        /// Whether to add jitter to retry delays.
        retry_jitter: Option<bool>,
    },
    Parallel {
        /// Named branches to execute concurrently.
        branches: Vec<PxParallelBranch>,
        /// Optional variable to collect results into (map of branch_name → last output).
        output_var: Option<String>,
    },
    /// Early return from the procedure with an optional value.
    Return {
        value: Option<serde_json::Value>,
    },
    /// Abort the procedure with an optional error/reason value.
    Abort {
        value: Option<serde_json::Value>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PxMatchArm {
    pub condition: String,
    pub result: String,
}

/// A named branch within a parallel step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PxParallelBranch {
    /// Branch name (used as key in output map).
    pub name: String,
    /// Steps to execute within this branch.
    pub steps: Vec<PxStep>,
    /// Retry count (0 = no retry, N = up to N additional attempts).
    pub retry: Option<u64>,
    /// Delay between retries in milliseconds.
    pub retry_delay_ms: Option<u64>,
    /// Backoff strategy: "fixed" or "exponential".
    pub retry_backoff: Option<String>,
    /// Maximum delay cap in milliseconds (for exponential backoff).
    pub retry_max_delay_ms: Option<u64>,
    /// Whether to add jitter to retry delays.
    pub retry_jitter: Option<bool>,
}

/// A test scenario — setup, run a procedure, check expectations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PxScenario {
    /// Scenario name.
    pub name: String,
    /// Human-readable description of the precondition.
    pub given: String,
    /// Setup steps to populate state before execution.
    pub setup: Vec<PxStep>,
    /// Optional procedure to run after setup.
    pub run: Option<PxScenarioRun>,
    /// Expectations to verify after execution.
    pub expectations: Vec<PxExpectation>,
}

/// Which procedure to execute in a scenario.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PxScenarioRun {
    /// Name of the procedure to invoke.
    pub procedure: String,
    /// Optional parameters to pass.
    pub params: Option<serde_json::Value>,
}

/// A single expectation check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PxExpectation {
    /// Whether this expectation is negated (NOT).
    pub negated: bool,
    /// The check function name (e.g. "has_entry", "event_emitted").
    pub check: String,
    /// Parameters for the check.
    pub params: Option<serde_json::Value>,
}

/// Parse a .px source string into a document AST.
pub fn parse(source: &str) -> Result<PxDocument, String> {
    let pairs = PxParser::parse(Rule::document, source).map_err(|e| format!("parse error: {e}"))?;

    Ok(builder::build(pairs))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_compiles() {
        let _ = PxParser::parse(Rule::ident, "hello");
    }

    #[test]
    fn parse_simple_fact() {
        let result = PxParser::parse(Rule::ident, "pr_state");
        assert!(result.is_ok());
    }

    #[test]
    fn parse_constraint_expr() {
        let result = PxParser::parse(Rule::expr, "pr.ci_status == green");
        assert!(result.is_ok(), "failed to parse expression");
    }

    #[test]
    fn parse_expr_with_symbolic_logic_ops() {
        // && and || should work alongside 'and' and 'or'
        let cases = [
            "a == b && c == d",
            "a == b || c == d",
            "x > 1 && y < 10 || z == 0",
            "a and b",
            "a or b",
        ];
        for case in cases {
            let result = PxParser::parse(Rule::expr, case);
            assert!(result.is_ok(), "failed to parse expr: {case}");
        }
    }

    #[test]
    fn parse_value_types() {
        assert!(PxParser::parse(Rule::value, "\"hello\"").is_ok());
        assert!(PxParser::parse(Rule::value, "42").is_ok());
        assert!(PxParser::parse(Rule::value, "3.14").is_ok());
        assert!(PxParser::parse(Rule::value, "true").is_ok());
        assert!(PxParser::parse(Rule::value, "false").is_ok());
    }

    #[test]
    fn parse_document_extracts_rule_constraint_and_contract() {
        let source = r#"
fact pr_state:
  ci_status: enum(green, failing, pending)
  has_review: bool

rule auto_merge:
  when:
    - pr_state.ci_status == green
    - pr_state.has_review == true
  then:
    - action: merge_pr method: "squash"
  capture:
    - fact: "Merged PR" category: work_in_progress tags: ["lifecycle", "merge"]

constraint merge_requires_review:
  when: pr_state.ci_status == green
  require: pr_state.has_review == true
  severity: error

contract auto_merge_behavior:
  given: "CI green + reviewed"
  when: "lifecycle evaluates"
  then: "PR merged"
  examples:
    - input: { ci_status: green, has_review: true }
      expect: [merge_pr]
"#;

        let document = parse(source).expect("expected valid .px document");
        assert_eq!(document.facts.len(), 1);
        assert_eq!(document.rules.len(), 1);
        assert_eq!(document.constraints.len(), 1);
        assert_eq!(document.contracts.len(), 1);

        let rule = &document.rules[0];
        assert_eq!(rule.conditions.len(), 2);
        assert_eq!(rule.actions.len(), 1);
        assert_eq!(rule.captures.len(), 1);
        assert_eq!(rule.captures[0].content, "Merged PR");
        assert_eq!(
            rule.captures[0].category.as_deref(),
            Some("work_in_progress")
        );
        assert_eq!(rule.captures[0].tags, vec!["lifecycle", "merge"]);
    }

    #[test]
    fn parse_personality_constraint() {
        let source = r#"
constraint warmth:
  phase: user_communication, error_reporting
  trait: warmth
  weight: 0.8
  prompt: "Use a warm, approachable tone."
  severity: info
"#;

        let doc = parse(source).expect("expected valid .px");
        assert_eq!(doc.constraints.len(), 1);
        let c = &doc.constraints[0];
        assert_eq!(c.name, "warmth");
        assert_eq!(c.trait_category.as_deref(), Some("warmth"));
        assert_eq!(c.phases, vec!["user_communication", "error_reporting"]);
        assert!((c.weight.unwrap() - 0.8).abs() < f64::EPSILON);
        assert_eq!(
            c.prompt_injection.as_deref(),
            Some("Use a warm, approachable tone.")
        );
        assert_eq!(c.severity, "info");
        // when/require are optional for personality constraints
        assert!(c.when_expr.is_none());
        assert!(c.require_expr.is_none());
    }

    #[test]
    fn parse_constraint_with_symbolic_operators() {
        let source = r#"
constraint deploy_gate:
  when: ci.status == green && review.approved == true
  require: deploy.target != production || deploy.canary_ok == true
  severity: error
  message: "Cannot deploy to production without canary pass"
"#;

        let doc = parse(source).expect("expected valid .px");
        assert_eq!(doc.constraints.len(), 1);
        let c = &doc.constraints[0];
        assert_eq!(c.name, "deploy_gate");
        assert!(c.when_expr.as_ref().unwrap().contains("&&"));
        assert!(c.require_expr.as_ref().unwrap().contains("||"));
        assert_eq!(c.severity, "error");
    }

    #[test]
    fn parse_procedure_with_loop() {
        let source = "procedure batch_process:\n  trigger: manual\n  fetch_items {} -> $items\n  loop over $items as item -> $results:\n    process_item {val: $item}\n  end\n";

        let doc = parse(source).expect("parse failed");
        assert_eq!(doc.procedures.len(), 1);
        let proc = &doc.procedures[0];
        assert_eq!(proc.name, "batch_process");
        assert_eq!(proc.steps.len(), 2);

        match &proc.steps[1] {
            PxStep::Loop {
                over,
                times,
                item_var,
                key_var: _,
                steps,
                output_var,
            } => {
                assert_eq!(over.as_deref(), Some("$items"));
                assert!(times.is_none());
                assert_eq!(item_var, "item");
                assert_eq!(output_var.as_deref(), Some("results"));
                assert_eq!(steps.len(), 1);
            }
            other => panic!("expected Loop step, got {:?}", other),
        }
    }

    #[test]
    fn parse_procedure_loop_key_as() {
        let source = "procedure iter_config:\n  trigger: manual\n  loop over $config as val key_as k -> $out:\n    process {key: $k, value: $val}\n  end\n";

        let doc = parse(source).expect("parse failed");
        assert_eq!(doc.procedures.len(), 1);
        let proc = &doc.procedures[0];
        assert_eq!(proc.steps.len(), 1);

        match &proc.steps[0] {
            PxStep::Loop {
                over,
                times,
                item_var,
                key_var,
                steps,
                output_var,
            } => {
                assert_eq!(over.as_deref(), Some("$config"));
                assert!(times.is_none());
                assert_eq!(item_var, "val");
                assert_eq!(key_var.as_deref(), Some("k"));
                assert_eq!(output_var.as_deref(), Some("out"));
                assert_eq!(steps.len(), 1);
            }
            other => panic!("expected Loop step, got {:?}", other),
        }
    }

    #[test]
    fn parse_procedure_with_emit() {
        let source =
            "procedure notify:\n  trigger: manual\n  emit {type: \"alert\", level: \"high\"}\n";

        let doc = parse(source).expect("parse failed");
        assert_eq!(doc.procedures.len(), 1);
        match &doc.procedures[0].steps[0] {
            PxStep::Emit { event } => {
                assert_eq!(event["type"], "alert");
                assert_eq!(event["level"], "high");
            }
            other => panic!("expected Emit step, got {:?}", other),
        }
    }

    #[test]
    fn parse_procedure_with_try_catch() {
        let source = "procedure resilient:\n  trigger: manual\n  try:\n    risky_action {}\n  catch:\n    fallback {}\n  end\n";

        let doc = parse(source).expect("parse failed");
        assert_eq!(doc.procedures.len(), 1);
        match &doc.procedures[0].steps[0] {
            PxStep::Try { steps, catch, .. } => {
                assert_eq!(steps.len(), 1);
                assert_eq!(catch.len(), 1);
            }
            other => panic!("expected Try step, got {:?}", other),
        }
    }

    #[test]
    fn parse_try_with_retry() {
        let source = "procedure resilient:\n  trigger: manual\n  try retry 3 delay 1000 ms backoff exponential max_delay 10000 ms jitter:\n    risky_action {}\n  catch:\n    fallback {}\n  end\n";

        let doc = parse(source).expect("parse failed");
        assert_eq!(doc.procedures.len(), 1);
        match &doc.procedures[0].steps[0] {
            PxStep::Try { steps, catch, retry, retry_delay_ms, retry_backoff, retry_max_delay_ms, retry_jitter } => {
                assert_eq!(steps.len(), 1);
                assert_eq!(catch.len(), 1);
                assert_eq!(*retry, Some(3));
                assert_eq!(*retry_delay_ms, Some(1000));
                assert_eq!(retry_backoff.as_deref(), Some("exponential"));
                assert_eq!(*retry_max_delay_ms, Some(10000));
                assert_eq!(*retry_jitter, Some(true));
            }
            other => panic!("expected Try step, got {:?}", other),
        }
    }

    #[test]
    fn parse_try_retry_no_options() {
        let source = "procedure simple_retry:\n  trigger: manual\n  try retry 2:\n    flaky_call {}\n  end\n";

        let doc = parse(source).expect("parse failed");
        match &doc.procedures[0].steps[0] {
            PxStep::Try { retry, retry_delay_ms, retry_backoff, .. } => {
                assert_eq!(*retry, Some(2));
                assert!(retry_delay_ms.is_none());
                assert!(retry_backoff.is_none());
            }
            other => panic!("expected Try step, got {:?}", other),
        }
    }

    #[test]
    fn parse_try_retry_compiles_to_json() {
        use crate::px::compiler::compile;

        let source = "procedure with_retry:\n  trigger: manual\n  try retry 2 delay 500 ms backoff fixed:\n    call_api {}\n  catch:\n    handle_error {}\n  end\n";

        let doc = parse(source).expect("parse failed");
        let records = compile(&doc);
        let data = &records[0].data;
        let steps = data["steps"].as_array().unwrap();
        let try_step = &steps[0];
        assert_eq!(try_step["kind"], "try");
        assert_eq!(try_step["retry"], 2);
        assert_eq!(try_step["retry_delay_ms"], 500);
        assert_eq!(try_step["retry_backoff"], "fixed");
        assert!(try_step.get("retry_max_delay_ms").is_none() || try_step["retry_max_delay_ms"].is_null());
        assert!(try_step.get("retry_jitter").is_none() || try_step["retry_jitter"].is_null());
    }

    #[test]
    fn parse_procedure_with_parallel() {
        let source = "procedure fan_out:\n  trigger: manual\n  parallel -> $results:\n    branch fetch_users:\n      get_users {}\n    end\n    branch fetch_posts:\n      get_posts {}\n    end\n  end\n";

        let doc = parse(source).expect("parse failed");
        assert_eq!(doc.procedures.len(), 1);
        match &doc.procedures[0].steps[0] {
            PxStep::Parallel { branches, output_var } => {
                assert_eq!(branches.len(), 2);
                assert_eq!(branches[0].name, "fetch_users");
                assert_eq!(branches[1].name, "fetch_posts");
                assert_eq!(branches[0].steps.len(), 1);
                assert_eq!(branches[1].steps.len(), 1);
                assert_eq!(output_var.as_deref(), Some("results"));
            }
            other => panic!("expected Parallel step, got {:?}", other),
        }
    }

    #[test]
    fn full_pipeline_loop_emit_try() {
        // Parse → Compile → Execute with all new step kinds
        use crate::px::compiler::compile;
        use crate::px::executor::{self, ActionHandler, ExecutionError};
        use serde_json::{json, Value};

        struct TestHandler;
        impl ActionHandler for TestHandler {
            fn call(&self, name: &str, _params: &Value) -> Result<Value, ExecutionError> {
                match name {
                    "get_items" => Ok(json!(["a", "b", "c"])),
                    "transform" => Ok(json!("done")),
                    _ => Err(ExecutionError::UnknownAction(name.to_string())),
                }
            }
        }

        let source = "procedure pipeline:\n  trigger: manual\n  get_items {} -> $items\n  loop over $items as item -> $results:\n    transform {val: $item}\n  end\n  emit {type: \"complete\", count: 3}\n";

        let doc = parse(source).expect("parse failed");
        let records = compile(&doc);
        assert_eq!(records.len(), 1);

        let result = executor::execute(&records[0].data, &TestHandler).unwrap();
        assert!(result.success);
        assert_eq!(
            result.variables.get("results"),
            Some(&json!(["done", "done", "done"]))
        );
        // Check emit was captured
        let emit = result.variables.get("emit").unwrap().as_array().unwrap();
        assert_eq!(emit.len(), 1);
        assert_eq!(emit[0]["type"], "complete");
    }

    #[test]
    fn full_pipeline_loop_key_as() {
        // Parse → Compile → Execute with key_as syntax
        use crate::px::compiler::compile;
        use crate::px::executor::{self, ActionHandler, ExecutionError};
        use serde_json::{json, Value};

        struct KvHandler;
        impl ActionHandler for KvHandler {
            fn call(&self, name: &str, params: &Value) -> Result<Value, ExecutionError> {
                match name {
                    "collect" => {
                        let k = params.get("k").and_then(|v| v.as_str()).unwrap_or("");
                        let v = params.get("v").and_then(|v| v.as_str()).unwrap_or("");
                        Ok(json!(format!("{k}={v}")))
                    }
                    _ => Err(ExecutionError::UnknownAction(name.to_string())),
                }
            }
        }

        let source = "procedure map_kv:\n  trigger: manual\n  loop over $config as val key_as k -> $pairs:\n    collect {k: $k, v: $val}\n  end\n";

        let doc = parse(source).expect("parse failed");
        let records = compile(&doc);
        assert_eq!(records.len(), 1);

        let mut vars = std::collections::HashMap::new();
        vars.insert("config".to_string(), json!({"host": "localhost", "port": "8080"}));

        let result = executor::execute_with_vars(&records[0].data, &KvHandler, vars).unwrap();
        assert!(result.success);
        let pairs = result.variables.get("pairs").unwrap().as_array().unwrap();
        assert_eq!(pairs.len(), 2);
        // Values should be "key=val" strings (order varies for maps)
        let mut sorted: Vec<&str> = pairs.iter().map(|v| v.as_str().unwrap()).collect();
        sorted.sort();
        assert_eq!(sorted, vec!["host=localhost", "port=8080"]);
    }
}
#[cfg(test)]
mod parse_value_tests {
    use super::*;
    use pest::Parser;

    #[test]
    fn parse_var_ref() {
        let r = PxParser::parse(Rule::var_ref, "$item");
        assert!(r.is_ok(), "var_ref failed: {:?}", r.err());
    }

    #[test]
    fn parse_value_with_var_ref() {
        let r = PxParser::parse(Rule::value, "$item");
        assert!(r.is_ok(), "value($item) failed: {:?}", r.err());
    }

    #[test]
    fn parse_map_val_with_var_ref() {
        let r = PxParser::parse(Rule::map_val, "{val: $item}");
        assert!(r.is_ok(), "map_val failed: {:?}", r.err());
    }
}

#[cfg(test)]
mod parse_step_tests {
    use super::*;

    #[test]
    fn parse_procedure_call_with_var_ref_in_map() {
        let source = "procedure test:\n  trigger: manual\n  do_thing {val: $foo}\n";
        let doc = parse(source).expect("parse failed");
        assert_eq!(doc.procedures.len(), 1);
        assert_eq!(doc.procedures[0].steps.len(), 1);
    }

    #[test]
    fn parse_parallel_branch_with_retry() {
        let source = "procedure resilient_fan_out:\n  trigger: manual\n  parallel -> $results:\n    branch fetch_users retry 3 delay 500 ms backoff exponential max_delay 5000 ms jitter:\n      get_users {}\n    end\n    branch fetch_posts retry 2 delay 100 ms:\n      get_posts {}\n    end\n  end\n";

        let doc = parse(source).expect("parse failed");
        assert_eq!(doc.procedures.len(), 1);
        match &doc.procedures[0].steps[0] {
            PxStep::Parallel { branches, output_var } => {
                assert_eq!(branches.len(), 2);
                assert_eq!(output_var.as_deref(), Some("results"));

                // First branch: full retry config
                let b0 = &branches[0];
                assert_eq!(b0.name, "fetch_users");
                assert_eq!(b0.retry, Some(3));
                assert_eq!(b0.retry_delay_ms, Some(500));
                assert_eq!(b0.retry_backoff.as_deref(), Some("exponential"));
                assert_eq!(b0.retry_max_delay_ms, Some(5000));
                assert_eq!(b0.retry_jitter, Some(true));
                assert_eq!(b0.steps.len(), 1);

                // Second branch: partial retry config
                let b1 = &branches[1];
                assert_eq!(b1.name, "fetch_posts");
                assert_eq!(b1.retry, Some(2));
                assert_eq!(b1.retry_delay_ms, Some(100));
                assert!(b1.retry_backoff.is_none());
                assert!(b1.retry_max_delay_ms.is_none());
                assert!(b1.retry_jitter.is_none());
                assert_eq!(b1.steps.len(), 1);
            }
            other => panic!("expected Parallel step, got {:?}", other),
        }
    }

    #[test]
    fn parse_parallel_branch_without_retry() {
        // Ensure backwards compat — branches without retry still parse
        let source = "procedure simple:\n  trigger: manual\n  parallel:\n    branch a:\n      do_a {}\n    end\n    branch b:\n      do_b {}\n    end\n  end\n";

        let doc = parse(source).expect("parse failed");
        match &doc.procedures[0].steps[0] {
            PxStep::Parallel { branches, .. } => {
                assert_eq!(branches.len(), 2);
                assert!(branches[0].retry.is_none());
                assert!(branches[1].retry.is_none());
            }
            other => panic!("expected Parallel step, got {:?}", other),
        }
    }

    #[test]
    fn parse_parallel_branch_retry_compiles_to_json() {
        use crate::px::compiler::compile;

        let source = "procedure with_retry:\n  trigger: manual\n  parallel -> $out:\n    branch api retry 2 delay 200 ms backoff fixed jitter:\n      call_api {}\n    end\n  end\n";

        let doc = parse(source).expect("parse failed");
        let records = compile(&doc);
        assert_eq!(records.len(), 1);

        let data = &records[0].data;
        let steps = data["steps"].as_array().unwrap();
        let parallel = &steps[0];
        assert_eq!(parallel["kind"], "parallel");

        let branches = parallel["branches"].as_array().unwrap();
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0]["name"], "api");
        assert_eq!(branches[0]["retry"], 2);
        assert_eq!(branches[0]["retry_delay_ms"], 200);
        assert_eq!(branches[0]["retry_backoff"], "fixed");
        assert_eq!(branches[0]["retry_jitter"], true);
    }

    // === Match Expression Grammar Tests ===

    #[test]
    fn parse_match_expr_simple() {
        let result = PxParser::parse(Rule::match_expr, r#"match status { "active" => "green", _ => "gray" }"#);
        assert!(result.is_ok(), "failed to parse simple match expr: {:?}", result.err());
    }

    #[test]
    fn parse_match_expr_variable_subject() {
        let result = PxParser::parse(Rule::match_expr, r#"match $state { "running" => "ok", "stopped" => "warn", _ => "unknown" }"#);
        assert!(result.is_ok(), "failed to parse match expr with var subject: {:?}", result.err());
    }

    #[test]
    fn parse_match_expr_multi_pattern() {
        let result = PxParser::parse(Rule::match_expr, r#"match level { "error" | "fatal" => "red", "warn" => "yellow", _ => "white" }"#);
        assert!(result.is_ok(), "failed to parse match expr with multi-pattern: {:?}", result.err());
    }

    #[test]
    fn parse_match_expr_numeric_patterns() {
        let result = PxParser::parse(Rule::match_expr, r#"match code { 200 => "ok", 404 => "not_found", _ => "error" }"#);
        assert!(result.is_ok(), "failed to parse match expr with numeric patterns: {:?}", result.err());
    }

    #[test]
    fn parse_match_expr_dotted_subject() {
        let result = PxParser::parse(Rule::match_expr, r#"match event.kind { "push" => "build", _ => "skip" }"#);
        assert!(result.is_ok(), "failed to parse match expr with dotted subject: {:?}", result.err());
    }

    #[test]
    fn parse_match_expr_multiline() {
        let input = "match status {\n\"active\" => \"green\",\n\"paused\" => \"yellow\",\n_ => \"gray\"\n}";
        let result = PxParser::parse(Rule::match_expr, input);
        assert!(result.is_ok(), "failed to parse multiline match expr: {:?}", result.err());
    }

    #[test]
    fn parse_match_expr_in_expression_context() {
        // match_expr should be valid as part of a term/expr
        let result = PxParser::parse(Rule::expr, r#"match x { "a" => "1", _ => "0" }"#);
        assert!(result.is_ok(), "failed to parse match expr as expression: {:?}", result.err());
    }

    #[test]
    fn parse_match_expr_result_is_var_ref() {
        let result = PxParser::parse(Rule::match_expr, r#"match mode { "fast" => $speed_val, _ => $default_val }"#);
        assert!(result.is_ok(), "failed to parse match expr with var_ref results: {:?}", result.err());
    }

    #[test]
    fn parse_match_expr_multi_pattern_three_values() {
        let result = PxParser::parse(Rule::match_expr, r#"match tier { "s1" | "s2" | "s3" => "standard", "p1" | "p2" => "premium", _ => "unknown" }"#);
        assert!(result.is_ok(), "failed to parse match expr with 3-value multi-pattern: {:?}", result.err());
    }

    #[test]
    fn parse_match_expr_in_constraint_require() {
        // A constraint that uses match in its require clause
        let source = r#"constraint color_rule:
  require: match status { "active" => "green", _ => "gray" } == "green"
  severity: error
  message: "status must be active"
"#;
        let doc = parse(source);
        assert!(doc.is_ok(), "failed to parse constraint with match expr: {:?}", doc.err());
    }

    #[test]
    fn parse_procedure_with_return() {
        let source = "procedure guard_check:\n  trigger: manual\n  validate_input {} -> $valid\n  when $valid == false:\n    return 'invalid'\n  end\n  process_data {}\n";
        let doc = parse(source).expect("parse failed");
        assert_eq!(doc.procedures.len(), 1);
        let proc = &doc.procedures[0];
        assert_eq!(proc.steps.len(), 3); // validate, when, process
        match &proc.steps[1] {
            PxStep::When { steps, .. } => {
                assert_eq!(steps.len(), 1);
                match &steps[0] {
                    PxStep::Return { value } => {
                        assert!(value.is_some());
                    }
                    other => panic!("expected Return, got {:?}", other),
                }
            }
            other => panic!("expected When, got {:?}", other),
        }
    }

    #[test]
    fn parse_procedure_with_abort() {
        let source = "procedure fail_fast:\n  trigger: manual\n  check_system {} -> $ok\n  abort 'system down'\n  do_work {}\n";
        let doc = parse(source).expect("parse failed");
        let proc = &doc.procedures[0];
        assert_eq!(proc.steps.len(), 3);
        match &proc.steps[1] {
            PxStep::Abort { value } => {
                assert_eq!(value.as_ref().unwrap(), "system down");
            }
            other => panic!("expected Abort, got {:?}", other),
        }
    }

    #[test]
    fn parse_scenario_basic() {
        let source = r#"
scenario expired_entries_removed:
  given: "Cache has expired entries"
  setup:
    put_entry {key: "old", ttl_secs: 1}
    advance_time {secs: 10}
  run: invalidate_expired
  expect:
    - NOT has_entry {key: "old"}
    - has_entry {key: "fresh"}
    - event_emitted {event: "cache.invalidated", key: "old"}
"#;
        let doc = parse(source).expect("failed to parse scenario");
        assert_eq!(doc.scenarios.len(), 1);
        let s = &doc.scenarios[0];
        assert_eq!(s.name, "expired_entries_removed");
        assert_eq!(s.given, "Cache has expired entries");
        assert_eq!(s.setup.len(), 2);
        assert!(s.run.is_some());
        assert_eq!(s.run.as_ref().unwrap().procedure, "invalidate_expired");
        assert_eq!(s.expectations.len(), 3);
        assert!(s.expectations[0].negated);
        assert_eq!(s.expectations[0].check, "has_entry");
        assert!(!s.expectations[1].negated);
        assert_eq!(s.expectations[1].check, "has_entry");
        assert!(!s.expectations[2].negated);
        assert_eq!(s.expectations[2].check, "event_emitted");
    }

    #[test]
    fn parse_scenario_without_run() {
        let source = r#"
scenario negative_ttl_rejected:
  given: "Attempting to create an entry with negative TTL"
  setup:
    put_entry {key: "bad", value: "x", ttl_secs: 0}
  expect:
    - constraint_violated {name: "ttl_positive"}
"#;
        let doc = parse(source).expect("failed to parse scenario without run");
        assert_eq!(doc.scenarios.len(), 1);
        let s = &doc.scenarios[0];
        assert_eq!(s.name, "negative_ttl_rejected");
        assert!(s.run.is_none());
        assert_eq!(s.setup.len(), 1);
        assert_eq!(s.expectations.len(), 1);
        assert!(!s.expectations[0].negated);
        assert_eq!(s.expectations[0].check, "constraint_violated");
    }

    #[test]
    fn parse_scenario_without_setup() {
        let source = r#"
scenario simple_check:
  given: "System is in default state"
  run: check_health
  expect:
    - is_healthy {}
"#;
        let doc = parse(source).expect("failed to parse scenario without setup");
        assert_eq!(doc.scenarios.len(), 1);
        let s = &doc.scenarios[0];
        assert_eq!(s.name, "simple_check");
        assert!(s.setup.is_empty());
        assert!(s.run.is_some());
        assert_eq!(s.expectations.len(), 1);
    }

    #[test]
    fn parse_document_with_scenario_and_constraint() {
        let source = r#"
constraint ttl_positive:
  require: entry.ttl_secs > 0
  severity: error

scenario ttl_enforced:
  given: "Testing TTL constraint"
  setup:
    put_entry {key: "test", ttl_secs: 5}
  expect:
    - NOT constraint_violated {name: "ttl_positive"}
"#;
        let doc = parse(source).expect("failed to parse mixed doc");
        assert_eq!(doc.constraints.len(), 1);
        assert_eq!(doc.scenarios.len(), 1);
    }
}
