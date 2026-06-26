//! Dataflow-driven procedure execution.
//!
//! Procedures are pure functions: typed inputs → typed output.
//! PluresDB materializes inputs into queues; procedures fire when all
//! input queues have data. No scheduler, no dispatcher, no priority
//! system — the queues ARE the executor.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────┐
//! │ DataflowGraph                                    │
//! │                                                  │
//! │  Queue("chat:history") ──┐                       │
//! │                          ├→ invoke_model() ──→ Queue("model_response")
//! │  Queue("system_prompt") ─┘                       │
//! │                                                  │
//! │  Queue("model_response") → execute_tools() ──→ Queue("model_request")
//! │                                                  │
//! │  Queue("model_request") ─→ [feeds back to invoke_model inputs]
//! └─────────────────────────────────────────────────┘
//! ```
//!
//! Key properties:
//! - Procedures are pure: no side effects, don't mutate inputs
//! - 100% concurrent: independent procedures with satisfied inputs run simultaneously
//! - No cycles in execution: same topology can loop, but each datum is distinct
//! - Termination: natural — when a procedure returns nothing, downstream queues stay empty
//! - Depth guard: queue rejects writes when lineage depth exceeds configured limit
//!
//! # Effect Boundary
//!
//! IO actions (model calls, tool dispatch, network) are NOT procedures.
//! They are "actors" that live at the boundary, triggered BY queue writes.
//! Procedures compose pure logic; actors perform side effects.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, VecDeque};

#[cfg(feature = "async")]
use std::sync::Arc;
#[cfg(feature = "async")]
use tokio::sync::{Mutex as AsyncMutex, Notify};

/// A single datum flowing through the graph. Tagged with lineage for
/// depth tracking and debugging.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Datum {
    /// The actual value.
    pub value: Value,
    /// Lineage depth: how many procedure invocations produced this datum.
    /// Starts at 0 for external inputs (user messages, events).
    pub depth: u32,
    /// Trace of which procedures produced this datum.
    pub lineage: Vec<String>,
}

impl Datum {
    /// Create a root datum (external input, depth 0).
    pub fn root(value: Value) -> Self {
        Self {
            value,
            depth: 0,
            lineage: Vec::new(),
        }
    }

    /// Derive a new datum from this one (increments depth, appends producer).
    pub fn derive(&self, value: Value, producer: &str) -> Self {
        let mut lineage = self.lineage.clone();
        lineage.push(producer.to_string());
        Self {
            value,
            depth: self.depth + 1,
            lineage,
        }
    }
}

/// A typed input slot for a procedure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputSlot {
    /// Name of this input parameter.
    pub name: String,
    /// The queue name that feeds this slot.
    pub source: String,
    /// Expected type (for validation).
    pub type_hint: Option<String>,
}

/// A typed output from a procedure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputSlot {
    /// Name of this output.
    pub name: String,
    /// The queue name this output writes to.
    pub destination: String,
    /// Type hint for downstream consumers.
    pub type_hint: Option<String>,
}

/// A procedure node in the dataflow graph.
/// Pure function: takes inputs, produces outputs. No side effects.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcedureNode {
    /// Procedure name (unique within graph).
    pub name: String,
    /// Typed input slots — procedure fires when ALL have data.
    pub inputs: Vec<InputSlot>,
    /// Typed output slots — results are written here after execution.
    pub outputs: Vec<OutputSlot>,
    /// The compiled procedure body (steps).
    pub body: Value,
    /// Optional description.
    pub description: Option<String>,
}

/// Configuration for the dataflow graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataflowConfig {
    /// Maximum lineage depth before queue rejects writes.
    pub max_depth: u32,
    /// Maximum queue length before backpressure (drops oldest).
    pub max_queue_length: usize,
}

impl Default for DataflowConfig {
    fn default() -> Self {
        Self {
            max_depth: 25,
            max_queue_length: 1000,
        }
    }
}

/// Error types for dataflow operations.
#[derive(Debug, Clone)]
pub enum DataflowError {
    DepthLimitExceeded { depth: u32, max: u32 },
    QueueFull { queue: String, length: usize, max: usize },
    ProcedureError { procedure: String, message: String },
    NoSuchQueue(String),
    ValidationError(String),
}

