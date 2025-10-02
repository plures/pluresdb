# Third-Party Software Attributions

This document lists all third-party software components used in Rusty Gun Database and their respective licenses.

## Rust Dependencies

### Core Runtime
- **tokio** (MIT License) - Async runtime for Rust
  - Copyright (c) 2023 Tokio Contributors
  - https://github.com/tokio-rs/tokio

- **serde** (MIT License) - Serialization framework
  - Copyright (c) 2014 The Rust Project Developers
  - https://github.com/serde-rs/serde

- **serde_json** (MIT License) - JSON serialization
  - Copyright (c) 2014 The Rust Project Developers
  - https://github.com/serde-rs/serde

### HTTP and Web
- **axum** (MIT License) - Web framework
  - Copyright (c) 2021 Axum Contributors
  - https://github.com/tokio-rs/axum

- **tower** (MIT License) - Service abstraction
  - Copyright (c) 2021 Tower Contributors
  - https://github.com/tower-rs/tower

- **tower-http** (MIT License) - HTTP middleware
  - Copyright (c) 2021 Tower Contributors
  - https://github.com/tower-rs/tower-http

### Database and Storage
- **sqlx** (MIT License) - Async SQL toolkit
  - Copyright (c) 2019-2023 LaunchBadge LLC
  - https://github.com/launchbadge/sqlx

- **sled** (MIT License) - Embedded database
  - Copyright (c) 2018 Tyler Neely
  - https://github.com/spacejam/sled

- **rocksdb** (Apache-2.0 License) - Key-value store
  - Copyright (c) 2011 The LevelDB Authors
  - https://github.com/facebook/rocksdb

### Networking
- **quinn** (MIT License) - QUIC protocol implementation
  - Copyright (c) 2019 Quinn Contributors
  - https://github.com/quinn-rs/quinn

- **webrtc** (MIT License) - WebRTC implementation
  - Copyright (c) 2020 WebRTC Contributors
  - https://github.com/webrtc-rs/webrtc

- **libp2p** (MIT License) - P2P networking
  - Copyright (c) 2018 Parity Technologies
  - https://github.com/libp2p/rust-libp2p

### Vector Search
- **hnsw_rs** (MIT License) - Hierarchical Navigable Small World
  - Copyright (c) 2024 Rusty Gun Team
  - https://github.com/rusty-gun/hnsw_rs

### Cryptography
- **ring** (MIT License) - Cryptography library
  - Copyright (c) 2015-2019 The ring-rs developers
  - https://github.com/briansmith/ring

- **ed25519-dalek** (MIT License) - Ed25519 signatures
  - Copyright (c) 2017 isis lovecruft
  - https://github.com/dalek-cryptography/ed25519-dalek

- **aes-gcm** (MIT License) - AES-GCM encryption
  - Copyright (c) 2019 RustCrypto Developers
  - https://github.com/RustCrypto/AEADs

### CLI and Utilities
- **clap** (MIT License) - Command line parser
  - Copyright (c) 2015-2023 Kevin B. Knapp
  - https://github.com/clap-rs/clap

- **console** (MIT License) - Terminal styling
  - Copyright (c) 2016 Armin Ronacher
  - https://github.com/console-rs/console

- **indicatif** (MIT License) - Progress bars
  - Copyright (c) 2017 Armin Ronacher
  - https://github.com/console-rs/indicatif

### Testing and Benchmarking
- **criterion** (MIT License) - Benchmarking framework
  - Copyright (c) 2017 Jorge Aparicio
  - https://github.com/bheisler/criterion.rs

- **proptest** (MIT License) - Property-based testing
  - Copyright (c) 2017 The Proptest Developers
  - https://github.com/proptest-rs/proptest

### Logging
- **tracing** (MIT License) - Application tracing
  - Copyright (c) 2019 Tokio Contributors
  - https://github.com/tokio-rs/tracing

- **tracing-subscriber** (MIT License) - Tracing subscriber
  - Copyright (c) 2019 Tokio Contributors
  - https://github.com/tokio-rs/tracing

## JavaScript/TypeScript Dependencies

### HTTP Client
- **axios** (MIT License) - HTTP client
  - Copyright (c) 2014-present Matt Zabriskie
  - https://github.com/axios/axios

### Graph Visualization
- **cytoscape** (MIT License) - Graph visualization
  - Copyright (c) 2013-2023 Cytoscape Consortium
  - https://github.com/cytoscape/cytoscape.js

- **cytoscape-cose-bilkent** (MIT License) - Cose layout
  - Copyright (c) 2015-2023 Cytoscape Consortium
  - https://github.com/cytoscape/cytoscape.js-cose-bilkent

- **cytoscape-dagre** (MIT License) - Dagre layout
  - Copyright (c) 2015-2023 Cytoscape Consortium
  - https://github.com/cytoscape/cytoscape.js-dagre

- **dagre** (MIT License) - Directed graph layout
  - Copyright (c) 2012-2013 Chris Pettitt
  - https://github.com/dagrejs/dagre

### Code Editor
- **monaco-editor** (MIT License) - Code editor
  - Copyright (c) Microsoft Corporation
  - https://github.com/microsoft/monaco-editor

## Web UI Dependencies

### Frontend Framework
- **leptos** (MIT License) - Rust frontend framework
  - Copyright (c) 2022 Leptos Contributors
  - https://github.com/leptos-rs/leptos

### Build Tools
- **wasm-bindgen** (MIT License) - WebAssembly bindings
  - Copyright (c) 2016 The Rust and WebAssembly Working Group
  - https://github.com/rustwasm/wasm-bindgen

- **wasm-pack** (MIT License) - WebAssembly packager
  - Copyright (c) 2018 The Rust and WebAssembly Working Group
  - https://github.com/rustwasm/wasm-pack

## Development Dependencies

### TypeScript
- **typescript** (Apache-2.0 License) - TypeScript compiler
  - Copyright (c) Microsoft Corporation
  - https://github.com/microsoft/TypeScript

### Linting
- **eslint** (MIT License) - JavaScript linter
  - Copyright (c) 2012-2023 OpenJS Foundation
  - https://github.com/eslint/eslint

### Testing
- **@vscode/test-electron** (MIT License) - VSCode testing
  - Copyright (c) Microsoft Corporation
  - https://github.com/microsoft/vscode-test

### Packaging
- **vsce** (MIT License) - VSCode extension packager
  - Copyright (c) Microsoft Corporation
  - https://github.com/microsoft/vscode-vsce

## License Summary

All dependencies are licensed under permissive open source licenses:
- **MIT License**: Most dependencies
- **Apache-2.0 License**: Some Microsoft and Rust projects

## Compliance Notes

1. **Attribution**: All copyright notices preserved
2. **License Compatibility**: All licenses compatible with MIT
3. **Commercial Use**: All dependencies allow commercial use
4. **Modification**: All dependencies allow modification
5. **Distribution**: All dependencies allow distribution

## Updates

This file is updated whenever new dependencies are added or existing ones are updated. Last updated: January 2024.

---

**For questions about third-party licenses, contact: legal@rusty-gun.com**
