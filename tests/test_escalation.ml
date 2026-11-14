(* tests/test_escalation.ml - Test suite for Task R006: Iteration budget, deadlock and escalation rules *)

open Inos_models.Escalation

let assert_true msg cond =
  if not cond then (
    Printf.eprintf "[FAIL] %s\n" msg;
    exit 1
  ) else
    Printf.printf "[PASS] %s\n" msg

let test_deadlock_detection () =
  let tracker = create_tracker ~max_iterations:5 () in
  let res1 = record_iteration tracker ~proposal_hash:"hash-abc" ~critique:"Add boundary check" in
  (match res1 with
   | WithinBudget { current_iteration; _ } -> assert_true "Iteration 1 within budget" (current_iteration = 1)
   | _ -> assert_true "Expected WithinBudget" false);

  (* Model B re-submits exact same proposal hash "hash-abc" *)
  let res2 = record_iteration tracker ~proposal_hash:"hash-abc" ~critique:"Still missing check" in
  match res2 with
  | DeadlockDetected { repeated_hash; at_iteration; useful_status } ->
      assert_true "Deadlock detected on repeated proposal hash" (repeated_hash = "hash-abc" && at_iteration = 2);
      assert_true "Useful diagnostic status returned" (String.length useful_status > 0)
  | _ -> assert_true "Deadlock was not detected!" false

let test_iteration_budget_exhaustion () =
  let tracker = create_tracker ~max_iterations:3 () in
  let _ = record_iteration tracker ~proposal_hash:"hash-1" ~critique:"Fix syntax error" in
  let _ = record_iteration tracker ~proposal_hash:"hash-2" ~critique:"Fix type mismatch" in
  let res3 = record_iteration tracker ~proposal_hash:"hash-3" ~critique:"Fix failing test" in
  match res3 with
  | BudgetExhausted { iterations_completed; max_budget; last_critique; useful_status } ->
      assert_true "Iteration budget exhausted stops loop" (iterations_completed = 3 && max_budget = 3);
      assert_true "Last critique recorded" (last_critique = "Fix failing test");
      assert_true "Useful escalation status returned" (String.length useful_status > 0)
  | _ -> assert_true "Loop failed to stop at configured limit!" false

let () =
  Printf.printf "=== Running Iteration Budget & Deadlock Escalation Suite (R006) ===\n";
  test_deadlock_detection ();
  test_iteration_budget_exhaustion ();
  Printf.printf "=== ALL ESCALATION TESTS PASSED ===\n"
