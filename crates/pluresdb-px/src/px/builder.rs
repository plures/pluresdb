//! AST builder — walks pest parse pairs and produces typed PxDocument.

use pest::iterators::{Pair, Pairs};
use std::collections::HashMap;

use super::{
    FunctionMode, PxAction, PxCapture, PxConstraint, PxContract, PxDocument, PxExample,
    PxExpectation, PxFact, PxField, PxFunction, PxImport, PxMatchArm, PxParallelBranch,
    PxProcedure, PxProcedureTrigger, PxRule, PxScenario, PxScenarioRun, PxStep, PxTrigger, Rule,
};

/// Build a PxDocument from parsed pest pairs.
pub fn build(pairs: Pairs<'_, Rule>) -> PxDocument {
        let mut doc = PxDocument {
        imports: vec![],
        facts: vec![],
        rules: vec![],
        constraints: vec![],
        contracts: vec![],
        functions: vec![],
        triggers: vec![],
        procedures: vec![],
        scenarios: vec![],
    };

    for pair in pairs {
        push_pair_into_document(pair, &mut doc);
    }

    doc
}

fn push_pair_into_document(pair: Pair<'_, Rule>, doc: &mut PxDocument) {
    match pair.as_rule() {
        Rule::document => {
            for inner in pair.into_inner() {
                push_pair_into_document(inner, doc);
            }
        }
        Rule::statement => {
            for inner in pair.into_inner() {
                push_pair_into_document(inner, doc);
            }
        }
        Rule::import_decl => doc.imports.push(build_import(pair)),
        Rule::fact_decl => doc.facts.push(build_fact(pair)),
        Rule::rule_decl => doc.rules.push(build_rule(pair)),
        Rule::constraint_decl => doc.constraints.push(build_constraint(pair)),
        Rule::contract_decl => doc.contracts.push(build_contract(pair)),
        Rule::function_decl => doc.functions.push(build_function(pair)),
        Rule::trigger_decl => doc.triggers.push(build_trigger(pair)),
        Rule::procedure_decl => doc.procedures.push(build_procedure(pair)),
        Rule::scenario_decl => doc.scenarios.push(build_scenario(pair)),
        Rule::EOI => {}
        _ => {}
    }
}

fn build_import(pair: Pair<'_, Rule>) -> PxImport {
    let mut inner = pair.into_inner();
    let path = inner
        .next()
        .map(|p| p.as_str().to_string())
        .unwrap_or_default();
    let alias = inner.next().map(|p| p.as_str().to_string());
    PxImport { path, alias }
}

fn build_fact(pair: Pair<'_, Rule>) -> PxFact {
    let mut inner = pair.into_inner();
    let name = next_str(&mut inner);
    let fields = inner
        .find(|p| p.as_rule() == Rule::field_list)
        .map(|field_list| {
            field_list
                .into_inner()
                .filter(|p| p.as_rule() == Rule::field)
                .map(build_field)
                .collect()
        })
        .unwrap_or_default();
    PxFact { name, fields }
}

fn build_field(pair: Pair<'_, Rule>) -> PxField {
    let mut inner = pair.into_inner();
    let name = next_str(&mut inner);
    let type_expr = inner
        .next()
        .map(|p| p.as_str().to_string())
        .unwrap_or_default();
    PxField { name, type_expr }
}

