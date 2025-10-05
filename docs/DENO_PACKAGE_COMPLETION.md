# Deno Package Distribution

## Overview

PluresDB now ships as a dedicated Deno module alongside the Node.js/npm build. The Deno package exposes only browser- and runtime-compatible APIs so that Deno applications can consume the database without the Node shim.

## Installation

```bash
deno add @plures/pluresdb
```

After the dependency is added you can import the library using the JSR specifier:

```ts
import { GunDB } from "jsr:@plures/pluresdb";
```

## CLI Usage

The native Deno CLI continues to live in `src/main.ts` and is exported at the subpath `jsr:@plures/pluresdb/cli`.

```bash
deno run -A --unstable-kv jsr:@plures/pluresdb/cli serve
```

## Dual Distribution Notes

- The npm package (`pluresdb`) continues to target Node 18+ and bundles the VSCode helper APIs and Node wrapper.
- The Deno package exports the same core database, HTTP server, and mesh APIs from `mod.ts`, omitting Node-specific wrappers.
- `deno.json` now declares the module metadata (`name`, `version`, and `exports`) so `deno publish`/`deno add` resolve the JSR package without extra config.
