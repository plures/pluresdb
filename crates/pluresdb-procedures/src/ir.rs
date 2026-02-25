//! AST / JSON-IR types for the PluresDB Query DSL.
//!
//! Every [`Step`] serialises to the wire format consumed by NAPI and the mesh
//! transport layer:
//!
//! ```json
//! [
//!   { "op": "filter", "predicate": { "field": "category", "cmp": "==", "value": "decision" } },
//!   { "op": "sort", "by": "updated_at", "dir": "desc" },
//!   { "op": "limit", "n": 10 },
//!   { "op": "project", "fields": ["id", "data.text"] }
//! ]
//! ```

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Scalar value
// ---------------------------------------------------------------------------

/// A scalar value that appears on the right-hand side of a predicate comparison
/// or inside a mutate step.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum IrValue {
    String(String),
    Number(f64),
    Bool(bool),
    Null,
}

impl IrValue {
    /// Convert the value to a `serde_json::Value` for field-level comparisons.
    pub fn to_json(&self) -> serde_json::Value {
        match self {
            IrValue::String(s) => serde_json::Value::String(s.clone()),
            IrValue::Number(n) => serde_json::json!(*n),
            IrValue::Bool(b) => serde_json::Value::Bool(*b),
            IrValue::Null => serde_json::Value::Null,
        }
    }
}

impl From<&str> for IrValue {
    fn from(s: &str) -> Self {
        IrValue::String(s.to_string())
    }
}
impl From<String> for IrValue {
    fn from(s: String) -> Self {
        IrValue::String(s)
    }
}
impl From<f64> for IrValue {
    fn from(n: f64) -> Self {
        IrValue::Number(n)
    }
}
impl From<i64> for IrValue {
    fn from(n: i64) -> Self {
        IrValue::Number(n as f64)
    }
}
impl From<bool> for IrValue {
    fn from(b: bool) -> Self {
        IrValue::Bool(b)
    }
}

// ---------------------------------------------------------------------------
// Comparison operators
// ---------------------------------------------------------------------------

/// Comparison operators supported in filter predicates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CmpOp {
    #[serde(rename = "==")]
    Eq,
    #[serde(rename = "!=")]
    Ne,
    #[serde(rename = ">")]
    Gt,
    #[serde(rename = ">=")]
    Ge,
    #[serde(rename = "<")]
    Lt,
    #[serde(rename = "<=")]
    Le,
    #[serde(rename = "contains")]
    Contains,
    #[serde(rename = "starts_with")]
    StartsWith,
    #[serde(rename = "matches")]
    Matches,
}

impl CmpOp {
    /// DSL string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            CmpOp::Eq => "==",
            CmpOp::Ne => "!=",
            CmpOp::Gt => ">",
            CmpOp::Ge => ">=",
            CmpOp::Lt => "<",
            CmpOp::Le => "<=",
            CmpOp::Contains => "contains",
            CmpOp::StartsWith => "starts_with",
            CmpOp::Matches => "matches",
        }
    }
}

// ---------------------------------------------------------------------------
// Predicate
// ---------------------------------------------------------------------------

/// A predicate used inside a `filter` step.
///
/// Serialises using untagged enum so that the JSON IR looks clean:
/// - `{ "field": "x", "cmp": "==", "value": "y" }` → `Comparison`
/// - `{ "and": [...] }` → `And`
/// - `{ "or": [...] }` → `Or`
/// - `{ "not": {...} }` → `Not`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Predicate {
    Comparison {
        field: String,
        cmp: CmpOp,
        value: IrValue,
    },
    And {
        and: Vec<Predicate>,
    },
    Or {
        or: Vec<Predicate>,
    },
    Not {
        not: Box<Predicate>,
    },
}

impl Predicate {
    /// Convenience constructor for an equality comparison.
    pub fn eq(field: impl Into<String>, value: impl Into<IrValue>) -> Self {
        Predicate::Comparison {
            field: field.into(),
            cmp: CmpOp::Eq,
            value: value.into(),
        }
    }

    /// Convenience constructor for `AND`.
    pub fn and(children: Vec<Predicate>) -> Self {
        Predicate::And { and: children }
    }

    /// Convenience constructor for `OR`.
    pub fn or(children: Vec<Predicate>) -> Self {
        Predicate::Or { or: children }
    }

    /// Convenience constructor for `NOT`.
    pub fn not(inner: Predicate) -> Self {
        Predicate::Not {
            not: Box::new(inner),
        }
    }
}

// ---------------------------------------------------------------------------
// Sort direction
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortDir {
    Asc,
    Desc,
}

impl Default for SortDir {
    fn default() -> Self {
        SortDir::Asc
    }
}

