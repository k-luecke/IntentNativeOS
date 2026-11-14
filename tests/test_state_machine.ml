(* tests/test_state_machine.ml - Test suite for Task I009: Intent state machine with legal transitions *)

open Inos_ir.Authority
open Inos_ir.Clarification
open Inos_ir.State_machine

let assert_true msg cond =
  if not cond then (
    Printf.eprintf "[FAIL] %s\n" msg;
    exit 1
  ) else
    Printf.printf "[PASS] %s\n" msg

let test_legal_flow_to_completion () =
  let m0 = create () in
  assert_true "Initial state is Received" (m0.current_state = Received);

  match step m0 ResolveContext with
  | Error _ -> assert_true "ResolveContext failed" false
  | Ok m1 ->
      assert_true "Moved to ContextResolved" (m1.current_state = ContextResolved);
      match step m1 DraftIntent with
      | Error _ -> assert_true "DraftIntent failed" false
      | Ok m2 ->
          assert_true "Moved to IntentDrafted" (m2.current_state = IntentDrafted);
          match step m2 MarkReady with
          | Error _ -> assert_true "MarkReady failed" false
          | Ok m3 ->
              assert_true "Moved to Ready" (m3.current_state = Ready);
              match step m3 EnterSimulation with
              | Error _ -> assert_true "EnterSimulation failed" false
              | Ok m4 ->
                  assert_true "Moved to Simulation" (m4.current_state = Simulation);
                  match step m4 (RequestAuthorization ["fs:write:/tmp"]) with
                  | Error _ -> assert_true "RequestAuthorization failed" false
                  | Ok m5 ->
                      assert_true "Moved to AwaitingAuthorization"
                        (match m5.current_state with AwaitingAuthorization _ -> true | _ -> false);
                      let tok = {
                        token_id = "tok-1";
                        issuer = "broker";
                        granted_to = "intent-1";
                        scope = ["fs:write:/tmp"];
                        parent_contract_hash = "h123";
                        issued_at = 1.0;
                        valid_until = 100.0;
                        signature = "sig";
                      } in
                      match step m5 (GrantAuthorization tok) with
                      | Error _ -> assert_true "GrantAuthorization failed" false
                      | Ok m6 ->
                          assert_true "Moved to Authorized"
                            (match m6.current_state with Authorized _ -> true | _ -> false);
                          match step m6 StartPlanning with
                          | Error _ -> assert_true "StartPlanning failed" false
                          | Ok m7 ->
                              assert_true "Moved to Planning" (m7.current_state = Planning);
                              match step m7 StartExecuting with
                              | Error _ -> assert_true "StartExecuting failed" false
                              | Ok m8 ->
                                  assert_true "Moved to Executing" (m8.current_state = Executing);
                                  match step m8 StartVerifying with
                                  | Error _ -> assert_true "StartVerifying failed" false
                                  | Ok m9 ->
                                      assert_true "Moved to Verifying" (m9.current_state = Verifying);
                                      match step m9 (Complete "sha256:final") with
                                      | Error _ -> assert_true "Complete failed" false
                                      | Ok m10 ->
                                          assert_true "Moved to Completed"
                                            (m10.current_state = Completed { result_hash = "sha256:final" });
                                          assert_true "Audit trail contains 10 records" (List.length m10.audit_trail = 10);
                                          assert_true "All records in trail are marked legal"
                                            (List.for_all (fun a -> a.legal) m10.audit_trail)

let test_invalid_transition_fails_closed_and_is_audited () =
  let m0 = create () in
  (* Attempt to jump from Received straight to StartExecuting *)
  match step m0 StartExecuting with
  | Ok _ ->
      assert_true "Illegal transition succeeded!" false
  | Error (IllegalTransition { from_state; event; reason; audit }, updated_m) ->
      assert_true "Invalid transition fails closed" (from_state = "RECEIVED" && event = "StartExecuting");
      assert_true "Audit record exists" (audit.legal = false);
      assert_true "Audit recorded in updated machine" (List.length updated_m.audit_trail = 1);
      assert_true "Updated machine remains in Received state" (updated_m.current_state = Received);
      assert_true "Audit reason is descriptive" (String.length reason > 0)

let test_unauthorized_execution_blocked () =
  let m = {
    current_state = AwaitingAuthorization { requested_scope = ["fs:write:/kernel"] };
    audit_trail = [];
  } in
  (* Attempt to start executing without grant *)
  match step m StartExecuting with
  | Ok _ ->
      assert_true "AwaitingAuthorization transitioned to Executing without grant!" false
  | Error (IllegalTransition { from_state; event; audit; _ }, updated_m) ->
      assert_true "Direct execution from AwaitingAuthorization blocked"
        (from_state = "AWAITING_AUTHORIZATION" && event = "StartExecuting");
      assert_true "Audit records illegal attempt" (audit.legal = false);
      assert_true "Audit trail appended" (List.length updated_m.audit_trail = 1)

let test_clarification_loop () =
  let m = {
    current_state = IntentDrafted;
    audit_trail = [];
  } in
  let q = {
    question_id = "q-01";
    prompt = "Which folder?";
    target_candidates = ["a"; "b"];
    severity = "INFO";
    recommended_action = None;
  } in
  match step m (RequireClarification q) with
  | Error _ -> assert_true "RequireClarification failed" false
  | Ok m1 ->
      assert_true "Moved to ClarificationRequired"
        (match m1.current_state with ClarificationRequired _ -> true | _ -> false);
      match step m1 (SubmitClarification "folder a") with
      | Error _ -> assert_true "SubmitClarification failed" false
      | Ok m2 ->
          assert_true "Returned to IntentDrafted after clarification"
            (m2.current_state = IntentDrafted)

let () =
  Printf.printf "=== Running Intent State Machine Test Suite (I009) ===\n";
  test_legal_flow_to_completion ();
  test_invalid_transition_fails_closed_and_is_audited ();
  test_unauthorized_execution_blocked ();
  test_clarification_loop ();
  Printf.printf "=== ALL STATE MACHINE TESTS PASSED ===\n"
