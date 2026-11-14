# Design decisions, experiments, and non-goals

## Requirements
- Original OS architecture, not a required MirageOS deployment.
- Novice/expert parity through adaptive UI and shared typed intent contracts.
- A/B local model roles with independent LoRA specialization; C optional.
- Fully local capability even when both models are unloaded.
- Typed, auditable, reversible operations; device capabilities not conferred by model output.
- Dynamic app-scoped native/compatibility/VM execution and resource lending.
- Crypto algorithm agility, protected private keys, step-up approval and post-quantum migration.

## Candidates, not required foundations
OCaml; MirageOS library techniques; seL4-style capability principles; LLVM/MLIR; Qwen and independent small model; GGUF; SCP; tonic gating; Network-Native Computing; PLONK/RoKoko/other ZK schemes; ML-KEM and PQ signatures.

## Deferred work
Full Windows compatibility, broad GPU virtualization, all-device driver matrix, polished consumer distro, third independent model, novel cryptographic proof system. Do not mistake deferral for rejection.

## Gating decisions
1. Hosted semantic/runtime slice vs native kernel independently developed behind agreed IPC.
2. Proof-of-concept on x86-64 QEMU and 16 GiB consumer reference machine.
3. Hardware platform, kernel implementation language, OCI/guest ABI, desktop, model pair, and proof backend finalized only after ADR and empirical data.

## Risk register
- Human intent may be misclassified as executable; make mode transition explicit and test it.
- Native code tests can pass while semantics are wrong; retain intent fidelity acceptance tests.
- Full Windows guests require Windows licensing and can be resource-heavy.
- Unikernels do not transparently host arbitrary Windows executables.
- VM isolation is not a guarantee against hypervisor compromise.
- ZKP does not protect system integrity if the trusted verifier or kernel is compromised.
- Post-quantum migration requires protocol profiles and versioned credentials, not labels.