fn build_rule(pair: Pair<'_, Rule>) -> PxRule {
    let mut inner = pair.into_inner();
    let name = next_str(&mut inner);

    let mut priority = None;
    let mut conditions = vec![];
    let mut lets = vec![];
    let mut actions = vec![];
    let mut captures = vec![];

    let Some(rule_body) = inner.find(|p| p.as_rule() == Rule::rule_body) else {
        return PxRule {
            name,
            priority,
            conditions,
            lets,
            actions,
            captures,
        };
    };

    for child in rule_body.into_inner() {
        match child.as_rule() {
            Rule::priority_clause => {
                priority = child
                    .into_inner()
                    .next()
                    .and_then(|p| p.as_str().parse().ok());
            }
            Rule::when_clause => {
                if let Some(condition_list) = child
                    .into_inner()
                    .find(|p| p.as_rule() == Rule::condition_list)
                {
                    conditions = condition_list
                        .into_inner()
                        .filter(|p| p.as_rule() == Rule::expr)
                        .map(|p| p.as_str().to_string())
                        .collect();
                }
            }
            Rule::let_clause => {
                let mut lc = child.into_inner();
                let var = next_str(&mut lc);
                let expr = lc
                    .next()
                    .map(|p| p.as_str().to_string())
                    .unwrap_or_default();
                lets.push((var, expr));
            }
            Rule::action_list => {
                actions = child
                    .into_inner()
                    .filter(|p| p.as_rule() == Rule::action_stmt)
                    .map(build_action)
                    .collect();
            }
            Rule::then_clause => {
                if let Some(action_list) = child
                    .into_inner()
                    .find(|p| p.as_rule() == Rule::action_list)
                {
                    actions = action_list
                        .into_inner()
                        .filter(|p| p.as_rule() == Rule::action_stmt)
                        .map(build_action)
                        .collect();
                }
            }
            Rule::capture_clause => {
                captures = child
                    .into_inner()
                    .filter(|p| p.as_rule() == Rule::capture_entry)
                    .map(build_capture)
                    .collect();
            }
            _ => {}
        }
    }

    PxRule {
        name,
        priority,
        conditions,
        lets,
        actions,
        captures,
    }
}

fn build_action(pair: Pair<'_, Rule>) -> PxAction {
    let inner = pair.into_inner().next().unwrap();
    match inner.as_rule() {
        Rule::simple_action => {
            let mut parts = inner.into_inner();
            let kind = next_str(&mut parts);
            let params: HashMap<String, serde_json::Value> = parts
                .filter(|p| p.as_rule() == Rule::param_pair)
                .map(|p| {
                    let mut kv = p.into_inner();
                    let k = next_str(&mut kv);
                    let v = kv
                        .next()
                        .map(|p| parse_value(p))
                        .unwrap_or(serde_json::Value::Null);
                    (k, v)
                })
                .collect();
            PxAction {
                kind,
                params,
                condition: None,
            }
        }
        Rule::conditional_action => {
            let mut parts = inner.into_inner();
            let cond = parts.next().map(|p| p.as_str().to_string());
            let action_pair = parts.next().unwrap();
            let mut action = build_action_from_simple(action_pair);
            action.condition = cond;
            action
        }
        _ => PxAction {
            kind: "unknown".into(),
            params: HashMap::new(),
            condition: None,
        },
    }
}

fn build_action_from_simple(pair: Pair<'_, Rule>) -> PxAction {
    let mut parts = pair.into_inner();
    let kind = next_str(&mut parts);
    let params: HashMap<String, serde_json::Value> = parts
        .filter(|p| p.as_rule() == Rule::param_pair)
        .map(|p| {
            let mut kv = p.into_inner();
            let k = next_str(&mut kv);
            let v = kv
                .next()
                .map(|p| parse_value(p))
                .unwrap_or(serde_json::Value::Null);
            (k, v)
        })
        .collect();
    PxAction {
        kind,
        params,
        condition: None,
    }
}

fn build_capture(pair: Pair<'_, Rule>) -> PxCapture {
    let mut content = String::new();
    let mut category = None;
    let mut tags = vec![];

    for child in pair.into_inner() {
        match child.as_rule() {
            Rule::string if content.is_empty() => {
                content = unquote(child.as_str());
            }
            Rule::ident if category.is_none() => {
                category = Some(child.as_str().to_string());
            }
            Rule::list_val => {
                tags = child
                    .into_inner()
                    .map(parse_value)
                    .filter_map(|v| v.as_str().map(ToOwned::to_owned))
                    .collect();
            }
            _ => {}
        }
    }

    PxCapture {
        content,
        category,
        tags,
    }
}

