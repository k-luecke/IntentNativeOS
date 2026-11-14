(* tests/test_operation_registry.ml - Test suite for Task C002: Pure operation registry and deterministic resolver *)

open Inos_operation.Registry

let assert_true msg cond =
  if not cond then (
    Printf.eprintf "[FAIL] %s\n" msg;
    exit 1
  ) else
    Printf.printf "[PASS] %s\n" msg

let test_pure_transformation_without_llm_call () =
  let reg = default_registry () in
  let input_data = [("content", "hello intent-native os")] in
  match execute_pure_transformation reg "compute_sha256" input_data with
  | Ok (outputs, metrics) ->
      assert_true "Transformation executed successfully" true;
      assert_true "LLM call count is strictly zero" (metrics.llm_calls = 0);
      assert_true "Execution duration is sub-millisecond" (metrics.duration_ms < 50.0);
      let d = List.assoc "digest" outputs in
      assert_true "Output digest is 64-char SHA256 hex"
        (String.length d = 64 && d = "2551a3fd7b8feae5d03f89d21e51e3794ed0d07788a762fab119d2e66dbf8c22")
  | Error _ -> assert_true "Failed to execute registered transformation" false

let test_filter_lines_transformation () =
  let reg = default_registry () in
  let text = "alpha\nbeta\napple\nbanana" in
  match execute_pure_transformation reg "filter_lines" [("text", text); ("prefix", "a")] with
  | Ok (outputs, metrics) ->
      assert_true "Lines filtered deterministically" (List.assoc "filtered" outputs = "alpha\napple");
      assert_true "Zero LLM calls for text filtering" (metrics.llm_calls = 0)
  | Error _ -> assert_true "Line filtering failed" false

let test_synthesize_diff_transformation () =
  let reg = default_registry () in
  match execute_pure_transformation reg "synthesize_diff" [("original", "val a = 1"); ("updated", "val a = 2")] with
  | Ok (outputs, metrics) ->
      let diff = List.assoc "diff" outputs in
      assert_true "Diff generated natively" (String.contains diff '-' && String.contains diff '+');
      assert_true "Zero LLM calls for diff synthesis" (metrics.llm_calls = 0)
  | Error _ -> assert_true "Diff generation failed" false

let () =
  Printf.printf "=== Running Pure Operation Registry Suite (C002) ===\n";
  test_pure_transformation_without_llm_call ();
  test_filter_lines_transformation ();
  test_synthesize_diff_transformation ();
  Printf.printf "=== ALL OPERATION REGISTRY TESTS PASSED ===\n"
