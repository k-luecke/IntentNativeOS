(* tests/test_handoff_envelope.ml - Test suite for Task R004: Strict structured A-plan / B-result envelopes *)

open Inos_models.Envelope

let assert_true msg cond =
  if not cond then (
    Printf.eprintf "[FAIL] %s\n" msg;
    exit 1
  ) else
    Printf.printf "[PASS] %s\n" msg

let test_valid_envelope () =
  let env = make_envelope
    ~handoff_id:"h-001"
    ~intent_ref:{ id = "intent-1"; revision = 2; hash = "hash-1" }
    ~plan_ref_hash:"plan-hash-1"
    ~from_role:Architect
    ~to_role:Implementer
    ~iteration:1
    ~operation:"IMPLEMENT"
    ~allowed_effects:["READ_CONTEXT"; "WRITE_STAGING"]
    ~capability_scope:["fs:read:/tmp"; "fs:write:/staging"]
    ~payload:"Implement zero copy shm"
    ()
  in
  match validate_envelope env with
  | Ok () ->
      assert_true "Valid envelope passes validation" true;
      assert_true "Envelope hash computed" (String.length env.envelope_hash > 0)
  | Error _ -> assert_true "Valid envelope failed validation" false

let test_unknown_effect_rejected () =
  let env = make_envelope
    ~handoff_id:"h-002"
    ~intent_ref:{ id = "intent-1"; revision = 2; hash = "hash-1" }
    ~plan_ref_hash:"plan-hash-1"
    ~from_role:Architect
    ~to_role:Implementer
    ~iteration:1
    ~operation:"IMPLEMENT"
    ~allowed_effects:["READ_CONTEXT"; "ARBITRARY_REMOTE_SHELL"]
    ~capability_scope:[]
    ~payload:"Payload"
    ()
  in
  match validate_envelope env with
  | Error (UnknownEffectRejected eff) ->
      assert_true "Unknown effect rejected" (eff = "ARBITRARY_REMOTE_SHELL")
  | _ -> assert_true "Unknown effect was accepted!" false

let test_invalid_fields_rejected () =
  let env_bad_iter = make_envelope
    ~handoff_id:"h-003"
    ~intent_ref:{ id = "intent-1"; revision = 2; hash = "hash-1" }
    ~plan_ref_hash:"plan-hash-1"
    ~from_role:Architect
    ~to_role:Implementer
    ~iteration:0 (* invalid iteration *)
    ~operation:"IMPLEMENT"
    ~allowed_effects:["READ_CONTEXT"]
    ~capability_scope:[]
    ~payload:"Payload"
    ()
  in
  (match validate_envelope env_bad_iter with
   | Error (InvalidIterationCount 0) ->
       assert_true "Iteration count < 1 rejected" true
   | _ -> assert_true "Invalid iteration count accepted!" false);

  let env_empty_id = make_envelope
    ~handoff_id:"" (* empty handoff_id *)
    ~intent_ref:{ id = "intent-1"; revision = 2; hash = "hash-1" }
    ~plan_ref_hash:"plan-hash-1"
    ~from_role:Architect
    ~to_role:Implementer
    ~iteration:1
    ~operation:"IMPLEMENT"
    ~allowed_effects:["READ_CONTEXT"]
    ~capability_scope:[]
    ~payload:"Payload"
    ()
  in
  match validate_envelope env_empty_id with
  | Error (MissingRequiredField "handoff_id") ->
      assert_true "Missing handoff_id rejected" true
  | _ -> assert_true "Missing handoff_id accepted!" false

let () =
  Printf.printf "=== Running Structured Handoff Envelope Test Suite (R004) ===\n";
  test_valid_envelope ();
  test_unknown_effect_rejected ();
  test_invalid_fields_rejected ();
  Printf.printf "=== ALL HANDOFF ENVELOPE TESTS PASSED ===\n"
