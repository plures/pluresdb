# Mutation-testing baselines

Each `<crate>.survivors` file holds a single integer: the number of **surviving
mutants** (`MissedMutant`) currently tolerated for that crate by the
`Mutation Testing Gate` workflow (`.github/workflows/mutation-testing.yml`).

## Why baselines exist (ratchet, not a 100%-kill gate)

The critical-path crates (`pluresdb-core`, `pluresdb-storage`, `pluresdb-px`) do
**not** yet achieve a >95% mutation kill rate. A sampled run on a single file
(`crates/pluresdb-storage/src/rad.rs`, 19 mutants, 2026-06-28) already produced
**6 missed / 7 caught / 6 unviable (~54% kill)**. A "zero survivors" gate would
fail every merge to `main` from day one — a false gate, not foundation integrity.

So the gate is a **ratchet**:

- **First run (no baseline file):** the workflow records the current survivor
  count as the seed, prints it in the job summary, and **passes**. A maintainer
  commits that number here to arm the ratchet.
- **Subsequent runs:** the gate **fails only on regression** — when a change
  introduces *more* survivors than the committed baseline. When a run shows
  *fewer* survivors, it emits a notice to lower the baseline and lock in the gain.

## The goal

Drive these numbers **down to 0** (>=95% kill, the Level-0 foundation-integrity
bar in `development-guide/practices/WORK-PRIORITIZATION.md`). When a crate's
baseline reaches 0, the ratchet has effectively become a 100%-kill gate for it.

## Updating a baseline

1. Read the survivor count from the workflow's job summary (or an
   `actions/download-artifact` of `mutants-report-<crate>`).
2. Only ever **lower** a baseline (locking in real test improvements). Raising a
   baseline means accepting a regression — do that only with explicit review.
