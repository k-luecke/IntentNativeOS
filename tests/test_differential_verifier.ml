(* tests/test_differential_verifier.ml - Differential Verifier acceptance tests (C006) *)

open Inos_operation.Differential_verifier

let assert_true name cond =
  if cond then Printf.printf "[PASS] %s\n" name
  else (Printf.printf "[FAIL] %s\n" name; exit 1)

let test_conformant_execution () =
  let planned = [
    RequiredSubstring "STATUS: OK";
    UniqueLines;
    LineCount 3;
  ] in
  let tech = { exit_code = 0; stderr = ""; is_well_formed = true } in
  let output = "STATUS: OK\nITEM: alpha\nITEM: beta" in
  match verify ~planned_expectations:planned ~technical_status:tech ~actual_output:output with
  | Conformant { receipt_hash; checked_invariants } ->
      assert_true "Conformant result returned" true;
      assert_true "Checked 3 invariants" (checked_invariants = 3);
      assert_true "Receipt hash generated" (String.length receipt_hash > 0)
  | _ -> assert_true "Expected Conformant" false

let test_technical_failure () =
  let planned = [ RequiredSubstring "ANY" ] in
  let tech = { exit_code = 127; stderr = "command not found: cc"; is_well_formed = true } in
  let output = "" in
  match verify ~planned_expectations:planned ~technical_status:tech ~actual_output:output with
  | TechnicalFailure { exit_code; reason } ->
      assert_true "Technical failure flagged" true;
      assert_true "Exit code preserved" (exit_code = 127);
      assert_true "Reason explains failure" (String.length reason > 0)
  | _ -> assert_true "Expected TechnicalFailure" false

let test_intent_mismatched_but_technically_valid () =
  (* The crucial acceptance test for C006:
     Process ran with exit code 0, no error output, well formed.
     However, the intent required UNIQUE lines and SORTED ASCENDING.
     The output produced valid lines, but contained duplicate and unsorted lines! *)
  let planned = [
    UniqueLines;
    SortedLines `Ascending;
    ForbiddenSubstring "PROVISIONAL";
  ] in
  let tech = { exit_code = 0; stderr = ""; is_well_formed = true } in
  let output = "zebra\napple\nzebra\nPROVISIONAL" in
  match verify ~planned_expectations:planned ~technical_status:tech ~actual_output:output with
  | IntentMismatch { technical_validity; violations; diagnostic } ->
      assert_true "Intent-mismatched output flagged" true;
      assert_true "Technical validity is true" technical_validity;
      assert_true "Violations list is non-empty" (List.length violations >= 2);
      assert_true "Duplicate line violation identified"
        (List.exists (fun v -> String.length v > 0) violations);
      assert_true "Diagnostic explains divergence from intent"
        (String.length diagnostic > 0)
  | _ -> assert_true "Expected IntentMismatch for technically valid but intent-violating output" false

let () =
  Printf.printf "=== Running Differential Verifier Test Suite (C006) ===\n";
  test_conformant_execution ();
  test_technical_failure ();
  test_intent_mismatched_but_technically_valid ();
  Printf.printf "=== ALL DIFFERENTIAL VERIFIER TESTS PASSED ===\n"
