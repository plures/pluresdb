# ADR-0015: Queue-Driven Dataflow Procedures

**Status:** Accepted  
**Date:** 2026-06-09  
**Author:** kbristol + mswork  

## Context

The existing procedure system uses an event-driven model:
- Procedures declare `trigger: on_write {pattern: "..."}` 
- Multiple procedures fire on the same event
- Execution is sequential by priority order
- Dependencies between co-triggered procedures are invisible (read/write same PluresDB keys)
- The `ProcedureRegistry` + `Executor::dispatch()` loop reimplements what a queue already does

This creates correctness bugs (procedure A reads state that procedure B hasn't written yet),
prevents parallelism (everything is sequential even when independent), and requires manual
priority ordering to paper over implicit dependencies.

## Decision

**Procedures are pure functions. Queues are the executor.**

A procedure declares its inputs as typed parameters and its output as a return type.
PluresDB creates a queue for each input slot. When ALL input queues have data, the 
procedure fires. The output goes to a destination queue, which may feed other procedures.

No scheduler. No dispatcher. No priority system. No trigger keyword.

### Key Principles

1. **Procedures are pure** — no side effects, don't mutate inputs, return value is sole output
2. **Queues ARE the executor** — data arrives → function runs → output feeds downstream queues
3. **Termination is natural** — procedure returns nothing → queue stays empty → propagation stops
4. **Effect boundary is separate** — IO (model calls, tool dispatch, network) are "actors" triggered BY queue writes, not procedures
5. **Depth guard replaces iteration limits** — queue rejects writes when lineage depth exceeds config (default 25)

### Syntax

```px
# OLD (event-driven, hidden dependencies)
procedure invoke_model:
  trigger: on_write {pattern: "model_request:*"}
  pluresdb_read {key: "chat:history"} -> $history
  model_complete {messages: $history} -> $response
  pluresdb_write {key: "model_response", value: $response}

# NEW (dataflow, explicit dependencies, pure)
procedure invoke_model(history: list[message], prompt: string) -> model_response:
  model_complete {messages: $history, system_prompt: $prompt} -> $response
  return $response
```

### Execution Model

```
Queue has items → procedure fires → output goes to downstream queues → repeat
Queue is empty → nothing happens (natural termination)
Depth > limit → queue rejects write (safety termination)
```

Topology can be cyclic (e.g., tool execution loop) because each datum is distinct.
There are no "cycles" — just streams that drain when data exhausts.

### Multi-Consumer Semantics

When two procedures consume from the same queue, each gets one datum from the queue
(FIFO pop). To broadcast one datum to multiple consumers, the upstream procedure
must output to multiple destination queues, or a "fan-out" node copies one datum
to N queues.

## Consequences

### Positive
- Automatic maximum concurrency (independent procedures fire simultaneously)
- Dependencies are explicit in signatures (no invisible ordering bugs)
- No scheduler/dispatcher code (~500 lines eliminated)
- Pure functions are trivially testable (value in → value out)
- Depth guard is system-level (procedures don't know about limits)
- Natural termination (no explicit "stop" needed)

### Negative
- Breaking change to all existing `.px` procedure files
- Need type system for parameter validation
- Need binding declarations (which queue feeds which parameter)
- Effect boundary needs clear syntactic distinction

### Migration Path
1. Keep `trigger:` as backwards-compat sugar (compiles to single-input dataflow node)
2. New procedures use typed signatures
3. Incrementally migrate existing `.px` files to new syntax
4. Remove `trigger:` support once migration complete

## Implementation

- `pluresdb-px/src/px/dataflow.rs` — DataflowGraph, AsyncDataflowGraph, Datum, Queue, ProcedureNode
- Grammar extension: `dataflow_procedure_decl` rule
- 6 tests proving: quiescence, single-fire, chaining, depth limit, null termination, multi-consumer
