# ADR-0019: BitNet Native Embedding Support (Design-Stage, No Implementation)

- **Status:** Proposed (design-stage only — no implementation in this PR)
- **Date:** 2026-07-30
- **Author:** kbristol (via agent orchestration)
- **Related:** ADR-0016 (hardware-adaptive compute), pares-umbra `ADR-0007-bitnet-first`, pares-umbra `ADR-0010-bitnet-parallel-research`, pares-umbra merged epics `causal-bitnet-correctness-harness` and `bitnet-cpp-benchmark-adapter`.

## Context

`pluresdb-core` currently ships an optional `embeddings` Cargo feature
(`crates/pluresdb-core/Cargo.toml`) backed by the `fastembed` crate (ONNX
Runtime). The default/documented model is `BAAI/bge-small-en-v1.5`
(384-dim), with `bge-base-en-v1.5` (768), `bge-large-en-v1.5` (1024), and
`sentence-transformers/all-MiniLM-L6-v2` (384) also supported via
`model_id_to_fastembed()` in `crates/pluresdb-core/src/lib.rs`.

**Critically:** `fastembed`'s `TextEmbedding::try_new()` downloads the
selected HF model checkpoint over the network the first time it is
constructed (an ONNX file + tokenizer, cached under a local HF cache dir)
unless the caller pre-populates that cache out of band. This is a
**runtime, first-use, network-triggered install** — exactly the pattern
kbristol's 2026-07-30 decision prohibits for any embedding-default change
going forward.

Microsoft has since published purpose-built BitNet embedding models
research-validated for bitnet.cpp:

- `microsoft/bitnet-embedding-0.6b` — 1024-dim, i2_s-quantized GGUF,
  runs via bitnet.cpp's `llama-embedding` binary
  (`./build/bin/llama-embedding -m bitnet-embeddings-0.6b-bf16-i2_s.gguf -p "query: ..." --embd-normalize 2`).
- `microsoft/bitnet-embedding-270m` — 640-dim, same tooling, smaller/faster.

Both are hosted on Hugging Face and consumed through bitnet.cpp's
existing llama.cpp-derived GGUF loader — the same inference stack Umbra
has already been researching (pares-umbra `ADR-0007-bitnet-first`,
`ADR-0010-bitnet-parallel-research`, and the merged
`bitnet-cpp-benchmark-adapter` epic at `tools/bitnet-cpp-bench/` in that
repo).

**Existing Umbra bitnet.cpp research (reused, not repeated):**

- `pares-umbra` already vendors a standalone bitnet.cpp
  build/benchmark adapter (`tools/bitnet-cpp-bench/`, commit `bda3e67`),
  which documents the *real* upstream build contract: clone
  `microsoft/BitNet` (vendors a BitNet-specific llama.cpp fork
  `isHuangXin/llama.cpp@release-bitnet-embedding-0.6b-270m` as a
  submodule), `python setup_env.py -md <dir> --hf-repo <model> -q i2_s`
  to download+convert+build in one step (CMake ≥3.22, Clang ≥18 — no
  separable "build only" step exists upstream), and
  `utils/e2e_benchmark.py` / `llama-embedding` to run.
- That adapter's own execution log (`docs/bitnet-cpp-vs-umbra-benchmark.md`
  in pares-umbra) is an honest account: the umbra-bitnet Rust side ran
  and produced real numbers; the actual bitnet.cpp build was blocked in
  that sandbox on a missing `cmake`/`clang` toolchain requiring elevated
  install — a concrete, documented environment limitation, not a design
  flaw. **This is direct evidence for why "install at runtime" is
  unacceptable in production**: even a benign, human-initiated,
  once-only toolchain setup failed without elevation in a sandboxed
  environment. A live/automatic install path in `pluresdb` at normal
  operation time would fail identically (or worse, silently) in any
  constrained deployment (containers, locked-down endpoints, CI).
- `crates/umbra-pluresdb` in pares-umbra already integrates BitNet
  model I/O with pluresdb (`crates/umbra-bitnet/src/model_io.rs` —
  bincode/JSON frozen-model save/load, no runtime fetch), establishing
  precedent for "vendor the weights, load from disk, never fetch."

## Decision (proposed; NOT yet implemented)

Change or remove pluresdb's default text-embedding backend in favor of a
BitNet-native embedding model (`bitnet-embedding-0.6b` and/or
`bitnet-embedding-270m`), consumed via a `bitnet.cpp`-compatible GGUF
inference path, **under a hard constraint that supersedes convenience**:

