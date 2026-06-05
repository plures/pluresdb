//! Import resolver for `.px` files.
//!
//! Resolves `import` declarations by loading referenced files from disk,
//! parsing them recursively, and merging their declarations into the
//! importing document. Handles:
//!
//! - Path resolution (`module::sub` → `./module/sub.px`)
//! - Aliases (namespacing imported items)
//! - Circular import detection
//! - Recursive transitive imports

use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};

use super::{parse, PxDocument};

/// Errors that can occur during import resolution.
#[derive(Debug, Clone, PartialEq)]
pub enum ResolveError {
    /// A circular import was detected.
    CircularImport { path: PathBuf, chain: Vec<PathBuf> },
    /// The imported file could not be read.
    IoError { path: PathBuf, message: String },
    /// The imported file failed to parse.
    ParseError { path: PathBuf, message: String },
    /// The import path could not be resolved.
    InvalidPath {
        import_path: String,
        message: String,
    },
}

impl std::fmt::Display for ResolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CircularImport { path, chain } => {
                write!(
                    f,
                    "circular import: {} (chain: {})",
                    path.display(),
                    chain
                        .iter()
                        .map(|p| p.display().to_string())
                        .collect::<Vec<_>>()
                        .join(" → ")
                )
            }
            Self::IoError { path, message } => {
                write!(f, "cannot read {}: {}", path.display(), message)
            }
            Self::ParseError { path, message } => {
                write!(f, "parse error in {}: {}", path.display(), message)
            }
            Self::InvalidPath {
                import_path,
                message,
            } => {
                write!(f, "invalid import path '{}': {}", import_path, message)
            }
        }
    }
}

impl std::error::Error for ResolveError {}

/// Result of resolving all imports for a document.
#[derive(Debug, Clone)]
pub struct ResolvedDocument {
    /// The merged document with all imports inlined.
    pub document: PxDocument,
    /// Paths that were successfully resolved and merged.
    pub resolved_paths: Vec<PathBuf>,
}

/// Resolve all imports in a document, starting from the given base path.
///
/// The `base_path` should be the directory containing the root `.px` file.
/// Import paths like `module::sub` resolve to `<base_path>/module/sub.px`.
///
/// # Example
///
/// ```rust,ignore
/// use pluresdb_px::px::resolver::resolve_imports;
/// use std::path::Path;
///
/// let doc = pluresdb_px::px::parse(source)?;
/// let resolved = resolve_imports(&doc, Path::new("./praxis/skills/"))?;
/// ```
pub fn resolve_imports(
    doc: &PxDocument,
    base_path: &Path,
) -> Result<ResolvedDocument, ResolveError> {
    let mut visited = HashSet::new();
    let mut chain = Vec::new();
    resolve_recursive(doc, base_path, &mut visited, &mut chain)
}

/// Resolve imports from source string with a virtual path (useful for testing).
pub fn resolve_from_source(
    source: &str,
    base_path: &Path,
) -> Result<ResolvedDocument, ResolveError> {
    let doc = parse(source).map_err(|e| ResolveError::ParseError {
        path: base_path.to_path_buf(),
        message: e,
    })?;
    resolve_imports(&doc, base_path)
}

fn resolve_recursive(
    doc: &PxDocument,
    base_path: &Path,
    visited: &mut HashSet<PathBuf>,
    chain: &mut Vec<PathBuf>,
) -> Result<ResolvedDocument, ResolveError> {
    let mut merged = doc.clone();
    // Clear imports from the merged doc since we're inlining them
    merged.imports = Vec::new();
    let mut resolved_paths = Vec::new();

    for import in &doc.imports {
        let resolved_path = resolve_import_path(&import.path, base_path)?;
        let canonical = resolved_path
            .canonicalize()
            .unwrap_or_else(|_| resolved_path.clone());

        // Circular import detection — check the active recursion stack
        if chain.contains(&canonical) {
            return Err(ResolveError::CircularImport {
                path: canonical,
                chain: chain.clone(),
            });
        }

        // Diamond import dedup — already fully resolved, skip
        if visited.contains(&canonical) {
            continue;
        }

        // Read and parse the imported file
        let source =
            std::fs::read_to_string(&resolved_path).map_err(|e| ResolveError::IoError {
                path: resolved_path.clone(),
                message: e.to_string(),
            })?;

        let imported_doc = parse(&source).map_err(|e| ResolveError::ParseError {
            path: resolved_path.clone(),
            message: e,
        })?;

        // Push onto chain BEFORE recursing (cycle detection)
        chain.push(canonical.clone());

        // Recursively resolve the imported document's own imports
        let import_base = resolved_path.parent().unwrap_or(base_path);
        let child_resolved = resolve_recursive(&imported_doc, import_base, visited, chain)?;

        chain.pop();

        // Mark as fully resolved AFTER successful recursion (dedup)
        visited.insert(canonical);

        // Merge the resolved imported document into our merged doc
        merge_document(
            &mut merged,
            &child_resolved.document,
            import.alias.as_deref(),
        );
        resolved_paths.push(resolved_path);
        resolved_paths.extend(child_resolved.resolved_paths);
    }

    Ok(ResolvedDocument {
        document: merged,
        resolved_paths,
    })
}

