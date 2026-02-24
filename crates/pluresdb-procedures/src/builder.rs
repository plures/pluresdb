//! Fluent builder API for constructing query pipelines.
//!
//! The builder produces an identical JSON IR to the string DSL parser.
//!
//! # Example
//!
//! ```rust
//! use pluresdb_procedures::builder::QueryBuilder;
//! use pluresdb_procedures::ir::{Predicate, SortDir};
//!
//! let steps = QueryBuilder::new()
//!     .filter(Predicate::eq("category", "decision"))
//!     .sort_desc("score")
//!     .limit(10)
//!     .project(["id", "data.text"])
//!     .to_steps();
//!
//! assert_eq!(steps.len(), 4);
//! ```

use crate::ir::{AggFn, FieldSpec, MutateOp, Predicate, SortDir, Step};

/// Fluent builder that accumulates [`Step`]s into a query pipeline.
#[derive(Debug, Default, Clone)]
pub struct QueryBuilder {
    steps: Vec<Step>,
}

impl QueryBuilder {
    /// Create an empty builder.
    pub fn new() -> Self {
        QueryBuilder::default()
    }

    /// Append a `filter` step.
    pub fn filter(mut self, predicate: Predicate) -> Self {
        self.steps.push(Step::Filter { predicate });
        self
    }

    /// Append a `sort` step with ascending direction.
    pub fn sort(mut self, by: impl Into<String>) -> Self {
        self.steps.push(Step::Sort {
            by: by.into(),
            dir: SortDir::Asc,
            after: None,
        });
        self
    }

    /// Append a `sort` step with descending direction.
    pub fn sort_desc(mut self, by: impl Into<String>) -> Self {
        self.steps.push(Step::Sort {
            by: by.into(),
            dir: SortDir::Desc,
            after: None,
        });
        self
    }

    /// Append a `sort` step with explicit direction.
    pub fn sort_with(mut self, by: impl Into<String>, dir: SortDir) -> Self {
        self.steps.push(Step::Sort {
            by: by.into(),
            dir,
            after: None,
        });
        self
    }

    /// Append a `sort` step with a cursor for pagination.
    pub fn sort_after(mut self, by: impl Into<String>, dir: SortDir, after: impl Into<String>) -> Self {
        self.steps.push(Step::Sort {
            by: by.into(),
            dir,
            after: Some(after.into()),
        });
        self
    }

    /// Append a `limit` step.
    pub fn limit(mut self, n: usize) -> Self {
        self.steps.push(Step::Limit { n });
        self
    }

    /// Append a `project` step from an iterable of field paths or `FieldSpec`s.
    pub fn project<I, S>(mut self, fields: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let specs: Vec<FieldSpec> = fields
            .into_iter()
            .map(|s| FieldSpec::Plain(s.into()))
            .collect();
        self.steps.push(Step::Project { fields: specs });
        self
    }

    /// Append a `project` step from explicit [`FieldSpec`]s.
    pub fn project_specs(mut self, specs: Vec<FieldSpec>) -> Self {
        self.steps.push(Step::Project { fields: specs });
        self
    }

    /// Append a `mutate` step.
    pub fn mutate(mut self, ops: Vec<MutateOp>) -> Self {
        self.steps.push(Step::Mutate { ops, atomic: false });
        self
    }

    /// Append an atomic `mutate` step (all-or-nothing).
    pub fn mutate_atomic(mut self, ops: Vec<MutateOp>) -> Self {
        self.steps.push(Step::Mutate { ops, atomic: true });
        self
    }

    /// Append an `aggregate` step.
    pub fn aggregate(mut self, func: AggFn, field: Option<&str>) -> Self {
        self.steps.push(Step::Aggregate {
            func,
            field: field.map(|s| s.to_string()),
        });
        self
    }

    /// Consume the builder and return the accumulated steps.
    pub fn to_steps(self) -> Vec<Step> {
        self.steps
    }

    /// Serialise the pipeline to a JSON array.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(&self.steps).unwrap_or(serde_json::Value::Array(vec![]))
    }
}

/// A builder focused on constructing mutate-only pipelines.
#[derive(Debug, Default, Clone)]
pub struct MutateBuilder {
    ops: Vec<MutateOp>,
    atomic: bool,
}

impl MutateBuilder {
    pub fn new() -> Self {
        MutateBuilder::default()
    }

    /// Mark the batch as atomic (all-or-nothing).
    pub fn atomic(mut self) -> Self {
        self.atomic = true;
        self
    }

    pub fn put(mut self, id: impl Into<String>, data: serde_json::Value) -> Self {
        self.ops.push(MutateOp::Put {
            id: id.into(),
            data,
        });
        self
    }

    pub fn delete(mut self, id: impl Into<String>) -> Self {
        self.ops.push(MutateOp::Delete { id: id.into() });
        self
    }

    pub fn merge(mut self, id: impl Into<String>, patch: serde_json::Value) -> Self {
        self.ops.push(MutateOp::Merge {
            id: id.into(),
            patch,
        });
        self
    }

    pub fn put_edge(
        mut self,
        from: impl Into<String>,
        to: impl Into<String>,
        label: Option<&str>,
    ) -> Self {
        self.ops.push(MutateOp::PutEdge {
            from: from.into(),
            to: to.into(),
            label: label.map(|s| s.to_string()),
        });
        self
    }

    pub fn delete_edge(mut self, from: impl Into<String>, to: impl Into<String>) -> Self {
        self.ops.push(MutateOp::DeleteEdge {
            from: from.into(),
            to: to.into(),
        });
        self
    }

    /// Build the `Step::Mutate` from the accumulated operations.
    pub fn to_step(self) -> Step {
        Step::Mutate {
            ops: self.ops,
            atomic: self.atomic,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::*;

    #[test]
    fn builder_produces_same_ir_as_parser() {
        use crate::parser::parse_query;

        // Build via the fluent API
        let builder_steps = QueryBuilder::new()
            .filter(Predicate::eq("category", "decision"))
            .sort_desc("updated_at")
            .limit(10)
            .to_steps();

        // Parse the equivalent DSL string
        let dsl_steps = parse_query(
            r#"filter(category == "decision") |> sort(by: "updated_at", dir: "desc") |> limit(10)"#,
        )
        .unwrap();

        assert_eq!(builder_steps, dsl_steps);
    }

    #[test]
    fn builder_to_json_is_deserializable() {
        let builder = QueryBuilder::new()
            .filter(Predicate::eq("status", "open"))
            .limit(5);
        let json = builder.to_json();
        let steps: Vec<Step> = serde_json::from_value(json).unwrap();
        assert_eq!(steps.len(), 2);
    }

    #[test]
    fn mutate_builder_basic() {
        let step = MutateBuilder::new()
            .put("id1", serde_json::json!({"v": 1}))
            .delete("id2")
            .to_step();
        if let Step::Mutate { ops, atomic } = step {
            assert_eq!(ops.len(), 2);
            assert!(!atomic);
        } else {
            panic!("expected mutate step");
        }
    }

    #[test]
    fn mutate_builder_atomic() {
        let step = MutateBuilder::new().atomic().delete("x").to_step();
        if let Step::Mutate { atomic, .. } = step {
            assert!(atomic);
        }
    }

    #[test]
    fn builder_aggregate() {
        let steps = QueryBuilder::new()
            .aggregate(AggFn::Count, None)
            .to_steps();
        assert_eq!(steps.len(), 1);
        assert!(matches!(steps[0], Step::Aggregate { func: AggFn::Count, .. }));
    }
}
