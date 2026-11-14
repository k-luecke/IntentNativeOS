(* tests/test_sentinel_cycle1.ml - SDU-01 Sentinel Cycle 1 Verification Suite *)
open Inos_security
open Types

let passed = ref 0
let failed = ref 0

let check_true name cond =
  if cond then begin
    Printf.printf "[SENTINEL_PASS] %s\n%!" name;
    incr passed
  end else begin
    Printf.printf "[SENTINEL_FAIL] %s\n%!" name;
    incr failed
  end

let test_trust_placement () =
  Printf.printf "--- SDU-01: Trust Placement & Grant Minting Boundary ---\n%!";
  let broker = Broker.create_broker () in
  let op = {
    action = File_Write;
    target = "/sys/config/firewall_rules.json";
    parameters = [("action", "drop_all")];
    caller_id = "untrusted_agent";
    timestamp = 1700000000L;
    constraints = [];
  } in

  (* Invariant: Untrusted_Model cannot mint grants *)
  let res_model = Broker.mint_grant broker ~role:Untrusted_Model ~caller_id:"model_a" ~op ~current_time:1700000000L ~ttl:60L in
  (match res_model with
   | Denied _ -> check_true "S102: Untrusted_Model cannot mint authorization grant" true
   | Granted _ -> check_true "S102: Untrusted_Model cannot mint authorization grant" false);

  (* Invariant: Untrusted_Guest_VM cannot mint grants *)
  let res_guest = Broker.mint_grant broker ~role:Untrusted_Guest_VM ~caller_id:"guest_1" ~op ~current_time:1700000000L ~ttl:60L in
  (match res_guest with
   | Denied _ -> check_true "S102: Untrusted_Guest_VM cannot mint authorization grant" true
   | Granted _ -> check_true "S102: Untrusted_Guest_VM cannot mint authorization grant" false);

  (* Invariant: Host_Administrator can mint grants *)
  let res_admin = Broker.mint_grant broker ~role:Host_Administrator ~caller_id:"admin_hitl" ~op ~current_time:1700000000L ~ttl:60L in
  (match res_admin with
   | Granted _ -> check_true "S102: Host_Administrator can mint authorization grant" true
   | Denied _ -> check_true "S102: Host_Administrator can mint authorization grant" false)

let test_replay_resistance_and_mutation () =
  Printf.printf "--- SDU-01: Replay Resistance & Anti-Tamper Verification ---\n%!";
  let broker = Broker.create_broker () in
  let op = {
    action = VM_Capsule_Launch;
    target = "isolated_sandbox";
    parameters = [("mem", "128MB")];
    caller_id = "admin";
    timestamp = 1700000100L;
    constraints = [];
  } in

  let dec = Broker.mint_grant broker ~role:Host_Administrator ~caller_id:"admin" ~op ~current_time:1700000100L ~ttl:30L in
  let token = match dec with Granted t -> t | Denied _ -> failwith "mint failed" in

  (* 1. First consumption succeeds *)
  let v1 = Broker.validate_and_consume_grant broker token.grant_id op 1700000110L in
  check_true "S102: First consumption of authentic grant succeeds" (Result.is_ok v1);

  (* 2. Replay attack: repeated consumption fails *)
  let v2 = Broker.validate_and_consume_grant broker token.grant_id op 1700000115L in
  (match v2 with
   | Error Broker.GrantAlreadyConsumed ->
       check_true "S102: Repeated grant presentation rejected (Replay Resistance)" true
   | _ ->
       check_true "S102: Repeated grant presentation rejected (Replay Resistance)" false);

  (* 3. Mutation check: expired grant rejected *)
  let dec_exp = Broker.mint_grant broker ~role:Host_Administrator ~caller_id:"admin" ~op ~current_time:1700000100L ~ttl:10L in
  let tok_exp = match dec_exp with Granted t -> t | Denied _ -> failwith "mint failed" in
  let v_exp = Broker.validate_and_consume_grant broker tok_exp.grant_id op 1700000200L in
  (match v_exp with
   | Error (Broker.GrantExpired _) ->
       check_true "S102: Expired grant rejected fail-closed" true
   | _ ->
       check_true "S102: Expired grant rejected fail-closed" false);

  (* 4. Mutation check: altered target operation rejected *)
  let op_tampered = { op with target = "unauthorized_production_target" } in
  let dec_tamp = Broker.mint_grant broker ~role:Host_Administrator ~caller_id:"admin" ~op ~current_time:1700000100L ~ttl:60L in
  let tok_tamp = match dec_tamp with Granted t -> t | Denied _ -> failwith "mint failed" in
  let v_tamp = Broker.validate_and_consume_grant broker tok_tamp.grant_id op_tampered 1700000110L in
  (match v_tamp with
   | Error (Broker.DigestMismatch _) ->
       check_true "S102: Altered target digest rejected fail-closed" true
   | _ ->
       check_true "S102: Altered target digest rejected fail-closed" false)

let test_post_quantum_policy () =
  Printf.printf "--- SDU-01: Post-Quantum Cryptographic Policy Enforcement ---\n%!";
  let res_modern = Crypto_registry.is_suite_permitted Standard_Modern Suite_Ed25519_Sha256 in
  check_true "S003: Ed25519 permitted under Standard_Modern policy" (Result.is_ok res_modern);

  let res_pq_strict_ed = Crypto_registry.is_suite_permitted Post_Quantum_Strict Suite_Ed25519_Sha256 in
  check_true "S003: Classical Ed25519 rejected under Post_Quantum_Strict policy" (Result.is_error res_pq_strict_ed);

  let res_pq_strict_mldsa = Crypto_registry.is_suite_permitted Post_Quantum_Strict Suite_ML_DSA_65_Sha3 in
  check_true "S003: ML-DSA-65 permitted under Post_Quantum_Strict policy" (Result.is_ok res_pq_strict_mldsa);

  let res_pq_strict_mlkem = Crypto_registry.is_suite_permitted Post_Quantum_Strict Suite_ML_KEM_768 in
  check_true "S003: ML-KEM-768 permitted under Post_Quantum_Strict policy" (Result.is_ok res_pq_strict_mlkem)

let () =
  Printf.printf "========================================================\n%!";
  Printf.printf "=== SDU-01 SENTINEL CYCLE 1 OCAML INDEPENDENT HARNESS ===\n%!";
  Printf.printf "========================================================\n%!";
  test_trust_placement ();
  test_replay_resistance_and_mutation ();
  test_post_quantum_policy ();
  Printf.printf "--------------------------------------------------------\n%!";
  Printf.printf "Results: %d passed, %d failed\n%!" !passed !failed;
  if !failed > 0 then exit 1 else exit 0
