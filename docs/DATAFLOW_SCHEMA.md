# Dataflow Procedure Schema

> Reference for the dataflow procedure type system and AST representation.

## Grammar (pest)

```pest
dataflow_procedure_decl = {
    "procedure" ~ ident ~ "(" ~ dataflow_param_list? ~ ")" ~ dataflow_return_type? ~ ":" ~ NEWLINE ~
    given_clause? ~ step_list
}

dataflow_param_list = { dataflow_param ~ ("," ~ dataflow_param)* }
dataflow_param = { ident ~ ":" ~ type_expr ~ dataflow_source_binding? }
dataflow_source_binding = { "from" ~ string_lit }

dataflow_return_type = { "->" ~ type_expr ~ dataflow_dest_binding? }
dataflow_dest_binding = { "into" ~ string_lit }

type_expr = { enum_type | list_type | optional_type | base_type | ident_type }
base_type = { "bool" | "int" | "float" | "string" | "duration" }
ident_type = { ident }
```

## AST Types (Rust)

```rust
/// A dataflow procedure: pure function with typed inputs and outputs.
pub struct PxDataflowProcedure {
    pub name: String,
    pub params: Vec<PxDataflowParam>,
    pub return_type: Option<PxDataflowReturn>,
    pub given: Option<String>,
    pub steps: Vec<PxStep>,
}

/// A typed parameter with optional source queue binding.
pub struct PxDataflowParam {
    pub name: String,
    pub type_expr: String,
    /// Source queue name. If None, defaults to param name.
    pub source: Option<String>,
}

/// Return type with optional destination queue binding.
pub struct PxDataflowReturn {
    pub type_expr: String,
    /// Destination queue name. If None, defaults to procedure name.
    pub destination: Option<String>,
}
```

## Runtime Types

```rust
/// A node in the dataflow graph — one procedure.
pub struct ProcedureNode {
    pub name: String,
    pub inputs: Vec<InputSlot>,
    pub outputs: Vec<OutputSlot>,
    pub body: Value,          // Compiled steps as JSON
    pub description: Option<String>,
}

pub struct InputSlot {
    pub name: String,         // Parameter name
    pub source: String,       // Queue to read from
    pub type_hint: Option<String>,
}

pub struct OutputSlot {
    pub name: String,         // Output name (always "result")
    pub destination: String,  // Queue to write to
    pub type_hint: Option<String>,
}

/// A value flowing through the dataflow graph.
pub struct Datum {
    pub value: Value,         // The actual data (serde_json::Value)
    pub depth: u32,           // Lineage depth (incremented each hop)
}

/// Bounded FIFO queue with depth guards.
pub struct Queue {
    items: VecDeque<Datum>,
    max_depth: u32,           // Default 25 — rejects writes above this
    max_length: usize,        // Default 1000 — drops oldest on overflow
}
```

## Conversion Pipeline

```
.px source file
  → pest parser (grammar.pest)
  → PxDataflowProcedure (builder.rs)
  → ast_to_node() (dataflow.rs)
  → ProcedureNode
  → DataflowGraph.register()
  → Ready for execution
```

## Queue Semantics

| Behavior | Description |
|---|---|
| Push | Appends datum to queue, increments depth |
| Pop | Removes oldest datum (FIFO) |
| Depth guard | Rejects push if datum.depth > max_depth |
| Backpressure | Drops oldest if queue.len > max_length |
| Multi-consumer | Each consumer pops one datum (not broadcast) |
| Empty = stopped | No data → nothing fires → natural quiescence |

## Default Bindings

When `from` or `into` are omitted:

```px
# source defaults to parameter name
procedure foo(bar: string):         # reads from queue "bar"
  ...

# destination defaults to procedure name  
procedure foo(...) -> result:       # writes to queue "foo"
  ...

# explicit bindings override
procedure foo(bar: string from "inbound") -> result into "output":
  ...
```
