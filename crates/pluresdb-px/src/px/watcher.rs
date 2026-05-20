//! Hot-reload watcher for `.px` files.
//!
//! Watches a directory tree for `.px` file changes and emits compiled
//! records (or removal events) via a tokio mpsc channel. Consumers
//! can use these events to hot-swap rules, constraints, and procedures
//! without restarting the agent runtime.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use super::compiler::{compile, CompiledRecord};
use super::parse;

/// Events emitted by the PxWatcher.
#[derive(Debug, Clone)]
pub enum PxWatchEvent {
    /// A `.px` file was loaded or reloaded — here are its compiled records.
    Loaded {
        path: PathBuf,
        records: Vec<CompiledRecord>,
    },
    /// A `.px` file was removed — these keys should be evicted.
    Removed { path: PathBuf, keys: Vec<String> },
    /// A `.px` file had a parse/compile error.
    Error { path: PathBuf, error: String },
    /// Initial scan complete (all existing `.px` files loaded).
    Ready {
        file_count: usize,
        record_count: usize,
    },
}

/// Tracks which keys each `.px` file produced so we can emit
/// accurate removal events.
type KeyIndex = Arc<Mutex<HashMap<PathBuf, Vec<String>>>>;

/// Configuration for the PxWatcher.
#[derive(Debug, Clone)]
pub struct PxWatcherConfig {
    /// Root directory to watch for `.px` files.
    pub watch_path: PathBuf,
    /// Whether to perform an initial scan on start.
    pub initial_scan: bool,
    /// Debounce duration in milliseconds (to coalesce rapid saves).
    pub debounce_ms: u64,
}

impl Default for PxWatcherConfig {
    fn default() -> Self {
        Self {
            watch_path: PathBuf::from("."),
            initial_scan: true,
            debounce_ms: 100,
        }
    }
}

/// The PxWatcher — watches a directory for `.px` file changes and
/// emits compiled records for hot-reload.
pub struct PxWatcher {
    config: PxWatcherConfig,
}

impl PxWatcher {
    pub fn new(config: PxWatcherConfig) -> Self {
        Self { config }
    }

