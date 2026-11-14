(* scripts/verify_repo.ml - Structural monorepo validator for Gate G0 *)

let required_dirs = [
  "kernel";
  "core";
  "core/ipc";
  "core/telemetry";
  "compiler";
  "models";
  "capsules";
  "ui";
  "tests";
  "docs";
  "docs/adr";
  "tools";
  "tools/supervisor";
  "scripts";
  "evidence";
]

let required_files = [
  "dune-project";
  "intent_os.opam";
  "toolchain.lock.json";
  "Dockerfile";
  "START_HERE_IMPLEMENTATION_HANDOFF.md";
  "intent_native_os_engineering_spec_v0_2.md";
  "intent_os_granular_backlog_v0_3.csv";
  "intent_os_granular_backlog_v0_3.md";
  "intent_os_source_manifest.json";
  "docs/P001_MVP_DEFINITION_AND_NON_GOALS.md";
  "docs/P002_QEMU_VIRT_TARGET_SPEC.md";
  "docs/adr/ADR_TEMPLATE.md";
  "docs/adr/0000-decision-log.md";
  "docs/adr/0001-no-python-ocaml-rust-architecture.md";
  "docs/adr/0002-qemu-x86-64-reference-platform.md";
  "docs/P008_THREAT_MODEL_AND_TRUST_BOUNDARIES.md";
  "docs/P009_METRICS_AND_TELEMETRY_SPEC.md";
  "docs/P010_SUPPLY_CHAIN_AND_RELEASE_POLICY.md";
  "docs/P007_IPC_WIRE_SPECIFICATION.md";
  "core/ipc/envelope.ml";
  "core/ipc/dune";
  "core/telemetry/metrics.ml";
  "core/telemetry/dune";
  "tests/test_ipc_envelope.ml";
  "tests/dune";
  "ci/ci.yml";
  "scripts/run_smoke_tests.ps1";
]

let check_dir d =
  if Sys.file_exists d && Sys.is_directory d then (
    Printf.printf "[PASS] Directory: %s/\n" d;
    true
  ) else (
    Printf.eprintf "[FAIL] Missing directory: %s/\n" d;
    false
  )

let check_file f =
  if Sys.file_exists f && not (Sys.is_directory f) then (
    Printf.printf "[PASS] File: %s\n" f;
    true
  ) else (
    Printf.eprintf "[FAIL] Missing file: %s\n" f;
    false
  )

let () =
  Printf.printf "=== Intent-Native OS Gate G0 Structural Validator ===\n";
  let dirs_ok = List.for_all check_dir required_dirs in
  let files_ok = List.for_all check_file required_files in
  if dirs_ok && files_ok then (
    Printf.printf "=== ALL GATE G0 CHECKS PASSED ===\n";
    exit 0
  ) else (
    Printf.eprintf "=== GATE G0 CHECKS FAILED ===\n";
    exit 1
  )