fn build_constraint(pair: Pair<'_, Rule>) -> PxConstraint {
    let mut inner = pair.into_inner();
    let name = next_str(&mut inner);

    let mut scope = None;
    let mut phases = vec![];
    let mut trait_category = None;
    let mut weight = None;
    let mut prompt_injection = None;
    let mut when_expr = None;
    let mut require_expr = None;
    let mut severity = "warning".to_string();
    let mut message = None;

    let Some(constraint_body) = inner.find(|p| p.as_rule() == Rule::constraint_body) else {
        return PxConstraint {
            name,
            scope,
            phases,
            trait_category,
            weight,
            prompt_injection,
            when_expr,
            require_expr,
            severity,
            message,
        };
    };

    for child in constraint_body.into_inner() {
        match child.as_rule() {
            Rule::scope_clause => scope = child.into_inner().next().map(|p| p.as_str().to_string()),
            Rule::phase_clause => {
                if let Some(csv) = child.into_inner().find(|p| p.as_rule() == Rule::ident_csv) {
                    phases = csv
                        .into_inner()
                        .filter(|p| p.as_rule() == Rule::ident)
                        .map(|p| p.as_str().to_string())
                        .collect();
                }
            }
            Rule::trait_clause => {
                trait_category = child.into_inner().next().map(|p| p.as_str().to_string())
            }
            Rule::weight_clause => {
                weight = child
                    .into_inner()
                    .next()
                    .and_then(|p| p.as_str().parse().ok())
            }
            Rule::prompt_clause => {
                prompt_injection = child.into_inner().next().map(|p| unquote(p.as_str()))
            }
            Rule::when_expr => {
                when_expr = child.into_inner().next().map(|p| p.as_str().to_string())
            }
            Rule::require_expr => {
                require_expr = child.into_inner().next().map(|p| p.as_str().to_string())
            }
            Rule::severity_clause => {
                severity = child
                    .into_inner()
                    .next()
                    .map(|p| p.as_str().to_string())
                    .unwrap_or_default()
            }
            Rule::message_clause => {
                message = child.into_inner().next().map(|p| unquote(p.as_str()))
            }
            _ => {}
        }
    }

    PxConstraint {
        name,
        scope,
        phases,
        trait_category,
        weight,
        prompt_injection,
        when_expr,
        require_expr,
        severity,
        message,
    }
}

fn build_contract(pair: Pair<'_, Rule>) -> PxContract {
    let mut inner = pair.into_inner();
    let name = next_str(&mut inner);

    let mut given = None;
    let mut when_desc = None;
    let mut then_desc = None;
    let mut threshold = None;
    let mut examples = vec![];

    let Some(contract_body) = inner.find(|p| p.as_rule() == Rule::contract_body) else {
        return PxContract {
            name,
            given,
            when_desc,
            then_desc,
            threshold,
            examples,
        };
    };

    for child in contract_body.into_inner() {
        match child.as_rule() {
            Rule::given_clause => given = child.into_inner().next().map(|p| unquote(p.as_str())),
            Rule::when_desc => when_desc = child.into_inner().next().map(|p| unquote(p.as_str())),
            Rule::then_desc => then_desc = child.into_inner().next().map(|p| unquote(p.as_str())),
            Rule::threshold_clause => {
                threshold = child
                    .into_inner()
                    .next()
                    .and_then(|p| p.as_str().parse().ok())
            }
            Rule::example_list => {
                examples = child
                    .into_inner()
                    .filter(|p| p.as_rule() == Rule::example)
                    .map(build_example)
                    .collect();
            }
            _ => {}
        }
    }

    PxContract {
        name,
        given,
        when_desc,
        then_desc,
        threshold,
        examples,
    }
}

fn build_example(pair: Pair<'_, Rule>) -> PxExample {
    let mut inner = pair.into_inner();
    let input = inner
        .next()
        .map(|p| parse_value(p))
        .unwrap_or(serde_json::Value::Null);
    let expect = inner
        .next()
        .map(|p| parse_value(p))
        .unwrap_or(serde_json::Value::Null);
    let threshold = inner.next().and_then(|p| p.as_str().parse().ok());
    PxExample {
        input,
        expect,
        threshold,
    }
}

fn build_function(pair: Pair<'_, Rule>) -> PxFunction {
    let mut inner = pair.into_inner();
    let name = next_str(&mut inner);

    let mut params = vec![];
    let mut return_type = String::new();
    let mut mode = FunctionMode::Deterministic;
    let mut docstring = String::new();

    for child in inner {
        match child.as_rule() {
            Rule::param_list => {
                params = child
                    .into_inner()
                    .filter(|p| p.as_rule() == Rule::param)
                    .map(build_field)
                    .collect();
            }
            Rule::type_expr => return_type = child.as_str().to_string(),
            Rule::function_body => {
                for body_part in child.into_inner() {
                    match body_part.as_rule() {
                        Rule::mode_clause => {
                            let mode_str = body_part
                                .into_inner()
                                .next()
                                .map(|p| p.as_str())
                                .unwrap_or("deterministic");
                            mode = match mode_str {
                                "probabilistic" => FunctionMode::Probabilistic,
                                "hybrid" => FunctionMode::Hybrid,
                                _ => FunctionMode::Deterministic,
                            };
                        }
                        Rule::docstring => {
                            docstring = body_part.as_str().trim_matches('"').to_string()
                        }
                        _ => {}
                    }
                }
            }
            Rule::docstring => docstring = child.as_str().trim_matches('"').to_string(),
            _ => {}
        }
    }

    PxFunction {
        name,
        params,
        return_type,
        mode,
        docstring,
    }
}

