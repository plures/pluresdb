# ADR-0016: Hardware-Adaptive Compute — Self-Optimizing GPU/NPU/CPU Kernels in PluresDB

- **Status:** Accepted
- **Date:** 2026-06-17
- **Deciders:** kbristol (Paradox), mswork
- **Related:** ADR-0015 (queue-driven dataflow), pares-umbra ADR-0009 (self-improvement loop), pares-umbra ADR-0010 (bitnet parallel research)

## Context

kbristol: *"we need GPU / NPU support for procedures and/or pluresDB. I envision a set of procedures that self optimize for the available hardware."*

### Evidence table (tested facts)

| Fact | Evidence | Confidence |
|---|---|---|
| Zero hardware acceleration exists today | grep across pares-umbra + pluresdb for `gpu/cuda/rocm/vulkan/simd/wgpu/npu/avx/rayon` -> nothing in compute paths | tested (grep) |
| The hottest kernel is scalar single-core | `pares-umbra/crates/umbra-bitnet/src/weights.rs` `matvec`: nested `for r / for c` loop doing `sum += input[c]` / `sum -= input[c]` / skip, one element at a time | tested (read) |
| Ternary matmul is the most accel-friendly op possible | weights {-1,0,+1} => no multiplies, just add/subtract/skip => trivially vectorizable and GPU-parallel | analysis |
| praxisbot has idle parallel hardware | Ryzen 9 7900X (24 threads) + RX 7900 XT (20GB, ~5,376 shaders), idle ~23h/day | known |
| PluresDB is the foundation (C-PLURES) | "PluresDB IS the system... extend PluresDB, don't build beside it" | policy |

### The opportunity

The single most-used compute primitive in the stack (ternary matmul for BitNet, and any heavy procedure) runs **one element at a time on one core** while a 24-thread CPU and a 20GB GPU sit idle. Acceleration here is greenfield and pays off across **every** consumer.

## Decision

Build a **hardware-adaptive compute layer in PluresDB** (`pluresdb-compute`), not a one-off accelerator inside umbra.

### Forks decided

1. **Where it lives -> PluresDB (`pluresdb-compute`), NOT umbra-only.** (mswork rec; kbristol blessed.) Rationale: kbristol said *"procedures and/or pluresDB"*; C-PLURES mandates extending the foundation rather than building beside it. Putting it in PluresDB means umbra, BitNet, **and any future plures consumer** get hardware acceleration for free. umbra is the **first consumer**, not the owner.
2. **GPU backend -> `wgpu`, NOT CUDA/ROCm.** (mswork rec; kbristol blessed.) Rationale: portability matches "self-optimize for *available* hardware" — wgpu runs Vulkan/Metal/DX12, so the same code targets the RX 7900 XT (Vulkan), Apple Silicon (Metal), Intel, etc. A Vulkan compute shader for ternary matmul on the 7900 XT is plenty fast. CUDA/ROCm (vendor-locked) only if we later prove wgpu leaves too much on the table.

### "Self-optimizing for available hardware" — the core idea

This is not just "add a GPU backend." It is a set of **procedures that detect the machine and evolve/select their own execution strategy**, then persist the choice:

```
detect hardware (cores, SIMD width, GPU via wgpu, NPU where exposed)
  -> benchmark candidate kernels on THIS machine for THIS op-shape
  -> select/evolve the fastest strategy
  -> persist the choice in PluresDB (keyed by hardware fingerprint + op-shape)
  -> reuse on subsequent runs; re-benchmark on hardware change
```

So praxisbot picks GPU, the Surface picks SIMD, a Raspberry Pi picks scalar — **automatically**, with no per-host config. This makes Track C itself an instance of the ADR-0009 self-improvement pattern: the kernels are procedures whose fitness is *measured throughput on real hardware*.

## Track C — build order

- **C1 — Hardware detection.** Runtime probe: CPU core count + SIMD capability (AVX2/AVX-512/NEON), GPU presence/adapter via `wgpu`, NPU where the OS exposes it. Produce a stable **hardware fingerprint**.
- **C2 — Kernel backends for the hot ops.** Start with ternary `matvec`/`matmul` + activation/quantize: scalar (baseline, exists) -> SIMD (`std::simd` / `wide`) -> threaded (`rayon`) -> GPU (`wgpu` compute shader). Each backend is correctness-tested against the scalar reference (bit-exact for integer ternary).
- **C3 — Self-optimizing strategy selection.** Benchmark backends per (fingerprint, op-shape) on first encounter; persist the winner in PluresDB; reuse thereafter; re-benchmark when the fingerprint changes. This is the "procedures that self-optimize for available hardware" deliverable.
- **C4 — Consumer wiring.** umbra-bitnet `matvec` routes through `pluresdb-compute` (first consumer). Any heavy procedure (e.g. headroom's crusher, ADR-0009 A5 pilot) can opt in.

## Consequences

- **Positive:** the foundation gains hardware acceleration once, for all consumers; praxisbot's GPU stops idling; BitNet training (Track B) and heavy procedures (Track A) both speed up; new hardware is handled automatically with no config.
- **Cost/risk:** GPU kernels add build complexity (wgpu, shader compilation) and a correctness-vs-scalar test burden — mitigated by bit-exact integer-ternary reference tests. wgpu portability may cost some peak throughput vs. ROCm — accepted; revisit only if measured to matter.
- **C-PLURES alignment:** compute strategy choices are pure data persisted as PluresDB nodes (C-PLURES-003/004); the side-effecting GPU dispatch is the IO boundary. Detection/selection logic should be expressible as procedures (C-DEV-001: start with the logic, Rust is the side-effect actor).
- **Anti-drift (C-DRIFT-001):** kernel selection persisted in PluresDB must be derived/validated, not a manual per-host setting.

## Status of work

- [ ] C1 hardware detection + fingerprint
- [ ] C2 kernel backends (scalar->SIMD->threaded->wgpu) for ternary matmul
- [ ] C3 self-optimizing per-(fingerprint, op-shape) selection persisted in PluresDB
- [ ] C4 umbra-bitnet wired as first consumer; headroom/A5 opt-in
