(* tests/test_staged_execution.ml - Test suite for Task C005: Staged execution, verification, and rollback receipts *)

open Inos_operation.Staged_execution

let assert_true msg cond =
  if not cond then (
    Printf.eprintf "[FAIL] %s\n" msg;
    exit 1
  ) else
    Printf.printf "[PASS] %s\n" msg

let test_successful_staged_execution () =
  let state = ref 0 in
  let step1 = {
    step_id = "step-1";
    operation_id = "op-inc";
    is_reversible = true;
    forward_action = (fun () -> state := !state + 10; Ok "state is 10");
    rollback_action = (fun () -> state := !state - 10; Ok PerfectRestoration);
  } in
  let step2 = {
    step_id = "step-2";
    operation_id = "op-mul";
    is_reversible = true;
    forward_action = (fun () -> state := !state * 2; Ok "state is 20");
    rollback_action = (fun () -> state := !state / 2; Ok PerfectRestoration);
  } in
  match execute_staged_plan [step1; step2] with
  | Committed { completed_steps; commit_hash; _ } ->
      assert_true "Both steps committed" (List.length completed_steps = 2);
      assert_true "State updated to 20" (!state = 20);
      assert_true "Commit hash generated" (String.length commit_hash > 0)
  | RolledBack _ -> assert_true "Successful execution unexpectedly rolled back" false

let test_partial_failure_triggers_clean_rollback () =
  let rollback_log = ref [] in
  let step1 = {
    step_id = "step-copy";
    operation_id = "fs_copy";
    is_reversible = true;
    forward_action = (fun () -> Ok "staged copied");
    rollback_action = (fun () ->
      rollback_log := "step-copy" :: !rollback_log;
      Ok PerfectRestoration
    );
  } in
  let step2 = {
    step_id = "step-patch";
    operation_id = "apply_diff";
    is_reversible = true;
    forward_action = (fun () -> Ok "patch applied");
    rollback_action = (fun () ->
      rollback_log := "step-patch" :: !rollback_log;
      Ok PerfectRestoration
    );
  } in
  let step3_failing = {
    step_id = "step-verify";
    operation_id = "run_tests";
    is_reversible = true;
    forward_action = (fun () -> Error "Unit tests failed: assertion error in test_shm");
    rollback_action = (fun () -> Ok PerfectRestoration);
  } in
  match execute_staged_plan [step1; step2; step3_failing] with
  | RolledBack receipt ->
      assert_true "Partial failure rolls back" (receipt.failed_step_id = "step-verify");
      assert_true "Failure reason preserved" (receipt.failure_reason = "Unit tests failed: assertion error in test_shm");
      assert_true "Rollback executed in LIFO reverse order (patch then copy)"
        (!rollback_log = ["step-copy"; "step-patch"]);
      assert_true "Both executed steps marked restored" (List.length receipt.restored_steps = 2);
      assert_true "Rollback is clean (no irreversible items)" (receipt.is_clean_rollback);
      assert_true "Rollback receipt hash computed" (String.length receipt.receipt_hash > 0)
  | Committed _ -> assert_true "Failing plan committed!" false

let test_irreversible_step_recorded_in_receipt () =
  let step1_irreversible = {
    step_id = "step-burn-fuse";
    operation_id = "hardware_fuse";
    is_reversible = false; (* Hardware fuse burn cannot be restored *)
    forward_action = (fun () -> Ok "fuse burned");
    rollback_action = (fun () -> Ok (Irreversible { reason = "Hardware OTP fuse blown" }));
  } in
  let step2_failing = {
    step_id = "step-finalize";
    operation_id = "seal_vault";
    is_reversible = true;
    forward_action = (fun () -> Error "Vault seal timeout");
    rollback_action = (fun () -> Ok PerfectRestoration);
  } in
  match execute_staged_plan [step1_irreversible; step2_failing] with
  | RolledBack receipt ->
      assert_true "Irreversible step tracked in receipt" (List.length receipt.irreversible_steps = 1);
      assert_true "Clean rollback is false when irreversible effect exists" (not receipt.is_clean_rollback);
      assert_true "Failure step recorded" (receipt.failed_step_id = "step-finalize")
  | Committed _ -> assert_true "Failed plan committed!" false

let () =
  Printf.printf "=== Running Staged Execution & Rollback Receipt Suite (C005) ===\n";
  test_successful_staged_execution ();
  test_partial_failure_triggers_clean_rollback ();
  test_irreversible_step_recorded_in_receipt ();
  Printf.printf "=== ALL STAGED EXECUTION TESTS PASSED ===\n"
