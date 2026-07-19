## [3.10.0] — 2026-07-19

- fix(dependabot): auto-merge github_actions bumps (were wrongly held as major-production) (#1066) (51cd9f4)
- fix(release): correct version baseline 3.0.1 -> 3.9.1 (unblock releases since May) (#1065) (99b7636)
- ci: prevent lifecycle queue-advance failures on Dependabot PRs (#1064) (c4d084a)
- Initial plan (#1051) (b11ba39)
- chore(deps): bump regex from 1.13.0 to 1.13.1 (#1062) (281b622)
- chore(deps): bump clap from 4.6.1 to 4.6.2 (#1061) (45889bc)
- chore(deps): bump napi from 3.10.3 to 3.10.5 (#1060) (4a45cb3)
- chore(deps): bump futures from 0.3.32 to 0.3.33 (#1059) (de4a19a)
- chore(deps): bump tokio from 1.52.3 to 1.53.0 (#1058) (943ef1d)
- chore(deps): bump uuid from 1.23.4 to 1.24.0 (#1057) (e84921e)
- chore(deps): bump async-trait from 0.1.89 to 0.1.90 (#1056) (d8fe2f2)
- chore(deps): bump fastembed from 5.17.2 to 5.17.3 (#1055) (25a0d64)
- chore(deps): bump anyhow from 1.0.103 to 1.0.104 (#1054) (2243d7b)
- chore(deps-dev): bump the web-svelte-minor-patch group (#1053) (54d7d17)
- chore(deps): bump napi-derive from 3.5.9 to 3.5.10 (#1052) (acf5574)
- ci(lifecycle): skip queue-advance for dependabot[bot] actor (#1042) (22175b5)
- fix(build): bump praxis-lang px-* deps to BOM-free rev 37be99e (d08f88b)
- ci(dependabot-auto-merge): v4 - auto-merge 0.x breaking bumps (CI-gated) (1cfd895)
- chore(deps): bump aes-gcm from 0.10.3 to 0.11.0 (#1044) (637a2ed)
- chore(deps): bump tokio-tungstenite from 0.29.0 to 0.30.0 (#1046) (c158cc6)
- fix(node-test): pass callback to subscribe() + exit cleanly (9c2bf51)
- chore(deps-dev): bump @sveltejs/vite-plugin-svelte in /web/svelte (#1045) (7733007)
- chore(deps): bump regex from 1.12.4 to 1.13.0 (#1048) (b7e8c27)
- chore(deps-dev): bump vite from 6.4.3 to 8.1.4 in /web/svelte (#1047) (4bff918)
- chore(deps-dev): bump the web-svelte-minor-patch group (#1043) (90d196e)
- fix(sea): migrate pluresdb-sea to p256 0.14 / ecdsa 0.17 API (#1040) (296ef28)
- chore(deps): upgrade RustCrypto stack together (digest/sha2/hmac/pbkdf2/p256) to resolve EagerHash version-skew (#1037) (51c5992)
- chore(deps): bump p256 from 0.13.2 to 0.14.0 (#1032) (d5b97f2)
- chore(deps): bump pest_derive from 2.8.6 to 2.8.7 (#1033) (340b561)
- chore(deps): bump napi from 3.10.1 to 3.10.3 (#1031) (68175e2)
- chore(deps): bump the web-svelte-minor-patch group (#1029) (a07e86f)
- ci(web): build-gate Svelte web UI on web/** changes (ff6d977)
- fix(web): restore missing 'const tabs = [' decl in App.svelte (broken build on main) (6b5db24)
- chore(deps): bump cytoscape-dagre from 2.5.0 to 4.0.0 in /web/svelte (#1019) (f734501)
- ci(dependabot): v3 - do not auto-merge pre-1.0 (0.x) breaking bumps (0.x minor is breaking; aes-gcm 0.10->0.11 broke main) (473dcad)
- ci(dependabot): fix workflow corruption from -f wrapping; deploy clean security-aware auto-merge (382c303)
- fix(deps): pin aes-gcm to 0.10 to unblock main (revert breaking #1017 auto-merge) (2b7833e)
- ci(dependabot): auto-merge security advisories regardless of semver bump (security over function) (8b0f96f)
- fix(security): bump vite to 6.4.3 in web/svelte (#1016) (148c3a8)
- fix(ci): repair invalid tech-doc-writer.yml (block-scalar break) (1db2c65)
- fix(clippy): resolve -D warnings regression on main (headroom.rs, lib.rs) (9915668)
- chore(deps): bump unicode-segmentation from 1.12.0 to 1.13.3 (#1026) (5fec785)
- chore(deps): bump napi from 3.9.4 to 3.10.1 (#1025) (effd88a)
- chore(deps): bump rand from 0.10.1 to 0.10.2 (#1024) (a4681e8)
- chore(deps): bump tiktoken-rs from 0.6.0 to 0.12.0 (#1023) (081338b)
- chore(deps): bump napi-derive from 3.5.7 to 3.5.9 (#1022) (6d57d36)
- chore(deps): bump the web-svelte-minor-patch group (#1018) (321cc60)
- chore(deps): bump aes-gcm from 0.10.3 to 0.11.0 (#1017) (50ec6d3)
- chore(dependabot): manage web/svelte npm updates directly (8282617)
- fix(headroom): bracketed-timestamp logs auto-detect as log, not json (c72abdb)
- feat(headroom): Path-2 template-normalizing compress_log + L4 short-hex floor (f9e7af8)
- docs(epic-memory): S4 native integration DONE - canonical FF to reactive + fresh reactive .node + 3 S4 gates PASS; only gateway restart-verify + epic close remain (aedc66e)
- docs(epic-memory): mark S3 reactive .px on write DONE + verified (commit 8290344); S4 remains open (83c8469)
- feat(pluresdb-node): reactive .px evaluation on write (EPIC-MEMORY Effort 3 / S3) (8290344)
- feat(pluresdb-node): real reactive subscribe()/unsubscribe() (EPIC-MEMORY Effort 2) (5b1b683)
- fix(pluresdb-px): restore PX-L010/PX-L012 firing after praxis-lang migration (0ec9523)
- fix(pluresdb): honest vector-search scoring API (Bug 2) (a16764c)
- fix(pluresdb-node): correct stale test-node.js list() assertion (#1014) (d9a8ab0)
- verify(pluresdb-node): Headroom e2e compression proof on realistic context (58b42b2)
- test(pluresdb-node): Headroom compression test+QA suite (31 tests) + JSDoc fix (bc43e50)
- test(pluresdb-node): Headroom detector tests + log-detection fix (a131743)
- feat(pluresdb-node): NAPI token-compression surface (Headroom port) (350dba6)
- M6.4: re-export dataflow AST types under historical Px* names (consumer compat) (#1015) (6791762)
- M6.1-M6.3: pluresdb-px consumes praxis-lang (delete in-tree language engine) (#1013) (6f5c9c1)
- test(storage): add encryption+bridge-fmt behavioral tests (kill mutants) - Level-0 #6 (dd62623)
- test(storage): add WAL+replay behavioral tests (kill mutants) - Level-0 #6 (8094822)
- test(storage): kill storage-engine surface mutants (lib.rs 0 survivors) - Level-0 #6 (1b4db90)
- test(storage): kill 6 SledRadAdapter mutants — rad.rs 0 survivors (Level-0 #6) (e66669c)
- ci(mutation): add ratchet mutation-testing gate for critical crates (Level-0 gap #6) (89f1a06)
- test(pluresdb-px): hermetic .px conformance corpus + CI gate so grammar drift fails the build (TASK-PX-CANON Stage 4) (2d8d786)
- ci: lifecycle cron */30 -> 0 */2 (cut scheduled Actions spend; events still real-time) (fa9cc77)
- feat(pluresdb-node): unify px constraints onto CrdtStore single source of truth + real pxLoadPxSource/pxInsertConstraint (TASK-PX-CANON Stage 2) (00024d6)
- fix(pluresdb-px): clear pre-existing clippy lints + Windows import-path containment so the Stage 1 test/clippy gate is green (92b0e59)
- fix(pluresdb-px): compile_nl parses real structured predicates -- kill the Always-pass stub (TASK-PX-CANON Stage 1) (59b2611)
- chore(deps): bump quote from 1.0.45 to 1.0.46 (#1012) (016c72b)
- chore(deps): bump napi-derive from 3.5.6 to 3.5.7 (#1011) (9d2fa17)
- chore(deps): bump napi from 3.9.3 to 3.9.4 (#1010) (9df4adf)
- chore(deps): bump uuid from 1.23.3 to 1.23.4 (#1009) (6e11fd2)
- chore(deps): bump anyhow from 1.0.102 to 1.0.103 (#1008) (8c9f6b4)
- chore(deps): bump tower-http from 0.6.11 to 0.7.0 (#1007) (33fa7f9)
- chore(deps): bump cron from 0.16.0 to 0.17.0 (#1006) (8a6280f)
- chore(deps): bump syn from 2.0.117 to 2.0.118 (#1005) (84eaa5a)
- chore(deps): bump fastembed from 5.16.1 to 5.17.2 (#1004) (0328bb7)
- chore(deps): bump napi from 3.9.1 to 3.9.3 (#1003) (b76e834)
- docs(adr): ADR-0016 hardware-adaptive compute (self-optimizing GPU/NPU/CPU kernels) (f3603ae)
- chore(deps): bump uuid from 1.23.2 to 1.23.3 (#1001) (60de1e8)
- chore(deps): bump napi from 3.9.0 to 3.9.1 (#1000) (b876d92)
- chore(deps): bump fastembed from 5.16.0 to 5.16.1 (#998) (ba84bdf)
- chore(deps): bump futures from 0.3.31 to 0.3.32 (#997) (6371428)
- chore(deps): bump regex from 1.11.3 to 1.12.4 (#996) (07e392c)
- fix(px): enforce keyword boundaries for if/for/in/match/end (195c67b)
- feat(px): add Assign/If/For step variants to PxStep enum (ef91aa6)
- feat(px): add PxEntity + PxConfig to PxDocument AST (6f1e8b3)
- chore(px): grammar now generated — header updated to reflect ADR-0021 (cac84ba)
- feat(px): unified grammar v4 — single source of truth (40e87df)
- grammar: support negative integers and floats (6ba84fe)
- grammar: reorder value alternatives - arith_val before var_ref (8e457bf)
- grammar: arith_val for inline arithmetic in map/list values (8ede27f)
- grammar: add arithmetic expressions (+, -, *, /, %) (40e325f)
- grammar: call_expr as value, var_ref bracket access, flexible step_call (024396e)
- grammar: fix not_expr (!/NOT/not), proper multiline list/map vals (0c9c6d6)
- grammar: case-insensitive AND/OR + paren_expr as value + add action (9b5b01e)
- grammar: multiline list/map vals + dotted_ident as value (338e22a)
- grammar: allow blank lines inside step_list and block_step_list (3a308d4)
- grammar: extend var_ref with dot access + add define keyword (182fcf9)
- docs: dataflow schema reference + updated ADR-0015 implementation status (6d87b2e)
- feat(px): AsyncDataflowGraph::pop() + has_output() for reading results (27f4349)
- feat(px): ast_to_node() + end-to-end integration test (c3de16a)
- feat(px): dataflow procedure parser + builder + ident types (aaee3b5)
- docs: ADR-0015 queue-driven dataflow procedures (483af63)
- feat(px): dataflow-driven procedure execution — queues ARE the executor (b91cc03)
- fix(px): enforce keyword boundaries so return_* parses as call (#995) (31a560f)
- fix(px): apply when-return propagation to async executor (#994) (6bc8716)
- fix(px): propagate return/abort from when blocks + resolve vars in return values (#993) (fe79e34)
- fix(px): add end terminator to step_match for consistent block termination (a12e6af)
- feat: pluresdb-px — .px language runtime as foundation crate (86f744a)
- fix: resolve clippy 1.95 lints across workspace (159ce96)
- fix(sea): use fill_bytes() instead of fill() for rand 0.10 (5a09ad8)
- chore(deps): bump rusqlite from 0.40.0 to 0.40.1 (#989) (27ee01b)
- chore(deps): bump js-sys from 0.3.81 to 0.3.99 (#990) (de5ffea)
- chore(deps): bump fastembed from 5.15.0 to 5.16.0 (#988) (b4b182d)
- chore(deps): bump chrono from 0.4.44 to 0.4.45 (#987) (a6fa045)
- fix(sea): update rand API for rand 0.10 on main (a4d90c9)
- fix(wasm): add Default impl for WasmCrdtStore (d21c396)
- Initial plan (#984) (46189cc)
- fix(sea): revert pbkdf2 to 0.12 to resolve digest version conflict (#985) (058db44)
- Initial plan (#982) (5a9a8ba)
- chore(deps-dev): bump svelte (#983) (eae920c)
- chore(deps): bump openssl in the cargo group across 1 directory (#978) (697db71)
- chore(deps): bump devalue (#971) (8b29195)
- chore(deps): bump fastembed from 5.13.3 to 5.15.0 (#981) (e6a4ace)
- chore(deps): bump rusqlite from 0.39.0 to 0.40.0 (#980) (61b389a)
- chore(deps): bump uuid from 1.23.1 to 1.23.2 (#979) (c31825b)
- fix: migrate axum route syntax from :param to {param} (axum 0.7) (0f57680)
- chore(deps): bump tower-http from 0.6.8 to 0.6.11 (#977) (d340c11)
- chore(deps): bump serde_json from 1.0.149 to 1.0.150 (#976) (faca565)
- chore(deps): bump dashmap from 6.1.0 to 6.2.1 (#975) (bc17078)
- chore(deps): bump napi from 3.8.5 to 3.9.0 (#973) (03c59f6)
- chore(deps): bump napi-build from 2.3.1 to 2.3.2 (#972) (af80c7e)
- ci: add path filters to CI workflow (21ee13c)
- ci: change release trigger from push-to-main to tag-only (3863c85)
- chore(deps): bump tokio from 1.52.1 to 1.52.3 (#715) (fcac0f2)
- chore(deps): bump rand from 0.9.4 to 0.10.1 (#717) (65afc45)
- ci: add Dependabot auto-merge for patch and minor updates (3098c8e)
- fix(node): bump napi-derive to 3.5.5+ to fix register_class arity mismatch (7300c80)
- license: dual-license under BSL-1.1 OR MIT (e207634)
- chore(deps-dev): bump fast-uri (#712) (63e19b5)
- chore(deps): bump openssl in the cargo group across 1 directory (#694) (85c8bad)
- refactor: replace inline lifecycle with reusable workflow call (886e6b1)
- docs: refresh ROADMAP.md with OASIS strategic alignment (96eb6a5)
- chore(deps): bump cron from 0.12.1 to 0.16.0 (#390) (7880954)
- chore(deps): bump pbkdf2 from 0.12.2 to 0.13.0 (#383) (7697eea)
- chore(deps): bump tower from 0.5.2 to 0.5.3 (#389) (bfd70e7)
- chore(deps): bump fastembed from 5.13.2 to 5.13.3 (#382) (101b284)
- chore(deps): bump napi from 3.8.4 to 3.8.5 (#384) (4a34988)
- chore(deps): bump tokio from 1.51.0 to 1.52.1 (#385) (2638206)
- chore(deps): bump uuid from 1.23.0 to 1.23.1 (#386) (1604b53)
- chore(deps): bump clap from 4.6.0 to 4.6.1 (#387) (9727915)
- chore(deps-dev): bump postcss (#688) (f2e0688)
- fix: axum 0.8 route syntax — :id → {id} (e2064c4)
- fix: suppress ci-feedback issue spam — check closed issues within 24h (4587c32)
- docs: refresh ROADMAP.md with OASIS strategic alignment (b64b279)
- fix(ci): use `|-` block scalar in wasm_targets to prevent spurious pipeline failure (#376) (5812426)
- chore(deps): bump openssl in the cargo group across 1 directory (#374) (2dd7bb0)
- docs: update copilot-instructions with praxis, design-dojo, automation rules (4240ac3)
- Reduce embedding RSS by removing persistent-store vector duplication and capping sled cache (#372) (4aebc1e)
- chore(deps): bump the cargo group across 1 directory with 2 updates (#373) (423c7d8)
- feat(release): add target_version input for milestone-driven releases (00c7f18)
- feat(lifecycle): milestone-close triggers roadmap-aware release (2285743)

## [3.0.1] — 2026-04-23

- fix(sync): make broadcast publish best-effort when no subscribers exist (68436c5)
- docs: update copilot-instructions with Plures stack architecture (3928802)
- docs: update copilot-instructions with Plures stack architecture (385c359)

## [3.9.1] — 2026-04-20

- fix: memory-efficient storage — streaming iteration, right-sized HNSW index (3fac58a)

## [3.9.0] — 2026-04-18

- feat(lifecycle v12): auto-release when milestone completes (4197fa6)
- Fix CI regressions in wasm and node lanes after timer scheduler changes (#367) (4f26d54)

## [3.8.0] — 2026-04-18

- feat(procedures): add cron/interval/once TimerTable triggers with persisted run state and 10s runtime scheduler (#365) (30c32fa)
- Add root flake.nix with fixed-output ONNX Runtime prefetch for sandboxed Nix builds (#363) (efda652)
- Add `pluresdb doctor` health diagnostics command with stable JSON output and failure exit semantics (#362) (9cad284)
- Introduce canonical error codes and unified diagnostics across Rust core, Node/Deno bindings, and CLI (#361) (8d8aa1a)
- Blend memory quality into HNSW ranking with lazy quality backfill (#360) (e870768)
- Fix CI regressions in rust/wasm/node lanes for PR #348 follow-up (#359) (906bb60)

## [3.7.0] — 2026-04-18

- feat(storage): encryption-at-rest key rotation + on-disk verification tests (#348) (1feb9e0)

## [3.6.0] — 2026-04-18

- feat(lifecycle v11): smart CI failure handling — infra vs code (eec7545)
- fix(lifecycle): label-based retry counter + CI fix priority (17d0e69)
- chore(deps): bump rand from 0.8.5 to 0.9.2 (#354) (69ef072)
- chore(deps): bump dashmap from 5.5.3 to 6.1.0 (#350) (0b882f0)
- chore(deps): bump thiserror from 2.0.17 to 2.0.18 (#351) (8046c94)
- chore(deps): bump tower from 0.4.13 to 0.5.2 (#352) (e62518e)
- chore(deps): bump anyhow from 1.0.100 to 1.0.102 (#353) (a2dcd85)
- chore(deps): bump fastembed from 5.13.0 to 5.13.2 (#356) (f70c3ee)
- ci: lifecycle — add unmilestoned issue fallback + force-merge on CI exhaustion (b09deab)
- Harden WAL replay and corruption handling (#347) (df5d446)
- ci: lifecycle v10 — auto-retry transient failures, force-merge on exhaustion (1daa1e4)
- fix: resolve CI failures (node napi + rust fmt) (#345) (bbd27bf)

## [3.5.1] — 2026-04-07

- fix: inline reusable workflow to fix schedule trigger failures (ab83499)
- docs: add structured ROADMAP.md for automated issue generation (3e024de)
- chore: remove redundant workflow — handled by centralized ci-reusable.yml or obsolete (9a7957c)
- chore: remove redundant workflow — handled by centralized ci-reusable.yml or obsolete (16ace11)

## [3.5.0] — 2026-04-07

- feat: split-brain detection tests and conflict resolution policy docs (#343) (b1aec5e)
- [WIP] Implement Hyperswarm and Relay transports in pluresdb-sync (#342) (d8ff55a)

## [3.4.1] — 2026-04-07

- fix(fmt): apply cargo fmt to pluresdb-node and pluresdb-wasm (#341) (eb17228)
- chore: centralize release to org-wide reusable workflow (7bff4d0)
- chore: centralize CI to org-wide reusable workflow (4337da1)

## [3.4.0] - 2026-04-06

- fix(napi): bump napi-derive to 3.x to match napi 3.8.4 (#338) (b7e0b91)
- feat(wasm): expose Agens runtime bindings (#289) (e53cfa5)
- ci: standardize Node version to lts/* — remove hardcoded versions (9a37010)
- chore(deps): bump napi from 2.16.17 to 3.8.4 (#303) (eec14de)
- chore(deps): bump clap from 4.5.60 to 4.6.0 (#307) (59119e5)
- chore(deps): bump tokio-tungstenite from 0.28.0 to 0.29.0 (#308) (1f552ed)
- chore(deps): bump chrono from 0.4.42 to 0.4.44 (#309) (8e1460a)
- chore(deps): bump tower-http from 0.5.2 to 0.6.8 (#310) (ea4d21a)
- chore(deps): bump tokio from 1.47.1 to 1.50.0 (#311) (9acd05c)
- chore(deps): bump criterion from 0.5.1 to 0.8.2 (#312) (2de9ee5)

## [3.3.0] - 2026-04-06

- feat(node): expose AgensRuntime via NAPI — events, state, timers (#323) (dfa2de0)
- chore(deps-dev): bump vite from 7.2.2 to 7.3.2 in /web/svelte in the npm_and_yarn group across 1 directory (#334) (1f691cf)
- ci: skip lifecycle workflow for Dependabot PRs (#324) (18e4a05)
- chore(deps): bump lodash (#301) (c7d13d5)
- ci: tech-doc-writer triggers on minor prerelease only [actions-optimization] (d8c43d2)
- ci: add concurrency group to copilot-pr-lifecycle [actions-optimization] (fd82ba5)

## [3.2.0] - 2026-04-02

- feat: P2P sync integration test harness (Hyperswarm mesh + Relay transport) (#299) (22f16a1)
- fix(release): prevent stale local tags and duplicate CHANGELOG entries on rebase (#297) (2dd87cd)
- ci: improve bot-authored PR skip message in pr-lint.yml (#295) (e043ffb)
- ci: trigger pr-lint on ready_for_review to surface bot-PR rename reminder at merge time (#296) (22ca372)
- Initial plan (#294) (45d06b7)
- fix(ci): replace fragile reusable workflow with robust self-contained release job (#293) (26b2044)
- fix(ci): auto-close stale release failure issues on successful release (#292) (ff45a73)
- fix(ci): repair broken notify-on-failure script in release.yml (#290) (eb49621)
- fix(ci): repair broken notify-on-failure script in release.yml (#291) (5bef719)

## [3.1.2] — 2026-04-02

- fix(ci): serialize releases and restore failure notifications (#288) (aba9d6a)
- ci: centralize lifecycle — event-driven with schedule guard (e087c20)

## [3.1.1] — 2026-04-01

- refactor: centralize lifecycle — call reusable from plures/repo-template (8802bbe)

## [3.1.0] — 2026-04-01

- feat: unify PluresDB to Rust-first v3.0.0 (#281) (b629e22)

## [2.17.4] — 2026-04-01

- fix: lifecycle v4.4 — catch self-approval error, don't crash on own PRs (b8c9419)

## [2.17.3] — 2026-04-01

- fix: lifecycle v4.3 — guard notify step, escape PR title in JSON (a5c3b98)

## [2.17.2] - 2026-04-01

### Fixed

- prevent "tag already exists" failure in version-bump-and-tag job (#283)
- lifecycle v4.2 — filter out release/publish checks from CI evaluation


## [2.17.1] — 2026-04-01

- fix: lifecycle v4.1 — process all PRs independently, add Path F debug logging (4bfc41d)

## [2.17.0] — 2026-04-01

- feat: lifecycle v4 — merge all PRs, Copilot default reviewer, no nudges (ac36328)

## [2.16.11] - 2026-04-01

### Developer Experience

- skip PR title lint for bot-authored PRs to prevent feedback loop (#278)


## [2.16.10] — 2026-04-01

- fix(ci): skip PR title lint for draft PRs to prevent spurious failures (#276) (9aa58f5)

## [2.16.9] - 2026-03-31

### Fixed

- v9.1 — fix QA dispatch (client_payload as JSON object)


## [2.16.8] - 2026-03-31

### Fixed

- rewrite v9 — apply suggestions, merge, no nudges


## [2.16.7] - 2026-03-31

### Fixed

- fail explicitly when rebase fails during version-bump retry (#273)


## [2.16.6] — 2026-03-31

- fix(ci): rebase release commit on remote changes before retrying push (#271) (b5cb8de)

## [2.16.5] - 2026-03-31

### Fixed

- document PR #268 lifecycle auto-title-fix in CHANGELOG 2.16.4 (#270)


## [2.16.4] - 2026-03-31

### Fixed

- document PR #268 lifecycle auto-title-fix in CHANGELOG 2.16.3 (#270)


## [2.16.3] - 2026-03-31

### Fixed

- document PR #263 lifecycle auto-title-fix in CHANGELOG 2.16.1 (#265)


## [2.16.2] — 2026-03-30

- fix(ci): auto-fix Copilot PR titles to follow Conventional Commits format (#263) (ade7f2a)

## [2.16.1] - 2026-03-30

### Fixed

- resolve conventional commit title check failure (#261)
- auto-fix Copilot PR titles to follow Conventional Commits format via lifecycle workflow (#263)


## [2.16.0] - 2026-03-29

- feat: performance benchmark suite for CRUD/query/sync (#256) (91a2a31)
- fix(ci): resolve conventional commit title check failure (#261) (bee772a)
- chore(deps-dev): bump typescript from 5.9.3 to 6.0.2 (#260) (2843b1c)
- chore(deps): bump @plures/praxis from 2.4.35 to 2.4.39 (#259) (67036ec)

# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Initial changelog

## [2.15.21] - 2026-03-28

### Developer Experience

- dual license — Rust crates BSL 1.1, TypeScript MIT


## [2.15.20] - 2026-03-28

### Developer Experience

- standardize license to MIT
- standardize license to MIT


## [2.15.19] - 2026-03-28

### Fixed

- npm audit fix — update path-to-regexp (high severity)


## [2.15.18] - 2026-03-28

### Developer Experience

- standardize copilot-pr-lifecycle.yml to canonical version


## [2.15.17] - 2026-03-27

### Documentation

- API reference + migration guides for Node/Deno/Rust (#255)


## [2.15.16] - 2026-03-27

### Documentation

- improve api-documented coverage to ≥90% (#236)


## [2.15.15] - 2026-03-27

### Developer Experience

- standardize changelog format


## [2.15.14] - 2026-03-27

### Developer Experience

- standardize CI workflow


## [2.15.13] - 2026-03-27

### Fixed

- lint-clean 0% → 100% — expand ESLint scope to full codebase (#233)


## [2.15.12] - 2026-03-27

### Fixed

- downgrade typescript to ^5.8.0 to unblock CI and release pipeline (#231)

### Developer Experience

- bump proc-macro2 from 1.0.101 to 1.0.106 (#230)
- bump tempfile from 3.23.0 to 3.27.0 (#228)
- bump serde_json from 1.0.145 to 1.0.149 (#227)
- bump clap from 4.5.48 to 4.5.60 (#226)
- bump uuid from 1.18.1 to 1.23.0 (#224)
- bump axum from 0.8.6 to 0.8.8 (#225)
- bump hyperswarm from 4.16.0 to 4.17.0 (#223)
- bump typescript from 5.9.3 to 6.0.2 (#222)
- bump rusqlite from 0.32.1 to 0.39.0 (#221)
- bump ws from 8.18.3 to 8.20.0 (#220)
- bump fastembed from 5.11.0 to 5.13.0 (#219)
- bump @types/vscode from 1.104.0 to 1.110.0 (#218)
- bump prettier-plugin-svelte from 3.4.0 to 3.5.1 (#217)
- bump thiserror from 1.0.69 to 2.0.17 (#216)
- bump @types/node from 22.19.11 to 25.5.0 (#215)
- bump @plures/praxis from 1.4.0 to 2.4.35 (#214)
- bump cors from 2.8.5 to 2.8.6 (#213)


## [2.15.11] - 2026-03-27

### Developer Experience

- bump prettier from 3.6.2 to 3.8.1 (#212)
- bump @types/express from 5.0.5 to 5.0.6 (#211)


## [2.15.10] - 2026-03-27

### Fixed

- remediate no-known-vulns dimension — dependabot, audit CI, bincode 1→2 (#207)
- lint-clean 0% → 100% (ESLint + Clippy) (#210)


## [2.15.9] - 2026-03-26

### Developer Experience

- apply org-standard automation files (#205)


## [2.15.8] - 2026-03-26

### Fixed

- remediate brace-expansion ReDoS/DoS vulnerabilities in dev toolchain (GHSA-f886-m6hf-6m8v) (#206)


## [2.15.7] - 2026-03-26

### Documentation

- improve api-documented coverage from 0% → 90%+ (#202)


## [2.15.6] - 2026-03-26

### Fixed

- bump ajv 6.12.6 → 6.14.0 (GHSA-2g4f-4pwh-qvx6 ReDoS) (#199)


## [2.15.5] - 2026-03-26

### Developer Experience

- auto-close stale ci-feedback issues for merged PRs (#198)


## [2.15.4] - 2026-03-26

### Developer Experience

- auto-correct PR titles that fail conventional commit check (#196)


## [2.15.3] - 2026-03-26

### Other

- [WIP] Fix CI failures on PR #190 (#193)


## [2.15.2] - 2026-03-26

### Developer Experience

- bump the npm_and_yarn group across 2 directories with 1 update (#195)


## [2.15.1] - 2026-03-26

### Documentation

- improve api-documented dimension from 0% → 90% (#190)


## [2.15.0] - 2026-03-25

### Added

- add auto-approve workflow for Copilot runs
- add Copilot PR Lifecycle workflow with level/strategic buckets


## [2.14.1] - 2026-03-21

### Developer Experience

- bump rustls-webpki in the cargo group across 1 directory (#180)


## [2.14.0] - 2026-03-21

### Added

- Adopt @plures/praxis for declarative logic management (#179)


## [2.13.0] - 2026-03-21

### Added

- add hybrid storage backend with ObjectBridge (#165)


## [2.12.1] - 2026-03-20

### Developer Experience

- bump flatted (#177)


## [2.12.0] - 2026-03-20

### Added

- AI-specific procedures — decision audit, RL extraction, self-tuning (#167)


## [2.11.0] - 2026-03-19

### Added

- cognitive architecture step types (#163)


## [2.10.0] - 2026-03-18

### Added

- improve repository best practices compliance (40% → 90%+) (#151)

### Developer Experience

- bump the npm_and_yarn group across 2 directories with 1 update (#152)


## [2.9.10] - 2026-03-10

### Documentation

- fix npm package references and clarify version channels


## [2.9.9] - 2026-03-07

### Developer Experience

- add PR lane event relay to centralized merge FSM


## [2.9.8] - 2026-03-06

### Developer Experience

- bump bytes in the cargo group across 1 directory (#149)


## [2.9.7] - 2026-03-06

### Fixed

- add dist/ and scripts/postinstall.js to npm package files whitelist (#148)


## [2.9.6] — 2026-03-04



## [2.9.5] — 2026-03-04

- chore(release): 2.9.4 (368ab69)
- fix(release): add per-branch concurrency lock to prevent duplicate tag races (e9127cc)

## [2.9.4] - 2026-03-04

### Fixed

- add per-branch concurrency lock to prevent duplicate tag races


## [2.9.3] — 2026-03-04



## [2.9.2] — 2026-03-04



# Changelog

## [2.9.1] - 2026-03-04

### Fixed

- use github.event.inputs for push-compatible reusable release call


## [2.9.0] - 2026-03-03

### Added

- durable event contracts for Praxis analysis lifecycle (#146)


## [2.8.0] - 2026-03-02

### Added

- PluresDB/pluresLM architectural separation + Rust GUN.js networking (#144)


## [2.7.0] - 2026-03-02

### Added

- GUN.js wire-compat SEA, RAD adapter, blob CAS, git replication protocol (#142)


## [2.6.0] - 2026-03-01

### Added

- add training data processing procedures (#140)


## [2.5.6] - 2026-03-01

### Changed

- make embeddings + vector index eventual (async), not in put() hot path (#139)


## [2.5.5] - 2026-03-01

### Fixed

- add explicit `none` option to workflow_dispatch bump input (#137)


## [2.5.4] - 2026-03-01

### Fixed

- harden org release workflow migration (#135)

### Developer Experience

- disable automatic triggers on deprecated publish workflows (#132)


## [2.5.3] - 2026-03-01

### Fixed

- add permissions to release.yml caller workflow (#133)


## [2.5.2] - 2026-02-28

### Developer Experience

- migrate pluresdb to org release workflow (#129)


## [2.5.1] - 2026-02-28

### Fixed

- release pipeline should publish reliably (#128)


## [2.5.0] - 2026-02-28

### Added

- Rust sync transport + minimal GUN-compatible wire protocol (phase 1) (#127)


## [2.4.0] - 2026-02-28

### Added

- add document storage and enrichment procedures (#125)


## [2.3.1] - 2026-02-27

### Developer Experience

- bump the npm_and_yarn group across 2 directories with 3 updates (#122)


## [2.3.0] - 2026-02-26

### Added

- Pares Agens procedure execution API in pluresdb-procedures (#120)

### Developer Experience

- repo cleanup (#118)


## [2.2.0] - 2026-02-25

### Added

- Implement Phase 2B — graph clustering, path finding, PageRank, and stats (#117)


## [2.1.3] - 2026-02-25

### Other

- Add Phase 2A graph operations and comprehensive test coverage (#115)


## [2.1.2] - 2026-02-25

### Fixed

- NAPI binary build with embeddings support (#113)


## [2.1.1] - 2026-02-24

### Other

- cargo fix --lib -p pluresdb-storage


## [2.1.0] - 2026-02-24

### Added

- implement pluresdb-procedures crate — Phase 1 (DSL parser + core ops) (#111)


## [2.0.0] - 2026-02-24

### Other

- feat!: Remove SQLite dependency — wire CrdtStore to pluresdb-storage (v2.0.0) (#109)


## [2.0.0] - 2026-02-24

### BREAKING CHANGES

- **`CrdtStore::with_persistence` now accepts `Arc<dyn StorageEngine>`** instead
  of `Arc<Database>`.  The method now returns `Self` (infallible) instead of
  `Result<Self, DatabaseError>`.
- **`rusqlite` is no longer a default dependency** of `pluresdb-core`.  The
  `Database`, `SqlValue`, `QueryResult`, `ExecutionResult`, `DatabasePath`,
  `DatabaseOptions`, and `DatabaseError` types are only available when the
  `sqlite-compat` cargo feature is enabled.
- **`pluresdb-node`**: `query()` and `exec()` methods return an error at runtime
  unless the crate is compiled with the `sqlite-compat` feature.  The
  constructor now opens a sled store instead of a SQLite file when `db_path`
  is provided.
- **Version bumped from 1.15.0 → 2.0.0**.

### Added

- `sqlite-compat` feature flag in `pluresdb-core`, `pluresdb-node`,
  `pluresdb-cli`, and `pluresdb` umbrella crate.
- `pluresdb migrate-from-sqlite --source <path> --target <dir>` CLI command
  (requires `sqlite-compat` feature) to migrate v1.x SQLite databases to sled.
- New tests in `pluresdb-core` covering storage-engine-backed persistence
  (`MemoryStorage`) without requiring SQLite.
- `MIGRATION.md` upgrade guide for v1.x → v2.0 consumers.

### Changed

- `CrdtStore` persistence layer now uses `pluresdb-storage::StorageEngine`
  (sled-backed) instead of a SQLite `Database`.
- `MemoryStorage` now uses `parking_lot::RwLock` for synchronous interior
  access, eliminating the tokio runtime dependency from storage reads/writes.
- `SledStorage` now uses synchronous `sled::Db::flush()` instead of
  `flush_async()`.
- README updated: removed "SQLite Compatibility" from tagline; updated
  architecture table and quick-start example.
- Workspace and crate descriptions updated to remove "SQLite Compatibility".

### Removed

- `SQLITE_ACTOR` constant from `pluresdb-core` (no longer needed — full
  `NodeRecord` is serialised and deserialised from storage).

---

## [Unreleased]

### Breaking Changes

- ⚠️ **Renamed `GunDB` class to `PluresDB`** - The primary database class in `legacy/core/database.ts` has been renamed from `GunDB` to `PluresDB`. Consumers must update their imports: `import { PluresDB } from "pluresdb"`. A deprecated `GunDB` re-export is available for one major version to ease migration.
- Renamed `demo/gun-comparison.html` to `demo/benchmark-comparison.html`.

### Added

- ✅ **Complete implementation of pluresdb-node** - Full Node.js bindings with N-API
  - Complete CRUD operations
  - SQL query support (query, exec)
  - Metadata access (getWithMetadata)
  - Type filtering (listByType)
  - Text search with scoring
  - Vector search placeholder
  - Database statistics
  - Subscription infrastructure
  - Comprehensive TypeScript definitions
  - Full test suite

- ✅ **Complete implementation of pluresdb-deno** - Full Deno bindings with deno_bindgen
  - Complete CRUD operations
  - SQL query support (query, exec)
  - Metadata access (getWithMetadata)
  - Type filtering (listByType)
  - Text search with scoring
  - Vector search placeholder
  - Database statistics
  - SyncBroadcaster integration
  - Automatic TypeScript bindings generation
  - Comprehensive test suite

- ✅ **pluresdb-storage** - Ready for publishing
  - Complete storage abstraction layer
  - MemoryStorage and SledStorage backends
  - Encryption support
  - WAL and replay system
  - Full documentation

- ✅ **pluresdb-cli** - Ready for publishing
  - Complete command-line interface
  - All CRUD operations
  - SQL query execution
  - Search and vector search
  - Type system commands
  - Network commands
  - Configuration management
  - Maintenance commands
  - API server with Axum
  - Full documentation

- Added crates.io publishing workflow for Rust crates (pluresdb-core, pluresdb-storage, pluresdb-sync, pluresdb-cli)
- Added CARGO_REGISTRY_TOKEN configuration to release workflow
- Added comprehensive publishing guide (crates/PUBLISHING_GUIDE.md)
- Added README files for all crates

### Changed

- Updated release workflow to publish to crates.io in addition to npm, Docker Hub, JSR, and GitHub Releases
- Synchronized version across package.json (1.3.8), Cargo.toml (1.3.8), and deno.json (1.3.8)
- Updated RELEASE_PROCESS.md to document crates.io publishing workflow
- Updated README.md with crates.io badge and installation instructions
- Enhanced release channel documentation to include all platforms: npm, crates.io, JSR, Docker Hub, winget, and GitHub Releases
- Updated CI workflow to sync deno.json version during automated version bumps

### Fixed

- Fix release workflow entry point paths (legacy/main.ts)
- Fix Dockerfile to use correct entry point
- Fix Windows binary compilation in release workflow
- Add error checking for binary compilation failures
- Fix compilation error in pluresdb-cli (Result type in closure)

## [1.15.0] - 2026-02-24

### Added

- eliminate startup hydration — query SQLite directly from CrdtStore (#107)


## [1.14.1] - 2026-02-24

### Fixed

- add ./embedded export to package.json


## [1.14.0] - 2026-02-24

### Added

- add SQLite persistence to CrdtStore


## [1.13.0] - 2026-02-23

### Added

- expose embed() and embeddingDimension() via NAPI bindings


## [1.12.3] - 2026-02-23

### Fixed

- FastEmbedder compile errors — Mutex for interior mutability, manual Debug impl


## [1.12.2] - 2026-02-23

### Developer Experience

- remove transient progress/completion markdown from root and docs/ (#97)


## [1.12.1] - 2026-02-23

### Documentation

- comprehensive documentation overhaul — architecture, API reference, getting started (#99)


## [1.12.0] - 2026-02-23

### Added

- make pluresdb-node NAPI bindings the primary Node.js interface (#105)


## [1.11.1] - 2026-02-23

### Fixed

- resolve pluresdb-wasm IndexedDB web-sys API compatibility issues (#103)

### Other

- breaking: rename GunDB → PluresDB across the codebase (#95)


## [1.11.0] - 2026-02-22

### Added

- auto-embedding on insert via pluggable EmbedText backend (fastembed, feature-gated) (#93)


## [1.10.0] - 2026-02-21

### Added

- native HNSW vector index and similarity search (#92)


## [1.9.9] - 2026-02-21

### Developer Experience

- bump the npm_and_yarn group across 2 directories with 1 update (#89)


## [1.9.8] - 2026-02-18

### Other

- Add Transport trait abstraction for P2P sync with hyperswarm-rs integration points (#87)


## [1.9.7] - 2026-02-17

### Other

- Complete Rust core migration for V2.0 - 10x performance, 80% memory reduction (#85)


## [1.9.6] - 2026-02-17

### Other

- Skip Hyperswarm P2P tests in CI environments (#84)


## [1.9.5] - 2026-02-16

### Fixed

- Azure Credentials Not Configured - Scheduled Tests Skipped #81


## [1.9.4] - 2026-02-16

### Other

- Skip Hyperswarm test in Deno to fix release pipeline (#80)


## [1.9.3] - 2026-02-16

### Documentation

- add comprehensive DESIGN.md and ROADMAP.md


## [1.9.2] - 2026-02-14

### Other

- Fix TypeScript type errors and Deno compatibility issues blocking release (#78)


## [1.9.1] - 2026-02-14

### Developer Experience

- bump qs in the npm_and_yarn group across 1 directory (#76)

### Other

- Build and publish @plures/pluresdb-native N-API binary (#75)


## [1.9.0] - 2026-02-14

### Added

- P2P sync transport via Hyperswarm (DHT discovery + NAT holepunching) (#73)


## [1.8.0] - 2026-02-14

### Added

- pluggable sync transports — Azure relay (WSS:443), Vercel relay, Hyperswarm direct (#72)


## [1.7.1] - 2026-02-10

### Other

- Fix Azure relay tests authentication detection (#67)


## [1.7.0] - 2026-02-09

### Added

- add embedded export for pure embedded database usage (#69)


## [1.6.11] - 2026-01-30

### Other

- Add PowerShell and Bash modules for command history tracking with P2P sync (#66)


## [1.6.10] - 2026-01-26

### Other

- Fix npm publish failures from Deno/TypeScript import incompatibility (#64)


## [1.6.9] - 2026-01-26

### Fixed

- npm publish failures in unified-api.ts (#62)


## [1.6.8] - 2026-01-26

### Other

- Fix Deno compilation errors: process global access and duplicate exports (#60)


## [1.6.7] - 2026-01-26

### Other

- Initial plan (#58)


## [1.6.6] - 2026-01-25

### Fixed

- Remove duplicate closing button tag in ExampleDatasets.svelte (#56)


## [1.6.5] - 2026-01-25

### Other

- Complete local-first integration roadmap and refactor README.md (#54)


## [1.6.4] - 2026-01-25

### Other

- Document accurate local-first integration status: 90% complete (Rust done, TS integration pending) (#52)


## [1.6.3] - 2026-01-25

### Other

- Complete local-first integration: WASM IndexedDB, IPC shared memory, Tauri demos (#50)


## [1.6.2] - 2026-01-25

### Other

- Add WASM and IPC integration infrastructure for local-first operation (#48)


## [1.6.1] - 2026-01-25

### Other

- Add local-first integration API with runtime auto-detection (#46)


## [1.6.0] - 2026-01-25

### Added

- Complete project roadmap with guided tour, example datasets, E2E tests, and plugin system (#44)


## [1.5.8] - 2026-01-25

### Other

- Update roadmap with checklist items for enhancements


## [1.5.7] - 2026-01-25

### Other

- Update roadmap with optional notebooks for queries


## [1.5.6] - 2026-01-25

### Fixed

- gracefully skip Azure relay tests when credentials not configured + migrate to OIDC (#41)


## [1.5.5] - 2026-01-25

### Developer Experience

- bump lodash (#40)


## [1.5.4] - 2026-01-11

### Other

- Fix Azure Login authentication in relay tests workflow (#36)


## [1.5.3] - 2026-01-11

### Fixed

- Remove path dependencies from pluresdb crate for publishing


## [1.5.2] - 2026-01-11

### Other

- [WIP] Fix release-check errors in CI job (#38)


## [1.5.1] - 2026-01-11

### Documentation

- Add release trigger instructions

### Developer Experience

- Bump version to 1.4.3 for new release
- publish instr
- Add pluresdb unified crate to release workflow publishing order

### Other

- Release ready for publishing


## [1.5.0] - 2026-01-11

### Added

- Complete all missing crate implementations and add unified pluresdb crate


## [1.4.2] - 2026-01-10

### Other

- Update dependencies for create-github-release step


## [1.4.1] - 2026-01-10

### Other

- Update npm badge to reflect new scope


## [1.4.0] - 2026-01-10

### Added

- add crates.io publishing and synchronize versions across all manifests (#34)


## [1.3.8] - 2026-01-10

### Other

- Complete Pre-flight Hardening: PluresDB as Exclusive Local-First Agent Memory Store (#29)


## [1.3.7] - 2026-01-10

### Other

- Fix Azure Login authentication format for azure/login@v1 (#30)


## [1.3.6] - 2026-01-10

### Other

- Add Copilot instructions for repository (#32)


## [1.3.5] - 2026-01-10

### Developer Experience

- bump qs in the npm_and_yarn group across 1 directory (#26)


## [1.3.4] - 2025-12-29

### Other

- Change package name to @plures/pluresdb


## [1.3.3] - 2025-12-29

### Other

- Add Azure infrastructure automation and P2P relay testing (#25)


## [1.3.2] - 2025-12-27

### Other

- Sync versions to 1.3.1 and update README to reference changelog (#23)


## [1.3.1] - 2025-12-27

### Other

- Bump version from 1.0.1 to 1.3.0


## [1.3.0] - 2025-12-25

### Added

- implement Node.js N-API bindings for Rust core


## [1.2.10] - 2025-12-16

### Other

- Implement missing P2P API methods from README (#21)


## [1.2.9] - 2025-12-16

### Other

- Align README with current release channels and monorepo structure (#19)


## [1.2.8] - 2025-12-16

### Developer Experience

- bump body-parser (#17)


## [1.2.7] - 2025-11-15

### Developer Experience

- bump js-yaml (#16)


## [1.2.6] - 2025-11-15

### Other

- Fix formatting in dependabot.yml


## [1.2.5] - 2025-11-14

### Developer Experience

- update Cargo.lock to version 1.2.4


## [1.2.4] - 2025-11-14

### Other

- Add secrets configuration documentation


## [1.2.3] - 2025-11-14

### Other

- Fix GitHub Actions workflow: Replace invalid secret checks with step outputs


## [1.2.2] - 2025-11-14

### Fixed

- correct release workflow paths and improve reliability


## [1.2.1] - 2025-11-14

### Fixed

- correct entry point paths and improve release workflow reliability


## [1.2.0] - 2025-11-14

### Added

- Complete P2P Ecosystem & Comprehensive Packaging System
- Complete Phase 1 UI with WCAG AA accessibility and inline schema validation

### Developer Experience

- capture alt imp
- capture alt imp
- attribute
- checkpoint
- release 1.0.1
- apply merge-driven renames/branding (rusty-gun → pluresdb) and update packaging scripts
- remove tracked build artifacts (target/) and update .gitignore
- checkpoint
- reorg
- checkpoint
- plan
- checkpoint

### Other

- Fix release tag push by separating commit and tag operations (#15)
- Automate version bumping, tagging, and multi-platform releases (#13)
- Upgrade to Deno 2.x and latest package versions (#11)
- Complete the rust implementation (#9)
- Remove legacy rusty-gun directory and rebrand to PluresDB (#6)
- Fix CI failures: deno formatting, lint configuration, and test workflow (#8)
- Merge pull request #4 from plures/copilot/develop-personal-database
- Revert legacy/cli.ts to use Node.js APIs instead of Deno APIs
- Fix Deno lint errors: replace process with Deno, fix unused error vars, remove unnecessary async
- Fix CI failures: add npm lint/fmt scripts and upgrade Deno to v2.x
- Add issue response document summarizing Windows personal database readiness
- Add comprehensive Windows personal database status document
- Add Windows-specific documentation and launcher files
- Fix TypeScript compilation errors and add Windows documentation
- Initial plan
- Merge branch 'feature/better-sqllite3-support' of https://github.com/plures/pluresdb
- Merge pull request #1 from plures/copilot/restructure-pluresdb-project
- Add restructuring summary document
- Update config files to reference legacy directory instead of src
- Restructure project: move src to legacy, create pluresdb-node and pluresdb-deno crates
- Initial plan
- Merge branch 'main' of https://github.com/kayodebristol/rusty-gun
- Resolve merge conflicts: accept current branch (ours) for all files
- Merge pull request #5 from kayodebristol/revert-3-copilot/fix-cf2f0af2-221d-47ce-8064-4d58ed05c1d6
- Revert "[WIP] Rename nested 'rusty-gun' folder to 'pluresdb' and update references"
- Merge pull request #4 from kayodebristol/copilot/fix-db1e13fe-eea5-47d2-8ddf-18e1b5f69493
- Create five GitHub Actions workflows as specified
- Initial plan
- Merge pull request #3 from kayodebristol/copilot/fix-cf2f0af2-221d-47ce-8064-4d58ed05c1d6
- Initial plan
- Merge pull request #2 from kayodebristol/copilot/fix-edf5eacd-b439-43e7-af90-57eac7d6efb7
- Complete PluresDB rebrand: update packaging files, env vars, and Svelte components
- Update all branding references from rusty-gun to PluresDB throughout the codebase
- Remove .githooks, azure directories and update core branding to PluresDB
- Initial plan
- Phase 2 Complete: Data Modeling & Insight
- checkpoint
- new roadmap
- new reactive ui
- Checkpoint Continue Deno version
- Initial
- first commit


## [1.1.0] - 2025-11-14

### Added

- Complete P2P Ecosystem & Comprehensive Packaging System
- Complete Phase 1 UI with WCAG AA accessibility and inline schema validation

### Developer Experience

- capture alt imp
- capture alt imp
- attribute
- checkpoint
- release 1.0.1
- apply merge-driven renames/branding (rusty-gun → pluresdb) and update packaging scripts
- remove tracked build artifacts (target/) and update .gitignore
- checkpoint
- reorg
- checkpoint
- plan
- checkpoint

### Other

- Automate version bumping, tagging, and multi-platform releases (#13)
- Upgrade to Deno 2.x and latest package versions (#11)
- Complete the rust implementation (#9)
- Remove legacy rusty-gun directory and rebrand to PluresDB (#6)
- Fix CI failures: deno formatting, lint configuration, and test workflow (#8)
- Merge pull request #4 from plures/copilot/develop-personal-database
- Revert legacy/cli.ts to use Node.js APIs instead of Deno APIs
- Fix Deno lint errors: replace process with Deno, fix unused error vars, remove unnecessary async
- Fix CI failures: add npm lint/fmt scripts and upgrade Deno to v2.x
- Add issue response document summarizing Windows personal database readiness
- Add comprehensive Windows personal database status document
- Add Windows-specific documentation and launcher files
- Fix TypeScript compilation errors and add Windows documentation
- Initial plan
- Merge branch 'feature/better-sqllite3-support' of https://github.com/plures/pluresdb
- Merge pull request #1 from plures/copilot/restructure-pluresdb-project
- Add restructuring summary document
- Update config files to reference legacy directory instead of src
- Restructure project: move src to legacy, create pluresdb-node and pluresdb-deno crates
- Initial plan
- Merge branch 'main' of https://github.com/kayodebristol/rusty-gun
- Resolve merge conflicts: accept current branch (ours) for all files
- Merge pull request #5 from kayodebristol/revert-3-copilot/fix-cf2f0af2-221d-47ce-8064-4d58ed05c1d6
- Revert "[WIP] Rename nested 'rusty-gun' folder to 'pluresdb' and update references"
- Merge pull request #4 from kayodebristol/copilot/fix-db1e13fe-eea5-47d2-8ddf-18e1b5f69493
- Create five GitHub Actions workflows as specified
- Initial plan
- Merge pull request #3 from kayodebristol/copilot/fix-cf2f0af2-221d-47ce-8064-4d58ed05c1d6
- Initial plan
- Merge pull request #2 from kayodebristol/copilot/fix-edf5eacd-b439-43e7-af90-57eac7d6efb7
- Complete PluresDB rebrand: update packaging files, env vars, and Svelte components
- Update all branding references from rusty-gun to PluresDB throughout the codebase
- Remove .githooks, azure directories and update core branding to PluresDB
- Initial plan
- Phase 2 Complete: Data Modeling & Insight
- checkpoint
- new roadmap
- new reactive ui
- Checkpoint Continue Deno version
- Initial
- first commit


## [1.0.1] - 2025-10-03 — Core Security Hardening

### Changed

- Added payload sanitization before persistence to strip prototype pollution vectors and coerce injected functions into safe string placeholders.
- Hardened `PluresDB#get` responses with sanitized clones, ensuring consumer code receives benign `toString` implementations and no inherited attacker-controlled state.
- Expanded the security regression suite so the type-confusion prevention scenario now exercises the sanitization path and passes under `npm run verify` (51 tests green).

## [Unreleased] - Phase 1 UI Completion ✅

**Phase 1 is now COMPLETE!** All planned UI foundation and UX polish items have been implemented.

## [Unreleased] - Phase 1 Part 2: Accessibility & Validation

### Added - UI Foundation & UX Polish ✅

- **Accessibility Enhancements**
  - Keyboard navigation with arrow keys, Enter/Space for selection across all panels
  - Comprehensive ARIA labels, roles, and landmark regions throughout the UI
  - Screen reader support with sr-only class and aria-live regions for dynamic content
  - Semantic HTML structure with proper heading hierarchy
- **Node List Improvements**
  - Sort controls for ID and Type with visual indicators (↑/↓)
  - Enhanced keyboard navigation (ArrowUp/ArrowDown to navigate, Enter/Space to select)
  - Proper listbox/option ARIA roles for better assistive technology support
- **Editor Enhancements**
  - Copy-as-cURL button to generate curl commands for API calls
  - Revert changes button with change tracking (disabled when no changes)
  - Visual indication of unsaved changes
  - Tooltips on all editor action buttons
- **Search Panel Improvements**
  - Keyboard navigation for search results (Enter/Space to select)
  - Live result count announcement for screen readers
  - Proper ARIA labels for search input and results

- **Settings Panel Improvements**
  - Live save status announcement for screen readers
  - Descriptive help text for all configuration fields
  - Enhanced ARIA descriptions for all inputs

- **Main Navigation**
  - Proper menubar role with aria-current for active view indication
  - Enhanced dark mode toggle with descriptive aria-label

### Changed

- Reorganized editor toolbar into two rows (formatting actions + node actions)
- Delete button now has outline style to differentiate destructive action
- Improved reactive text handling to preserve unsaved changes when navigating

### Technical

- Built with Svelte 4, CodeMirror 6, Vite 5
- All components now follow WCAG 2.1 accessibility guidelines
- Proper separation of concerns with dedicated stores and components

### Added - Accessibility & Validation ✅

- **WCAG AA Color Contrast**
  - GitHub-inspired color palette with verified 4.5:1 minimum contrast ratios
  - Enhanced primary colors: #0969da (light) / #58a6ff (dark)
  - Improved muted text colors for better readability
  - Enhanced focus indicators (2px outline with offset)
  - Semantic colors for success/error/warning states
  - Accessible disabled states with proper opacity

- **Inline JSON Schema Validation**
  - Real-time validation in CodeMirror as you type
  - Inline error markers with CodeMirror's linter system
  - JSON syntax validation with position-aware error messages
  - Schema validation warnings when schema is provided
  - Automatic revalidation when schema or content changes
  - Clear error messages showing path and validation issue

### Technical Improvements

- Added @codemirror/lint package for inline diagnostics
- Integrated Ajv JSON Schema validator with CodeMirror linter
- Custom CSS variables for WCAG AA compliant color system
- Reactive schema updates trigger editor reconfiguration

### Phase 1 Status: ✅ COMPLETE

All Phase 1 deliverables have been implemented and tested. The UI now has:

- Comprehensive accessibility (keyboard nav, ARIA, WCAG AA colors)
- Real-time JSON Schema validation
- Professional data explorer with all planned features

## Previous Releases

See git history for earlier changes.