impl std::fmt::Display for DataflowError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DepthLimitExceeded { depth, max } => {
                write!(f, "depth limit exceeded: datum at depth {depth} exceeds max {max}")
            }
            Self::QueueFull { queue, length, max } => {
                write!(f, "queue full: {queue} has {length} items (max {max})")
            }
            Self::ProcedureError { procedure, message } => {
                write!(f, "procedure error in {procedure}: {message}")
            }
            Self::NoSuchQueue(name) => write!(f, "no such queue: {name}"),
            Self::ValidationError(msg) => write!(f, "graph validation: {msg}"),
        }
    }
}

impl std::error::Error for DataflowError {}

/// A queue in the dataflow graph.
#[derive(Debug)]
pub struct Queue {
    name: String,
    items: VecDeque<Datum>,
    max_depth: u32,
    max_length: usize,
}

impl Queue {
    pub fn new(name: String, config: &DataflowConfig) -> Self {
        Self {
            name,
            items: VecDeque::new(),
            max_depth: config.max_depth,
            max_length: config.max_queue_length,
        }
    }

    /// Push a datum. Rejects if depth exceeds limit.
    pub fn push(&mut self, datum: Datum) -> Result<(), DataflowError> {
        if datum.depth > self.max_depth {
            return Err(DataflowError::DepthLimitExceeded {
                depth: datum.depth,
                max: self.max_depth,
            });
        }
        if self.items.len() >= self.max_length {
            self.items.pop_front(); // backpressure: drop oldest
        }
        self.items.push_back(datum);
        Ok(())
    }

    pub fn pop(&mut self) -> Option<Datum> {
        self.items.pop_front()
    }

