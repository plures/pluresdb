//! Headroom token-compression — REAL ported algorithm (no stubs, no agens dep).
//!
//! This module is a faithful, dependency-free port of the **production**
//! headroom compressor that lives in `pares-agens`:
//!
//! * routing core + 4 strategies ← `pares-agens/crates/core/src/headroom_bridge.rs`
//!   (`HeadroomHook::compress_one` L231, `compress_prose` L266, `compress_code`
//!   L310, `compress_log` L349, `compress_whitespace`/`collapse_whitespace` L388/L406).
//! * 5 real leaf actors + their heuristics ← `pares-agens/crates/core/src/headroom.rs`
//!   (`detect_content_type` L68, `count_tokens` cl100k L97, `split_sentences` L156
//!   via `unicode_segmentation`, `extract_signatures_heuristic` L600,
//!   `detect_language_heuristic` L577, plus `is_log_content`/`is_code_content`/
//!   `looks_like_error`).
//!
//! Differences from the agens source — and *only* these:
//!   1. The `ActionHandler::call("name", &json!{...})` JSON indirection is removed;
//!      the hook called 5 real actors by name, so here we call those 5 real
//!      functions **directly**. Same logic, no JSON marshaling, no `.px` executor,
//!      and crucially **none of the ~160 canned-JSON "stub" actor arms** are ported.
//!   2. The transient `Vec<ChatMessage>` message-loop, the 500-token aggregate gate,
//!      the net-savings batch guard and observability writes stay OUT of native
//!      (they are the message-loop wrapper; per H-ANALYZE they live in TS). Native
//!      knows only `&str -> String`. The **per-message** net-savings guard (only
//!      accept the rewrite when it is actually smaller) is preserved here verbatim,
//!      because it is part of `compress_one`'s contract.
//!   3. No `pares_radix_core` / `pares-agens` / `CrdtStore` imports — only
//!      `tiktoken_rs` + `unicode_segmentation` (both already agens deps, neither is
//!      agens itself). Verified absent via `cargo tree` at build time.
//!
//! Contract preserved (H-ANALYZE §1a): the output is always a smaller-or-equal,
//! byte-derived transform of the SAME content (extractive head+tail window /
//! signature skeleton / consecutive-dup run-collapse / whitespace squeeze) — never
//! a paraphrase, never a fabricated ratio. A body can only shrink or stay identical.

use std::sync::OnceLock;

use tiktoken_rs::{cl100k_base, CoreBPE};
use unicode_segmentation::UnicodeSegmentation;

/// Per-message content length (chars) below which an individual body is left
/// untouched. Ported verbatim from `headroom_bridge.rs` `PER_MESSAGE_MIN_CHARS`.
pub const PER_MESSAGE_MIN_CHARS: usize = 200;

/// Cached tiktoken BPE tokenizer. `cl100k_base()` allocates ~100MB+ of BPE
/// tables, so we init once and reuse across all calls — ported from the
/// `OnceLock<CoreBPE>` pattern in `headroom.rs` L28-39.
static BPE: OnceLock<CoreBPE> = OnceLock::new();

fn bpe() -> &'static CoreBPE {
    BPE.get_or_init(|| cl100k_base().expect("tiktoken cl100k_base init failed"))
}

// ── public entry points (the NAPI free functions wrap these) ────────────────

/// Real cl100k_base token count (port of the `count_tokens` actor,
/// `headroom.rs` L97-103: `bpe.encode_with_special_tokens(content).len()`).
pub fn count_tokens(content: &str) -> usize {
    bpe().encode_with_special_tokens(content).len()
}

/// Detect content type: one of `json` | `log` | `code` | `error` | `prose`.
/// Port of the `detect_content_type` actor (`headroom.rs` L68-86).
pub fn detect_content_type(content: &str) -> &'static str {
    let trimmed = content.trim_start();
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        "json"
    } else if is_log_content(content) {
        "log"
    } else if is_code_content(content) {
        "code"
    } else if looks_like_error(content) {
        "error"
    } else {
        "prose"
    }
}

/// Compress a single body by detected (or caller-forced) content type.
///
/// Faithful port of `HeadroomHook::compress_one` (`headroom_bridge.rs` L231-258):
/// short bodies pass through; route by content type to the real strategy; only
/// accept the rewrite if it is actually smaller AND non-empty, otherwise return
/// the original unchanged (the per-message net-savings guard). `content_type`,
/// when `Some`, overrides detection (so the TS seam can pin a type it already
/// knows); `None` runs the real detector.
pub fn compress_text(content: &str, content_type: Option<&str>) -> String {
    if content.len() < PER_MESSAGE_MIN_CHARS {
        return content.to_string();
    }

    let ct = content_type.unwrap_or_else(|| detect_content_type(content));
    let compressed: Option<String> = match ct {
        "code" => compress_code(content),
        "log" => compress_log(content),
        "prose" | "error" => compress_prose(content),
        // json / other: structural whitespace trim is the safe default.
        _ => Some(collapse_whitespace(content)),
    };

    match compressed {
        Some(out) if out.len() < content.len() && !out.trim().is_empty() => out,
        _ => content.to_string(),
    }
}

