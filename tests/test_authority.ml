(* tests/test_authority.ml - Test suite for Task I004: Authority tokens and pending approval fields *)

open Inos_ir.Mode
open Inos_ir.Authority

let assert_true msg cond =
  if not cond then (
    Printf.eprintf "[FAIL] %s\n" msg;
    exit 1
  ) else
    Printf.printf "[PASS] %s\n" msg

let test_transition_without_grant_fails () =
  let result = transition_to_authorized
    ~secret:"broker-secret-key"
    ~token_opt:None
    ~contract_id:"intent-001"
    ~contract_hash:"hash-abc"
    ~required_scope:["fs:write:/app"]
    ~current_mode:Simulate
    ~current_approval:Unapproved
    ()
  in
  match result with
  | Error (MissingAuthorityGrant msg) ->
      assert_true "Simulated request cannot transition to authorized without grant" (String.length msg > 0)
  | Error _ -> assert_true "Expected MissingAuthorityGrant error" false
  | Ok _ -> assert_true "Simulated request transitioned to authorized without grant!" false

let test_expired_token_rejected () =
  let now = 1000.0 in
  let token = make_token
    ~token_id:"tok-001"
    ~issuer:"t1-cap-broker"
    ~granted_to:"intent-002"
    ~scope:["fs:write:/app"]
    ~parent_contract_hash:"hash-123"
    ~issued_at:900.0
    ~valid_until:950.0 (* expired *)
    ~secret:"broker-secret-key"
    ()
  in
  let result = transition_to_authorized
    ~secret:"broker-secret-key"
    ~token_opt:(Some token)
    ~contract_id:"intent-002"
    ~contract_hash:"hash-123"
    ~required_scope:["fs:write:/app"]
    ~current_mode:Simulate
    ~current_approval:Unapproved
    ~now
    ()
  in
  match result with
  | Error (TokenExpired _) ->
      assert_true "Expired token correctly rejected" true
  | Error _ -> assert_true "Expected TokenExpired error" false
  | Ok _ -> assert_true "Expired token accepted!" false

let test_contract_hash_mismatch_rejected () =
  let now = 1000.0 in
  let token = make_token
    ~token_id:"tok-002"
    ~issuer:"t1-cap-broker"
    ~granted_to:"intent-003"
    ~scope:["fs:write:/app"]
    ~parent_contract_hash:"hash-original"
    ~issued_at:900.0
    ~valid_until:1100.0
    ~secret:"broker-secret-key"
    ()
  in
  let result = transition_to_authorized
    ~secret:"broker-secret-key"
    ~token_opt:(Some token)
    ~contract_id:"intent-003"
    ~contract_hash:"hash-modified-by-adversary"
    ~required_scope:["fs:write:/app"]
    ~current_mode:Simulate
    ~current_approval:Unapproved
    ~now
    ()
  in
  match result with
  | Error (ContractHashMismatch _) ->
      assert_true "Contract hash mismatch correctly rejected" true
  | Error _ -> assert_true "Expected ContractHashMismatch error" false
  | Ok _ -> assert_true "Token bound to different contract accepted!" false

let test_scope_mismatch_rejected () =
  let now = 1000.0 in
  let token = make_token
    ~token_id:"tok-003"
    ~issuer:"t1-cap-broker"
    ~granted_to:"intent-004"
    ~scope:["fs:read:/tmp"] (* missing fs:write:/app *)
    ~parent_contract_hash:"hash-456"
    ~issued_at:900.0
    ~valid_until:1100.0
    ~secret:"broker-secret-key"
    ()
  in
  let result = transition_to_authorized
    ~secret:"broker-secret-key"
    ~token_opt:(Some token)
    ~contract_id:"intent-004"
    ~contract_hash:"hash-456"
    ~required_scope:["fs:read:/tmp"; "fs:write:/app"]
    ~current_mode:Simulate
    ~current_approval:Unapproved
    ~now
    ()
  in
  match result with
  | Error (ScopeMismatch { requested; granted }) ->
      assert_true "Insufficient scope rejected" (List.length requested = 2 && List.length granted = 1)
  | Error _ -> assert_true "Expected ScopeMismatch error" false
  | Ok _ -> assert_true "Insufficient scope token accepted!" false

let test_valid_grant_transitions_to_authorized () =
  let now = 1000.0 in
  let token = make_token
    ~token_id:"tok-004"
    ~issuer:"t1-cap-broker"
    ~granted_to:"intent-005"
    ~scope:["fs:read:/app"; "fs:write:/app"; "proc:spawn"]
    ~parent_contract_hash:"hash-789"
    ~issued_at:900.0
    ~valid_until:1100.0
    ~secret:"broker-secret-key"
    ()
  in
  let result = transition_to_authorized
    ~secret:"broker-secret-key"
    ~token_opt:(Some token)
    ~contract_id:"intent-005"
    ~contract_hash:"hash-789"
    ~required_scope:["fs:read:/app"; "fs:write:/app"]
    ~current_mode:Simulate
    ~current_approval:Unapproved
    ~now
    ()
  in
  match result with
  | Ok (Authorized tok, new_mode) ->
      assert_true "Successfully transitioned to Authorized state" (tok.token_id = "tok-004");
      assert_true "Simulate mode elevated to Execute upon authorized grant" (new_mode = Execute)
  | Ok _ -> assert_true "Expected Authorized state but got different approval state" false
  | Error _ -> assert_true "Valid transition failed" false

let () =
  Printf.printf "=== Running Authority Tokens & Approval Test Suite (I004) ===\n";
  test_transition_without_grant_fails ();
  test_expired_token_rejected ();
  test_contract_hash_mismatch_rejected ();
  test_scope_mismatch_rejected ();
  test_valid_grant_transitions_to_authorized ();
  Printf.printf "=== ALL AUTHORITY & APPROVAL TESTS PASSED ===\n"
