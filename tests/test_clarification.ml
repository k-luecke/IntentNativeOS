(* tests/test_clarification.ml - Test suite for Task I008: Clarification question selection by consequence *)

open Inos_ir.Clarification

let assert_true msg cond =
  if not cond then (
    Printf.eprintf "[FAIL] %s\n" msg;
    exit 1
  ) else
    Printf.printf "[PASS] %s\n" msg

let test_low_risk_known_move_asks_zero_questions () =
  let action = LowRiskKnownMove {
    source = "/workspace/drafts/report.txt";
    destination = "/workspace/archive/report.txt";
  } in
  let question_opt = select_clarification_question action in
  assert_true "Low-risk known move asks zero questions" (question_opt = None)

let test_low_risk_read_asks_zero_questions () =
  let action = LowRiskRead { target = "/workspace/README.md" } in
  let question_opt = select_clarification_question action in
  assert_true "Low-risk read asks zero questions" (question_opt = None)

let test_ambiguous_delete_asks_specific_question () =
  let candidates = ["/workspace/build/a.out"; "/workspace/bin/a.out"] in
  let action = AmbiguousDelete { candidate_targets = candidates } in
  match select_clarification_question action with
  | Some q ->
      assert_true "Ambiguous delete asks question" (q.question_id = "q-ambiguous-delete");
      assert_true "Question severity is CRITICAL" (q.severity = "CRITICAL");
      assert_true "Question prompt mentions candidates"
        (String.contains q.prompt 'a' && String.contains q.prompt 'b');
      assert_true "Candidate targets list preserved" (q.target_candidates = candidates)
  | None ->
      assert_true "Expected clarification question for ambiguous delete but got None" false

let test_destructive_mutation_asks_question () =
  let action = DestructiveMutation {
    target = "/etc/os-release";
    operation = "overwrite_system_manifest";
  } in
  match select_clarification_question action with
  | Some q ->
      assert_true "Destructive mutation asks question" (q.question_id = "q-destructive-mutation");
      assert_true "Prompt mentions staging snapshot" (String.contains q.prompt 's')
  | None ->
      assert_true "Expected question for destructive mutation" false

let () =
  Printf.printf "=== Running Clarification Question Selection Test Suite (I008) ===\n";
  test_low_risk_known_move_asks_zero_questions ();
  test_low_risk_read_asks_zero_questions ();
  test_ambiguous_delete_asks_specific_question ();
  test_destructive_mutation_asks_question ();
  Printf.printf "=== ALL CLARIFICATION TESTS PASSED ===\n"