    /// Start watching. Returns a receiver for watch events.
    ///
    /// This spawns a background task that:
    /// 1. Optionally performs an initial scan of existing `.px` files
    /// 2. Sets up filesystem notifications for create/modify/delete
    /// 3. Parses and compiles changed files, emitting events
    pub async fn start(&self) -> Result<mpsc::Receiver<PxWatchEvent>, std::io::Error> {
        let (tx, rx) = mpsc::channel(256);
        let config = self.config.clone();
        let key_index: KeyIndex = Arc::new(Mutex::new(HashMap::new()));

        // Initial scan
        if config.initial_scan {
            let mut file_count = 0;
            let mut record_count = 0;

            if config.watch_path.exists() {
                for entry in walkdir::WalkDir::new(&config.watch_path)
                    .follow_links(false)
                    .into_iter()
                    .filter_map(|e| e.ok())
                {
                    let path = entry.path().to_path_buf();
                    if is_px_file(&path) {
                        match load_and_compile(&path) {
                            Ok(records) => {
                                let keys: Vec<String> =
                                    records.iter().map(|r| r.key.clone()).collect();
                                record_count += records.len();
                                file_count += 1;

                                key_index.lock().unwrap().insert(path.clone(), keys);

                                let _ = tx.send(PxWatchEvent::Loaded { path, records }).await;
                            }
                            Err(e) => {
                                let _ = tx.send(PxWatchEvent::Error { path, error: e }).await;
                            }
                        }
                    }
                }
            }

            let _ = tx
                .send(PxWatchEvent::Ready {
                    file_count,
                    record_count,
                })
                .await;
        }

        // Spawn the filesystem watcher
        let key_index_clone = key_index.clone();
        let debounce_ms = config.debounce_ms;

        tokio::spawn(async move {
            use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

            let (notify_tx, mut notify_rx) = tokio::sync::mpsc::channel::<Event>(256);

            let mut watcher = match RecommendedWatcher::new(
                move |res: Result<Event, notify::Error>| {
                    if let Ok(event) = res {
                        let _ = notify_tx.blocking_send(event);
                    }
                },
                notify::Config::default(),
            ) {
                Ok(w) => w,
                Err(e) => {
                    warn!("failed to create .px watcher: {}", e);
                    return;
                }
            };

            if let Err(e) = watcher.watch(&config.watch_path, RecursiveMode::Recursive) {
                warn!(path = %config.watch_path.display(), "failed to watch for .px: {}", e);
                return;
            }

            info!(path = %config.watch_path.display(), "watching for .px file changes");

            // Simple debounce: track last-seen modify times
            let mut pending: HashMap<PathBuf, tokio::time::Instant> = HashMap::new();

            loop {
                tokio::select! {
                    Some(event) = notify_rx.recv() => {
                        for path in &event.paths {
                            if !is_px_file(path) {
                                continue;
                            }

                            match event.kind {
                                EventKind::Create(_) | EventKind::Modify(_) => {
                                    // Debounce: mark as pending
                                    pending.insert(
                                        path.clone(),
                                        tokio::time::Instant::now()
                                            + std::time::Duration::from_millis(debounce_ms),
                                    );
                                }
                                EventKind::Remove(_) => {
                                    pending.remove(path);
                                    let keys = key_index_clone
                                        .lock()
                                        .unwrap()
                                        .remove(path)
                                        .unwrap_or_default();

                                    if !keys.is_empty() {
                                        debug!(path = %path.display(), keys = keys.len(), "px file removed");
                                        let _ = tx
                                            .send(PxWatchEvent::Removed {
                                                path: path.clone(),
                                                keys,
                                            })
                                            .await;
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    _ = tokio::time::sleep(std::time::Duration::from_millis(50)) => {
                        // Process debounced events
                        let now = tokio::time::Instant::now();
                        let ready: Vec<PathBuf> = pending
                            .iter()
                            .filter(|(_, deadline)| now >= **deadline)
                            .map(|(p, _)| p.clone())
                            .collect();

                        for path in ready {
                            pending.remove(&path);

                            match load_and_compile(&path) {
                                Ok(records) => {
                                    let keys: Vec<String> =
                                        records.iter().map(|r| r.key.clone()).collect();
                                    debug!(path = %path.display(), records = keys.len(), "px file reloaded");

                                    key_index_clone
                                        .lock()
                                        .unwrap()
                                        .insert(path.clone(), keys);

                                    let _ = tx
                                        .send(PxWatchEvent::Loaded {
                                            path,
                                            records,
                                        })
                                        .await;
                                }
                                Err(e) => {
                                    warn!(path = %path.display(), "px compile error: {}", e);
                                    let _ = tx
                                        .send(PxWatchEvent::Error {
                                            path,
                                            error: e,
                                        })
                                        .await;
                                }
                            }
                        }
                    }
                }
            }
        });

        Ok(rx)
    }
}

/// Check if a path is a `.px` file.
fn is_px_file(path: &Path) -> bool {
    path.extension().is_some_and(|ext| ext == "px")
}

/// Load a `.px` file from disk, parse, and compile it.
fn load_and_compile(path: &Path) -> Result<Vec<CompiledRecord>, String> {
    let source = std::fs::read_to_string(path).map_err(|e| format!("read error: {e}"))?;

    let doc = parse(&source)?;
    Ok(compile(&doc))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio::time::{sleep, Duration};

    fn test_px_source() -> &'static str {
        r#"
fact test_state:
  status: string

rule hello_rule:
  when:
    - test_state.status == "ready"
  then:
    - action: greet

constraint must_be_ready:
  when: test_state.status != "ready"
  require: false
  severity: error
  message: "State must be ready"
"#
    }

    #[tokio::test]
    async fn initial_scan_loads_existing_files() {
        let dir = TempDir::new().unwrap();
        let px_path = dir.path().join("test.px");
        std::fs::write(&px_path, test_px_source()).unwrap();

        let watcher = PxWatcher::new(PxWatcherConfig {
            watch_path: dir.path().to_path_buf(),
            initial_scan: true,
            debounce_ms: 50,
        });

        let mut rx = watcher.start().await.unwrap();

        // Should get a Loaded event
        let event = tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .unwrap()
            .unwrap();

        match event {
            PxWatchEvent::Loaded { path, records } => {
                assert_eq!(path, px_path);
                // fact + rule + constraint = 3 records
                assert_eq!(records.len(), 3);
                assert!(records.iter().any(|r| r.key == "px:fact/test_state"));
                assert!(records.iter().any(|r| r.key == "px:rule/hello_rule"));
                assert!(records
                    .iter()
                    .any(|r| r.key == "px:constraint/must_be_ready"));
            }
            other => panic!("expected Loaded, got {:?}", other),
        }

        // Should get Ready event
        let event = tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .unwrap()
            .unwrap();

        match event {
            PxWatchEvent::Ready {
                file_count,
                record_count,
            } => {
                assert_eq!(file_count, 1);
                assert_eq!(record_count, 3);
            }
            other => panic!("expected Ready, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn detects_new_px_file() {
        let dir = TempDir::new().unwrap();

        let watcher = PxWatcher::new(PxWatcherConfig {
            watch_path: dir.path().to_path_buf(),
            initial_scan: true,
            debounce_ms: 50,
        });

        let mut rx = watcher.start().await.unwrap();

        // Consume Ready (no files yet)
        let event = tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(event, PxWatchEvent::Ready { file_count: 0, .. }));

        // Create a new .px file
        sleep(Duration::from_millis(100)).await;
        let px_path = dir.path().join("new.px");
        std::fs::write(&px_path, test_px_source()).unwrap();

        // Should get a Loaded event after debounce
        let event = tokio::time::timeout(Duration::from_secs(3), rx.recv())
            .await
            .unwrap()
            .unwrap();

        match event {
            PxWatchEvent::Loaded { path, records } => {
                assert_eq!(path, px_path);
                assert_eq!(records.len(), 3);
            }
            other => panic!("expected Loaded, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn detects_modified_px_file() {
        let dir = TempDir::new().unwrap();
        let px_path = dir.path().join("mutable.px");
        std::fs::write(&px_path, "fact original:\n  val: string\n").unwrap();

        let watcher = PxWatcher::new(PxWatcherConfig {
            watch_path: dir.path().to_path_buf(),
            initial_scan: true,
            debounce_ms: 50,
        });

        let mut rx = watcher.start().await.unwrap();

        // Consume initial Loaded + Ready
        let _ = tokio::time::timeout(Duration::from_secs(2), rx.recv()).await;
        let _ = tokio::time::timeout(Duration::from_secs(2), rx.recv()).await;

        // Modify the file
        sleep(Duration::from_millis(100)).await;
        std::fs::write(
            &px_path,
            "fact updated:\n  val: string\n\nfact extra:\n  x: int\n",
        )
        .unwrap();

        // Should reload with new content
        let event = tokio::time::timeout(Duration::from_secs(3), rx.recv())
            .await
            .unwrap()
            .unwrap();

        match event {
            PxWatchEvent::Loaded { records, .. } => {
                assert_eq!(records.len(), 2);
                assert!(records.iter().any(|r| r.key == "px:fact/updated"));
                assert!(records.iter().any(|r| r.key == "px:fact/extra"));
            }
            other => panic!("expected Loaded, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn detects_deleted_px_file() {
        let dir = TempDir::new().unwrap();
        let px_path = dir.path().join("doomed.px");
        std::fs::write(&px_path, "fact doomed:\n  x: int\n").unwrap();

        let watcher = PxWatcher::new(PxWatcherConfig {
            watch_path: dir.path().to_path_buf(),
            initial_scan: true,
            debounce_ms: 50,
        });

        let mut rx = watcher.start().await.unwrap();

        // Consume initial Loaded + Ready
        let _ = tokio::time::timeout(Duration::from_secs(2), rx.recv()).await;
        let _ = tokio::time::timeout(Duration::from_secs(2), rx.recv()).await;

        // Delete the file
        sleep(Duration::from_millis(100)).await;
        std::fs::remove_file(&px_path).unwrap();

        // Should get a Removed event
        let event = tokio::time::timeout(Duration::from_secs(3), rx.recv())
            .await
            .unwrap()
            .unwrap();

        match event {
            PxWatchEvent::Removed { path, keys } => {
                assert_eq!(path, px_path);
                assert_eq!(keys, vec!["px:fact/doomed"]);
            }
            other => panic!("expected Removed, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn parse_error_emits_error_event() {
        let dir = TempDir::new().unwrap();
        let px_path = dir.path().join("bad.px");
        std::fs::write(&px_path, "this is not valid px syntax !@#$%").unwrap();

        let watcher = PxWatcher::new(PxWatcherConfig {
            watch_path: dir.path().to_path_buf(),
            initial_scan: true,
            debounce_ms: 50,
        });

        let mut rx = watcher.start().await.unwrap();

        let event = tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .unwrap()
            .unwrap();

        match event {
            PxWatchEvent::Error { path, error } => {
                assert_eq!(path, px_path);
                assert!(error.contains("parse error"));
            }
            other => panic!("expected Error, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn ignores_non_px_files() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("readme.md"), "# Hello").unwrap();
        std::fs::write(dir.path().join("config.toml"), "[x]\ny=1").unwrap();

        let watcher = PxWatcher::new(PxWatcherConfig {
            watch_path: dir.path().to_path_buf(),
            initial_scan: true,
            debounce_ms: 50,
        });

        let mut rx = watcher.start().await.unwrap();

        // Should only get Ready with 0 files
        let event = tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .unwrap()
            .unwrap();

        match event {
            PxWatchEvent::Ready {
                file_count,
                record_count,
            } => {
                assert_eq!(file_count, 0);
                assert_eq!(record_count, 0);
            }
            other => panic!("expected Ready, got {:?}", other),
        }
    }

    #[test]
    fn is_px_file_works() {
        assert!(is_px_file(Path::new("/foo/bar.px")));
        assert!(is_px_file(Path::new("rules.px")));
        assert!(!is_px_file(Path::new("rules.rs")));
        assert!(!is_px_file(Path::new("rules.px.bak")));
        assert!(!is_px_file(Path::new("readme.md")));
    }
}