/// Resolve an import path string to a filesystem path.
///
/// Rules:
/// - `module::sub` → `<base>/module/sub.px`
/// - `./relative/path` → `<base>/relative/path.px` (if no .px extension)
/// - Absolute paths are rejected
fn resolve_import_path(import_path: &str, base_path: &Path) -> Result<PathBuf, ResolveError> {
    if import_path.is_empty() {
        return Err(ResolveError::InvalidPath {
            import_path: import_path.to_string(),
            message: "empty import path".to_string(),
        });
    }

    let import_as_path = Path::new(import_path);
    if import_as_path.is_absolute() {
        return Err(ResolveError::InvalidPath {
            import_path: import_path.to_string(),
            message: "absolute import paths are not allowed".to_string(),
        });
    }

    if import_as_path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(ResolveError::InvalidPath {
            import_path: import_path.to_string(),
            message: "parent path segments are not allowed".to_string(),
        });
    }

    // Handle Rust-style paths (module::sub)
    if import_path.contains("::") {
        let parts: Vec<&str> = import_path.split("::").collect();
        if parts.iter().any(|p| p.is_empty()) {
            return Err(ResolveError::InvalidPath {
                import_path: import_path.to_string(),
                message: "empty segment in import path".to_string(),
            });
        }
        let mut path = base_path.to_path_buf();
        for part in &parts {
            path.push(part);
        }
        path.set_extension("px");
        return Ok(path);
    }

    // Handle relative file paths
    let mut path = base_path.join(import_path);
    if path.extension().is_none() {
        path.set_extension("px");
    }

    let base_canonical = base_path
        .canonicalize()
        .unwrap_or_else(|_| base_path.to_path_buf());
    if let Ok(canonical_path) = path.canonicalize() {
        if !canonical_path.starts_with(&base_canonical) {
            return Err(ResolveError::InvalidPath {
                import_path: import_path.to_string(),
                message: "import path escapes base directory".to_string(),
            });
        }
    }

    Ok(path)
}

/// Merge an imported document's declarations into the target document.
///
/// If an alias is provided, imported item names are prefixed: `alias.name`.
fn merge_document(target: &mut PxDocument, source: &PxDocument, alias: Option<&str>) {
    let prefix = |name: &str| -> String {
        match alias {
            Some(a) => format!("{}.{}", a, name),
            None => name.to_string(),
        }
    };

    for mut fact in source.facts.clone() {
        fact.name = prefix(&fact.name);
        target.facts.push(fact);
    }

    for mut rule in source.rules.clone() {
        rule.name = prefix(&rule.name);
        target.rules.push(rule);
    }

    for mut constraint in source.constraints.clone() {
        constraint.name = prefix(&constraint.name);
        target.constraints.push(constraint);
    }

    for mut contract in source.contracts.clone() {
        contract.name = prefix(&contract.name);
        target.contracts.push(contract);
    }

    for mut function in source.functions.clone() {
        function.name = prefix(&function.name);
        target.functions.push(function);
    }

    for mut trigger in source.triggers.clone() {
        trigger.name = prefix(&trigger.name);
        target.triggers.push(trigger);
    }

    for mut procedure in source.procedures.clone() {
        procedure.name = prefix(&procedure.name);
        target.procedures.push(procedure);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write_px_file(dir: &Path, relative_path: &str, content: &str) {
        let path = dir.join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, content).unwrap();
    }

    #[test]
    fn resolve_rust_style_path() {
        let base = Path::new("/project/praxis");
        let result = resolve_import_path("common::types", base).unwrap();
        assert_eq!(result, PathBuf::from("/project/praxis/common/types.px"));
    }

    #[test]
    fn resolve_relative_path() {
        let base = Path::new("/project/praxis");
        let result = resolve_import_path("shared", base).unwrap();
        assert_eq!(result, PathBuf::from("/project/praxis/shared.px"));
    }

    #[test]
    fn resolve_relative_path_with_extension() {
        let base = Path::new("/project/praxis");
        let result = resolve_import_path("shared.px", base).unwrap();
        assert_eq!(result, PathBuf::from("/project/praxis/shared.px"));
    }

    #[test]
    fn reject_absolute_path() {
        let base = Path::new("/project/praxis");
        let result = resolve_import_path("/etc/passwd", base);
        assert!(matches!(result, Err(ResolveError::InvalidPath { .. })));
    }

    #[test]
    fn reject_parent_components() {
        let base = Path::new("/project/praxis");
        let result = resolve_import_path("../shared", base);
        assert!(matches!(result, Err(ResolveError::InvalidPath { .. })));
    }

    #[test]
    fn reject_empty_path() {
        let base = Path::new("/project/praxis");
        let result = resolve_import_path("", base);
        assert!(matches!(result, Err(ResolveError::InvalidPath { .. })));
    }

    #[test]
    fn reject_empty_segment() {
        let base = Path::new("/project/praxis");
        let result = resolve_import_path("a::::b", base);
        assert!(matches!(result, Err(ResolveError::InvalidPath { .. })));
    }

    #[test]
    fn simple_import_resolution() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();

        write_px_file(
            base,
            "shared.px",
            r#"
fact shared_state:
  value: string

constraint shared_rule:
  when: shared_state.value == ""
  require: shared_state.value != ""
  severity: error
"#,
        );

        let source = r#"
import shared

fact local_state:
  count: int
"#;

        let doc = parse(source).unwrap();
        let resolved = resolve_imports(&doc, base).unwrap();

        assert_eq!(resolved.document.facts.len(), 2);
        assert_eq!(resolved.document.constraints.len(), 1);
        assert_eq!(resolved.resolved_paths.len(), 1);
        // Without alias, names are unchanged
        assert!(resolved
            .document
            .facts
            .iter()
            .any(|f| f.name == "shared_state"));
        assert!(resolved
            .document
            .facts
            .iter()
            .any(|f| f.name == "local_state"));
    }

    #[test]
    fn import_with_alias() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();

        write_px_file(
            base,
            "types.px",
            r#"
fact state:
  active: bool
"#,
        );

        let source = r#"
