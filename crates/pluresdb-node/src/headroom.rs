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
///
/// FIDELITY NOTE (H-TEST stage): the real pares-agens detector checks the
/// `{`/`[` JSON prefix *first*, so a bracketed-level log line (`[ERROR] ...`)
/// short-circuits to `json` before `is_log_content` is ever consulted — a
/// detector-ordering gap. We add a single tightly-guarded carve-out: a line that
/// begins with `[ERROR]`/`[WARN]`/`[INFO]`/`[DEBUG]`/`[TRACE]` is *never* a valid
/// JSON array (a JSON array opens with whitespace, a digit, a quote, `{`, `[`, or
/// `]` after `[` — never an uppercase level word), so we let it fall through to
/// the log branch. This cannot reclassify any real JSON. Plain `[`-prefixed
/// timestamp logs without a `[LEVEL]` token (e.g. `[2026-..T..Z] ERROR ...`)
/// remain a documented limitation (still routed to json) — see
/// `detect_bracketed_timestamp_log_is_documented_limitation`.
pub fn detect_content_type(content: &str) -> &'static str {
    let trimmed = content.trim_start();
    if (trimmed.starts_with('{') || trimmed.starts_with('[')) && !starts_like_bracketed_log(trimmed)
    {
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

/// True when the (left-trimmed) content begins with a bracketed log token that
/// must never be treated as JSON. Covers two shapes:
///   1. a bracketed log-LEVEL token (`[ERROR]`/`[WARN]`/`[INFO]`/`[DEBUG]`/`[TRACE]`), and
///   2. a bracketed ISO-8601 TIMESTAMP prefix (`[2026-07-02 10:15:03]`, `[2026-07-02T10:15:03Z]`),
///      which is one of the most common real-log line shapes and starts with `[`,
///      so without this guard such lines are misclassified as a JSON array and
///      routed to whitespace-collapse instead of the template log compactor.
///
/// It never matches real JSON: a JSON array opens with `[` followed by a value
/// (`{`, `"`, digit, `[`, or literal), not `LEVEL]` and not a `YYYY-MM-DD` date
/// skeleton immediately after the bracket.
fn starts_like_bracketed_log(trimmed: &str) -> bool {
    const BRACKETED: [&str; 5] = ["[ERROR]", "[WARN]", "[INFO]", "[DEBUG]", "[TRACE]"];
    if BRACKETED.iter().any(|b| trimmed.starts_with(b)) {
        return true;
    }
    // Bracketed ISO-8601 date/time prefix: `[` then `YYYY-MM-DD` skeleton.
    let b = trimmed.as_bytes();
    if b.first() == Some(&b'[') && starts_with_iso_date(&b[1..]) {
        return true;
    }
    false
}

/// True when bytes begin with a `YYYY-MM-DD` date skeleton (4 digits, `-`, 2
/// digits, `-`, 2 digits). Cheap, allocation-free; the same date shape the log
/// detector and the `<TS>` masker already trust.
fn starts_with_iso_date(b: &[u8]) -> bool {
    if b.len() < 10 {
        return false;
    }
    b[0].is_ascii_digit()
        && b[1].is_ascii_digit()
        && b[2].is_ascii_digit()
        && b[3].is_ascii_digit()
        && b[4] == b'-'
        && b[5].is_ascii_digit()
        && b[6].is_ascii_digit()
        && b[7] == b'-'
        && b[8].is_ascii_digit()
        && b[9].is_ascii_digit()
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

// ── log: template-normalizing consecutive run collapse (Path-2) ─────────────
//
// Path-2 upgrade (design: .openclaw/tmp/headroom-path2-design.md §3). The old
// port collapsed only *byte-identical* adjacent lines, which achieves ~0% on
// real logs because a timestamp / request-id / duration varies on every line.
// Instead we render each line into a **template** by masking variable fields
// (§3.2) and collapse CONSECUTIVE lines that share the same rendered template.
// The first RAW line of a run is emitted verbatim (a human/embedding always
// sees a real example); singletons are emitted 100% unchanged. Pure `std` —
// NO regex, NO once_cell — so the identical source compiles under the port's
// current dep set (tiktoken-rs + unicode-segmentation only).

/// Minimum identical-template run before we collapse (§3.5 knob).
const MIN_RUN: usize = 2;

/// Log compression: group runs of adjacent lines with an identical *template*
/// (variable fields masked, §3.2) into `first_raw_line  [xN ~ <TEMPLATE>]`;
/// singletons pass through verbatim. Deterministic, streaming one-line
/// lookahead — same contract as the old run-collapse, but template-aware.
///
/// The marker uses a plain ASCII `x` (not the U+00D7 multiplication sign) to
/// avoid the mojibake-on-disk artifact the design flagged.
fn compress_log(content: &str) -> Option<String> {
    let mut out = String::with_capacity(content.len());
    let mut prev_line: Option<&str> = None;
    let mut prev_tmpl = String::new();
    let mut cur_tmpl = String::new();
    let mut run: usize = 0;

    let flush = |out: &mut String, rep_line: &str, tmpl: &str, run: usize| {
        out.push_str(rep_line);
        if run >= MIN_RUN {
            out.push_str("  [x");
            out.push_str(&run.to_string());
            // Only append `~ <template>` when the template actually differs from
            // the representative line (i.e. at least one variable field was
            // masked). When no field varies, the template == the raw line, so
            // repeating it is pure redundancy that can inflate the TOKEN count
            // above the original run (violating the net-savings contract) even
            // though bytes shrink. In that case emit the terse `[xN]` form (a
            // documented marker knob, design §3.5). Positive fixtures with real
            // placeholders keep the informative `[xN ~ <TEMPLATE>]` shape.
            if tmpl != rep_line {
                out.push_str(" ~ ");
                out.push_str(tmpl);
            }
            out.push(']');
        }
        out.push('\n');
    };

    for line in content.lines() {
        cur_tmpl.clear();
        log_template(line, &mut cur_tmpl);
        match prev_line {
            None => {
                prev_line = Some(line);
                prev_tmpl.clear();
                prev_tmpl.push_str(&cur_tmpl);
                run = 1;
            }
            Some(_) if cur_tmpl == prev_tmpl => run += 1,
            Some(p) => {
                flush(&mut out, p, &prev_tmpl, run);
                prev_line = Some(line);
                prev_tmpl.clear();
                prev_tmpl.push_str(&cur_tmpl);
                run = 1;
            }
        }
    }
    if let Some(p) = prev_line {
        flush(&mut out, p, &prev_tmpl, run);
    }

    Some(out.trim_end().to_string())
}

// ── log template masker (§3.2 / §3.4) — std-only byte scanners ──────────────

/// Render `line` into its template by masking variable fields (§3.2), writing
/// into a reused buffer. Ordering is most-specific-first so a span is consumed
/// before a broader masker can chip it (the over-masking defense):
/// TS -> UUID -> IP -> DUR -> HEX -> KV(keep key) -> INT.
fn log_template(line: &str, buf: &mut String) {
    let b = line.as_bytes();
    let mut i = 0usize;
    while i < b.len() {
        if let Some(j) = take_timestamp(b, i) {
            buf.push_str("<TS>");
            i = j;
            continue;
        }
        if let Some(j) = take_uuid(b, i) {
            buf.push_str("<UUID>");
            i = j;
            continue;
        }
        if let Some(j) = take_ipv4(b, i) {
            buf.push_str("<IP>");
            i = j;
            continue;
        }
        if let Some(j) = take_duration(b, i) {
            buf.push_str("<DUR>");
            i = j;
            continue;
        }
        if let Some(j) = take_hex(b, i) {
            buf.push_str("<HEX>");
            i = j;
            continue;
        }
        // key=value: emit the '=' and mask ONLY the value (keep the key), so
        // structurally different key-sets produce different templates.
        if b[i] == b'=' {
            buf.push('=');
            if let Some(j) = take_kv_value(b, i + 1) {
                buf.push_str("<KV>");
                i = j;
                continue;
            }
            i += 1;
            continue;
        }
        if let Some(j) = take_int(b, i) {
            buf.push_str("<INT>");
            i = j;
            continue;
        }
        copy_next_utf8_char(line, &mut i, buf);
    }
}

/// Advance `*i` by one full UTF-8 char and copy those bytes into `buf`, so
/// multibyte text is never split mid-codepoint.
fn copy_next_utf8_char(line: &str, i: &mut usize, buf: &mut String) {
    let b = line.as_bytes();
    let start = *i;
    // UTF-8 leading-byte length (1..=4); default 1 for any stray continuation.
    let len = match b[start] {
        x if x < 0x80 => 1,
        x if x >> 5 == 0b110 => 2,
        x if x >> 4 == 0b1110 => 3,
        x if x >> 3 == 0b11110 => 4,
        _ => 1,
    };
    let end = (start + len).min(b.len());
    // Safe because `line` is valid UTF-8 and we advance on codepoint boundaries.
    buf.push_str(&line[start..end]);
    *i = end;
}

#[inline]
fn is_word_byte(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

/// ISO-8601 timestamp: `YYYY-MM-DD[T ]hh:mm:ss(.frac)?(Z|±hh(:?mm)?)?` (§3.2 #1).
/// Requires the full date+time skeleton (offsets 4/7 '-', 10 'T'/' ', 13/16 ':').
fn take_timestamp(b: &[u8], i: usize) -> Option<usize> {
    let d = |k: usize| b.get(k).is_some_and(|c| c.is_ascii_digit());
    // YYYY-MM-DD
    if !(d(i) && d(i + 1) && d(i + 2) && d(i + 3)) {
        return None;
    }
    if b.get(i + 4) != Some(&b'-') || b.get(i + 7) != Some(&b'-') {
        return None;
    }
    if !(d(i + 5) && d(i + 6) && d(i + 8) && d(i + 9)) {
        return None;
    }
    // separator [T ]
    match b.get(i + 10) {
        Some(&b'T') | Some(&b' ') => {}
        _ => return None,
    }
    // hh:mm:ss
    if !(d(i + 11) && d(i + 12)) || b.get(i + 13) != Some(&b':') {
        return None;
    }
    if !(d(i + 14) && d(i + 15)) || b.get(i + 16) != Some(&b':') {
        return None;
    }
    if !(d(i + 17) && d(i + 18)) {
        return None;
    }
    let mut j = i + 19;
    // optional fractional seconds .\d{1,9}
    if b.get(j) == Some(&b'.') {
        let mut k = j + 1;
        while b.get(k).is_some_and(|c| c.is_ascii_digit()) {
            k += 1;
        }
        if k > j + 1 {
            j = k;
        }
    }
    // optional zone: Z | ±hh(:?mm)?
    match b.get(j) {
        Some(&b'Z') => j += 1,
        Some(&b'+') | Some(&b'-') if d(j + 1) && d(j + 2) => {
            let mut k = j + 3;
            if b.get(k) == Some(&b':') && d(k + 1) && d(k + 2) {
                k += 3;
            } else if d(k) && d(k + 1) {
                k += 2;
            }
            j = k;
        }
        _ => {}
    }
    Some(j)
}

/// UUID: 8-4-4-4-12 hex with hyphens (§3.2 #2). Left/right word-boundary guarded.
fn take_uuid(b: &[u8], i: usize) -> Option<usize> {
    if i > 0 && is_word_byte(b[i - 1]) {
        return None;
    }
    let hex = |k: usize| b.get(k).is_some_and(|c| c.is_ascii_hexdigit());
    let group = |start: usize, n: usize| -> bool { (0..n).all(|o| hex(start + o)) };
    let mut j = i;
    for (idx, &n) in [8usize, 4, 4, 4, 12].iter().enumerate() {
        if !group(j, n) {
            return None;
        }
        j += n;
        if idx < 4 {
            if b.get(j) != Some(&b'-') {
                return None;
            }
            j += 1;
        }
    }
    // must not continue into another hex/word char (else it's a longer token)
    if b.get(j).is_some_and(|&c| is_word_byte(c) || c == b'-') {
        return None;
    }
    Some(j)
}

/// IPv4 with optional `:port` (§3.2 #5). All four octets required.
fn take_ipv4(b: &[u8], i: usize) -> Option<usize> {
    if i > 0 && (is_word_byte(b[i - 1]) || b[i - 1] == b'.') {
        return None;
    }
    let mut j = i;
    let take_octet = |b: &[u8], mut k: usize| -> Option<usize> {
        let s = k;
        while b.get(k).is_some_and(|c| c.is_ascii_digit()) && k - s < 3 {
            k += 1;
        }
        if k == s {
            None
        } else {
            Some(k)
        }
    };
    for octet in 0..4 {
        j = take_octet(b, j)?;
        if octet < 3 {
            if b.get(j) != Some(&b'.') {
                return None;
            }
            j += 1;
        }
    }
    // reject a 5th dotted group (not an IPv4)
    if b.get(j) == Some(&b'.') && b.get(j + 1).is_some_and(|c| c.is_ascii_digit()) {
        return None;
    }
    // optional :port
    if b.get(j) == Some(&b':') && b.get(j + 1).is_some_and(|c| c.is_ascii_digit()) {
        let mut k = j + 1;
        while b.get(k).is_some_and(|c| c.is_ascii_digit()) && k - (j + 1) < 5 {
            k += 1;
        }
        j = k;
    }
    if b.get(j).is_some_and(|&c| is_word_byte(c)) {
        return None;
    }
    Some(j)
}

/// Duration / size: `\d+(\.\d+)?(ns|us|ms|s|m|h|kb|mb|gb|B)` word-bounded (§3.2 #4).
fn take_duration(b: &[u8], i: usize) -> Option<usize> {
    if i > 0 && (is_word_byte(b[i - 1]) || b[i - 1] == b'.') {
        return None;
    }
    let mut j = i;
    while b.get(j).is_some_and(|c| c.is_ascii_digit()) {
        j += 1;
    }
    if j == i {
        return None;
    }
    // optional fractional part
    if b.get(j) == Some(&b'.') && b.get(j + 1).is_some_and(|c| c.is_ascii_digit()) {
        j += 1;
        while b.get(j).is_some_and(|c| c.is_ascii_digit()) {
            j += 1;
        }
    }
    // unit suffix (longest-match; case-sensitive set from the spec)
    let rest = &b[j..];
    const UNITS: [&[u8]; 12] = [
        b"ns", b"us", b"ms", b"kb", b"mb", b"gb", b"KB", b"MB", b"GB", b"s", b"m", b"h",
    ];
    let mut unit_len = 0usize;
    for u in UNITS {
        if rest.starts_with(u) && u.len() > unit_len {
            unit_len = u.len();
        }
    }
    // single-byte 'B' (bytes) only if not part of a longer unit already matched
    if unit_len == 0 && rest.first() == Some(&b'B') {
        unit_len = 1;
    }
    if unit_len == 0 {
        return None;
    }
    let end = j + unit_len;
    // must be word-bounded: next byte not alphanumeric/_ (so `500ms` yes, `500msg` no)
    if b.get(end).is_some_and(|&c| is_word_byte(c)) {
        return None;
    }
    Some(end)
}

/// Hex id: `0x[0-9a-fA-F]+` OR bare `[0-9a-fA-F]{6,}` containing >=1 a-f (§3.2 #3).
/// A purely-decimal run is left for `take_int`.
fn take_hex(b: &[u8], i: usize) -> Option<usize> {
    if i > 0 && is_word_byte(b[i - 1]) {
        return None;
    }
    // 0x-prefixed
    if b.get(i) == Some(&b'0') && matches!(b.get(i + 1), Some(&b'x') | Some(&b'X')) {
        let mut j = i + 2;
        while b.get(j).is_some_and(|c| c.is_ascii_hexdigit()) {
            j += 1;
        }
        if j > i + 2 {
            if b.get(j).is_some_and(|&c| is_word_byte(c)) {
                return None;
            }
            return Some(j);
        }
        return None;
    }
    // bare hex run: a token of hex digits containing at least one a-f/A-F
    // (so pure-digit ints are excluded and handled by take_int). Length floor
    // is >=4 (L4 fix 2026-07-02): real request/correlation ids are frequently
    // 4-8 hex chars (`req-1a2b`, short shas). The both-sides non-word guards
    // (entry guard on b[i-1] + the right guard below) keep this from chipping
    // hex-looking fragments out of a larger word, and the mandatory alpha keeps
    // it from swallowing short decimals. Fixture case1 used 8-char ids and
    // masked fine; a 4-char id (`1a2b`) slipped through at the old >=6 floor and
    // produced 0% compression on an otherwise-identical access-log template.
    let mut j = i;
    let mut has_alpha = false;
    while let Some(&c) = b.get(j) {
        if c.is_ascii_digit() {
            j += 1;
        } else if c.is_ascii_hexdigit() {
            has_alpha = true;
            j += 1;
        } else {
            break;
        }
    }
    if j - i >= 4 && has_alpha && !b.get(j).is_some_and(|&c| is_word_byte(c)) {
        return Some(j);
    }
    None
}

/// Standalone integer (§3.2 #7), applied LAST. Not adjacent to `.`/word char,
/// so it can't chip a version/float/id already handled.
fn take_int(b: &[u8], i: usize) -> Option<usize> {
    if !b.get(i).is_some_and(|c| c.is_ascii_digit()) {
        return None;
    }
    // left guard: preceding byte not a word char or '.'
    if i > 0 && (is_word_byte(b[i - 1]) || b[i - 1] == b'.') {
        return None;
    }
    let mut j = i;
    while b.get(j).is_some_and(|c| c.is_ascii_digit()) {
        j += 1;
    }
    // right guard: following byte not a word char or '.'
    if b.get(j).is_some_and(|&c| is_word_byte(c) || c == b'.') {
        return None;
    }
    Some(j)
}

/// Mask a `key=value` VALUE span starting at byte `i` (just after the `=`),
/// keeping the key (§3.2 #6). A quoted `"..."` value is consumed whole;
/// otherwise the value runs up to whitespace or a comma. Returns None when
/// there is no value (so a bare `=` stays literal).
fn take_kv_value(b: &[u8], i: usize) -> Option<usize> {
    match b.get(i) {
        None => None,
        Some(&b'"') => {
            let mut j = i + 1;
            while let Some(&c) = b.get(j) {
                j += 1;
                if c == b'"' {
                    break;
                }
            }
            Some(j)
        }
        Some(&c) if c == b' ' || c == b'\t' || c == b',' => None,
        Some(_) => {
            let mut j = i;
            while let Some(&c) = b.get(j) {
                if c == b' ' || c == b'\t' || c == b',' {
                    break;
                }
                j += 1;
            }
            Some(j)
        }
    }
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
///
/// FIDELITY NOTE (H-TEST stage, 2026-06-29): this is a *strict superset* of the
/// pares-agens `is_log_content`. The real detector only matches a level token
/// when it appears space-delimited *inside* a line (`" ERROR "`), so a line that
/// literally **begins** with the level word (`ERROR worker crashed`, `WARN ...`)
/// trims to `ERROR worker crashed`, does NOT contain `" ERROR "`, and was
/// mis-classified as prose. The orchestrator caught exactly this on a repeated
/// `ERROR`-prefixed log sample. Real, captured tool/CI logs routinely start with
/// the bare level word, so we additionally recognize a *line-leading* level
/// token (`is_level_prefixed`). This never false-positives on English prose
/// (sentences do not start with a bare `ERROR `/`WARN `/`INFO `/`DEBUG `/`TRACE `
/// followed by a space) and it still matches every input the real detector did.
/// The remaining genuinely-ambiguous case (repeated identical lines with NO
/// level token AND NO timestamp) stays prose — see the pinned
/// `detect_repeated_bare_lines_is_documented_limitation` test.
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
                // Faithful superset: a line that *starts* with a level token is
                // unambiguously a log line (the real detector already keys on
                // these exact words; it just required them mid-line).
                || is_level_prefixed(t)
                || (t.len() >= 19
                    && t.as_bytes().get(4) == Some(&b'-')
                    && t.as_bytes().get(7) == Some(&b'-')
                    && (t.as_bytes().get(10) == Some(&b'T') || t.as_bytes().get(10) == Some(&b' '))
                    && t.as_bytes().get(13) == Some(&b':'))
        })
        .count();
    log_line_count >= 2
}

/// True when a trimmed line *begins* with a canonical log-level token (the bare
/// word followed by a space, or a bracketed form). Used to extend the real
/// detector so line-leading levels (`ERROR ...`, `WARN ...`, `[ERROR] ...`) are
/// recognized — the gap the orchestrator caught on repeated `ERROR`-prefixed
/// logs. Deliberately requires the trailing space / bracket so it does not fire
/// on identifiers like `ERRORS` or `INFORMATION`.
fn is_level_prefixed(trimmed: &str) -> bool {
    const LEVELS: [&str; 5] = ["ERROR", "WARN", "INFO", "DEBUG", "TRACE"];
    for lvl in LEVELS {
        // Bare leading level: "ERROR something" / "WARN: something".
        if let Some(rest) = trimmed.strip_prefix(lvl) {
            match rest.chars().next() {
                Some(c) if c == ' ' || c == ':' || c == '\t' => return true,
                _ => {}
            }
        }
        // Bracketed leading level: "[ERROR] something".
        let bracketed = format!("[{lvl}]");
        if trimmed.starts_with(&bracketed) {
            return true;
        }
    }
    false
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

    // Compression ratio computed in REAL cl100k tokens (never a hardcoded ratio).
    fn ratio(original: &str, compressed: &str) -> f64 {
        let b = count_tokens(original);
        let c = count_tokens(compressed);
        if b == 0 {
            return 1.0;
        }
        c as f64 / b as f64
    }

    // ── unit-level building blocks ─────────────────────────────────────

    #[test]
    fn whitespace_collapse_shrinks() {
        let input = "a\n\n\n   b      c\t\t d";
        assert_eq!(collapse_whitespace(input), "a b c d");
    }

    #[test]
    fn token_count_is_real_cl100k() {
        // "hello world" is 2 tokens under cl100k_base (matches tiktoken).
        assert_eq!(count_tokens("hello world"), 2);
        assert_eq!(count_tokens(""), 0);
    }

    // ── PROSE: head+tail retained, middle elided, REAL tokens drop ─────────────
    // Fidelity vs pares-agens `compress_prose` (headroom_bridge.rs L266-301):
    // ≤6 sentences -> whitespace collapse; else keep first 3 + elision marker +
    // last 3. We assert head/tail sentinels survive, middle is gone, marker is
    // present, and the real cl100k token count strictly drops.
    #[test]
    fn prose_keeps_head_tail_elides_middle_and_drops_tokens() {
        let prose: String = (0..40)
            .map(|i| format!("Paragraph fact number {i} describes a distinct point about the subsystem. "))
            .collect();
        let out = compress_text(&prose, Some("prose"));
        assert!(out.len() < prose.len(), "prose should shrink in bytes");
        let r = ratio(&prose, &out);
        assert!(r < 1.0, "prose tokens must drop (ratio={r})");
        // head sentinel (sentence 0,1,2) and tail sentinel (37,38,39) survive
        assert!(out.contains("fact number 0 "), "head sentence 0 lost: {out}");
        assert!(out.contains("fact number 39 "), "tail sentence 39 lost: {out}");
        // a clearly-middle sentence is gone
        assert!(!out.contains("fact number 20 "), "middle sentence leaked: {out}");
        assert!(out.contains("elided"), "elision marker missing: {out}");
        // structural fidelity: first 3 + marker + last 3 == exactly head+tail kept
        assert!(out.contains("fact number 1 ") && out.contains("fact number 2 "));
        assert!(out.contains("fact number 37 ") && out.contains("fact number 38 "));
    }

    // ── CODE: signatures retained, bodies dropped, REAL tokens drop ────────────
    // Fidelity vs pares-agens `compress_code` (headroom_bridge.rs L310-342) +
    // `extract_signatures_heuristic` (headroom.rs L600-657): emit a
    // `// [headroom: <lang> body elided — N signature(s) kept]` header then the
    // signature lines only. We assert every signature survives, bulky body lines
    // are pruned, the header is present, and real tokens drop.
    #[test]
    fn code_keeps_signatures_drops_bodies_and_drops_tokens() {
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
        assert!(code.len() >= PER_MESSAGE_MIN_CHARS, "fixture under floor");
        let out = compress_text(code, Some("code"));
        assert!(out.len() < code.len(), "code should shrink: {out}");
        let r = ratio(code, &out);
        assert!(r < 1.0, "code tokens must drop (ratio={r})");
        // every signature retained
        assert!(out.contains("pub fn alpha"), "alpha sig lost: {out}");
        assert!(out.contains("pub fn beta"), "beta sig lost: {out}");
        assert!(out.contains("struct Foo"), "struct sig lost: {out}");
        assert!(out.contains("impl Foo"), "impl sig lost: {out}");
        assert!(out.contains("pub fn total"), "method sig lost: {out}");
        // header present
        assert!(out.contains("body elided"), "code header missing: {out}");
        // bulky body lines pruned
        assert!(!out.contains("acc += i"), "alpha body leaked: {out}");
        assert!(!out.contains("out.push(ch)"), "beta body leaked: {out}");
    }

    // ── LOG: consecutive-dup runs collapse to `line  [×N]`, distinct preserved ──
    // Fidelity vs pares-agens `compress_log` (headroom_bridge.rs L349-383): the
    // run-collapse marker format is `line` + `"  [×N]"` for runs > 1, singletons
    // pass through. We assert the marker format, distinct-line preservation, and
    // real token drop.
    #[test]
    fn log_collapses_runs_preserves_distinct_and_drops_tokens() {
        let mut log = String::new();
        for _ in 0..30 {
            log.push_str("2026-06-29 12:00:00 ERROR upstream connection refused\n");
        }
        log.push_str("2026-06-29 12:00:01 INFO recovery scheduled\n");
        for _ in 0..12 {
            log.push_str("2026-06-29 12:00:02 WARN retry backoff engaged\n");
        }
        let out = compress_text(&log, Some("log"));
        assert!(out.len() < log.len(), "log should shrink: {out}");
        let r = ratio(&log, &out);
        assert!(r < 1.0, "log tokens must drop (ratio={r})");
        // exact run-collapse marker format from the Path-2 algorithm (ASCII 'x',
        // template appended). Identical lines share a template so still collapse.
        assert!(out.contains("[x30 ~ "), "missing x30 run marker: {out}");
        assert!(out.contains("[x12 ~ "), "missing x12 run marker: {out}");
        // the singleton distinct line is preserved verbatim, not collapsed
        assert!(out.contains("INFO recovery scheduled"), "distinct line lost: {out}");
        assert!(!out.contains("INFO recovery scheduled  [x"), "singleton wrongly marked");
    }

    #[test]
    fn log_singletons_pass_through_unmarked() {
        // direct compress_log unit check of the marker boundary. `alpha`/`beta`/
        // `gamma` have no maskable tokens so their templates are the literals
        // themselves; the two adjacent `beta` lines collapse.
        let log = "alpha\nbeta\nbeta\ngamma\n";
        let out = compress_log(log).unwrap();
        // `beta` has no maskable field so template == line -> terse [xN] marker.
        assert!(out.contains("beta  [x2]"), "run not marked: {out}");
        assert!(out.starts_with("alpha"), "alpha lost: {out}");
        assert!(out.contains("gamma"), "gamma lost: {out}");
        assert!(!out.contains("alpha  [x"), "alpha wrongly marked: {out}");
    }

    // ── JSON/OTHER: whitespace squeeze, tokens drop-or-equal ─────────────────
    // Fidelity vs `collapse_whitespace` (headroom_bridge.rs L406-422).
    #[test]
    fn json_whitespace_squeeze_does_not_grow_tokens() {
        // Fixture deliberately over the 200-char floor so the strategy actually runs.
        let json = "{\n    \"name\":     \"widget-assembly-component\",\n    \"description\": \"a fairly long descriptive label used to pad this object well past the floor\",\n    \"values\": [\n        100,\n        200,\n        300,\n        400\n    ],\n    \"nested\": {\n        \"deep\":   true,\n        \"owner\":  \"subsystem-alpha\",\n        \"label\":  \"another reasonably long descriptive string value here\"\n    }\n}";
        assert!(json.len() >= PER_MESSAGE_MIN_CHARS, "json fixture under floor");
        let out = compress_text(json, Some("json"));
        assert!(out.len() <= json.len(), "json must not grow in bytes");
        let r = ratio(json, &out);
        assert!(r <= 1.0, "json tokens must not grow (ratio={r})");
        // whitespace actually squeezed (byte shrink on this padded fixture)
        assert!(out.len() < json.len(), "padded json should whitespace-shrink: {out}");
        // keys preserved (whitespace-only squeeze, no key loss)
        assert!(out.contains("\"name\"") && out.contains("\"nested\""), "key lost: {out}");
    }

    // ── NET-SAVINGS GUARD: incompressible/short input returned UNCHANGED ──────
    // This is the per-message contract from `compress_one` (headroom_bridge.rs
    // L231-258): output is only accepted if strictly smaller AND non-empty,
    // else the original is returned verbatim. Output can never GROW.
    #[test]
    fn net_savings_guard_short_input_unchanged() {
        let s = "too short to bother compressing";
        assert_eq!(compress_text(s, None), s, "short input must pass through");
    }

    #[test]
    fn net_savings_guard_incompressible_never_grows() {
        // A dense, already-tight blob over the 200-char floor with no compressible
        // structure (no dup runs, no signatures, single whitespace runs). The
        // guard must return it UNCHANGED rather than emit a larger rewrite.
        let tight: String = std::iter::repeat_n("x7Qk", 80)
            .collect::<Vec<_>>()
            .join(" "); // "x7Qk x7Qk ..." ~ 399 chars, single spaces, no structure
        assert!(tight.len() >= PER_MESSAGE_MIN_CHARS, "guard fixture under floor");
        let out = compress_text(&tight, None);
        assert!(out.len() <= tight.len(), "guard: output grew in bytes");
        let r = ratio(&tight, &out);
        assert!(r <= 1.0, "guard: output grew in tokens (ratio={r})");
        // when nothing beats the original, the contract returns it verbatim
        assert_eq!(out, tight, "incompressible input must be returned unchanged");
    }

    #[test]
    fn net_savings_guard_holds_across_all_autodetected_types() {
        // Auto-detect path (content_type = None) must also never grow any type.
        let json_s =
            "{\"a\":1,\"b\":2,\"c\":[1,2,3,4,5,6,7,8,9,10],\"d\":\"some value here padding the length\"}"
                .to_string();
        let prose_s = "This is a normal English sentence used as prose padding. ".repeat(8);
        for s in [json_s, prose_s] {
            let out = compress_text(&s, None);
            assert!(
                count_tokens(&out) <= count_tokens(&s),
                "auto-detect grew tokens for sample: {s}"
            );
        }
    }

    // ── DETECTOR ACCURACY ───────────────────────────────────────────
    // Canonical samples of each content type must classify correctly.
    #[test]
    fn detector_classifies_canonical_samples() {
        // json
        assert_eq!(detect_content_type("{\"a\":1}"), "json");
        assert_eq!(detect_content_type("[1,2,3]"), "json");
        // prose
        assert_eq!(
            detect_content_type(
                "This is an ordinary English paragraph that simply describes a situation in words."
            ),
            "prose"
        );
        // code
        let code = "pub fn run(x: i32) -> i32 {\n    let y = x + 1;\n    y * 2\n}\nfn helper() {\n    let z = 3;\n}\n";
        assert_eq!(detect_content_type(code), "code");
        // error (>=2 indicators: panic line + stack frames)
        let err = "thread 'main' panicked at src/lib.rs:10:5:\nindex out of bounds\n  at foo (src/a.rs:1)\n  at bar (src/b.rs:2)\n";
        assert_eq!(detect_content_type(err), "error");
    }

    // The orchestrator's canonical catch: a repeated LOG sample whose lines
    // *start* with the level word (no leading space) was mis-classified as prose
    // by the real pares-agens heuristic (which only matched " ERROR " mid-line).
    // The H-TEST fix (is_level_prefixed) makes this classify as `log`.
    #[test]
    fn detector_repeated_level_prefixed_log_is_log_not_prose() {
        let log = "ERROR worker crashed: connection timeout\nERROR worker crashed: connection timeout\nERROR worker crashed: connection timeout\n";
        // sanity: this is the exact shape the orchestrator flagged
        assert_eq!(
            detect_content_type(log),
            "log",
            "line-leading-level log must detect as log (orchestrator regression)"
        );
        // and detection drives correct routing end-to-end (real token drop)
        let big = log.repeat(12); // push over the 200-char floor with many dup runs
        let out = compress_text(&big, None); // None = auto-detect
        assert!(out.contains("[x"), "auto-detected log did not run-collapse: {out}");
        assert!(
            count_tokens(&out) < count_tokens(&big),
            "auto-detected log did not save tokens"
        );
    }

    #[test]
    fn detector_bracketed_and_warn_prefixed_logs_detect_as_log() {
        let bracketed = "[ERROR] disk full on /var\n[ERROR] disk full on /var\n[WARN] retrying write\n";
        assert_eq!(detect_content_type(bracketed), "log", "[LEVEL]-prefixed must be log");
        let warn = "WARN: cache miss for key abc\nWARN: cache miss for key abc\nINFO ready\n";
        assert_eq!(detect_content_type(warn), "log", "WARN-prefixed must be log");
    }

    #[test]
    fn detector_level_prefix_does_not_false_positive_on_prose() {
        // Guard the superset: prose words that merely START with the letters of a
        // level token (ERRORS, INFORMATION) must NOT trip log detection.
        let prose = "ERRORS in judgment are common.\nINFORMATION wants to be free.\nWARNING signs were ignored.\n";
        assert_ne!(
            detect_content_type(prose),
            "log",
            "is_level_prefixed false-positived on prose: {prose}"
        );
    }

    // DOCUMENTED LIMITATION (pinned, not hidden): repeated identical lines with
    // NO level token and NO timestamp are structurally indistinguishable from
    // repeated prose. The real pares-agens detector has the SAME gap, and any
    // heuristic aggressive enough to catch this would mis-classify genuine
    // prose. So it stays `prose`. CRITICAL: compression is still CORRECT when the
    // caller pins `Some("log")` (the TS seam can), and even under auto-detect the
    // prose strategy still SAVES tokens — it just does not use run-collapse.
    // This test pins the current behavior so any future change is intentional.
    #[test]
    fn detect_repeated_bare_lines_is_documented_limitation() {
        let bare = "Connection refused to upstream host\nConnection refused to upstream host\nConnection refused to upstream host\n";
        // Pinned faithful behavior: classified as prose (NOT log). Documented gap.
        assert_eq!(
            detect_content_type(bare),
            "prose",
            "behavior changed: bare repeated lines now detect differently — update H-TEST-NOTES"
        );
        // Honesty check: explicitly pinning `log` still compresses correctly,
        // so the limitation is detection-only, never a compression failure.
        let big = bare.repeat(20);
        let pinned = compress_text(&big, Some("log"));
        assert!(pinned.contains("[x"), "explicit log type must still run-collapse: {pinned}");
        assert!(
            count_tokens(&pinned) < count_tokens(&big),
            "explicit log type must save tokens"
        );
    }

    // DOCUMENTED LIMITATION (pinned, not hidden): a `[`-prefixed *timestamp* log
    // (e.g. `[2026-06-29T12:00:00Z] ERROR ...`) opens with `[`, so the JSON
    // branch claims it before the log branch is reached. Unlike a `[LEVEL]`
    // prefix (which we safely carve out, since it is never valid JSON), a
    // `[<timestamp>]` prefix is syntactically a plausible JSON-array opener, so
    // routing it to log would risk reclassifying real JSON arrays. We therefore
    // leave it as the faithful pares-agens behavior (json) and pin it here.
    // Bracketed-ISO-timestamp log lines (`[2026-...] LEVEL msg`) now AUTO-detect
    // as `log` and auto-collapse. Previously the leading `[` misrouted them to
    // the JSON branch (documented gap); the `starts_like_bracketed_log` guard was
    // extended to recognize a leading `[YYYY-MM-DD` date skeleton. (Fixed 2026-07-02.)
    #[test]
    fn detect_bracketed_timestamp_log_autodetects_as_log_and_collapses() {
        let ts_log = "[2026-06-29T12:00:00Z] ERROR upstream refused\n[2026-06-29T12:00:00Z] ERROR upstream refused\n[2026-06-29T12:00:01Z] INFO retry\n";
        // Corrected behavior: leading `[YYYY-MM-DD` date skeleton is recognized as
        // a bracketed log prefix, so detection is `log` (NOT `json`).
        assert_eq!(
            detect_content_type(ts_log),
            "log",
            "bracketed-timestamp log must auto-detect as log (guard recognizes [YYYY-MM-DD)"
        );
        // AUTO path (no forced type) must run-collapse + save tokens, losslessly.
        let big = ts_log.repeat(20);
        let auto = compress_text(&big, None);
        assert!(auto.contains("[x"), "auto-detected log must run-collapse: {auto}");
        assert!(
            count_tokens(&auto) < count_tokens(&big),
            "auto-detected log must save tokens"
        );
        // A real JSON array must still classify as json (guard never matches JSON).
        assert_eq!(detect_content_type("[1, 2, 3]"), "json");
        assert_eq!(detect_content_type("[{\"a\":1}]"), "json");
    }

    // ════════════════════════════════════════════════════════════════════
    // H-QA STAGE: ADVERSARIAL / HARDENING TESTS (added 2026-06-29).
    // These exercise the breakage the happy-path TEST-stage tests miss:
    // boundary/degenerate inputs, mixed/adversarial content, idempotency +
    // determinism, exact-count structure integrity, and token fidelity.
    // Contract under all of them: NEVER panic, NEVER grow tokens (net-savings
    // guard), NEVER corrupt structure, detector always returns a known label.
    // ════════════════════════════════════════════════════════════════════

    /// All content-type labels the detector is allowed to emit.
    const KNOWN_TYPES: [&str; 5] = ["json", "log", "code", "error", "prose"];

    /// Core safety contract for ANY input, under auto-detect AND every forced
    /// type: compress_text must not panic, must not grow tokens, count_tokens
    /// must be finite/sane (usize is always >=0), detect must return a known
    /// label. Returns the auto-detected output for further inspection.
    fn assert_safe(input: &str) -> String {
        // detector always yields a known label
        let dt = detect_content_type(input);
        assert!(
            KNOWN_TYPES.contains(&dt),
            "detect_content_type returned unknown label {dt:?} for {input:?}"
        );
        let base = count_tokens(input);
        // auto-detect path
        let auto = compress_text(input, None);
        assert!(
            count_tokens(&auto) <= base,
            "NET-SAVINGS GUARD VIOLATED (auto): base={base} comp={} input={input:?}",
            count_tokens(&auto)
        );
        // every forced type must also be safe (caller-pinned route)
        for t in KNOWN_TYPES {
            let forced = compress_text(input, Some(t));
            assert!(
                count_tokens(&forced) <= base,
                "NET-SAVINGS GUARD VIOLATED (forced={t}): base={base} comp={} input={input:?}",
                count_tokens(&forced)
            );
        }
        auto
    }

    // ── 1. BOUNDARY / DEGENERATE INPUTS ────────────────────────────────────

    #[test]
    fn qa_boundary_empty_and_tiny_inputs_are_safe() {
        // empty, single char, single line, all-whitespace, only-newlines.
        for s in [
            "",
            "x",
            "just one single line of text with no terminator",
            "   \t   \n   \t  \n    ",
            "\n\n\n\n\n\n\n\n",
        ] {
            assert_safe(s);
        }
        // sub-floor inputs are returned verbatim (the floor short-circuit).
        assert_eq!(compress_text("", None), "");
        assert_eq!(compress_text("x", None), "x");
        // count_tokens on degenerate inputs is finite/correct.
        assert_eq!(count_tokens(""), 0);
        assert_eq!(count_tokens("\n"), 1);
    }

    #[test]
    fn qa_boundary_extremely_long_single_line_no_sentence_breaks_is_safe() {
        // ~20k chars, a single "line", no sentence-ending punctuation. Forces
        // the prose splitter to treat it as ~1 sentence; must never grow or panic.
        let blob = "word ".repeat(4000);
        let auto = assert_safe(&blob);
        // <=6 "sentences" -> whitespace-collapse path; never an elision marker,
        // never larger.
        assert!(!auto.contains("elided"), "single-line blob wrongly elided");
        // explicit prose on the same blob is still guarded.
        let prose = compress_text(&blob, Some("prose"));
        assert!(count_tokens(&prose) <= count_tokens(&blob));
    }

    #[test]
    fn qa_boundary_unicode_emoji_cjk_is_safe_and_byte_boundary_clean() {
        // Multi-byte graphemes must never cause a panic (no byte-index slicing
        // into the middle of a codepoint) and must never grow tokens.
        let u = "日本語のテキスト 🚀🔥💯 émojis café résumé 中文字符 한국어 ".repeat(40);
        let auto = assert_safe(&u);
        // the output is valid UTF-8 by construction (String), and not larger.
        assert!(count_tokens(&auto) <= count_tokens(&u));
        // a single emoji / CJK char under the floor passes through untouched.
        assert_eq!(compress_text("🚀", None), "🚀");
        assert_eq!(compress_text("中", None), "中");
    }

    #[test]
    fn qa_boundary_token_dense_blob_never_grows() {
        // A high-entropy, structureless blob over the floor: no dup runs, no
        // signatures, no sentence breaks. The guard must return it unchanged
        // (or smaller) under every route — it must NEVER emit a larger rewrite.
        let blob = "a1B2c3D4e5F6g7H8".repeat(200); // 3200 chars, single token-dense run
        let base = count_tokens(&blob);
        for t in [None, Some("prose"), Some("code"), Some("log"), Some("json"), Some("error")] {
            let out = compress_text(&blob, t);
            assert!(
                count_tokens(&out) <= base,
                "token-dense blob grew under {t:?}: {base} -> {}",
                count_tokens(&out)
            );
        }
    }

    // ── 2. MIXED / ADVERSARIAL CONTENT ─────────────────────────────────────

    #[test]
    fn qa_mixed_prose_with_embedded_code_fence_is_safe() {
        let mixed = format!(
            "Here is some explanatory prose about the system. It describes a problem in plain words.\n```rust\nfn handler() {{\n    let x = 1;\n    x + 1\n}}\n```\n{}",
            "And then the prose continues after the fence with more discussion of the matter at hand. ".repeat(3)
        );
        assert_safe(&mixed);
    }

    #[test]
    fn qa_mixed_log_with_interleaved_prose_lines_is_safe() {
        let mixed = "INFO starting service alpha\nINFO starting service alpha\nThis is an interjected prose sentence the operator left in the log file by hand.\nERROR connection refused to db\nERROR connection refused to db\nERROR connection refused to db\n";
        assert_safe(mixed);
    }

    #[test]
    fn qa_mixed_json_with_escaped_newlines_is_safe_and_keeps_keys() {
        // Escaped \n inside JSON string values must not be treated as real line
        // breaks in a way that corrupts the object; whitespace squeeze only.
        let json = format!(
            "{{\"note\": \"line one\\nline two\\nline three\", \"tags\": [\"a\", \"b\", \"c\"], \"padding\": \"{}\"}}",
            "x".repeat(220)
        );
        let out = compress_text(&json, None);
        assert!(count_tokens(&out) <= count_tokens(&json), "json grew");
        assert!(out.contains("\"note\"") && out.contains("\"tags\""), "json key lost: {out}");
        // the escaped sequence is preserved (not expanded into real newlines).
        assert!(out.contains("line one\\nline two"), "escaped newline corrupted: {out}");
    }

    #[test]
    fn qa_mixed_all_unique_log_lines_does_not_claim_collapse() {
        // A "log" where every line is STRUCTURALLY unique (different literal
        // words between fields, not merely a different masked integer): there is
        // no shared template, so template-run-collapse must NOT fire (no `[xN`
        // marker) and the result must stay safe. This is the Path-2 over-masking
        // guard — distinct message shapes must never be flattened into one run.
        const WORDS: [&str; 8] = [
            "alpha", "bravo", "charlie", "delta", "echo", "foxtrot", "golf", "hotel",
        ];
        let mut log = String::new();
        for i in 0..40 {
            // rotate a distinct verb+noun so each line's *template* differs
            let verb = WORDS[i % WORDS.len()];
            let noun = WORDS[(i / WORDS.len()) % WORDS.len()];
            log.push_str(&format!(
                "2026-06-29 12:00:{:02} ERROR {verb} subsystem {noun} reported anomaly\n",
                i % 60
            ));
        }
        let out = compress_text(&log, Some("log"));
        assert!(
            !out.contains("[x"),
            "claimed a run-collapse on structurally-unique log lines: {out}"
        );
        assert!(count_tokens(&out) <= count_tokens(&log), "all-unique log grew");
        // every distinct event survives (lossless — nothing collapsed away).
        for i in 0..40 {
            let verb = WORDS[i % WORDS.len()];
            let noun = WORDS[(i / WORDS.len()) % WORDS.len()];
            assert!(
                out.contains(&format!("{verb} subsystem {noun} reported anomaly")),
                "lost event {i}: {out}"
            );
        }
    }

    #[test]
    fn qa_path2_same_template_varying_field_collapses_keeps_first_raw() {
        // Path-2 headline: lines that differ ONLY in masked fields (here the
        // integer id) DO share a template and collapse to one run, with the
        // FIRST raw line kept verbatim. This is the exact-match code's blind
        // spot that Path-2 fixes. Contrast with the structurally-unique test
        // above (which must NOT collapse).
        let mut log = String::new();
        for i in 0..40 {
            log.push_str(&format!(
                "2026-06-29 12:00:00 ERROR unique event number {i} occurred\n"
            ));
        }
        let out = compress_text(&log, Some("log"));
        // one run of 40 -> a single [x40 ~ ...] marker
        assert!(out.contains("[x40 ~ "), "same-template run did not collapse: {out}");
        // the FIRST raw line survives verbatim as the representative
        assert!(
            out.starts_with("2026-06-29 12:00:00 ERROR unique event number 0 occurred"),
            "first raw line not kept verbatim: {out}"
        );
        // real token savings + net-savings guard honored
        assert!(count_tokens(&out) < count_tokens(&log), "Path-2 collapse saved no tokens");
    }

    #[test]
    fn qa_mixed_prose_boundary_1_2_3_sentences_no_dup_no_drop() {
        // At <=6 sentences the prose path is whitespace-collapse only: it must
        // NOT insert an elision marker and must NOT duplicate or drop the unique
        // sentence markers. Tests the head+tail window boundary precisely.
        for n in 1..=3usize {
            let mut body = String::new();
            for i in 0..n {
                body.push_str(&format!("Sentence {i} carries a uniqueZ{i} distinct marker. "));
            }
            // pad over the floor WITHOUT re-introducing the watched markers.
            while body.len() < 260 {
                body.push_str("Extra neutral filler clause keeps the body above the floor. ");
            }
            let out = compress_text(&body, Some("prose"));
            assert!(
                !out.contains("elided"),
                "unexpected elision at {n} sentence(s): {out}"
            );
            for i in 0..n {
                let marker = format!("uniqueZ{i}");
                let count = out.matches(&marker).count();
                assert_eq!(
                    count, 1,
                    "marker {marker} should appear exactly once, appeared {count}x: {out}"
                );
            }
        }
    }

    // ── 3. IDEMPOTENCY / DETERMINISM ───────────────────────────────────────

    #[test]
    fn qa_determinism_same_input_same_output() {
        // Same input twice -> byte-identical output (no nondeterminism from
        // hashing, iteration order, etc.) across all real strategies.
        let cases: [(&str, String); 3] = [
            ("prose", "Distinct prose sentence number A about subsystems. ".repeat(40)),
            (
                "code",
                "pub fn one(x: i32) -> i32 {\n    let y = x + 1;\n    y\n}\npub fn two(z: i32) -> i32 {\n    let w = z - 1;\n    w\n}\n".repeat(3),
            ),
            ("log", {
                let mut s = String::new();
                for _ in 0..30 { s.push_str("ERROR upstream connection refused mid run\n"); }
                s.push_str("INFO recovered ok\n");
                for _ in 0..10 { s.push_str("WARN retry backoff\n"); }
                s
            }),
        ];
        for (t, inp) in &cases {
            let a = compress_text(inp, Some(t));
            let b = compress_text(inp, Some(t));
            assert_eq!(a, b, "non-deterministic output for type {t}");
        }
    }

    #[test]
    fn qa_idempotency_second_pass_never_grows_and_reaches_fixpoint() {
        // compress_text(compress_text(x)) must not corrupt or further-mangle
        // beyond the guard: pass 2 never grows tokens, and a fixpoint is reached
        // by pass 3 (pass2 == pass3) — the transform stabilizes, no oscillation.
        let cases: [(&str, String); 3] = [
            ("prose", "Distinct prose sentence number A about subsystems. ".repeat(40)),
            (
                "code",
                "pub fn one(x: i32) -> i32 {\n    let y = x + 1;\n    y\n}\npub fn two(z: i32) -> i32 {\n    let w = z - 1;\n    w\n}\n".repeat(3),
            ),
            ("log", {
                let mut s = String::new();
                for _ in 0..30 { s.push_str("ERROR upstream connection refused mid run\n"); }
                s.push_str("INFO recovered ok\n");
                for _ in 0..10 { s.push_str("WARN retry backoff\n"); }
                s
            }),
        ];
        for (t, inp) in &cases {
            let p1 = compress_text(inp, Some(t));
            let p2 = compress_text(&p1, Some(t));
            assert!(
                count_tokens(&p2) <= count_tokens(&p1),
                "idempotency {t}: pass2 grew tokens {} -> {}",
                count_tokens(&p1),
                count_tokens(&p2)
            );
            let p3 = compress_text(&p2, Some(t));
            assert_eq!(p2, p3, "idempotency {t}: no fixpoint reached by pass 3");
        }
    }

    // ── 4. STRUCTURE INTEGRITY (exact counts on crafted inputs) ────────────

    #[test]
    fn qa_structure_code_every_signature_present_bodies_dropped() {
        // Crafted code where the exact set of signatures is known: all four must
        // survive verbatim, and the bulky body lines must be gone.
        let code = "pub fn sig_alpha(a: i32) -> i32 {\n    let body_alpha = a * 2;\n    body_alpha + 1\n}\npub fn sig_beta(b: i32) -> i32 {\n    let body_beta = b - 3;\n    body_beta\n}\nstruct SigGamma {\n    f1: i32,\n    f2: i32,\n}\nimpl SigGamma {\n    pub fn sig_method_delta(&self) -> i32 {\n        self.f1 + self.f2\n    }\n}\n";
        let out = compress_text(code, Some("code"));
        for s in ["sig_alpha", "sig_beta", "SigGamma", "sig_method_delta"] {
            assert!(out.contains(s), "dropped signature {s}: {out}");
        }
        assert!(!out.contains("body_alpha = a"), "alpha body leaked: {out}");
        assert!(!out.contains("body_beta = b"), "beta body leaked: {out}");
    }

    #[test]
    fn qa_structure_log_run_counts_are_exactly_accurate() {
        // A crafted log with a run of EXACTLY 7 then EXACTLY 4, then 1 distinct
        // line. The `[xN ~ ...]` markers must match the real run lengths
        // precisely, with no off-by-one and no spurious counts.
        let mut log = String::new();
        for _ in 0..7 { log.push_str("ERROR exact run of seven identical lines here\n"); }
        for _ in 0..4 { log.push_str("WARN exact run of four identical lines here\n"); }
        log.push_str("INFO one distinct trailing line\n");
        let out = compress_text(&log, Some("log"));
        // these lines have no maskable field (template == line) -> terse [xN].
        assert!(out.contains("[x7]"), "expected [x7]: {out}");
        assert!(out.contains("[x4]"), "expected [x4]: {out}");
        // the distinct line is preserved and NOT marked as a run.
        assert!(out.contains("INFO one distinct trailing line"), "distinct line lost: {out}");
        assert!(
            !out.contains("distinct trailing line  [x"),
            "distinct singleton wrongly marked: {out}"
        );
        // no off-by-one / spurious run counts.
        for spurious in ["[x6]", "[x8]", "[x3]", "[x5]"] {
            assert!(!out.contains(spurious), "spurious run count {spurious}: {out}");
        }
    }

    #[test]
    fn qa_structure_prose_elision_count_matches_real_elided_count() {
        // 40 distinct sentences -> head 3 + tail 3 kept -> 34 elided. The marker
        // must say EXACTLY "34 sentences elided", appear EXACTLY once, keep the
        // exact head/tail sentences, and drop a known middle sentence.
        let mut prose = String::new();
        for i in 0..40 {
            prose.push_str(&format!("Distinct prose unit {i} with numeric tokenZ {i}. "));
        }
        let out = compress_text(&prose, Some("prose"));
        assert!(out.contains("34 sentences elided"), "wrong elided count: {out}");
        assert_eq!(
            out.matches("sentences elided").count(),
            1,
            "elision marker must appear exactly once: {out}"
        );
        for i in [0, 1, 2, 37, 38, 39] {
            assert!(out.contains(&format!("tokenZ {i}.")), "lost head/tail sentence {i}: {out}");
        }
        assert!(!out.contains("tokenZ 20."), "middle sentence 20 leaked: {out}");
    }

    // ── 5. TOKEN-COUNT FIDELITY (known cl100k expectations) ─────────────────

    #[test]
    fn qa_token_count_matches_known_cl100k_values() {
        // Spot-check count_tokens against fixed, tiktoken-verified cl100k_base
        // counts — proves it is the REAL tokenizer, not an approximation.
        assert_eq!(count_tokens(""), 0, "empty");
        assert_eq!(count_tokens("hello world"), 2, "hello world");
        assert_eq!(count_tokens("\n"), 1, "newline");
        assert_eq!(
            count_tokens("The quick brown fox jumps over the lazy dog"),
            9,
            "pangram"
        );
    }

    // ═══════════════════════════════════════════════════════════════
    // PATH-2 FIXTURE GATE (design §4). The 8 gate fixtures from
    // `.openclaw/tmp/headroom-fixtures/` are inlined here verbatim (unit tests
    // must not depend on an external absolute path). Each POSITIVE case must
    // collapse to the design's expected run-count, carry a `[x` marker, and keep
    // its first raw line verbatim. Each NEGATIVE case must be byte-identical to
    // the input (trimmed) with NO `[x` marker — the over-masking guard.
    //
    // Ambiguity resolved: design §4.1's worked example shows `handler=GET` /
    // `host=<IP>` kept, but the §3.4 pseudocode masks ALL `key=value` values to
    // `<KV>` (keeping the key). We follow §3.4 (the authoritative Rust spec).
    // The collapse RUN-COUNTS are invariant to this choice — only the marker's
    // template text differs — so every count below matches design §4.
    // ═══════════════════════════════════════════════════════════════

    const FX_CASE1: &str = concat!(
        "2026-07-02T10:15:03.123Z INFO  [req-a1b2c3d4] handler=GET /api/users status=200 dur=12ms\n",
        "2026-07-02T10:15:03.451Z INFO  [req-e5f6a7b8] handler=GET /api/users status=200 dur=9ms\n",
        "2026-07-02T10:15:03.998Z INFO  [req-9c0d1e2f] handler=GET /api/users status=200 dur=14ms\n",
        "2026-07-02T10:15:04.203Z INFO  [req-3a4b5c6d] handler=GET /api/users status=200 dur=11ms\n",
        "2026-07-02T10:15:04.774Z INFO  [req-7e8f9a0b] handler=GET /api/users status=200 dur=10ms\n",
        "2026-07-02T10:15:05.012Z INFO  [req-1c2d3e4f] handler=GET /api/users status=200 dur=13ms\n",
    );

    // L4 fix fixture: SHORT (4-char) hex request ids. Same access-log template as
    // case1 but with `req-<4hex>` instead of 8 — the width that slipped through the
    // old >=6 hex floor and produced 0% compression. Must now collapse 6 -> 1.
    const FX_CASE7: &str = concat!(
        "2026-07-02T10:15:03.123Z INFO  [req-1a2b] handler=GET /api/users status=200 dur=12ms\n",
        "2026-07-02T10:15:03.451Z INFO  [req-1a2c] handler=GET /api/users status=200 dur=9ms\n",
        "2026-07-02T10:15:03.998Z INFO  [req-1a2d] handler=GET /api/users status=200 dur=14ms\n",
        "2026-07-02T10:15:04.203Z INFO  [req-1a2e] handler=GET /api/users status=200 dur=11ms\n",
        "2026-07-02T10:15:04.774Z INFO  [req-1a2f] handler=GET /api/users status=200 dur=10ms\n",
        "2026-07-02T10:15:05.012Z INFO  [req-1a30] handler=GET /api/users status=200 dur=13ms\n",
    );

    const FX_CASE2: &str = concat!(
        "2026-07-02T10:15:03.123Z INFO  worker=3 processed batch id=48213 items=500\n",
        "2026-07-02T10:15:03.451Z INFO  worker=1 processed batch id=48214 items=500\n",
        "2026-07-02T10:15:03.998Z WARN  worker=3 retry batch id=48215 attempt=1\n",
        "2026-07-02T10:15:04.203Z INFO  worker=2 processed batch id=48216 items=500\n",
        "2026-07-02T10:15:04.774Z INFO  worker=1 processed batch id=48217 items=500\n",
        "2026-07-02T10:15:05.012Z ERROR worker=2 failed batch id=48218 err=timeout\n",
    );

    const FX_CASE3: &str = concat!(
        "[2026-07-02 10:15:03] DEBUG conn 0x7f3a1c00 opened from 10.0.0.14:52001\n",
        "[2026-07-02 10:15:03] DEBUG conn 0x7f3a2d10 opened from 10.0.0.22:52002\n",
        "[2026-07-02 10:15:04] DEBUG conn 0x7f3a3e20 opened from 10.0.0.9:52003\n",
        "[2026-07-02 10:15:04] DEBUG conn 0x7f3a4f30 opened from 10.0.0.31:52004\n",
        "[2026-07-02 10:15:05] DEBUG conn 0x7f3a5a40 opened from 10.0.0.7:52005\n",
    );

    const FX_CASE4: &str = concat!(
        "INFO 2026-07-02T10:15:03.001Z request 550e8400-e29b-41d4-a716-446655440000 completed in 45ms\n",
        "INFO 2026-07-02T10:15:03.502Z request 6ba7b810-9dad-11d1-80b4-00c04fd430c8 completed in 52ms\n",
        "INFO 2026-07-02T10:15:04.003Z request 6ba7b811-9dad-11d1-80b4-00c04fd430c8 completed in 38ms\n",
        "INFO 2026-07-02T10:15:04.504Z request 6ba7b812-9dad-11d1-80b4-00c04fd430c8 completed in 61ms\n",
        "INFO 2026-07-02T10:15:05.005Z request 6ba7b814-9dad-11d1-80b4-00c04fd430c8 completed in 47ms\n",
    );

    const FX_CASE5: &str = concat!(
        "2026-07-02T10:15:03.123Z INFO  cache hit key=user:1001 ttl=300s\n",
        "2026-07-02T10:15:03.201Z INFO  cache hit key=user:1002 ttl=300s\n",
        "2026-07-02T10:15:03.288Z INFO  cache miss key=user:1003 -> fetch db\n",
        "2026-07-02T10:15:03.377Z INFO  cache hit key=user:1004 ttl=300s\n",
        "2026-07-02T10:15:03.466Z INFO  cache hit key=user:1005 ttl=300s\n",
        "2026-07-02T10:15:03.555Z INFO  cache miss key=user:1006 -> fetch db\n",
        "2026-07-02T10:15:03.644Z INFO  cache hit key=user:1007 ttl=300s\n",
    );

    const FX_CASE6: &str = concat!(
        "2026-07-02T10:15:03.123Z ERROR db connection failed host=10.0.0.5 port=5432 err=\"connection refused\"\n",
        "2026-07-02T10:15:03.623Z ERROR db connection failed host=10.0.0.5 port=5432 err=\"connection refused\"\n",
        "2026-07-02T10:15:04.123Z ERROR db connection failed host=10.0.0.5 port=5432 err=\"connection refused\"\n",
        "2026-07-02T10:15:04.623Z ERROR db connection failed host=10.0.0.5 port=5432 err=\"connection refused\"\n",
        "2026-07-02T10:15:05.123Z ERROR db connection failed host=10.0.0.5 port=5432 err=\"connection refused\"\n",
    );

    const FX_NEG1: &str = concat!(
        "2026-07-02T10:15:03.123Z INFO  starting service version=2.4.1 pid=8842\n",
        "2026-07-02T10:15:03.140Z INFO  loaded config from /etc/app/config.yaml sections=12\n",
        "2026-07-02T10:15:03.155Z WARN  deprecated flag --legacy-mode will be removed in 3.0\n",
        "2026-07-02T10:15:03.170Z INFO  bound listener addr=0.0.0.0:8080 tls=true\n",
        "2026-07-02T10:15:03.185Z ERROR failed to open plugin dir /opt/app/plugins: no such file or directory\n",
        "2026-07-02T10:15:03.200Z INFO  service ready in 77ms\n",
    );

    const FX_NEG2: &str = concat!(
        "2026-07-02T10:15:03.123Z INFO  user=alice action=login result=ok\n",
        "2026-07-02T10:15:03.451Z INFO  user=bob action=purchase amount=49.99 currency=USD\n",
        "2026-07-02T10:15:03.998Z INFO  user=carol action=logout session_dur=3600s\n",
        "2026-07-02T10:15:04.203Z INFO  user=dave action=login result=fail reason=bad_password\n",
        "2026-07-02T10:15:04.774Z INFO  user=erin action=update_profile fields=email,phone\n",
    );

    /// Assert a POSITIVE fixture collapses to `expected_lines`, shows a `[x`
    /// marker, and keeps its first RAW input line verbatim as line 1.
    fn assert_positive(name: &str, input: &str, expected_lines: usize) {
        let out = compress_log(input).expect("compress_log returned None");
        let n = out.lines().count();
        assert_eq!(
            n, expected_lines,
            "{name}: expected {expected_lines} output lines, got {n}\n---\n{out}\n---"
        );
        assert!(out.contains("[x"), "{name}: missing [x run marker:\n{out}");
        // The representative is the first RAW line verbatim, with the marker
        // appended on the SAME line (design §3.3): so line 1 STARTS WITH the raw
        // first input line and the marker is its suffix.
        let first_raw = input.lines().next().unwrap();
        let first_out = out.lines().next().unwrap();
        assert!(
            first_out.starts_with(first_raw),
            "{name}: first raw line not kept verbatim as representative\n  raw: {first_raw}\n  out: {first_out}"
        );
        assert!(
            first_out.ends_with(']') && first_out[first_raw.len()..].contains("[x"),
            "{name}: marker not appended to first raw line\n  out: {first_out}"
        );
        // net win: template-collapsed output is strictly shorter than input.
        assert!(out.len() < input.len(), "{name}: output not shorter than input");
    }

    /// Assert a NEGATIVE fixture is emitted byte-identical to `input.trim_end()`
    /// with NO `[x` marker anywhere (no over-masking / spurious collapse).
    fn assert_negative(name: &str, input: &str) {
        let out = compress_log(input).expect("compress_log returned None");
        assert!(!out.contains("[x"), "{name}: spurious run-collapse marker:\n{out}");
        assert_eq!(
            out,
            input.trim_end(),
            "{name}: negative fixture was mutated (expected verbatim passthrough)"
        );
    }

    #[test]
    fn compress_log_gate_case1_access_log_same_template() {
        // 6 access lines, identical template (TS/req-id/dur vary) -> 1.
        assert_positive("case1", FX_CASE1, 1);
    }

    #[test]
    fn compress_log_gate_case2_mixed_levels_interspersed() {
        // INFO,INFO -> [x2]; WARN single; INFO,INFO -> [x2]; ERROR single => 4.
        assert_positive("case2", FX_CASE2, 4);
    }

    #[test]
    fn compress_log_gate_case3_bracketed_hex_ip() {
        // 5 DEBUG conn lines (hex ptr + IP:port vary), identical template -> 1.
        assert_positive("case3", FX_CASE3, 1);
    }

    #[test]
    fn compress_log_gate_case4_level_prefixed_uuid_duration() {
        // 5 level-prefixed lines (UUID + Nms vary), identical template -> 1.
        assert_positive("case4", FX_CASE4, 1);
    }

    #[test]
    fn compress_log_gate_case5_two_alternating_templates() {
        // hit,hit -> [x2]; miss single; hit,hit -> [x2]; miss single; hit single => 5.
        assert_positive("case5", FX_CASE5, 5);
    }

    #[test]
    fn compress_log_gate_case6_only_timestamp_varies() {
        // 5 ERROR lines differing ONLY in timestamp -> 1 (the headline Path-2 win).
        assert_positive("case6", FX_CASE6, 1);
    }

    #[test]
    fn compress_log_gate_case7_short_hex_req_id() {
        // L4 regression: 6 access lines with 4-char hex req-ids -> 1. Guards the
        // >=4 hex floor so short correlation ids normalize (was 0% at >=6).
        assert_positive("case7", FX_CASE7, 1);
    }

    #[test]
    fn compress_log_gate_neg1_all_distinct_startup() {
        // 6 structurally-distinct startup lines -> 0 collapses, verbatim.
        assert_negative("neg1", FX_NEG1);
    }

    #[test]
    fn compress_log_gate_neg2_same_prefix_different_structure() {
        // Shared prefix but different trailing key-sets (KV keeps keys) -> verbatim.
        assert_negative("neg2", FX_NEG2);
    }

    #[test]
    fn compress_log_gate_all_fixtures_summary() {
        // Single roll-up so a run of `compress_log` prints every before->after.
        let cases: [(&str, &str, Option<usize>); 8] = [
            ("case1", FX_CASE1, Some(1)),
            ("case2", FX_CASE2, Some(4)),
            ("case3", FX_CASE3, Some(1)),
            ("case4", FX_CASE4, Some(1)),
            ("case5", FX_CASE5, Some(5)),
            ("case6", FX_CASE6, Some(1)),
            ("neg1", FX_NEG1, None),
            ("neg2", FX_NEG2, None),
        ];
        for (name, input, expected) in cases {
            let before = input.lines().count();
            let out = compress_log(input).unwrap();
            let after = out.lines().count();
            match expected {
                Some(exp) => assert_eq!(
                    after, exp,
                    "{name}: {before}->{after} (expected {exp})"
                ),
                None => assert_eq!(
                    after, before,
                    "{name}: negative changed line count {before}->{after}"
                ),
            }
        }
    }
}

