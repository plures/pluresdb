//! Integration test: os.package state survives across separate `CrdtStore`
//! instances when backed by `SledStorage` at the same path — the same
//! machinery `px-shell --db <path>` now wires up. This proves the fix
//! described in memory/praxis-os-stage1-persistence-*.md is real, not
//! assumed: two independent `CrdtStore` values (simulating two separate
//! process invocations) opened against the same on-disk sled path see the
//! same `os.package` node.

use std::sync::Arc;

use pluresdb_core::CrdtStore;
use pluresdb_os_package::node::{self, DesiredState};
use pluresdb_storage::{SledStorage, StorageEngine};

#[test]
fn os_package_state_survives_across_separate_crdtstore_instances_via_sled() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("sled-db");

    // First "process": write desired_state=present.
    {
        let storage = Arc::new(SledStorage::open(&db_path).unwrap());
        let store = CrdtStore::default().with_persistence(storage as Arc<dyn StorageEngine>);
        node::set_desired_state(&store, "test-writer", "ripgrep", "nix", DesiredState::Present)
            .unwrap();
    }

    // Second "process": fresh CrdtStore, same sled path — must see the write.
    {
        let storage = Arc::new(SledStorage::open(&db_path).unwrap());
        let store = CrdtStore::default().with_persistence(storage as Arc<dyn StorageEngine>);
        let n = node::OsPackageNode::get(&store, "ripgrep")
            .expect("node must be visible from a fresh CrdtStore backed by the same sled path");
        assert_eq!(n.desired_state, DesiredState::Present);
    }
}

#[test]
fn os_package_state_does_not_leak_across_distinct_sled_paths() {
    let dir_a = tempfile::tempdir().unwrap();
    let dir_b = tempfile::tempdir().unwrap();

    {
        let storage = Arc::new(SledStorage::open(dir_a.path().join("db")).unwrap());
        let store = CrdtStore::default().with_persistence(storage as Arc<dyn StorageEngine>);
        node::set_desired_state(&store, "test-writer", "ripgrep", "nix", DesiredState::Present)
            .unwrap();
    }

    {
        let storage = Arc::new(SledStorage::open(dir_b.path().join("db")).unwrap());
        let store = CrdtStore::default().with_persistence(storage as Arc<dyn StorageEngine>);
        assert!(
            node::OsPackageNode::get(&store, "ripgrep").is_none(),
            "a distinct sled path must not see the other path's writes"
        );
    }
}