// ── prose: extractive head+tail sentence window ─────────────────────────────

/// Prose compression: split into sentences (real `split_sentences` actor via
/// `unicode_segmentation::split_sentence_bounds`) and keep a head+tail extractive
/// window, collapsing the middle. Port of `compress_prose`
/// (`headroom_bridge.rs` L266-301) fused with the `split_sentences` actor
/// (`headroom.rs` L156-166).
fn compress_prose(content: &str) -> Option<String> {
    let sentences: Vec<&str> = split_sentences(content);

    // Not enough sentences to meaningfully trim.
    if sentences.len() <= 6 {
        return Some(collapse_whitespace(content));
    }

    let head = 3usize;
    let tail = 3usize;
    let dropped = sentences.len() - head - tail;
    let mut out = String::with_capacity(content.len());
    for s in &sentences[..head] {
        out.push_str(s);
        out.push(' ');
    }
    out.push_str(&format!("[… {dropped} sentences elided …] "));
    for s in &sentences[sentences.len() - tail..] {
        out.push_str(s);
        out.push(' ');
    }
    Some(out.trim_end().to_string())
}

/// Real `split_sentences` actor (`headroom.rs` L156-166): unicode sentence
/// bounds, trimmed, empties dropped.
fn split_sentences(content: &str) -> Vec<&str> {
    content
        .split_sentence_bounds()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect()
}

// ── code: signature skeleton ────────────────────────────────────────────────

/// Code compression: detect language, then replace the body with the extracted
/// signature lines; fall back to whitespace collapse when no signatures.
/// Port of `compress_code` (`headroom_bridge.rs` L310-342) fused with the
/// `detect_language`/`extract_ast_signatures` actors.
fn compress_code(content: &str) -> Option<String> {
    let language = detect_language_heuristic(content);
    let sigs = extract_signatures_heuristic(content, language);

    if sigs.is_empty() {
        return Some(collapse_whitespace(content));
    }

    let mut out = String::with_capacity(content.len() / 2);
    out.push_str(&format!(
        "// [headroom: {language} body elided — {} signature(s) kept]\n",
        sigs.len()
    ));
    for s in sigs {
        out.push_str(&s);
        out.push('\n');
    }
    Some(out.trim_end().to_string())
}

// ── log: consecutive-duplicate run collapse ─────────────────────────────────

/// Log compression: collapse runs of identical adjacent lines into `line  [×N]`;
/// singletons pass through. Pure-Rust port of `compress_log`
/// (`headroom_bridge.rs` L349-383) — no actor was involved upstream either.
fn compress_log(content: &str) -> Option<String> {
    let mut out = String::with_capacity(content.len());
    let mut prev: Option<&str> = None;
    let mut run: usize = 0;

    let flush = |out: &mut String, line: &str, run: usize| {
        if run > 1 {
            out.push_str(line);
            out.push_str(&format!("  [×{run}]\n"));
        } else {
            out.push_str(line);
            out.push('\n');
        }
    };

    for line in content.lines() {
        match prev {
            Some(p) if p == line => run += 1,
            Some(p) => {
                flush(&mut out, p, run);
                prev = Some(line);
                run = 1;
            }
            None => {
                prev = Some(line);
                run = 1;
            }
        }
    }
    if let Some(p) = prev {
        flush(&mut out, p, run);
    }

    Some(out.trim_end().to_string())
}

// ── json / other: whitespace collapse ───────────────────────────────────────

/// Collapse runs of whitespace (including blank lines) into single spaces,
/// trimming each line. Port of `collapse_whitespace` (`headroom_bridge.rs`
/// L406-422).
fn collapse_whitespace(content: &str) -> String {
    let mut out = String::with_capacity(content.len());
    let mut last_was_space = false;
    for ch in content.chars() {
        if ch.is_whitespace() {
            if !last_was_space {
                out.push(' ');
                last_was_space = true;
            }
        } else {
            out.push(ch);
            last_was_space = false;
        }
    }
    out.trim().to_string()
}

// ── content-type heuristics (ported from headroom.rs helpers) ───────────────

