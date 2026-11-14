# models/ (Trust Boundary T3 - Untrusted)

Local Model A (Architect/Reviewer) and Model B (Implementer/Challenger) adapters, context projection filters, context budget managers, and offline execution contracts.

- **Language:** OCaml 5.x
- **Safety Invariant:** All model outputs are untrusted data (T3). They cannot issue self-authorizations or bypass kernel/broker checks.