> **No installs, downloads, or model fetches may be triggered by normal
> pluresdb operation, ever.** The BitNet embedding model (GGUF weights)
> and any required native runtime component (a `llama-embedding`-derived
> library, statically linked or vendored as a build artifact) MUST be
> either:
> 1. **Bundled/vendored at build time** — the GGUF file (or a quantized
>    variant) is fetched and pinned during the crate's build/release
>    pipeline (e.g. `build.rs` fetching a *pinned, hash-verified* asset
>    into `OUT_DIR` during `cargo build` in CI, or committed via Git LFS /
>    release asset), so the artifact that ships to a consumer already
>    contains everything it needs; **or**
> 2. **A one-time, explicit, documented offline setup step** that a human
>    or an install script runs deliberately (e.g. `pluresdb-cli
>    embeddings setup --model bitnet-embedding-0.6b`), clearly separate
>    from any code path that fires during normal read/write/query
>    operations, with the tool refusing to silently fall back to a
>    network fetch if invoked from a non-interactive/production context.
>
> Under no circumstances may `EmbedText`/`FastEmbedder`-equivalent code
> reach out to Hugging Face, GitHub, or any other network endpoint the
> first time a user calls an embedding API in production. If the model
> asset is absent, the correct behavior is a clear, actionable
> `EmbeddingModelNotBundled` error — never a silent or automatic fetch.

This mirrors the "NO STUBS" gate documented in NATIVE-MEMORY-TASK.md (C-NOSTUB-001):
a hollow/auto-fetching embedding path is exactly the kind of invisible-until-it-breaks-in-prod behavior those gates exist to prevent.

## Options Considered

1. **Keep fastembed/ONNX as-is.** Rejected as the sole path going
   forward per kbristol's 2026-07-30 decision — BitNet native support is
   the target, though fastembed may remain as a fallback/alternate
   feature flag for parity comparison during the transition (see Open
   Questions).
2. **Runtime pip/npm/HF-hub style auto-install of BitNet weights on
   first use.** Explicitly rejected — this is the exact anti-pattern the
   2026-07-30 decision bans, and pares-umbra's own bitnet-cpp-bench
   experience demonstrates why (toolchain/model installs fail silently
   or loudly in constrained/sandboxed/production environments).
3. **Vendor GGUF weights + a statically-linked bitnet.cpp-derived
   inference shim at build time (chosen direction, pending validation).**
   Two sub-variants to evaluate in the implementation epic:
   a. **FFI/NAPI binding to a vendored `llama-embedding`-equivalent C++
      library** (reusing bitnet.cpp's I2_S GGUF kernels, statically
      linked into `pluresdb-core`/`pluresdb-node` at build time — no
      subprocess, no runtime download).
   b. **Pure-Rust GGUF reader + BitNet inference**, potentially sharing
      code with `pares-umbra`'s `crates/umbra-bitnet` (`transformer.rs`,
      `bitlinear.rs`, `model_io.rs`), which already has a working,
      *from-scratch* Rust BitLinear/transformer implementation with no
      C++ toolchain dependency at all — this avoids the CMake/Clang
      build-toolchain fragility that blocked the bitnet.cpp side of the
      Umbra benchmark entirely, and is the **preferred variant** because
      it removes an entire class of build-time failure risk.
4. **Documented offline setup command that fetches once, deliberately.**
   Retained as the fallback for teams that cannot/do not want to bundle
   multi-hundred-MB weights into every build artifact (e.g. Docker image
   size sensitivity) — but this is opt-in tooling invoked by a human, not
   something normal database operations trigger.

## Migration / A-B Evaluation Plan (required before any implementation PR)

Existing databases may already contain vectors computed with
`bge-small-en-v1.5` (384-dim) or another current default. Changing the
default embedding model breaks vector-space compatibility (different
dimension, different semantic geometry) — this must be validated, not
assumed, before cutover:

1. **Dimension/schema compatibility audit.** Confirm current on-disk
   vector index schema in `pluresdb-core`/`pluresdb-procedures` (search
   ops in `crates/pluresdb-procedures/src/ops/search.rs`) can either (a)
   support multiple embedding dimensions side-by-side during a
   transition window, or (b) requires a full reindex. Document which.
2. **Golden query/answer set.** Assemble a representative, versioned
   evaluation set (real documents + real queries from an existing
   pluresdb consumer — no synthetic/mock data per the NO STUBS +
   Demos-real-data gates) large enough for statistically meaningful
   recall/precision comparison (target: ≥200 query-document pairs with
   human or existing-system-labeled relevance).
