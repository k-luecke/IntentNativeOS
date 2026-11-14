# P007: Language-Neutral IPC Wire Specification

**Task ID:** P007  
**Gate:** G0 (Foundational Controls)  
**Schema Identifier:** `inos.ipc.v0.1`  
**Status:** Approved Standard  

---

## 1. Overview & Purpose

The Intent-Native Operating System enforces strict privilege and trust boundaries (T0–T4). All communication across subsystem boundaries (e.g., between the Rust Kernel, the OCaml Intent Compiler, Model Adapters, and Untrusted Capsules) MUST occur over explicit, versioned, and content-addressed message envelopes.

This specification defines the language-neutral wire envelope to allow seamless interoperability between Rust, OCaml, C, and future native services.

---

## 2. Wire Envelope Schema (`inos.ipc.v0.1`)

An IPC message consists of a canonical JSON object (or binary CBOR encoding in high-throughput channels) containing metadata and a structured payload:

```json
{
  "schema": "inos.ipc.v0.1",
  "msg_id": "550e8400-e29b-41d4-a716-446655440000",
  "timestamp": 1791568000.123,
  "sender": "INTENT_COMPILER",
  "recipient": "MODEL_A",
  "msg_type": "REQUEST",
  "correlation_id": "00000000-0000-0000-0000-000000000000",
  "payload": "{\"objective\":\"organize_files\",\"mode\":\"EXPLORE\"}",
  "digest": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
}
```

---

## 3. Field Definitions

| Field | Type | Description |
| :--- | :--- | :--- |
| `schema` | String | Strictly `inos.ipc.v0.1`. Unknown schemas must be rejected. |
| `msg_id` | String | Unique message identifier (UUID v4 or monotonic sequence). |
| `timestamp` | Float | POSIX epoch timestamp in seconds with millisecond precision. |
| `sender` | String | Principal role identifier (`KERNEL`, `BROKER`, `INTENT_COMPILER`, `MODEL_A`, `MODEL_B`, `CAPSULE`, `UI`). |
| `recipient` | String | Target principal role identifier. |
| `msg_type` | String | Interaction pattern: `REQUEST`, `RESPONSE`, `NOTIFICATION`, `ERROR`. |
| `correlation_id`| String | `msg_id` of the parent request for request-response pairing. |
| `payload` | String | Encoded payload body. |
| `digest` | String | Hex-encoded SHA-256 hash computed over canonical canonicalization of `(schema, msg_id, timestamp, sender, recipient, msg_type, correlation_id, payload)`. |

---

## 4. Invariants & Security Rules

1. **Tamper Proofing:** If the computed digest does not match the `digest` field upon receipt, the message MUST be dropped immediately and an audit alarm logged.
2. **Schema Invariance:** Recipients MUST reject envelopes with unsupported schema versions fail-closed.
3. **No Dynamic Elevation:** Senders cannot spoof higher trust boundaries (e.g., Model A claiming sender `KERNEL`). The underlying transport (IPC channel / domain socket / microkernel endpoint) authenticates the sender identity.
