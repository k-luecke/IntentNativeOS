(* tests/test_classifier.ml - Test suite for Task I007: Deterministic request classifier *)

open Inos_ir.Mode
open Inos_ir.Contract
open Inos_ir.Resolver
open Inos_ir.Classifier

let assert_true msg cond =
  if not cond then (
    Printf.eprintf "[FAIL] %s\n" msg;
    exit 1
  ) else
    Printf.printf "[PASS] %s\n" msg

let test_go_with_no_target_yields_clarification () =
  let ws = empty_context in
  let result = classify_request
    ~raw_input:"go"
    ~mode:Execute
    ~workspace:ws
    ~pending_targets:[]
  in
  match result with
  | ClarificationRequired { reason; blocking_ambiguities } ->
      assert_true "'go' with no selected target yields clarification"
        (reason = "'go' with no selected target yields clarification");
      assert_true "Blocking ambiguity recorded" (List.length blocking_ambiguities = 1)
  | DirectAction _ ->
      assert_true "Expected ClarificationRequired but got DirectAction" false

let test_go_with_single_pending_target_yields_direct_action () =
  let ws = empty_context in
  let target = FileTarget "/workspace/build.sh" in
  let result = classify_request
    ~raw_input:"go"
    ~mode:Execute
    ~workspace:ws
    ~pending_targets:[target]
  in
  match result with
  | DirectAction { target = t; action; mode } ->
      assert_true "Direct action dispatched for single target" (t = target);
      assert_true "Action is execute" (action = "execute");
      assert_true "Mode is Execute" (mode = Execute)
  | ClarificationRequired _ ->
      assert_true "Expected DirectAction for uniquely approved target" false

let test_go_with_multiple_targets_yields_clarification () =
  let ws = empty_context in
  let targets = [FileTarget "/a.ml"; FileTarget "/b.ml"] in
  let result = classify_request
    ~raw_input:"go"
    ~mode:Execute
    ~workspace:ws
    ~pending_targets:targets
  in
  match result with
  | ClarificationRequired { reason; blocking_ambiguities } ->
      assert_true "Multiple targets require clarification" (List.length blocking_ambiguities = 2);
      assert_true "Reason identifies multiple targets" (String.length reason > 0)
  | DirectAction _ ->
      assert_true "Expected ClarificationRequired for multiple ambiguous targets" false

let test_go_with_workspace_selection () =
  let ws = { empty_context with selected_files = ["/workspace/kernel/main.rs"] } in
  let result = classify_request
    ~raw_input:"go"
    ~mode:Execute
    ~workspace:ws
    ~pending_targets:[]
  in
  match result with
  | DirectAction { target = FileTarget f; _ } ->
      assert_true "Workspace selected file used when no pending target" (f = "/workspace/kernel/main.rs")
  | _ -> assert_true "Expected DirectAction with single workspace selection" false

let () =
  Printf.printf "=== Running Request Classifier Test Suite (I007) ===\n";
  test_go_with_no_target_yields_clarification ();
  test_go_with_single_pending_target_yields_direct_action ();
  test_go_with_multiple_targets_yields_clarification ();
  test_go_with_workspace_selection ();
  Printf.printf "=== ALL CLASSIFIER TESTS PASSED ===\n"