impl SortDir {
    pub fn as_str(&self) -> &'static str {
        match self {
            SortDir::Asc => "asc",
            SortDir::Desc => "desc",
        }
    }
}

// ---------------------------------------------------------------------------
// Aggregate function
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AggFn {
    Count,
    Sum,
    Avg,
    Min,
    Max,
    Distinct,
    Collect,
}

// ---------------------------------------------------------------------------
// Mutate operations
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MutateOp {
    Put {
        id: String,
        data: serde_json::Value,
    },
    Delete {
        id: String,
    },
    /// Merge `patch` into an existing node's data using a **shallow** strategy:
    /// top-level fields from `patch` overwrite the corresponding fields in the
    /// stored document.  Nested objects are replaced entirely rather than merged
    /// recursively.  If you need deep-merge semantics, read the node first,
    /// merge client-side, then use `Put`.
    Merge {
        id: String,
        patch: serde_json::Value,
    },
    PutEdge {
        from: String,
        to: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        label: Option<String>,
    },
    DeleteEdge {
        from: String,
        to: String,
    },
}

// ---------------------------------------------------------------------------
// Field specification for project
// ---------------------------------------------------------------------------

/// A field specification inside a `project` step.
///
/// A plain string like `"data.text"` means extract that field with its original
/// name.  An aliased spec uses `{ "path": "data.score", "as": "score" }`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FieldSpec {
    Plain(String),
    Aliased {
        path: String,
        #[serde(rename = "as")]
        alias: String,
    },
}

impl FieldSpec {
    pub fn path(&self) -> &str {
        match self {
            FieldSpec::Plain(s) => s.as_str(),
            FieldSpec::Aliased { path, .. } => path.as_str(),
        }
    }

    pub fn output_name(&self) -> &str {
        match self {
            FieldSpec::Plain(s) => {
                // Use the last segment of a dotted path as the output name.
                s.rsplit('.').next().unwrap_or(s.as_str())
            }
            FieldSpec::Aliased { alias, .. } => alias.as_str(),
        }
    }
}

// ---------------------------------------------------------------------------
// Query step (the JSON IR)
// ---------------------------------------------------------------------------

fn default_graph_depth() -> usize {
    1
}

/// A single step in a query pipeline.
///
/// Steps are tagged with `"op"` in JSON:
/// ```json
/// { "op": "filter", "predicate": { ... } }
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Step {
    Filter {
        predicate: Predicate,
    },
    Sort {
        by: String,
        #[serde(default)]
        dir: SortDir,
        #[serde(skip_serializing_if = "Option::is_none")]
        after: Option<String>,
    },
    Limit {
        n: usize,
    },
    Project {
        fields: Vec<FieldSpec>,
    },
    Mutate {
        ops: Vec<MutateOp>,
        #[serde(default)]
        atomic: bool,
    },
    Aggregate {
        func: AggFn,
        #[serde(skip_serializing_if = "Option::is_none")]
        field: Option<String>,
    },
    /// Traverse the graph from `root` using BFS up to `depth` hops.
    ///
    /// Returns the [`NodeRecord`]s of all reachable nodes (excluding the root).
    GraphNeighbors {
        root: String,
        #[serde(default = "default_graph_depth")]
        depth: usize,
        #[serde(skip_serializing_if = "Option::is_none")]
        min_strength: Option<f64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        link_type: Option<String>,
        #[serde(default)]
        bidirectional: bool,
    },
    /// Query existing links (edges) stored in the graph.
    ///
    /// Returns edge [`NodeRecord`]s matching all provided filters.
    GraphLinks {
        #[serde(skip_serializing_if = "Option::is_none")]
        from: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        to: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        min_strength: Option<f64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        link_type: Option<String>,
    },
    /// Automatically create links between nodes in the current set.
    ///
    /// Supported `algorithms`: `"semantic"`, `"category"`, `"temporal"`.
    /// Returns the newly created edge [`NodeRecord`]s.
    AutoLink {
        #[serde(default)]
        algorithms: Vec<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        min_strength: Option<f64>,
    },
}

// ---------------------------------------------------------------------------
// Aggregate result
// ---------------------------------------------------------------------------

/// Outcome of an `aggregate` step.
///
/// `Null` is returned by numeric aggregations (`min`, `max`, `avg`) when the
/// input set is empty or contains no values for the requested field — to
/// distinguish "no data" from a legitimate zero result (matching SQL `NULL`
/// semantics for `MIN`/`MAX`/`AVG` over empty sets).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AggResult {
    Count(u64),
    Number(f64),
    Values(Vec<serde_json::Value>),
    /// No numeric values were found (empty input or field absent on all nodes).
    Null,
}

// ---------------------------------------------------------------------------
// Procedure result
// ---------------------------------------------------------------------------