fn build_trigger(pair: Pair<'_, Rule>) -> PxTrigger {
    let mut inner = pair.into_inner();
    let name = next_str(&mut inner);

    let mut on_event = String::new();
    let mut schedule = None;
    let mut run = String::new();

    let Some(trigger_body) = inner.find(|p| p.as_rule() == Rule::trigger_body) else {
        return PxTrigger {
            name,
            on_event,
            schedule,
            run,
        };
    };

    for child in trigger_body.into_inner() {
        match child.as_rule() {
            Rule::on_clause => {
                on_event = child
                    .into_inner()
                    .next()
                    .map(|p| p.as_str().to_string())
                    .unwrap_or_default()
            }
            Rule::schedule_clause => {
                schedule = child.into_inner().next().map(|p| unquote(p.as_str()))
            }
            Rule::run_clause => {
                run = child
                    .into_inner()
                    .next()
                    .map(|p| p.as_str().to_string())
                    .unwrap_or_default()
            }
            _ => {}
        }
    }

    PxTrigger {
        name,
        on_event,
        schedule,
        run,
    }
}

fn build_procedure(pair: Pair<'_, Rule>) -> PxProcedure {
    let mut inner = pair.into_inner();
    let name = next_str(&mut inner);

    let mut trigger = None;
    let mut given = None;
    let mut steps = vec![];

    let Some(body) = inner.find(|p| p.as_rule() == Rule::procedure_body) else {
        return PxProcedure {
            name,
            trigger,
            given,
            steps,
        };
    };

    for child in body.into_inner() {
        match child.as_rule() {
            Rule::procedure_trigger_clause => {
                if let Some(kind_pair) = child
                    .into_inner()
                    .find(|p| p.as_rule() == Rule::procedure_trigger_kind)
                {
                    let kind_text = kind_pair.as_str().to_string();
                    let mut ki = kind_pair.into_inner();
                    // For compound triggers like `on_write {...}` or `cron {...}`,
                    // the map_val is an inner pair. For bare keywords like `manual`,
                    // inner pairs are empty.
                    let params = ki.find(|p| p.as_rule() == Rule::map_val).map(parse_value);
                    // Extract just the keyword part (before any params)
                    let kind_str = kind_text
                        .split_whitespace()
                        .next()
                        .unwrap_or(&kind_text)
                        .to_string();
                    trigger = Some(PxProcedureTrigger {
                        kind: kind_str,
                        params,
                    });
                }
            }
            Rule::given_clause => given = child.into_inner().next().map(|p| unquote(p.as_str())),
            Rule::step_list => {
                steps = child
                    .into_inner()
                    .filter(|p| p.as_rule() == Rule::step_decl)
                    .map(build_step)
                    .collect();
            }
            _ => {}
        }
    }

    PxProcedure {
        name,
        trigger,
        given,
        steps,
    }
}

