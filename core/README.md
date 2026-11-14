# core/ (Trust Boundary T1 / T2)

Foundational runtime services, versioned IPC message envelopes, capability brokers, append-only event journal, and cryptographic primitives.

- **Language:** OCaml 5.x
- **Subdirectories:**
  - `ipc/`: Versioned message envelopes (`envelope.ml`), schema validation, canonical serialization.
  - `journal/`: Append-only event journal and replay engine.
  - `capabilities/`: Capability descriptor catalog and revocation broker.
  - `telemetry/`: Operational metrics, latency, and RSS telemetry.