/// The result returned by `ProcedureEngine::exec`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcedureResult {
    /// Resulting nodes (empty when the last step was an aggregate or mutate).
    pub nodes: Vec<serde_json::Value>,
    /// Present when the last step was an `aggregate`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aggregate: Option<AggResult>,
    /// Number of nodes affected by the last `mutate` step.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mutated: Option<usize>,
}

impl ProcedureResult {
    pub fn from_nodes(nodes: Vec<serde_json::Value>) -> Self {
        ProcedureResult {
            nodes,
            aggregate: None,
            mutated: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_filter_roundtrip() {
        let step = Step::Filter {
            predicate: Predicate::Comparison {
                field: "category".to_string(),
                cmp: CmpOp::Eq,
                value: IrValue::String("decision".to_string()),
            },
        };
        let json = serde_json::to_string(&step).unwrap();
        let back: Step = serde_json::from_str(&json).unwrap();
        assert_eq!(step, back);
    }

    #[test]
    fn step_sort_roundtrip() {
        let step = Step::Sort {
            by: "updated_at".to_string(),
            dir: SortDir::Desc,
            after: None,
        };
        let json = serde_json::to_string(&step).unwrap();
        let back: Step = serde_json::from_str(&json).unwrap();
        assert_eq!(step, back);
    }

    #[test]
    fn step_limit_roundtrip() {
        let step = Step::Limit { n: 10 };
        let json = serde_json::to_string(&step).unwrap();
        let back: Step = serde_json::from_str(&json).unwrap();
        assert_eq!(step, back);
    }

    #[test]
    fn step_project_roundtrip() {
        let step = Step::Project {
            fields: vec![
                FieldSpec::Plain("id".to_string()),
                FieldSpec::Plain("data.text".to_string()),
            ],
        };
        let json = serde_json::to_string(&step).unwrap();
        let back: Step = serde_json::from_str(&json).unwrap();
        assert_eq!(step, back);
    }

    #[test]
    fn predicate_and_roundtrip() {
        let pred = Predicate::And {
            and: vec![
                Predicate::Comparison {
                    field: "category".to_string(),
                    cmp: CmpOp::Eq,
                    value: IrValue::String("decision".to_string()),
                },
                Predicate::Comparison {
                    field: "data.score".to_string(),
                    cmp: CmpOp::Gt,
                    value: IrValue::Number(0.7),
                },
            ],
        };
        let json = serde_json::to_string(&pred).unwrap();
        let back: Predicate = serde_json::from_str(&json).unwrap();
        assert_eq!(pred, back);
    }

    #[test]
    fn ir_value_null_roundtrip() {
        let v: IrValue = serde_json::from_str("null").unwrap();
        assert_eq!(v, IrValue::Null);
    }

    #[test]
    fn step_graph_neighbors_roundtrip() {
        let step = Step::GraphNeighbors {
            root: "memory:123".to_string(),
            depth: 2,
            min_strength: Some(0.6),
            link_type: Some("related".to_string()),
            bidirectional: true,
        };
        let json = serde_json::to_string(&step).unwrap();
        let back: Step = serde_json::from_str(&json).unwrap();
        assert_eq!(step, back);
    }

    #[test]
    fn step_graph_neighbors_defaults() {
        // Deserialise with only required fields; depth and bidirectional should use defaults.
        let json = r#"{"op":"graph_neighbors","root":"n1"}"#;
        let step: Step = serde_json::from_str(json).unwrap();
        if let Step::GraphNeighbors { depth, bidirectional, min_strength, link_type, .. } = step {
            assert_eq!(depth, 1);
            assert!(!bidirectional);
            assert!(min_strength.is_none());
            assert!(link_type.is_none());
        } else {
            panic!("expected GraphNeighbors");
        }
    }

    #[test]
    fn step_graph_links_roundtrip() {
        let step = Step::GraphLinks {
            from: Some("n1".to_string()),
            to: None,
            min_strength: Some(0.8),
            link_type: Some("semantic".to_string()),
        };
        let json = serde_json::to_string(&step).unwrap();
        let back: Step = serde_json::from_str(&json).unwrap();
        assert_eq!(step, back);
    }

    #[test]
    fn step_auto_link_roundtrip() {
        let step = Step::AutoLink {
            algorithms: vec!["semantic".to_string(), "category".to_string()],
            min_strength: Some(0.5),
        };
        let json = serde_json::to_string(&step).unwrap();
        let back: Step = serde_json::from_str(&json).unwrap();
        assert_eq!(step, back);
    }

    #[test]
    fn step_auto_link_empty_algorithms_roundtrip() {
        let step = Step::AutoLink { algorithms: vec![], min_strength: None };
        let json = serde_json::to_string(&step).unwrap();
        let back: Step = serde_json::from_str(&json).unwrap();
        assert_eq!(step, back);
    }
}