fn build_step(pair: Pair<'_, Rule>) -> PxStep {
    let inner = pair.into_inner().next().unwrap();
    match inner.as_rule() {
        Rule::step_call => {
            let mut parts = inner.into_inner();
            let name = next_str(&mut parts);
            let mut params = serde_json::Value::Object(serde_json::Map::new());
            let mut output_var = None;

            for p in parts {
                match p.as_rule() {
                    Rule::map_val => params = parse_value(p),
                    Rule::param_pair => {
                        let mut kv = p.into_inner();
                        let k = next_str(&mut kv);
                        let v = kv
                            .next()
                            .map(parse_value)
                            .unwrap_or(serde_json::Value::Null);
                        if let Some(obj) = params.as_object_mut() {
                            obj.insert(k, v);
                        }
                    }
                    Rule::ident => output_var = Some(p.as_str().to_string()),
                    _ => {}
                }
            }

            PxStep::Call {
                name,
                params,
                output_var,
            }
        }
        Rule::step_match => {
            let arms = inner
                .into_inner()
                .filter(|p| p.as_rule() == Rule::match_arm_list)
                .flat_map(|mal| mal.into_inner().filter(|p| p.as_rule() == Rule::match_arm))
                .map(|arm| {
                    let mut ai = arm.into_inner();
                    let condition = ai
                        .next()
                        .map(|p| p.as_str().to_string())
                        .unwrap_or_default();
                    let result = ai
                        .next()
                        .map(|p| p.as_str().to_string())
                        .unwrap_or_default();
                    PxMatchArm { condition, result }
                })
                .collect();
            PxStep::Match { arms }
        }
        Rule::step_when => {
            let mut wi = inner.into_inner();
            let condition = wi
                .next()
                .map(|p| p.as_str().to_string())
                .unwrap_or_default();
            let steps = wi
                .find(|p| p.as_rule() == Rule::block_step_list)
                .map(|sl| {
                    sl.into_inner()
                        .filter(|p| p.as_rule() == Rule::step_decl)
                        .map(build_step)
                        .collect()
                })
                .unwrap_or_default();
            PxStep::When { condition, steps }
        }
        Rule::step_loop => {
            let li = inner.into_inner();
            let mut over = None;
            let mut times = None;
            let mut item_var = "item".to_string();
            let mut key_var = None;
            let mut output_var = None;
            let mut steps = vec![];
            let mut ident_index = 0;

            for child in li {
                match child.as_rule() {
                    Rule::loop_source => {
                        let src_text = child.as_str();
                        let mut src_inner = child.into_inner();
                        if src_text.starts_with("over") {
                            over = src_inner
                                .find(|p| p.as_rule() == Rule::ident)
                                .map(|p| format!("${}", p.as_str()));
                        } else if src_text.starts_with("times") {
                            times = src_inner
                                .find(|p| p.as_rule() == Rule::integer)
                                .and_then(|p| p.as_str().parse().ok());
                        }
                    }
                    Rule::key_as_clause => {
                        key_var = child
                            .into_inner()
                            .find(|p| p.as_rule() == Rule::ident)
                            .map(|p| p.as_str().to_string());
                    }
                    Rule::ident => {
                        if ident_index == 0 {
                            item_var = child.as_str().to_string();
                        } else {
                            output_var = Some(child.as_str().to_string());
                        }
                        ident_index += 1;
                    }
                    Rule::block_step_list => {
                        steps = child
                            .into_inner()
                            .filter(|p| p.as_rule() == Rule::step_decl)
                            .map(build_step)
                            .collect();
                    }
                    _ => {}
                }
            }

            PxStep::Loop {
                over,
                times,
                item_var,
                key_var,
                steps,
                output_var,
            }
        }
        Rule::step_emit => {
            let mut event = serde_json::Value::Object(serde_json::Map::new());
            for child in inner.into_inner() {
                match child.as_rule() {
                    Rule::map_val => event = parse_value(child),
                    Rule::param_pair => {
                        let mut kv = child.into_inner();
                        let k = next_str(&mut kv);
                        let v = kv
                            .next()
                            .map(parse_value)
                            .unwrap_or(serde_json::Value::Null);
                        if let Some(obj) = event.as_object_mut() {
                            obj.insert(k, v);
                        }
                    }
                    _ => {}
                }
            }
            PxStep::Emit { event }
        }
        Rule::step_try => {
            let mut try_steps = vec![];
            let mut catch_steps = vec![];
            let mut retry: Option<u64> = None;
            let mut retry_delay_ms: Option<u64> = None;
            let mut retry_backoff: Option<String> = None;
            let mut retry_max_delay_ms: Option<u64> = None;
            let mut retry_jitter: Option<bool> = None;

            for child in inner.into_inner() {
                match child.as_rule() {
                    Rule::try_retry_clause => {
                        for rc in child.into_inner() {
                            match rc.as_rule() {
                                Rule::integer => {
                                    retry = Some(rc.as_str().parse().unwrap_or(0));
                                }
                                Rule::branch_retry_opt => {
                                    let opt = rc.into_inner().next().unwrap();
                                    match opt.as_rule() {
                                        Rule::retry_delay_opt => {
                                            let val = opt.into_inner().next().unwrap();
                                            retry_delay_ms = Some(val.as_str().parse().unwrap_or(0));
                                        }
                                        Rule::retry_backoff_opt => {
                                            let strategy = opt.into_inner().next().unwrap();
                                            retry_backoff = Some(strategy.as_str().to_string());
                                        }
                                        Rule::retry_max_delay_opt => {
                                            let val = opt.into_inner().next().unwrap();
                                            retry_max_delay_ms = Some(val.as_str().parse().unwrap_or(0));
                                        }
                                        Rule::retry_jitter_opt => {
                                            retry_jitter = Some(true);
                                        }
                                        _ => {}
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    Rule::try_step_list => {
                        try_steps = child
                            .into_inner()
                            .filter(|p| p.as_rule() == Rule::step_decl)
                            .map(build_step)
                            .collect();
                    }
                    Rule::catch_clause => {
                        catch_steps = child
                            .into_inner()
                            .filter(|p| p.as_rule() == Rule::step_decl)
                            .map(build_step)
                            .collect();
                    }
                    _ => {}
                }
            }

            PxStep::Try {
                steps: try_steps,
                catch: catch_steps,
                retry,
                retry_delay_ms,
                retry_backoff,
                retry_max_delay_ms,
                retry_jitter,
            }
        }
        Rule::step_parallel => {
            let mut branches = vec![];
            let mut output_var = None;

            for child in inner.into_inner() {
                match child.as_rule() {
                    Rule::ident => {
                        output_var = Some(child.as_str().to_string());
                    }
                    Rule::parallel_branch_list => {
                        for branch_pair in child.into_inner() {
                            if branch_pair.as_rule() == Rule::parallel_branch {
                                let mut bi = branch_pair.into_inner();
                                let name = bi
                                    .next()
                                    .map(|p| p.as_str().to_string())
                                    .unwrap_or_default();

                                // Parse optional retry clause
                                let mut retry = None;
                                let mut retry_delay_ms = None;
                                let mut retry_backoff = None;
                                let mut retry_max_delay_ms = None;
                                let mut retry_jitter = None;
                                let mut steps = vec![];

                                for part in bi {
                                    match part.as_rule() {
                                        Rule::branch_retry_clause => {
                                            let mut rc = part.into_inner();
                                            if let Some(count_pair) = rc.next() {
                                                retry = count_pair.as_str().parse().ok();
                                            }
                                            for opt in rc {
                                                if opt.as_rule() == Rule::branch_retry_opt {
                                                    let inner_opt = opt.into_inner().next().unwrap();
                                                    match inner_opt.as_rule() {
                                                        Rule::retry_delay_opt => {
                                                            let val = inner_opt.into_inner().next().unwrap();
                                                            retry_delay_ms = val.as_str().parse().ok();
                                                        }
                                                        Rule::retry_backoff_opt => {
                                                            let val = inner_opt.into_inner().next().unwrap();
                                                            retry_backoff = Some(val.as_str().to_string());
                                                        }
                                                        Rule::retry_max_delay_opt => {
                                                            let val = inner_opt.into_inner().next().unwrap();
                                                            retry_max_delay_ms = val.as_str().parse().ok();
                                                        }
                                                        Rule::retry_jitter_opt => {
                                                            retry_jitter = Some(true);
                                                        }
                                                        _ => {}
                                                    }
                                                }
                                            }
                                        }
                                        Rule::block_step_list => {
                                            steps = part
                                                .into_inner()
                                                .filter(|p| p.as_rule() == Rule::step_decl)
                                                .map(build_step)
                                                .collect();
                                        }
                                        _ => {}
                                    }
                                }

                                branches.push(PxParallelBranch {
                                    name,
                                    steps,
                                    retry,
                                    retry_delay_ms,
                                    retry_backoff,
                                    retry_max_delay_ms,
                                    retry_jitter,
                                });
                            }
                        }
                    }
                    _ => {}
                }
            }

            PxStep::Parallel {
                branches,
                output_var,
            }
        }
        Rule::step_return => {
            let value = inner.into_inner().next().map(|p| parse_value(p));
            PxStep::Return { value }
        }
        Rule::step_abort => {
            let value = inner.into_inner().next().map(|p| parse_value(p));
            PxStep::Abort { value }
        }
        _ => PxStep::Call {
            name: "unknown".into(),
            params: serde_json::Value::Null,
            output_var: None,
        },
    }
}
fn build_scenario(pair: Pair<'_, Rule>) -> PxScenario {
    let mut inner = pair.into_inner();
    let name = next_str(&mut inner);

    let mut given = String::new();
    let mut setup = vec![];
    let mut run = None;
    let mut expectations = vec![];

    let Some(scenario_body) = inner.find(|p| p.as_rule() == Rule::scenario_body) else {
        return PxScenario { name, given, setup, run, expectations };
    };

    for child in scenario_body.into_inner() {
        match child.as_rule() {
            Rule::given_clause => {
                given = child.into_inner().next().map(|p| unquote(p.as_str())).unwrap_or_default();
            }
            Rule::setup_clause => {
                if let Some(step_list) = child.into_inner().find(|p| p.as_rule() == Rule::step_list) {
                    setup = step_list.into_inner().filter(|p| p.as_rule() == Rule::step_decl).map(build_step).collect();
                }
            }
            Rule::scenario_run_clause => {
                let mut parts = child.into_inner();
                let procedure = next_str(&mut parts);
                let params = parts.find(|p| p.as_rule() == Rule::map_val).map(parse_value);
                run = Some(PxScenarioRun { procedure, params });
            }
            Rule::expect_clause => {
                if let Some(expectation_list) = child.into_inner().find(|p| p.as_rule() == Rule::expectation_list) {
                    expectations = expectation_list.into_inner().filter(|p| p.as_rule() == Rule::expectation).map(build_expectation).collect();
                }
            }
            _ => {}
        }
    }

    PxScenario { name, given, setup, run, expectations }
}

fn build_expectation(pair: Pair<'_, Rule>) -> PxExpectation {
    let inner = pair.into_inner().next().unwrap();
    match inner.as_rule() {
        Rule::not_expectation => {
            let positive = inner.into_inner().find(|p| p.as_rule() == Rule::positive_expectation).unwrap();
            let mut parts = positive.into_inner();
            let check = next_str(&mut parts);
            let params = parts.find(|p| p.as_rule() == Rule::map_val).map(parse_value);
            PxExpectation { negated: true, check, params }
        }
        Rule::positive_expectation => {
            let mut parts = inner.into_inner();
            let check = next_str(&mut parts);
            let params = parts.find(|p| p.as_rule() == Rule::map_val).map(parse_value);
            PxExpectation { negated: false, check, params }
        }
        _ => PxExpectation { negated: false, check: "unknown".into(), params: None },
    }
}


// === Helpers ===

fn next_str(pairs: &mut Pairs<'_, Rule>) -> String {
    pairs
        .next()
        .map(|p| p.as_str().to_string())
        .unwrap_or_default()
}

fn unquote(s: &str) -> String {
    s.trim_matches('"').trim_matches('\'').to_string()
}

fn parse_value(pair: Pair<'_, Rule>) -> serde_json::Value {
    match pair.as_rule() {
        Rule::string => serde_json::Value::String(unquote(pair.as_str())),
        Rule::integer => serde_json::json!(pair.as_str().parse::<i64>().unwrap_or(0)),
        Rule::float => serde_json::json!(pair.as_str().parse::<f64>().unwrap_or(0.0)),
        Rule::boolean => serde_json::json!(pair.as_str() == "true"),
        Rule::var_ref => serde_json::Value::String(pair.as_str().to_string()),
        Rule::list_val => {
            let items: Vec<serde_json::Value> = pair.into_inner().map(parse_value).collect();
            serde_json::Value::Array(items)
        }
        Rule::map_val => {
            let mut map = serde_json::Map::new();
            let mut inner = pair.into_inner();
            while let Some(key) = inner.next() {
                if let Some(val) = inner.next() {
                    map.insert(key.as_str().to_string(), parse_value(val));
                }
            }
            serde_json::Value::Object(map)
        }
        Rule::ident => serde_json::Value::String(pair.as_str().to_string()),
        _ => {
            // Try to parse inner value
            if let Some(inner) = pair.into_inner().next() {
                parse_value(inner)
            } else {
                serde_json::Value::Null
            }
        }
    }
}