/// Returns true if the content looks like log output (timestamp + level
/// patterns). Port of `is_log_content` (`headroom.rs` L~470-490).
fn is_log_content(content: &str) -> bool {
    let lines: Vec<&str> = content.lines().take(10).collect();
    if lines.len() < 3 {
        return false;
    }
    let log_line_count = lines
        .iter()
        .filter(|l| {
            let t = l.trim();
            t.contains(" INFO ")
                || t.contains(" WARN ")
                || t.contains(" ERROR ")
                || t.contains(" DEBUG ")
                || t.contains(" TRACE ")
                || t.contains("[info]")
                || t.contains("[warn]")
                || t.contains("[error]")
                || (t.len() >= 19
                    && t.as_bytes().get(4) == Some(&b'-')
                    && t.as_bytes().get(7) == Some(&b'-')
                    && (t.as_bytes().get(10) == Some(&b'T') || t.as_bytes().get(10) == Some(&b' '))
                    && t.as_bytes().get(13) == Some(&b':'))
        })
        .count();
    log_line_count >= 2
}

/// Returns true if content looks like an error or stack trace. Requires multiple
/// error indicators to avoid false positives. Port of `looks_like_error`
/// (`headroom.rs` L492-520).
fn looks_like_error(content: &str) -> bool {
    let indicators = [
        content.contains("panicked at"),
        content.contains("Traceback"),
        content.contains("stack trace"),
        content.contains("thread '") && content.contains("panicked"),
        content.lines().any(|l| {
            let t = l.trim();
            t.starts_with("Error:") || t.starts_with("error:") || t.starts_with("error[")
        }),
        content.lines().any(|l| l.trim().starts_with("Exception:")),
        content
            .lines()
            .filter(|l| {
                let t = l.trim();
                (t.starts_with("at ") || t.starts_with("  at "))
                    && (t.contains('(') || t.contains("::"))
            })
            .count()
            >= 2,
        content.contains("Caused by:"),
    ];
    indicators.iter().filter(|&&b| b).count() >= 2
}

/// Returns true if content looks like source code (structural analysis, not just
/// keywords). Port of `is_code_content` (`headroom.rs` L524-573).
fn is_code_content(content: &str) -> bool {
    let lines: Vec<&str> = content.lines().collect();
    let total = lines.len() as f64;

    if lines.len() <= 2 {
        let t = content.trim();
        return t.starts_with("fn ")
            || t.starts_with("pub fn ")
            || t.starts_with("def ")
            || t.starts_with("function ")
            || t.starts_with("class ")
            || t.starts_with("impl ")
            || t.starts_with("struct ")
            || t.starts_with("enum ");
    }

    let brace_lines = lines
        .iter()
        .filter(|l| l.contains('{') || l.contains('}'))
        .count() as f64;
    let indented_lines = lines
        .iter()
        .filter(|l| l.starts_with("    ") || l.starts_with('\t'))
        .count() as f64;
    let semicolons = lines
        .iter()
        .filter(|l| l.trim_end().ends_with(';'))
        .count() as f64;

    let keyword_lines = lines
        .iter()
        .filter(|l| {
            let t = l.trim();
            t.starts_with("fn ")
                || t.starts_with("pub fn ")
                || t.starts_with("pub(crate) fn ")
                || t.starts_with("def ")
                || t.starts_with("function ")
                || t.starts_with("async fn ")
                || t.starts_with("class ")
                || t.starts_with("impl ")
                || t.starts_with("struct ")
                || t.starts_with("enum ")
                || t.starts_with("trait ")
                || t.starts_with("const ")
                || t.starts_with("let ")
                || t.starts_with("use ")
                || t.starts_with("import ")
                || t.starts_with("from ")
                || t.starts_with("#[")
        })
        .count() as f64;

    let structural_ratio = (brace_lines + indented_lines + semicolons) / (3.0 * total);
    let keyword_ratio = keyword_lines / total;

    structural_ratio > 0.15 && keyword_ratio > 0.05
}

/// Heuristic language detection from source content. Port of
/// `detect_language_heuristic` (`headroom.rs` L577-595).
fn detect_language_heuristic(content: &str) -> &'static str {
    if content.contains("fn ") && (content.contains("let ") || content.contains("impl ")) {
        "rust"
    } else if content.contains("def ") && content.contains(':') {
        "python"
    } else if content.contains("function ")
        || content.contains("const ")
        || content.contains("let ")
    {
        "javascript"
    } else if content.contains("public class ") || content.contains("void ") {
        "java"
    } else if content.contains("#include") || content.contains("std::") {
        "cpp"
    } else if content.contains("package main") || content.contains("func ") {
        "go"
    } else if content.contains("SELECT ") || content.contains("FROM ") {
        "sql"
    } else {
        "unknown"
    }
}

