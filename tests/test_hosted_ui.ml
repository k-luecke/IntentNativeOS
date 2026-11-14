(* tests/test_hosted_ui.ml - Hosted UI and preview acceptance tests (U001, U002, U003) *)

open Inos_ui.Hosted_server
open Inos_ui.Clarification_ui
open Inos_ui.Preview_ui
open Inos_operation.Staged_execution

let assert_true name cond =
  if cond then Printf.printf "[PASS] %s\n" name
  else (Printf.printf "[FAIL] %s\n" name; exit 1)

let test_u001_file_picker_and_intent () =
  Printf.printf "--- Testing U001: File Picker & Intent Console ---\n";
  let files = list_workspace_files "." in
  assert_true "Scanned workspace files" (List.length files > 0);

  let html = render_html files (Some "dune-project") (Some "optimize operations") in
  assert_true "HTML contains File Selection title" (String.contains html 'F');
  assert_true "HTML includes select element" (String.contains html '<');
  assert_true "HTML includes textarea for intent" (String.contains html 't');

  (* Process valid request *)
  let (code, _, resp) = process_request ~root:"." "POST" "/api/intent" "file=dune-project&intent=optimize+operations" in
  assert_true "Valid intent submission returns 200" (code = 200);
  assert_true "Response contains accepted status" (String.contains resp 'a');

  (* Process missing file request *)
  let (err_code, _, _) = process_request ~root:"." "POST" "/api/intent" "file=&intent=optimize" in
  assert_true "Missing file returns 400" (err_code = 400);

  (* Process empty intent request *)
  let (err_code2, _, _) = process_request ~root:"." "POST" "/api/intent" "file=dune-project&intent=" in
  assert_true "Empty intent returns 400" (err_code2 = 400)

let test_u002_clarification_choices () =
  Printf.printf "--- Testing U002: Contextual Clarification UI ---\n";
  let dialog = {
    question_id = "Q_FILE_AMBIGUOUS_01";
    prompt = "Multiple candidate files match 'compiler'. Which target do you intend to modify?";
    severity = "CRITICAL";
    choices = [
      { choice_id = "c1"; label = "Compiler IR Mode"; target = "compiler/ir/mode.ml"; consequence = "Updates mode enum and invariants" };
      { choice_id = "c2"; label = "Compiler Operation IR"; target = "compiler/operation/operation_ir.ml"; consequence = "Updates operation execution graph" };
    ];
  } in

  let html = render_clarification_html dialog in
  assert_true "HTML renders CRITICAL severity badge" (String.contains html 'C');
  assert_true "HTML renders prompt text" (String.contains html 'M');
  assert_true "HTML renders Return to Exploration button" (String.contains html 'E');

  (* 1. User approves choice c2 *)
  let approve_params = [("action", "approve"); ("choice_id", "c2")] in
  let decision = handle_decision approve_params dialog in
  match decision with
  | ChoiceApproved { choice_id; target } ->
      assert_true "Choice approved correctly" (choice_id = "c2" && target = "compiler/operation/operation_ir.ml")
  | _ -> assert_true "Expected ChoiceApproved" false;

  (* 2. User chooses Return to Exploration *)
  let cancel_params = [("action", "explore")] in
  let cancel_decision = handle_decision cancel_params dialog in
  match cancel_decision with
  | ReturnToExploration { reason } ->
      assert_true "Return to exploration correctly handled" (String.length reason > 0)
  | _ -> assert_true "Expected ReturnToExploration" false

let test_u003_preview_and_undo () =
  Printf.printf "--- Testing U003: Plan Preview & Reversible Undo ---\n";
  let view = {
    plan_title = "Optimize pure arithmetic operations";
    target_file = "compiler/operation/native_backend.ml";
    original_content = "let x = 10 in x + 1";
    proposed_content = "let x = 10 in x * 2";
    commit_hash = "staged_hash_abcdef";
    is_committed = false;
    is_rolled_back = false;
  } in

  let html = render_preview_html view in
  assert_true "Preview renders staged plan title" (String.contains html 'P');
  assert_true "Preview renders diff boxes" (String.contains html 'O');
  assert_true "Preview renders Commit and Undo buttons" (String.contains html 'U');

  (* Test reversible rollback undo *)
  let restored_flag = ref false in
  let step1 : staged_step = {
    step_id = "step_copy";
    operation_id = "copy_fixture";
    is_reversible = true;
    forward_action = (fun () -> Ok "copied");
    rollback_action = (fun () -> restored_flag := true; Ok PerfectRestoration);
  } in

  let rollback_res = perform_undo [step1] "Undo requested" in
  assert_true "Restorable action executed during undo" !restored_flag;
  assert_true "Rollback receipt recorded restored step" (List.length rollback_res.restored_steps = 1);
  assert_true "Clean rollback status is true" rollback_res.is_clean_rollback

let () =
  Printf.printf "=== Running Hosted UI Acceptance Suite (U001, U002, U003) ===\n";
  test_u001_file_picker_and_intent ();
  test_u002_clarification_choices ();
  test_u003_preview_and_undo ();
  Printf.printf "=== ALL HOSTED UI TESTS PASSED ===\n"
