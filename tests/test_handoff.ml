(* tests/test_handoff.ml - Test suite for Task R005: A to B to A handoff state machine and crash resumption *)

open Inos_memory.Journal
open Inos_models.Handoff

let assert_true msg cond =
  if not cond then (
    Printf.eprintf "[FAIL] %s\n" msg;
    exit 1
  ) else
    Printf.printf "[PASS] %s\n" msg

let test_interrupted_cycle_resumes_cleanly () =
  let path = Filename.temp_file "inos_handoff_test_" ".ndjson" in
  let j = match open_journal path with
    | Ok j -> j
    | Error _ -> failwith "Failed to open journal"
  in
  let session = start_session ~session_id:"sess-1" ~intent_id:"intent-42" j in
  assert_true "Started session at ArchitectPlanning"
    (match session.state with ArchitectPlanning { iteration = 1 } -> true | _ -> false);

  (* Step 1: Model A produces plan *)
  record_step session "PLAN_CREATED"
    (ImplementerWorking { plan_hash = "plan:hash1"; iteration = 1 })
    "{\"plan_id\": \"plan-01\"}";

  (* Step 2: Model B produces proposal *)
  record_step session "PROPOSAL_CREATED"
    (DeterministicVerifying { proposal_hash = "prop:hash1"; iteration = 1 })
    "{\"proposal_id\": \"prop-01\"}";

  (* SIMULATE PROCESS TERMINATION / CRASH at Step 2 *)
  let j_reloaded = match open_journal path with
    | Ok j -> j
    | Error _ -> failwith "Failed to reload journal after crash"
  in
  let resumed = resume_session ~session_id:"sess-1" ~intent_id:"intent-42" j_reloaded in

  (* Invariant check: session resumes at DeterministicVerifying without repeating Step 1 or 2 *)
  assert_true "Resumed session is at DeterministicVerifying"
    (match resumed.state with DeterministicVerifying _ -> true | _ -> false);
  assert_true "Completed steps list preserved"
    (List.mem "PLAN_CREATED" resumed.completed_steps && List.mem "PROPOSAL_CREATED" resumed.completed_steps);

  (* Step 3: Run deterministic verification on resumed session *)
  record_step resumed "VERIFICATION_RUN"
    (ArchitectReview { verification_receipt = "rcpt:pass"; iteration = 1 })
    "{\"tests_passed\": true}";

  (* Step 4: Architect review completes cycle *)
  record_step resumed "REVIEW_PASSED"
    (CycleCompleted { final_hash = "sha256:final"; iterations_used = 1 })
    "{\"status\": \"APPROVED\"}";

  assert_true "Interrupted cycle resumes without duplicate side effects"
    (match resumed.state with CycleCompleted _ -> true | _ -> false);
  assert_true "Total steps completed is 4" (List.length resumed.completed_steps = 4);

  Sys.remove path

let () =
  Printf.printf "=== Running A-to-B-to-A Handoff Resumption Suite (R005) ===\n";
  test_interrupted_cycle_resumes_cleanly ();
  Printf.printf "=== ALL HANDOFF TESTS PASSED ===\n"