    pub fn has_data(&self) -> bool {
        !self.items.is_empty()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

/// The dataflow graph: procedures connected by queues.
/// The queues ARE the executor.
pub struct DataflowGraph {
    procedures: HashMap<String, ProcedureNode>,
    queues: HashMap<String, Queue>,
    /// Reverse index: queue → procedures that consume from it.
    consumers: HashMap<String, Vec<String>>,
    config: DataflowConfig,
}

impl DataflowGraph {
    pub fn new() -> Self {
        Self::with_config(DataflowConfig::default())
    }

    pub fn with_config(config: DataflowConfig) -> Self {
        Self {
            procedures: HashMap::new(),
            queues: HashMap::new(),
            consumers: HashMap::new(),
            config,
        }
    }

    /// Register a procedure node. Creates queues for all inputs/outputs.
    pub fn register(&mut self, node: ProcedureNode) -> Result<(), DataflowError> {
        if self.procedures.contains_key(&node.name) {
            return Err(DataflowError::ValidationError(format!(
                "duplicate procedure: {}",
                node.name
            )));
        }

        for input in &node.inputs {
            self.queues
                .entry(input.source.clone())
                .or_insert_with(|| Queue::new(input.source.clone(), &self.config));
            self.consumers
                .entry(input.source.clone())
                .or_default()
                .push(node.name.clone());
        }

        for output in &node.outputs {
            self.queues
                .entry(output.destination.clone())
                .or_insert_with(|| Queue::new(output.destination.clone(), &self.config));
        }

        self.procedures.insert(node.name.clone(), node);
        Ok(())
    }

    /// Push external data into a named queue.
    pub fn push(&mut self, queue_name: &str, datum: Datum) -> Result<(), DataflowError> {
        let queue = self
            .queues
            .get_mut(queue_name)
            .ok_or_else(|| DataflowError::NoSuchQueue(queue_name.to_string()))?;
        queue.push(datum)
    }

    /// All procedures whose input queues ALL have data.
    pub fn ready_procedures(&self) -> Vec<&str> {
        self.procedures
            .iter()
            .filter(|(_, node)| {
                node.inputs
                    .iter()
                    .all(|input| self.queues.get(&input.source).is_some_and(|q| q.has_data()))
            })
            .map(|(name, _)| name.as_str())
            .collect()
    }

    /// Pop inputs for a procedure (consumes from queues).
    pub fn pop_inputs(&mut self, procedure_name: &str) -> Option<HashMap<String, Datum>> {
        let node = self.procedures.get(procedure_name)?;

        let all_ready = node
            .inputs
            .iter()
            .all(|input| self.queues.get(&input.source).is_some_and(|q| q.has_data()));

        if !all_ready {
            return None;
        }

        let mut inputs = HashMap::new();
        for input in &node.inputs {
            let queue = self.queues.get_mut(&input.source)?;
            let datum = queue.pop()?;
            inputs.insert(input.name.clone(), datum);
        }

        Some(inputs)
    }

    /// Push procedure outputs to destination queues.
    pub fn push_outputs(
        &mut self,
        procedure_name: &str,
        outputs: HashMap<String, Value>,
        input_datum: &Datum,
    ) -> Result<(), DataflowError> {
        let node = self
            .procedures
            .get(procedure_name)
            .ok_or_else(|| DataflowError::NoSuchQueue(procedure_name.to_string()))?
            .clone();

        for output_slot in &node.outputs {
            if let Some(value) = outputs.get(&output_slot.name) {
                if !value.is_null() {
                    let derived = input_datum.derive(value.clone(), procedure_name);
                    self.push(&output_slot.destination, derived)?;
                }
            }
            // null or missing output → nothing pushed → downstream stays empty → natural stop
        }

        Ok(())
    }

    /// Run one step: find ready procedures, execute them, push outputs.
    /// Returns number of procedures fired.
    pub fn step(
        &mut self,
        handler: &dyn super::executor::ActionHandler,
    ) -> Result<usize, DataflowError> {
        let ready: Vec<String> = self
            .ready_procedures()
            .iter()
            .map(|s| s.to_string())
            .collect();

        if ready.is_empty() {
            return Ok(0);
        }

        let mut fired = 0;

        for proc_name in ready {
            let inputs = match self.pop_inputs(&proc_name) {
                Some(inputs) => inputs,
                None => continue,
            };

            let mut vars: HashMap<String, Value> = HashMap::new();
            let mut max_depth_datum = Datum::root(Value::Null);

            for (name, datum) in &inputs {
                vars.insert(name.clone(), datum.value.clone());
                if datum.depth >= max_depth_datum.depth {
                    max_depth_datum = datum.clone();
                }
            }

            let node = self.procedures.get(&proc_name).unwrap().clone();
            let result =
                super::executor::execute_with_vars(&node.body, handler, vars).map_err(|e| {
                    DataflowError::ProcedureError {
                        procedure: proc_name.clone(),
                        message: e.to_string(),
                    }
                })?;

            if result.success {
                let mut outputs = HashMap::new();

                if let Some(ret) = result.variables.get("__return__") {
                    if node.outputs.len() == 1 {
                        outputs.insert(node.outputs[0].name.clone(), ret.clone());
                    }
                } else {
                    for output_slot in &node.outputs {
                        if let Some(val) = result.variables.get(&output_slot.name) {
                            outputs.insert(output_slot.name.clone(), val.clone());
                        }
                    }
                }

                self.push_outputs(&proc_name, outputs, &max_depth_datum).unwrap_or_else(|e| {
                    // Depth limit exceeded is not an error — it's natural termination.
                    // The queue refuses the write, propagation stops.
                    match &e {
                        DataflowError::DepthLimitExceeded { .. } => {},
                        _ => {
                            // Log but don't propagate — the procedure itself succeeded.
                            #[cfg(not(test))]
                            tracing::warn!(procedure = proc_name.as_str(), error = %e, "output push failed");
                        }
                    }
                });
            }

            fired += 1;
        }

        Ok(fired)
    }

    /// Run the graph to quiescence.
    pub fn run_to_completion(
        &mut self,
        handler: &dyn super::executor::ActionHandler,
    ) -> Result<usize, DataflowError> {
        let mut total = 0;
        loop {
            let fired = self.step(handler)?;
            if fired == 0 {
                break;
            }
            total += fired;
        }
        Ok(total)
    }

    pub fn queue_names(&self) -> Vec<&str> {
        self.queues.keys().map(|s| s.as_str()).collect()
    }

    pub fn procedure_names(&self) -> Vec<&str> {
        self.procedures.keys().map(|s| s.as_str()).collect()
    }

    pub fn queue_stats(&self) -> HashMap<&str, usize> {
        self.queues.iter().map(|(k, q)| (k.as_str(), q.len())).collect()
    }
}

impl Default for DataflowGraph {
    fn default() -> Self {
        Self::new()
    }
}

/// Async version: ready procedures fire as concurrent tasks.
#[cfg(feature = "async")]
pub struct AsyncDataflowGraph {
    inner: Arc<AsyncMutex<DataflowGraph>>,
    notify: Arc<Notify>,
}

#[cfg(feature = "async")]
impl AsyncDataflowGraph {
    pub fn new() -> Self {
        Self::with_config(DataflowConfig::default())
    }

    pub fn with_config(config: DataflowConfig) -> Self {
        Self {
            inner: Arc::new(AsyncMutex::new(DataflowGraph::with_config(config))),
            notify: Arc::new(Notify::new()),
        }
    }

    pub async fn register(&self, node: ProcedureNode) -> Result<(), DataflowError> {
        let mut graph = self.inner.lock().await;
        graph.register(node)
    }

    pub async fn push(&self, queue_name: &str, datum: Datum) -> Result<(), DataflowError> {
        {
            let mut graph = self.inner.lock().await;
            graph.push(queue_name, datum)?;
        }
        self.notify.notify_one();
        Ok(())
    }

    /// Run to quiescence. Ready procedures execute concurrently via tokio::spawn.
    pub async fn run_to_completion(
        &self,
        handler: Arc<dyn super::async_executor::AsyncActionHandler>,
    ) -> Result<usize, DataflowError> {
        let mut total = 0;

        loop {
            let ready_work: Vec<(String, HashMap<String, Datum>, ProcedureNode)> = {
                let mut graph = self.inner.lock().await;
                let ready_names: Vec<String> = graph
                    .ready_procedures()
                    .iter()
                    .map(|s| s.to_string())
                    .collect();

                let mut work = Vec::new();
                for name in ready_names {
                    if let Some(inputs) = graph.pop_inputs(&name) {
                        let node = graph.procedures.get(&name).unwrap().clone();
                        work.push((name, inputs, node));
                    }
                }
                work
            };

            if ready_work.is_empty() {
                break;
            }

            // Fire all ready procedures concurrently
            let mut handles = Vec::new();
            for (proc_name, inputs, node) in ready_work {
                let handler = handler.clone();
                let handle = tokio::spawn(async move {
                    let mut vars: HashMap<String, Value> = HashMap::new();
                    let mut max_depth_datum = Datum::root(Value::Null);

                    for (name, datum) in &inputs {
                        vars.insert(name.clone(), datum.value.clone());
                        if datum.depth >= max_depth_datum.depth {
                            max_depth_datum = datum.clone();
                        }
                    }

                    let result = super::async_executor::execute_async_with_vars(
                        &node.body,
                        handler.as_ref(),
                        vars,
                    )
                    .await;

                    (proc_name, node, result, max_depth_datum)
                });
                handles.push(handle);
            }

            for handle in handles {
                let (proc_name, node, result, max_depth_datum) =
                    handle.await.map_err(|e| DataflowError::ProcedureError {
                        procedure: "join".to_string(),
                        message: e.to_string(),
                    })?;

                let result = result.map_err(|e| DataflowError::ProcedureError {
                    procedure: proc_name.clone(),
                    message: e.to_string(),
                })?;

                if result.success {
                    let mut outputs = HashMap::new();
                    if let Some(ret) = result.variables.get("__return__") {
                        if node.outputs.len() == 1 {
                            outputs.insert(node.outputs[0].name.clone(), ret.clone());
                        }
                    } else {
                        for output_slot in &node.outputs {
                            if let Some(val) = result.variables.get(&output_slot.name) {
                                outputs.insert(output_slot.name.clone(), val.clone());
                            }
                        }
                    }

                    let mut graph = self.inner.lock().await;
                    graph.push_outputs(&proc_name, outputs, &max_depth_datum)?;
                }

                total += 1;
            }
        }

        Ok(total)
    }

    /// Pop a datum from a named queue (for reading output after quiescence).
    pub async fn pop(&self, queue_name: &str) -> Option<Datum> {
        let mut graph = self.inner.lock().await;
        graph.queues.get_mut(queue_name).and_then(|q| q.pop())
    }

    /// Check if a named queue has data available.
    pub async fn has_output(&self, queue_name: &str) -> bool {
        let graph = self.inner.lock().await;
        graph
            .queues
            .get(queue_name)
            .map(|q| q.has_data())
            .unwrap_or(false)
    }
}

#[cfg(feature = "async")]
impl Default for AsyncDataflowGraph {
    fn default() -> Self {
        Self::new()
    }
}

// === Signature types for typed procedure syntax ===

/// Parsed from: `procedure name(arg1: type, arg2: type) -> output_type:`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcedureSignature {
    pub name: String,
    pub inputs: Vec<SignatureParam>,
    pub output: Option<SignatureOutput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignatureParam {
    pub name: String,
    pub type_expr: String,
    /// Source queue binding. If None, inferred from param name.
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignatureOutput {
    pub type_expr: String,
    /// Destination queue. If None, inferred from procedure name.
    pub destination: Option<String>,
}

/// Convert a signature + compiled body into a ProcedureNode.
pub fn signature_to_node(sig: &ProcedureSignature, body: Value) -> ProcedureNode {
    let inputs = sig
        .inputs
        .iter()
        .map(|p| InputSlot {
            name: p.name.clone(),
            source: p.source.clone().unwrap_or_else(|| p.name.clone()),
            type_hint: Some(p.type_expr.clone()),
        })
        .collect();

    let outputs = match &sig.output {
        Some(out) => vec![OutputSlot {
            name: "result".to_string(),
            destination: out.destination.clone().unwrap_or_else(|| sig.name.clone()),
            type_hint: Some(out.type_expr.clone()),
        }],
        None => Vec::new(),
    };

    ProcedureNode {
        name: sig.name.clone(),
        inputs,
        outputs,
        body,
        description: None,
    }
}

/// Convert a parsed PxDataflowProcedure AST node into a runtime ProcedureNode.
///
/// This bridges the parser output to the dataflow graph:
/// 1. Parse .px file → PxDataflowProcedure (via builder)
/// 2. This function → ProcedureNode (for DataflowGraph)
/// 3. Register into DataflowGraph → ready for execution
pub fn ast_to_node(proc: &super::PxDataflowProcedure) -> ProcedureNode {
    let inputs = proc
        .params
        .iter()
        .map(|p| InputSlot {
            name: p.name.clone(),
            source: p.source.clone().unwrap_or_else(|| p.name.clone()),
            type_hint: Some(p.type_expr.clone()),
        })
        .collect();

    let outputs = match &proc.return_type {
        Some(ret) => vec![OutputSlot {
            name: "result".to_string(),
            destination: ret.destination.clone().unwrap_or_else(|| proc.name.clone()),
            type_hint: Some(ret.type_expr.clone()),
        }],
        None => Vec::new(),
    };

    // Compile steps to the JSON format the executor expects.
    // Each PxStep becomes a JSON step object.
    let steps: Vec<Value> = proc
        .steps
        .iter()
        .map(super::compiler::compile_step)
        .collect();

    let body = serde_json::json!({
        "name": proc.name,
        "steps": steps
    });

    ProcedureNode {
        name: proc.name.clone(),
        inputs,
        outputs,
        body,
        description: proc.given.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::px::executor::{ActionHandler, ExecutionError};
    use serde_json::json;

    struct EchoHandler;
    impl ActionHandler for EchoHandler {
        fn call(&self, name: &str, params: &Value) -> Result<Value, ExecutionError> {
            match name {
                "model_complete" => Ok(json!({
                    "content": "Hello! I can help with that.",
                    "tool_calls": []
                })),
                "classify" => Ok(json!({ "category": "question", "confidence": 0.9 })),
                "format_response" => {
                    let content = params.get("content").and_then(|v| v.as_str()).unwrap_or("");
                    Ok(json!({ "formatted": content, "channel": "telegram" }))
                }
                _ => Ok(params.clone()),
            }
        }
    }

    #[test]
    fn empty_graph_is_quiescent() {
        let mut graph = DataflowGraph::new();
        let fired = graph.run_to_completion(&EchoHandler).unwrap();
        assert_eq!(fired, 0);
    }

    #[test]
    fn single_procedure_fires_when_input_ready() {
        let mut graph = DataflowGraph::new();

        graph
            .register(ProcedureNode {
                name: "classify".to_string(),
                inputs: vec![InputSlot {
                    name: "message".to_string(),
                    source: "inbound".to_string(),
                    type_hint: Some("string".to_string()),
                }],
                outputs: vec![OutputSlot {
                    name: "result".to_string(),
                    destination: "classification".to_string(),
                    type_hint: Some("string".to_string()),
                }],
                body: json!({
                    "name": "classify",
                    "steps": [
                        { "kind": "call", "name": "classify", "params": {"message": "$message"}, "output_var": "result" }
                    ]
                }),
                description: None,
            })
            .unwrap();

        assert!(graph.ready_procedures().is_empty());

        graph.push("inbound", Datum::root(json!("What is Rust?"))).unwrap();
        assert_eq!(graph.ready_procedures(), vec!["classify"]);

        let fired = graph.run_to_completion(&EchoHandler).unwrap();
        assert_eq!(fired, 1);

        let datum = graph.queues.get("classification").unwrap().items.front().unwrap();
        assert_eq!(datum.depth, 1);
        assert_eq!(datum.lineage, vec!["classify"]);
    }

    #[test]
    fn chain_propagates_through_queues() {
        let mut graph = DataflowGraph::new();

        // classify: inbound → classification
        graph
            .register(ProcedureNode {
                name: "classify".to_string(),
                inputs: vec![InputSlot {
                    name: "message".to_string(),
                    source: "inbound".to_string(),
                    type_hint: None,
                }],
                outputs: vec![OutputSlot {
                    name: "result".to_string(),
                    destination: "classification".to_string(),
                    type_hint: None,
                }],
                body: json!({
                    "name": "classify",
                    "steps": [{ "kind": "call", "name": "classify", "params": {}, "output_var": "result" }]
                }),
                description: None,
            })
            .unwrap();

        // invoke_model: classification + chat_history → model_response
        graph
            .register(ProcedureNode {
                name: "invoke_model".to_string(),
                inputs: vec![
                    InputSlot { name: "classification".to_string(), source: "classification".to_string(), type_hint: None },
                    InputSlot { name: "history".to_string(), source: "chat_history".to_string(), type_hint: None },
                ],
                outputs: vec![OutputSlot {
                    name: "result".to_string(),
                    destination: "model_response".to_string(),
                    type_hint: None,
                }],
                body: json!({
                    "name": "invoke_model",
                    "steps": [{ "kind": "call", "name": "model_complete", "params": {}, "output_var": "result" }]
                }),
                description: None,
            })
            .unwrap();

        // Push inbound — classify fires, invoke_model doesn't (no history)
        graph.push("inbound", Datum::root(json!("Hello"))).unwrap();
        let fired = graph.step(&EchoHandler).unwrap();
        assert_eq!(fired, 1);

        // Push history — now invoke_model fires
        graph.push("chat_history", Datum::root(json!([]))).unwrap();
        let fired = graph.step(&EchoHandler).unwrap();
        assert_eq!(fired, 1);

        let resp = graph.queues.get("model_response").unwrap().items.front().unwrap();
        assert_eq!(resp.depth, 2);
        assert_eq!(resp.lineage, vec!["classify", "invoke_model"]);
    }

    #[test]
    fn depth_limit_stops_propagation() {
        let config = DataflowConfig {
            max_depth: 3,
            max_queue_length: 100,
        };
        let mut graph = DataflowGraph::with_config(config);

        // A self-feeding loop: proc reads from "loop_q", writes back to "loop_q"
        graph
            .register(ProcedureNode {
                name: "looper".to_string(),
                inputs: vec![InputSlot {
                    name: "input".to_string(),
                    source: "loop_q".to_string(),
                    type_hint: None,
                }],
                outputs: vec![OutputSlot {
                    name: "result".to_string(),
                    destination: "loop_q".to_string(),
                    type_hint: None,
                }],
                body: json!({
                    "name": "looper",
                    "steps": [{ "kind": "call", "name": "echo", "params": {}, "output_var": "result" }]
                }),
                description: None,
            })
            .unwrap();

        // Seed the loop
        graph.push("loop_q", Datum::root(json!("start"))).unwrap();

        // Run — should fire until depth guard rejects the output write.
        // Fires at depth 0→1, 1→2, 2→3, 3→4(rejected). Procedure fires 4 times total
        // (the 4th execution succeeds but its output push is rejected).
        let fired = graph.run_to_completion(&EchoHandler).unwrap();
        assert_eq!(fired, 4);
        // Queue should be empty (last write was rejected)
        assert!(graph.queues.get("loop_q").unwrap().is_empty());
    }

    #[test]
    fn null_output_stops_propagation_naturally() {
        let mut graph = DataflowGraph::new();

        struct NullHandler;
        impl ActionHandler for NullHandler {
            fn call(&self, _name: &str, _params: &Value) -> Result<Value, ExecutionError> {
                Ok(Value::Null) // returns null → nothing downstream
            }
        }

        graph
            .register(ProcedureNode {
                name: "maybe_produce".to_string(),
                inputs: vec![InputSlot {
                    name: "input".to_string(),
                    source: "in_q".to_string(),
                    type_hint: None,
                }],
                outputs: vec![OutputSlot {
                    name: "result".to_string(),
                    destination: "out_q".to_string(),
                    type_hint: None,
                }],
                body: json!({
                    "name": "maybe_produce",
                    "steps": [{ "kind": "call", "name": "noop", "params": {}, "output_var": "result" }]
                }),
                description: None,
            })
            .unwrap();

        graph
            .register(ProcedureNode {
                name: "downstream".to_string(),
                inputs: vec![InputSlot {
                    name: "data".to_string(),
                    source: "out_q".to_string(),
                    type_hint: None,
                }],
                outputs: vec![],
                body: json!({
                    "name": "downstream",
                    "steps": [{ "kind": "call", "name": "should_not_fire", "params": {}, "output_var": "x" }]
                }),
                description: None,
            })
            .unwrap();

        graph.push("in_q", Datum::root(json!("trigger"))).unwrap();
        let fired = graph.run_to_completion(&NullHandler).unwrap();
        // Only maybe_produce fires; downstream never fires (null output)
        assert_eq!(fired, 1);
        assert!(graph.queues.get("out_q").unwrap().is_empty());
    }

    #[test]
    fn multiple_consumers_same_queue_each_get_one_datum() {
        let mut graph = DataflowGraph::new();

        // Two procedures consuming from same queue — each needs its own datum
        graph
            .register(ProcedureNode {
                name: "consumer_a".to_string(),
                inputs: vec![InputSlot {
                    name: "msg".to_string(),
                    source: "shared_q".to_string(),
                    type_hint: None,
                }],
                outputs: vec![OutputSlot {
                    name: "result".to_string(),
                    destination: "out_a".to_string(),
                    type_hint: None,
                }],
                body: json!({
                    "name": "consumer_a",
                    "steps": [{ "kind": "call", "name": "classify", "params": {}, "output_var": "result" }]
                }),
                description: None,
            })
            .unwrap();

        graph
            .register(ProcedureNode {
                name: "consumer_b".to_string(),
                inputs: vec![InputSlot {
                    name: "msg".to_string(),
                    source: "shared_q".to_string(),
                    type_hint: None,
                }],
                outputs: vec![OutputSlot {
                    name: "result".to_string(),
                    destination: "out_b".to_string(),
                    type_hint: None,
                }],
                body: json!({
                    "name": "consumer_b",
                    "steps": [{ "kind": "call", "name": "classify", "params": {}, "output_var": "result" }]
                }),
                description: None,
            })
            .unwrap();

        // Push two datums — one for each consumer
        graph.push("shared_q", Datum::root(json!("msg1"))).unwrap();
        graph.push("shared_q", Datum::root(json!("msg2"))).unwrap();

        // Both procedures are ready (queue has 2 items, each pops one)
        let fired = graph.run_to_completion(&EchoHandler).unwrap();
        assert_eq!(fired, 2);

        // Each consumer got one
        assert!(graph.queues.get("out_a").unwrap().has_data());
        assert!(graph.queues.get("out_b").unwrap().has_data());
    }

    #[test]
    fn end_to_end_parse_to_graph() {
        // Parse a dataflow .px source, convert to ProcedureNode, register, run
        let source = r#"
procedure classify_message(message: string from "inbound") -> classification into "classification":
  given: "Classify a message"
  classify {text: $message} -> $result
  return $result
"#;
        let doc = crate::px::parse(source).expect("parse failed");
        assert_eq!(doc.dataflow_procedures.len(), 1);

        // Convert AST to graph node
        let node = super::ast_to_node(&doc.dataflow_procedures[0]);
        assert_eq!(node.name, "classify_message");
        assert_eq!(node.inputs.len(), 1);
        assert_eq!(node.inputs[0].source, "inbound");
        assert_eq!(node.outputs.len(), 1);
        assert_eq!(node.outputs[0].destination, "classification");

        // Register and run
        let mut graph = DataflowGraph::new();
        graph.register(node).unwrap();
        graph.push("inbound", Datum::root(json!("Hello world"))).unwrap();

        let fired = graph.run_to_completion(&EchoHandler).unwrap();
        assert_eq!(fired, 1);

        // Output should be in classification queue
        assert!(graph.queues.get("classification").unwrap().has_data());
    }
}
