//! Plugin trait for clean architectural separation between PluresDB and
//! higher-level layers such as pluresLM.
//!
//! # Design
//!
//! The core PluresDB engine (`pluresdb-core`) is intentionally kept free of
//! any knowledge about pluresLM schemas, training pipelines, or LM-specific
//! conventions.  Instead, callers that need those capabilities implement the
//! [`PluresLmPlugin`] trait and attach an instance to a [`CrdtStore`] via
//! [`CrdtStore::with_lm_plugin`].
//!
//! ```text
//! ┌─────────────────────────────────────────────────────┐
//! │                  pluresLM layer                     │
//! │  Implements PluresLmPlugin to inject schema hooks   │
//! └────────────────────┬────────────────────────────────┘
//!                      │ Arc<dyn PluresLmPlugin>
//!                      ▼
//! ┌─────────────────────────────────────────────────────┐
//! │               pluresdb-core engine                  │
//! │  CrdtStore, VectorIndex, procedures, …              │
//! └─────────────────────────────────────────────────────┘
//! ```
//!
//! [`CrdtStore`]: crate::CrdtStore

use crate::{NodeData, NodeId};
use std::fmt;

/// Plugin interface that separates pluresLM (and other high-level layers)
/// from the PluresDB core engine.
///
/// Implement this trait to inject LM-specific behaviour into the node
/// lifecycle without creating a dependency from `pluresdb-core` on any
/// particular schema or model library.
///
/// All methods have no-op default implementations so that partial
/// implementations remain valid.
pub trait PluresLmPlugin: Send + Sync + fmt::Debug {
    /// Return a human-readable identifier for the plugin
    /// (e.g. `"pluresLM/v1"`, used in tracing and diagnostics).
    fn plugin_id(&self) -> &str;

    /// Called after a node is written to the store.
    ///
    /// The plugin receives the node's identifier and a reference to its
    /// JSON payload.  It may perform asynchronous work in a detached task
    /// (e.g. kick off training-data extraction) but must not block the
    /// caller.
    ///
    /// The default implementation does nothing.
    fn on_node_written(&self, _id: &NodeId, _data: &NodeData) {}

    /// Called before a node is deleted from the store.
    ///
    /// The default implementation does nothing.
    fn on_node_deleted(&self, _id: &NodeId) {}

    /// Validate a node payload before it is stored.
    ///
    /// Return `Ok(())` to allow the write, or an error string to reject it.
    /// The default implementation accepts every payload.
    fn validate_node(&self, _id: &NodeId, _data: &NodeData) -> Result<(), String> {
        Ok(())
    }

    /// Return an optional schema version string for the node type.
    ///
    /// PluresDB core uses this to populate a `_schema` field in stored nodes
    /// when the plugin recognises the node type.  If the plugin does not
    /// recognise the node it should return `None`.
    fn schema_version_for(&self, _id: &NodeId, _data: &NodeData) -> Option<&'static str> {
        None
    }
}

/// A no-op plugin that satisfies the [`PluresLmPlugin`] bound without adding
/// any behaviour.  This is the default when no plugin is registered.
#[derive(Debug)]
pub struct NoOpPlugin;

impl PluresLmPlugin for NoOpPlugin {
    fn plugin_id(&self) -> &str {
        "no-op"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[derive(Debug)]
    struct TestPlugin;

    impl PluresLmPlugin for TestPlugin {
        fn plugin_id(&self) -> &str {
            "test-plugin/v1"
        }

        fn on_node_written(&self, id: &NodeId, _data: &NodeData) {
            assert!(!id.is_empty());
        }

        fn validate_node(&self, _id: &NodeId, data: &NodeData) -> Result<(), String> {
            if data.get("_reject").is_some() {
                Err("rejected by test plugin".into())
            } else {
                Ok(())
            }
        }

        fn schema_version_for(
            &self,
            _id: &NodeId,
            data: &NodeData,
        ) -> Option<&'static str> {
            if data.get("_type")
                .and_then(|t| t.as_str())
                .map_or(false, |t| t.starts_with("pluresLM:"))
            {
                Some("pluresLM/v1")
            } else {
                None
            }
        }
    }

    #[test]
    fn test_plugin_id() {
        let p = TestPlugin;
        assert_eq!(p.plugin_id(), "test-plugin/v1");
    }

    #[test]
    fn test_validate_accept() {
        let p = TestPlugin;
        let data = json!({"name": "ok"});
        assert!(p.validate_node(&"id".to_string(), &data).is_ok());
    }

    #[test]
    fn test_validate_reject() {
        let p = TestPlugin;
        let data = json!({"_reject": true});
        let err = p.validate_node(&"id".to_string(), &data).unwrap_err();
        assert!(err.contains("rejected"));
    }

    #[test]
    fn test_schema_version_for_plures_lm() {
        let p = TestPlugin;
        let data = json!({"_type": "pluresLM:memory"});
        assert_eq!(
            p.schema_version_for(&"id".to_string(), &data),
            Some("pluresLM/v1")
        );
    }

    #[test]
    fn test_schema_version_for_unknown() {
        let p = TestPlugin;
        let data = json!({"_type": "other"});
        assert!(p.schema_version_for(&"id".to_string(), &data).is_none());
    }

    #[test]
    fn test_noop_plugin() {
        let p = NoOpPlugin;
        assert_eq!(p.plugin_id(), "no-op");
        assert!(p.validate_node(&"id".to_string(), &json!({})).is_ok());
        assert!(p.schema_version_for(&"id".to_string(), &json!({})).is_none());
    }

    #[test]
    fn test_default_on_node_deleted_is_noop() {
        // Should not panic.
        let p = NoOpPlugin;
        p.on_node_deleted(&"some-id".to_string());
    }
}