import types as t

fact local:
  x: int
"#;

        let doc = parse(source).unwrap();
        let resolved = resolve_imports(&doc, base).unwrap();

        assert_eq!(resolved.document.facts.len(), 2);
        // Aliased import gets prefixed
        assert!(resolved.document.facts.iter().any(|f| f.name == "t.state"));
        assert!(resolved.document.facts.iter().any(|f| f.name == "local"));
    }

    #[test]
    fn nested_imports() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();

        write_px_file(
            base,
            "base.px",
            r#"
fact base_fact:
  x: int
"#,
        );

        write_px_file(
            base,
            "middle.px",
            r#"
import base

fact middle_fact:
  y: string
"#,
        );

        let source = r#"
import middle

fact top_fact:
  z: bool
"#;

        let doc = parse(source).unwrap();
        let resolved = resolve_imports(&doc, base).unwrap();

        assert_eq!(resolved.document.facts.len(), 3);
        assert!(resolved
            .document
            .facts
            .iter()
            .any(|f| f.name == "base_fact"));
        assert!(resolved
            .document
            .facts
            .iter()
            .any(|f| f.name == "middle_fact"));
        assert!(resolved.document.facts.iter().any(|f| f.name == "top_fact"));
        assert_eq!(resolved.resolved_paths.len(), 2);
    }

    #[test]
    fn diamond_import_deduplication() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();

        write_px_file(
            base,
            "shared.px",
            r#"
fact shared:
  v: int
"#,
        );

        write_px_file(
            base,
            "a.px",
            r#"
import shared

fact a_fact:
  x: int
"#,
        );

        write_px_file(
            base,
            "b.px",
            r#"
import shared

fact b_fact:
  y: int
"#,
        );

        let source = r#"
import a
import b
"#;

        let doc = parse(source).unwrap();
        let resolved = resolve_imports(&doc, base).unwrap();

        // shared should only appear once (diamond dedup)
        let shared_count = resolved
            .document
            .facts
            .iter()
            .filter(|f| f.name == "shared")
            .count();
        assert_eq!(shared_count, 1);
        assert!(resolved.document.facts.iter().any(|f| f.name == "a_fact"));
        assert!(resolved.document.facts.iter().any(|f| f.name == "b_fact"));
    }

    #[test]
    fn circular_import_detected() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();

        write_px_file(base, "a.px", "import b\n\nfact a_state:\n  x: int\n");

        write_px_file(base, "b.px", "import a\n\nfact b_state:\n  y: int\n");

        let source = r#"import a"#;
        let doc = parse(source).unwrap();
        let result = resolve_imports(&doc, base);
        assert!(
            matches!(result, Err(ResolveError::CircularImport { .. })),
            "expected CircularImport, got: {:?}",
            result
        );
    }

    #[test]
    fn missing_file_returns_io_error() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();

        let source = r#"import nonexistent"#;
        let doc = parse(source).unwrap();
        let result = resolve_imports(&doc, base);
        assert!(matches!(result, Err(ResolveError::IoError { .. })));
    }

    #[test]
    fn rust_style_nested_path() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();

        write_px_file(
            base,
            "common/types.px",
            r#"
fact common_type:
  name: string
"#,
        );

        let source = r#"
import common::types as ct

fact local:
  x: int
"#;

        let doc = parse(source).unwrap();
        let resolved = resolve_imports(&doc, base).unwrap();

        assert_eq!(resolved.document.facts.len(), 2);
        assert!(resolved
            .document
            .facts
            .iter()
            .any(|f| f.name == "ct.common_type"));
    }

    #[test]
    fn resolve_from_source_helper() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();

        write_px_file(
            base,
            "lib.px",
            r#"
fact lib_state:
  ready: bool
"#,
        );

        let source = r#"
import lib
fact app:
  running: bool
"#;

        let resolved = resolve_from_source(source, base).unwrap();
        assert_eq!(resolved.document.facts.len(), 2);
    }
}