3. **A/B parity harness.** Compute embeddings for the same corpus with
   both (a) current default (`bge-small-en-v1.5`) and (b) candidate
   BitNet model(s), then measure:
   - Recall@k / nDCG@k on the golden set for both.
   - Embedding latency (CPU, single-thread and multi-thread) — reuse
     pares-umbra's `run-umbra-benchmark.ps1` pattern for methodology.
   - Memory footprint (model size on disk + resident inference memory).
   - Dimension-driven storage cost delta (1024 vs 384 dims = ~2.7x raw
     vector storage per document).
4. **Regression gate.** BitNet model must meet or exceed current
   recall/precision within an agreed tolerance (proposed: no worse than
   -2% nDCG@10) AND show a measurable benefit (latency, memory, size, or
   accuracy) to justify the migration cost. If it does not clear this
   bar, this ADR's decision is to **not** cut over, and the finding gets
   recorded as an update to this ADR (not silently dropped).
5. **Reindex/dual-write plan for existing consumers.** Any consumer with
   already-stored vectors needs an explicit, versioned migration path
   (e.g. `pluresdb-cli embeddings reindex --from bge-small-en-v1.5 --to
   bitnet-embedding-0.6b`) that is itself a deliberate, one-time,
   human/ops-triggered operation — never automatic on schema mismatch
   detection at query time.
6. **Only after 1-5 are complete and pass** does an implementation PR
   get opened, per the "design-stage only, no code yet" scope of this
   ADR.

## Consequences

- **Positive:** removes a real runtime-network-fetch dependency
  (`fastembed`'s HF Hub client) from pluresdb's embedding path, aligning
  with the hard no-live-install constraint. Potential quality/efficiency
  gains from a purpose-built, ternary-quantized (1.58-bit) embedding
  model with much smaller memory/CPU footprint than full-precision ONNX
  BGE models. Reuses/validates existing pares-umbra BitNet
  infrastructure rather than building a parallel stack from zero.
- **Negative/risk:** vendoring GGUF weights at build time increases
  release artifact size (hundreds of MB) unless quantized aggressively;
  a pure-Rust GGUF+BitLinear inference path (option 3b) is not yet
  proven for the embedding-specific architecture variant (evolved from
  the existing causal-LM `umbra-bitnet` work, not identical); dimension
  change forces a real migration for any existing pluresdb consumer with
  stored vectors — this is not a drop-in swap.
- **Not yet decided:** whether `fastembed`/ONNX remains as a permanent
  parallel feature flag (for users who prefer it) or is fully removed
  once BitNet parity is proven. Deferred to the A/B evaluation outcome.

## Open Questions

1. **Which variant (0.6b vs 270m) should be the new default?** 270m is
   smaller/faster (640-dim) but 0.6b (1024-dim) may have materially
   better recall — the A/B harness (step 3 above) must answer this with
   real numbers before default selection, not assumption.
2. **FFI-to-vendored-C++-lib vs. pure-Rust reimplementation (option 3a
   vs 3b)?** 3b avoids the CMake/Clang toolchain fragility documented in
   pares-umbra's benchmark adapter experience, and could share code with
   `crates/umbra-bitnet`, but the embedding model architecture (encoder,
   pooling/normalization head) differs from `umbra-bitnet`'s
   causal-decoder focus and has not been validated in that crate yet.
   This needs a short spike before implementation, not a design-doc-only
   assumption.
3. **Does `pluresdb-wasm`/`pluresdb-deno` need a different bundling
   strategy than `pluresdb-node`/native** (e.g. WASM build size limits
   may make full weight-bundling impractical, pushing those targets
   toward the "one-time offline setup" path instead of "bundled at build
   time")? Needs a target-by-target artifact-size budget before
   implementation.
4. **Cross-repo ownership:** should the actual BitNet embedding
   inference code live in `pares-umbra` (as a reusable crate pluresdb
   depends on) or directly in `pluresdb-core`? Per
   `development-guide/design/PLURES-FOUNDATION.md` repo-routing rules,
   this should be resolved explicitly in the implementation epic, not
   assumed — leaning toward pares-umbra owning the BitNet inference
   primitive (it already has `umbra-bitnet`) and pluresdb consuming it
   as a dependency, avoiding duplicate BitNet implementations across
   repos.

## Non-Goals of This ADR

- No code implementation ships in the PR that introduces this ADR.
- No decision is made here on removing `fastembed` outright — that
  depends on the A/B outcome.
- No runtime auto-install path is proposed, evaluated as viable, or
  left as an implicit fallback anywhere in this document.
