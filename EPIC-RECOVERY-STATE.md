# EPIC RECOVERY STATE — native-first memory (Efforts 1 & 2)

**Purpose:** durable, machine-readable resume point. On ANY context loss / gateway restart / fresh session,
read THIS file first, run the STATUS PROBE, and continue from the first non-DONE stage. Do NOT wait for the human.

**Directive (Paradox 2026-07-02 11:22):** publish APPROVED; don't wait on me; always use detached/managed sessions
to survive gateway restarts; hook recovery mechanisms so you continue on loss of context/gateway.

**Worktree:** C:\Projects\_worktrees\pluresdb-native-memory  (branch feat/native-memory-reactive off main 0ec9523)\n**NAPI crate:** crates/pluresdb-node   **Plugin:** C:\Projects\plureslm-openclaw   **Store:** C:\Users\kbristol\.pluresLM\migrated-store\n**Epic issues:** plures/plureslm-openclaw #1 (umbrella), #6 (H). **Goal id:** 023bee68-08a2-4632-8cce-f23cede06af7

## STATUS PROBE (run these to learn true state, don't trust memory)
- .node built:   `Test-Path C:\Projects\_worktrees\pluresdb-native-memory\crates\pluresdb-node\*.node`
- headroom live: `cd <crate>; node -e "const m=require('./index.js'); console.log(typeof m.compressText)"` => 'function'
- plugin consumes new native: grep plureslm-openclaw/package.json for pluresdb-native dep path/version
- resolver wired: grep plureslm-openclaw/src/index.ts for `registerMemoryFlushPlan`
- subscribe real: grep crates/pluresdb-node/src/lib.rs for `ThreadsafeFunction` in subscribe (stub if absent)
- published: `npm view @plures/pluresdb-native version` vs local crate version

## ⚠️ S1 SIMPLIFIED (discovered 2026-07-02 11:25): NO NPM PUBLISH NEEDED
The plugin consumes native via a **`file:` link**: package.json `"@plures/pluresdb-native": "file:../pluresdb/crates/pluresdb-node"` (pnpm symlink into CANONICAL C:\Projects\pluresdb, NOT the worktree). The pkg is NOT on npmjs.org (404) and never was. Canonical crate ALREADY has: built .node (55MB, exports compressText/countTokens) + headroom commit 350dba6 in HEAD. So S1b(publish)/S1c(bump dep) are MOOT — native headroom is already on-disk where the plugin links. Remaining real S1 work = **S1d resolver only** (+ repair the pnpm native-binding symlink so the .node is reachable). The worktree build was still useful (proved fresh build green + 95% compression), but delivery goes through the CANONICAL crate the plugin links to.

## 🚨 S1d BLOCKED BY ARCHITECTURAL MISMATCH (discovered 2026-07-02 ~11:45) — NEEDS HUMAN DECISION
Subagent extracted the REAL SDK contract (C:\Users\kbristol\.openclaw\workspace\.openclaw\tmp\flushplan-contract.md). The epic's H premise is WRONG against the actual SDK:
- `MemoryFlushPlanResolver = (params:{cfg?,nowMs?}) => MemoryFlushPlan | null` — returns a POLICY (softThresholdTokens, forceFlushTranscriptBytes, reserveTokensFloor, model?, prompt, systemPrompt, relativePath), NOT compressed content. It tells the host to run an **LLM summarization of the TRANSCRIPT** and write it to relativePath. It NEVER calls compressText.
- The native headroom (`compressText`) is EXTRACTIVE token-compression of MEMORY CONTENT (prose head+tail/code-sig/log-collapse, deterministic, no LLM). Different input (memory chunks vs transcript), different mechanism (extractive vs LLM), different trigger.
- Other memory seams checked: `promptBuilder:(params:{availableTools,citationsMode?})=>string[]` (no token budget passed in); `runtime` (search-manager plumbing); `MemoryCorpusSearchResult.snippet` (could shrink snippets — minor). NONE is a natural home for corpus extractive compression.
=> Wiring compressText into flushPlanResolver is a square-peg. H as specced is architecturally incompatible with the current OpenClaw memory SDK.

OPTIONS TO PUT TO PARADOX:
(1) flushPlanResolver AS-DESIGNED: register a real transcript-flush policy (LLM summary) — delivers pre-compaction headroom the SDK actually supports, but does NOT use the native compressText IP.
(2) Apply native compressText where it DOES fit: compress oversized recall snippets in the memory-capability search path + optionally the promptBuilder output (pre-injection shrink). Real use of the native IP, but it's snippet/section compression, not transcript headroom.
(3) BOTH (1)+(2) — policy flush for transcript + extractive compression for injected memory. Most complete.
(4) Extend the OpenClaw SDK itself to add a real memory-corpus compression seam (biggest scope; may need upstream openclaw change).
Paradox directive is "complete not quick" + "improve native bindings, don't be limited by them" → leans (3) or (4). ASK.

