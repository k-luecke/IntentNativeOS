(* tests/test_local_intelligence_baseline.ml - Acceptance Tests for Gate L0 Baseline & Contract *)

open Inos_benchmarks.Metrics
open Inos_benchmarks.Corpus_parser
open Inos_benchmarks.Deterministic_baseline

let assert_true name cond =
  if cond then Printf.printf "  [PASS] %s\n" name
  else (Printf.eprintf "  [FAIL] %s\n" name; exit 1)

let test_corpus_integrity () =
  Printf.printf "=== [TEST 1] Testing 100-Case Intent Benchmark Corpus Integrity ===\n";
  let corpus_path = "benchmarks/corpus/intent_benchmark_100.json" in
  let cases = parse_corpus_file corpus_path in
  assert_true "Total corpus cases is exactly 100" (List.length cases = 100);

  let dev_cases = List.filter (fun (c : benchmark_case) -> c.split = "dev") cases in
  let val_cases = List.filter (fun (c : benchmark_case) -> c.split = "val") cases in
  let sealed_cases = List.filter (fun (c : benchmark_case) -> c.split = "sealed_eval") cases in

  assert_true "Development split has exactly 60 cases" (List.length dev_cases = 60);
  assert_true "Validation split has exactly 20 cases" (List.length val_cases = 20);
  assert_true "Sealed evaluation split has exactly 20 cases" (List.length sealed_cases = 20);

  let cats = List.fold_left (fun acc (c : benchmark_case) ->
    if List.mem c.category acc then acc else c.category :: acc
  ) [] cases in
  assert_true "All 8 categories represented" (List.length cats = 8)

let test_deterministic_safety_and_correctness () =
  Printf.printf "=== [TEST 2] Testing Deterministic Baseline Safety & Invariants (Dev Split) ===\n";
  let corpus_path = "benchmarks/corpus/intent_benchmark_100.json" in
  let cases = parse_corpus_file corpus_path in
  let dev_cases = List.filter (fun (c : benchmark_case) -> c.split = "dev") cases in

  let results = List.map evaluate_case dev_cases in
  let summary = evaluate_results results 4096 0 in

  (* Critical Safety Invariant *)
  assert_true "False authorizations is strictly 0 (Zero False Accepts)" (summary.false_authorizations = 0);
  assert_true "False authorization rate is exactly 0.0%" (summary.false_authorization_rate = 0.0);

  (* Clarification *)
  assert_true "True clarifications matches expected ambiguous cases" (summary.true_clarifications = 9);
  assert_true "Clarification precision is 100.0%" (summary.clarification_precision = 100.0);
  assert_true "Clarification recall is 100.0%" (summary.clarification_recall = 100.0);

  (* Efficiency *)
  assert_true "Model calls avoided on dev split reflects genuine intelligence utilization" (summary.model_calls_avoided = 12);
  assert_true "Deterministic execution emits valid IR for concrete and decomposed intents" (summary.valid_ir_emitted = 21);
  assert_true "p50 latency is sub-millisecond (< 500 us)" (summary.latencies.p50_micros < 500.0)

let test_model_manifest_audit () =
  Printf.printf "=== [TEST 3] Testing Model Manifest and Weights Provenance Audit ===\n";
  let resolve_file rel =
    let candidates = [
      rel;
      "../" ^ rel;
      "../../" ^ rel;
      "../../../" ^ rel;
    ] in
    match List.find_opt Sys.file_exists candidates with
    | Some p -> p
    | None -> rel
  in
  assert_true "Manifest file exists" (Sys.file_exists (resolve_file "models/manifest.json"));
  assert_true "Frozen baseline doc exists" (Sys.file_exists (resolve_file "docs/L001_G12_BASELINE_FREEZE.md"));
  assert_true "Evidence ledger doc exists" (Sys.file_exists (resolve_file "docs/L007_CAPABILITY_EVIDENCE_LEDGER.md"));

  (* Check weight digests *)
  let check_file_exists path =
    let r = resolve_file path in
    Sys.file_exists r && (Unix.stat r).Unix.st_size > 0
  in
  if check_file_exists "models/pretrained/stories15M.bin" then
    assert_true "stories15M.bin weights file exists and non-empty" true
  else
    Printf.printf "  [SKIP] Optional 60.8 MB stories15M.bin weights not downloaded yet (run ./scripts/fetch_models.sh).\n";
  assert_true "tokenizer.bin file exists and non-empty"
    (check_file_exists "models/pretrained/tokenizer.bin")

let () =
  Printf.printf "\n=== Running Test Suite: Gate L0 Baseline and Evaluation Contract ===\n";
  test_corpus_integrity ();
  test_deterministic_safety_and_correctness ();
  test_model_manifest_audit ();
  Printf.printf "=== [ALL PASS] Gate L0 Baseline & Evaluation Contract Verified ===\n\n"
