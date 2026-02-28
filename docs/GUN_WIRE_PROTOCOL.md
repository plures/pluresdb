# GUN Wire Protocol — Phase 1 Subset

This document defines the minimal subset of the [GUN.js](https://gun.eco/) wire protocol
that PluresDB implements in Phase 1.  The goal is basic graph-node replication between
Rust peers and interoperability with GUN.js peers.

---

## Overview

GUN uses a JSON-based protocol transported over WebSockets (or any framed stream).
Each message is a single JSON object with a message-ID field (`"#"`) plus a
type-discriminating key (`put`, `get`, or `@`).

Phase 1 supports three message types:

| Message | Key        | Direction | Purpose                          |
|---------|------------|-----------|----------------------------------|
| PUT     | `"put"`    | any→any   | Insert / merge node data         |
| GET     | `"get"`    | any→any   | Request a node (or single field) |
| ACK     | `"@"`      | any→any   | Acknowledge a PUT or GET         |

---

## Message Formats

### PUT — Insert or Merge Node Data

Pushes one or more graph nodes to the remote peer.  The remote peer SHOULD merge
the received data using CRDT semantics (last-write-wins per field, resolved by
comparing HAM state timestamps).

```json
{
  "#": "<message-id>",
  "put": {
    "<soul>": {
      "_": {
        "#": "<soul>",
        ">": {
          "<field>": <state-timestamp-ms>
        }
      },
      "<field>": <value>
    }
  }
}
```

**Fields**

| Field                  | Type              | Description                                          |
|------------------------|-------------------|------------------------------------------------------|
| `#`                    | string (opaque)   | Unique message identifier (e.g. `<peer-id>-<uuid>`) |
| `put`                  | object            | Map of soul → GUN node                               |
| `put.<soul>._`         | object            | Node metadata                                        |
| `put.<soul>._["#"]`    | string            | Soul (same as the map key)                           |
| `put.<soul>._[">"]`    | object            | HAM state: field name → f64 timestamp (ms since epoch) |
| `put.<soul>.<field>`   | any JSON value    | Data field                                           |

**Example**

```json
{
  "#": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
  "put": {
    "user:alice": {
      "_": {
        "#": "user:alice",
        ">": {
          "name": 1700000000000.0,
          "role": 1700000000000.0
        }
      },
      "name": "Alice",
      "role": "admin"
    }
  }
}
```

---

### GET — Request a Node

Requests a full node or a single field from the remote peer.
The remote peer SHOULD respond with a PUT containing the requested data, or an
ACK with `"err"` set if the soul is unknown.

```json
{
  "#": "<message-id>",
  "get": {
    "#": "<soul>",
    ".": "<field>"
  }
}
```

The `"."` (field filter) is **optional**.  When omitted the entire node is requested.

**Example — full node**

```json
{ "#": "msg-002", "get": { "#": "user:alice" } }
```

**Example — single field**

```json
{ "#": "msg-003", "get": { "#": "user:alice", ".": "name" } }
```

---

### ACK — Acknowledge

Acknowledges a previously-received message.

```json
{
  "#": "<ack-message-id>",
  "@": "<original-message-id>",
  "ok": 1,
  "err": null
}
```

| Field      | Type          | Description                                      |
|------------|---------------|--------------------------------------------------|
| `#`        | string (UUID) | This ACK's unique identifier                     |
| `@`        | string        | ID of the message being acknowledged             |
| `ok`       | integer (1)   | Present and `1` on success; omitted on error     |
| `err`      | string\|null  | Error description, or `null` / absent on success |

**Example — success**

```json
{
  "#": "ack-001",
  "@": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
  "ok": 1,
  "err": null
}
```

**Example — error**

```json
{
  "#": "ack-002",
  "@": "msg-002",
  "err": "soul not found"
}
```

---

## HAM State and CRDT Merge

GUN uses the **Hypothetical Amnesia Machine (HAM)** algorithm for conflict resolution.
In Phase 1, PluresDB uses a simplified subset:

- Each field carries an independent f64 timestamp (milliseconds since Unix epoch).
- When merging, the value with the **higher state timestamp** for a given field wins.
- Ties are broken deterministically by lexicographic comparison of the serialized values.

This is fully compatible with GUN.js default behaviour.

---

## Wire Encoding

All messages are encoded as **UTF-8 JSON** with no trailing newline.  Framing is
transport-dependent:

| Transport      | Framing               |
|----------------|-----------------------|
| WebSocket      | one message per frame |
| TCP            | newline-delimited     |
| In-memory (test) | length-prefixed or channel per message |

---

## Phase 1 Limitations

The following GUN features are **out of scope** for Phase 1:

- Authentication (`SEA` — Soul Encryption Authority)
- Graph traversal through links (`{"#": "<soul>"}` reference values)
- Subscriptions / real-time push (`DAM` — Daisy-chain Ad-hoc Mesh)
- Relay/mesh routing

These may be added in subsequent phases.

---

## References

- [GUN.js wire protocol](https://gun.eco/docs/wire)
- [HAM algorithm](https://gun.eco/docs/Conflict-Resolution-with-Guns)
- [PluresDB sync transport](./SYNC_TRANSPORT.md)
- [Hyperswarm P2P sync](./HYPERSWARM_SYNC.md)