## 🔍 S1 DESIGN CORRECTED (2026-07-02 11:59) after real SDK contract (flushplan-contract.md)
The epic's original assumption — "register flushPlanResolver that calls compressText on oversized memory context" — is a CATEGORY ERROR vs the real SDK. Ground truth from installed SDK (memory-state-FIOhoe_D.d.ts):
- `MemoryFlushPlan = { softThresholdTokens, forceFlushTranscriptBytes, reserveTokensFloor, model?, prompt, systemPrompt, relativePath }` — a pre-compaction SUMMARIZATION descriptor.
- `MemoryFlushPlanResolver = (params:{cfg?,nowMs?}) => MemoryFlushPlan | null` — SYNC, gets ONLY cfg+nowMs, returns summarization PARAMS. It does NOT receive content and does NOT return compressed content.
- Registration: field `flushPlanResolver?` on the SAME object passed to `api.registerMemoryCapability(cap)` (exclusive slot). Plugin today passes ONLY `{ runtime }` (memory-capability.ts:586 buildMemoryCapability) — no flushPlanResolver, no promptBuilder.
**=> Wiring compressText into flushPlanResolver would be a FAKE (C-NOSTUB-001). DO NOT.**
**Honest headroom delivery seam = the plugin's OWN write path** (PluresLmStore.put/store in pluresdb.ts): compress oversized log/code node bodies before persistence, with a min-size floor so small/prose bodies are untouched; record real before/after token counts in write accounting. That is where the plugin handles content and where 95% compression is real+measurable on the actual store. `compressContent/countTokens/detectContentType` already exported from pluresdb.ts for this.
(Optionally ALSO provide a real config-driven flushPlanResolver later — separate honest capability, NOT the headroom vehicle. Not required for E1.)

## STAGES (continue from first not-DONE)
- [DONE] S0 baseline: build green + 31/31 headroom tests (2026-07-02 07:40).
- [DONE] S1a build .node addon: emitted 11:18, 35MB, exports compressText/countTokens/detectContentType; verified 320->16 tok (log). 
- [MOOT] S1b PUBLISH: not needed — plugin uses file: link to canonical crate (see note above). Canonical already has headroom .node + commit.
- [DONE] S1c native reachable through plugin's OWN loader (ensureNativeLibraryPath sets NAPI_RS_NATIVE_LIBRARY_PATH -> canonical .node); verified PluresDatabase + compressText both load, 150->7 tok live. pnpm link quirk (#4828) is already handled by the plugin's resolver.
- [DONE] S1d-a extend PluresNativeModule type + export compressContent/countTokens/detectContentType from pluresdb.ts (honest presence-checks, throw if native predates headroom — no stub).
- [DONE] S1d-b wire compression into WRITE PATH: PluresLmStore.#maybeCompress + store() accumulate compressed/tokensSaved; StoreWriteResult gained compressed+tokensSaved; PluresLmStoreOptions/PluresLmCapabilityConfig/PluresLmPluginConfig + toStoreOptions + readConfig + buildMemoryCapability all plumb compressAboveTokens; index.ts docs updated; stale "No flush-plan resolver" note superseded. PLUGIN BUILD EXIT=0 (dist reflects all wiring).
- [DONE] S1e VERIFY on REAL store copy (non-destructive temp copy of migrated-store) — **S1E_GATE: PASS** (2026-07-02 ~12:20). bigLog 11400ch->110ch (99%), tokensSaved=4633, compressed=1; small prose 58ch UNCHANGED (floor honored); native collapses 120 repeated lines -> "<line> [×120]"; recall found=true (connection refused/backoff) + get(id) present; plugin build EXIT=0. **EFFORT 1 (HEADROOM DELIVERY) COMPLETE.** Harness: .openclaw/tmp/s1e-verify.mjs.

## ▶️ EFFORT 2 NOW ACTIVE: real async subscribe() in pluresdb-node (the genuine native-binding work)
- [DONE] S2 real async subscribe(cb)+unsubscribe in pluresdb-node — **COMPLETE & VERIFIED 2026-07-02 11:47.** Design as specced: added `tokio.workspace=true` to crate Cargo.toml; replaced the drop-the-receiver stub with real `subscribe(cb)->u32` (spawns a named std::thread draining `broadcast::Receiver<SyncEvent>` via `blocking_recv()` into a napi `ThreadsafeFunction<SyncEventJs,(),SyncEventJs,Status,false>`; CalleeHandled=false so JS gets the value directly; handles Closed=>exit, Lagged=>skip, per-dispatch cancel-flag check) + `unsubscribe(id)` (sets AtomicBool cancel flag, removes from registry; idempotent). Added `#[napi(object)] SyncEventJs { kind, id }` with exhaustive `From<SyncEvent>` (upsert/delete/peer-connected/peer-disconnected). Registry = `Arc<Mutex<HashMap<u32,Arc<AtomicBool>>>>` + `AtomicU32` on PluresDatabase (init in BOTH constructors). No second Tokio runtime (std::thread + blocking_recv only). `cargo check` GREEN (2m28s); release `.node` rebuilt 11:46:52 (34.8MB) with bindings `subscribe(cb)->number`/`unsubscribe(id)`. **Node smoke test PASS (exit 0):** put => `{kind:'upsert',id}` cb WITHOUT polling; 2nd put delivered; unsubscribe stopped all further cbs; idempotent unsubscribe safe; clean process exit (no leak/hang). Test at crates/pluresdb-node/test-s2-subscribe.mjs.
- [ ] S3 reactive .px on write via S2 path.
- [ ] S4 integrate: bump plugin to reactive native, restart gateway, verify live, close loop. Update epic #1/#6.

## HELD gates now CLEARED by Paradox 2026-07-02 11:22:
- npm publish of @plures/pluresdb-native => APPROVED. Proceed.
- (babc70f redact push on plugin: treat as part of shipping S1c/S1d if needed; publish approval implies forward motion.)

## RULES
- Every build/publish/long op => DETACHED managed session (exec background) + log file. Never inline-blocking.
- After each stage: update this file + .native-memory-milestones.md BEFORE moving on (crash-safe).
- Self-heal cron `native-epic-driver` re-checks every 15m and continues if idle.