/// Heuristic AST signature extraction without tree-sitter grammars. Extracts
/// lines that look like function/method/type signatures. Port of
/// `extract_signatures_heuristic` (`headroom.rs` L600-657).
fn extract_signatures_heuristic(content: &str, language: &str) -> Vec<String> {
    let mut sigs = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with('#') {
            continue;
        }

        let is_sig = match language {
            "rust" => {
                (trimmed.starts_with("pub fn ")
                    || trimmed.starts_with("fn ")
                    || trimmed.starts_with("pub async fn ")
                    || trimmed.starts_with("async fn ")
                    || trimmed.starts_with("pub struct ")
                    || trimmed.starts_with("pub enum ")
                    || trimmed.starts_with("struct ")
                    || trimmed.starts_with("enum ")
                    || trimmed.starts_with("trait ")
                    || trimmed.starts_with("pub trait ")
                    || trimmed.starts_with("impl "))
                    && !trimmed.ends_with(',')
            }
            "python" => {
                trimmed.starts_with("def ")
                    || trimmed.starts_with("async def ")
                    || trimmed.starts_with("class ")
            }
            "javascript" | "typescript" => {
                trimmed.starts_with("function ")
                    || trimmed.starts_with("async function ")
                    || trimmed.starts_with("class ")
                    || trimmed.contains("=> {")
                    || (trimmed.starts_with("const ") && trimmed.contains("function"))
            }
            "java" | "cpp" | "c" => {
                (trimmed.contains('(') && trimmed.contains(')'))
                    && !trimmed.starts_with("//")
                    && !trimmed.starts_with('*')
                    && (trimmed.ends_with('{') || trimmed.ends_with(';'))
            }
            _ => {
                trimmed.contains('(')
                    && trimmed.contains(')')
                    && !trimmed.starts_with("//")
                    && trimmed.len() < 200
            }
        };

        if is_sig {
            sigs.push(trimmed.to_string());
        }
    }

    sigs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whitespace_collapse_shrinks() {
        let input = "a\n\n\n   b      c\t\t d";
        assert_eq!(collapse_whitespace(input), "a b c d");
    }

    #[test]
    fn log_dedup_collapses_runs() {
        let log = "ERROR boom\nERROR boom\nERROR boom\nINFO ok\n";
        let out = compress_log(log).unwrap();
        assert!(out.contains("[×3]"), "expected run marker, got: {out}");
        assert!(out.contains("INFO ok"));
    }

    #[test]
    fn large_prose_trims_middle_keeps_head_tail() {
        let prose: String = (0..80)
            .map(|i| format!("This is sentence number {i} with some filler words. "))
            .collect();
        let out = compress_text(&prose, None);
        assert!(out.len() < prose.len(), "expected shrink");
        assert!(out.contains("elided"), "expected elision marker");
        // head sentinel (sentence 0) and tail sentinel (sentence 79) survive
        assert!(out.contains("sentence number 0 "), "head sentence lost");
        assert!(out.contains("sentence number 79 "), "tail sentence lost");
    }

    #[test]
    fn code_keeps_signature_lines() {
        // Realistic Rust body well over the 200-char floor, with bulky function
        // bodies that the signature skeleton should prune away.
        let code = r#"pub fn alpha(x: i32) -> i32 {
    let mut acc = 0;
    for i in 0..x {
        acc += i * 2;
        acc -= 1;
    }
    let scaled = acc.saturating_mul(3);
    scaled + x
}

pub fn beta(name: &str) -> String {
    let trimmed = name.trim().to_lowercase();
    let mut out = String::new();
    for ch in trimmed.chars() {
        out.push(ch);
    }
    out
}

struct Foo {
    a: i32,
    b: i32,
    c: String,
}

impl Foo {
    pub fn total(&self) -> i32 {
        self.a + self.b
    }
}
"#;
        assert!(code.len() >= PER_MESSAGE_MIN_CHARS, "fixture too small");
        let out = compress_text(code, None);
        assert!(out.len() < code.len(), "expected code shrink: {out}");
        assert!(out.contains("pub fn alpha"), "alpha sig lost: {out}");
        assert!(out.contains("pub fn beta"), "beta sig lost: {out}");
        assert!(out.contains("struct Foo"), "struct sig lost: {out}");
        assert!(out.contains("body elided"), "expected code header: {out}");
        // bulky body lines must be gone
        assert!(!out.contains("acc += i"), "body line leaked: {out}");
    }

    #[test]
    fn short_input_passes_through_unchanged() {
        let s = "too short to bother compressing";
        assert_eq!(compress_text(s, None), s);
    }

    #[test]
    fn token_count_is_real_cl100k() {
        // "hello world" is 2 tokens under cl100k_base.
        assert_eq!(count_tokens("hello world"), 2);
        assert!(count_tokens("") == 0);
    }

    #[test]
    fn detect_routes_by_type() {
        assert_eq!(detect_content_type("{\"a\":1}"), "json");
        assert_eq!(
            detect_content_type("just some normal english prose about a topic"),
            "prose"
        );
    }
}
