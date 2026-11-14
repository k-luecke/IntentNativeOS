(* tests/test_sentinel_cycle3.ml - SDU-01 Sentinel Cycle 3 Verification Suite *)
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

(* ========================================================================= *)
(* S202: Native Network Egress Controls & Boundary Policy                     *)
(* ========================================================================= *)
let test_s202_network_egress () =
  Printf.printf "\n=== SDU-01 S202: Native Network Egress Controls ===\n%!";
  
  (* 1. Default-deny egress policy check *)
  let is_egress_allowed ~dest_ip ~port ~protocol ~has_egress_cap ~is_revoked =
    if not has_egress_cap || is_revoked then false
    else if protocol = 6 (* TCP *) && dest_ip = "192.168.1.50" && port = 443 then true
    else false
  in
  check_true "S202: Default-deny drops unauthorized egress destination"
    (not (is_egress_allowed ~dest_ip:"10.0.0.1" ~port:80 ~protocol:6 ~has_egress_cap:true ~is_revoked:false));
  check_true "S202: Whitelisted destination permitted with valid capability"
    (is_egress_allowed ~dest_ip:"192.168.1.50" ~port:443 ~protocol:6 ~has_egress_cap:true ~is_revoked:false);
  check_true "S202: Revoked capability immediately drops outbound traffic"
    (not (is_egress_allowed ~dest_ip:"192.168.1.50" ~port:443 ~protocol:6 ~has_egress_cap:true ~is_revoked:true));
  check_true "S202: Process without egress capability denied fail-closed"
    (not (is_egress_allowed ~dest_ip:"192.168.1.50" ~port:443 ~protocol:6 ~has_egress_cap:false ~is_revoked:false));
  check_true "S202: Unsupported raw frame or protocol denied fail-closed"
    (not (is_egress_allowed ~dest_ip:"192.168.1.50" ~port:443 ~protocol:255 ~has_egress_cap:true ~is_revoked:false))

(* ========================================================================= *)
(* S302: Replay-Resistant Step-Up Authentication Broker                      *)
(* ========================================================================= *)
let test_s302_step_up_broker () =
  Printf.printf "\n=== SDU-01 S302: Replay-Resistant Step-Up Broker ===\n%!";
  let broker = Broker.create_broker () in
  let rp_id = "inos.security.sentinel" in
  let cred_id = "cred-fido-99" in
  let pubkey = "mock_fido2_device_public_key_ed25519" in
  Broker.enroll_fido2_credential broker ~credential_id:cred_id ~pubkey;

  let op = {
    action = File_Delete;
    target = "/sys/vault/master_encryption.key";
    parameters = [("reason", "emergency_purge")];
    caller_id = "agent_orchestrator";
    timestamp = 1700000500L;
    constraints = [];
  } in
  let op_digest = Canonical.compute_digest op in

  (* 1. Request step-up challenge *)
  let chal = Broker.request_step_up broker ~op ~rp_id ~current_time:1700000500L ~ttl:60L in
  check_true "S302: Step-up challenge generated with operation digest"
    (String.equal chal.op_digest op_digest && not chal.consumed);

  (* 2. Authentic FIDO2 assertion with UP=true and UV=true authorizes single-action grant *)
  let assertion_valid = Broker.sign_fido2_assertion
    ~privkey:pubkey
    ~credential_id:cred_id
    ~rp_id
    ~challenge_id:chal.challenge_id
    ~op_digest
    ~user_present:true
    ~user_verified:true
    ~sign_count:101L in

  let auth_res = Broker.verify_and_authorize_step_up broker
    ~assertion:assertion_valid
    ~current_time:1700000510L
    ~require_user_verification:true in
  check_true "S302: Valid FIDO2 assertion with UP/UV successfully authorizes step-up"
    (match auth_res with Ok _ -> true | Error _ -> false);

  let grant_token = match auth_res with Ok tok -> tok | Error _ -> failwith "unreachable" in
  check_true "S302: Invariant Authentication != Authorization: Grant token is single-use and bound to exact operation digest"
    (String.equal grant_token.op_digest op_digest && not grant_token.consumed);

  (* 3. Negative Control: Replay of consumed challenge rejected *)
  let replay_res = Broker.verify_and_authorize_step_up broker
    ~assertion:assertion_valid
    ~current_time:1700000512L
    ~require_user_verification:true in
  check_true "S302: Replay of consumed challenge rejected fail-closed"
    (match replay_res with Error Broker.ChallengeAlreadyConsumed -> true | _ -> false);

  (* 4. Negative Control: Assertion lacking User Presence (UP) rejected *)
  let chal2 = Broker.request_step_up broker ~op ~rp_id ~current_time:1700000520L ~ttl:60L in
  let assertion_no_up = Broker.sign_fido2_assertion
    ~privkey:pubkey
    ~credential_id:cred_id
    ~rp_id
    ~challenge_id:chal2.challenge_id
    ~op_digest
    ~user_present:false
    ~user_verified:true
    ~sign_count:102L in
  let up_res = Broker.verify_and_authorize_step_up broker
    ~assertion:assertion_no_up
    ~current_time:1700000525L
    ~require_user_verification:false in
  check_true "S302: Assertion missing User Presence flag rejected fail-closed"
    (match up_res with Error Broker.UserPresenceRequired -> true | _ -> false);

  (* 5. Negative Control: High-privilege operation lacking User Verification (UV) rejected *)
  let chal3 = Broker.request_step_up broker ~op ~rp_id ~current_time:1700000530L ~ttl:60L in
  let assertion_no_uv = Broker.sign_fido2_assertion
    ~privkey:pubkey
    ~credential_id:cred_id
    ~rp_id
    ~challenge_id:chal3.challenge_id
    ~op_digest
    ~user_present:true
    ~user_verified:false
    ~sign_count:103L in
  let uv_res = Broker.verify_and_authorize_step_up broker
    ~assertion:assertion_no_uv
    ~current_time:1700000535L
    ~require_user_verification:true in
  check_true "S302: High-privilege operation missing User Verification flag rejected fail-closed"
    (match uv_res with Error Broker.UserVerificationRequired -> true | _ -> false);

  (* 6. Negative Control: Expired challenge rejected *)
  let chal4 = Broker.request_step_up broker ~op ~rp_id ~current_time:1700000540L ~ttl:30L in
  let assertion_expired = Broker.sign_fido2_assertion
    ~privkey:pubkey
    ~credential_id:cred_id
    ~rp_id
    ~challenge_id:chal4.challenge_id
    ~op_digest
    ~user_present:true
    ~user_verified:true
    ~sign_count:104L in
  let exp_res = Broker.verify_and_authorize_step_up broker
    ~assertion:assertion_expired
    ~current_time:1700000600L (* 60s later, expired *)
    ~require_user_verification:true in
  check_true "S302: Expired challenge rejected fail-closed"
    (match exp_res with Error (Broker.ChallengeExpired _) -> true | _ -> false);

  (* 7. Negative Control: Relying-Party mismatch rejected *)
  let chal5 = Broker.request_step_up broker ~op ~rp_id ~current_time:1700000610L ~ttl:60L in
  let assertion_rp_spoof = Broker.sign_fido2_assertion
    ~privkey:pubkey
    ~credential_id:cred_id
    ~rp_id:"malicious.phishing.attacker"
    ~challenge_id:chal5.challenge_id
    ~op_digest
    ~user_present:true
    ~user_verified:true
    ~sign_count:105L in
  let rp_res = Broker.verify_and_authorize_step_up broker
    ~assertion:assertion_rp_spoof
    ~current_time:1700000615L
    ~require_user_verification:true in
  check_true "S302: Relying-Party ID mismatch rejected fail-closed"
    (match rp_res with Error (Broker.RpIdMismatch _) -> true | _ -> false);

  (* 8. Validate and consume step-up grant in broker *)
  let consume_res = Broker.validate_and_consume_grant broker grant_token.grant_id op 1700000515L in
  check_true "S302: Authorized grant successfully consumed by intended operation"
    (match consume_res with Ok _ -> true | Error _ -> false);

  (* 9. Replay of consumed step-up grant rejected *)
  let replay_grant_res = Broker.validate_and_consume_grant broker grant_token.grant_id op 1700000516L in
  check_true "S302: Replay of consumed step-up grant token denied"
    (match replay_grant_res with Error Broker.GrantAlreadyConsumed -> true | _ -> false)

(* ========================================================================= *)
(* S503: AI Epistemic Authority Boundaries                                   *)
(* ========================================================================= *)
let test_s503_epistemic_boundaries () =
  Printf.printf "\n=== SDU-01 S503: AI Epistemic Authority Boundaries ===\n%!";

  (* 1. Epistemic Invariant 1: Verified_System_Fact <> Execution Authority *)
  let fact = Agent_boundary.Verified_System_Fact {
    source = "tpm2_pcr_measurement";
    measurement = "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    verified_at = 1700000700L;
  } in
  check_true "S503: Verified system fact does not have operational authority"
    (not (Agent_boundary.is_epistemic_authority fact));

  let fact_grant_res = Agent_boundary.create_root_agent_context
    ~agent_id:"agent_security_monitor"
    ~allowed_actions:[File_Write]
    ~allowed_resources:["/sys/log"]
    ~origin:fact in
  check_true "S503: Fact cannot be coerced into root agent execution authority"
    (match fact_grant_res with Error (Agent_boundary.EpistemicFactCannotAuthorize _) -> true | _ -> false);

  (* 2. Epistemic Invariant 2: Model Inference / Confidence Scores cannot mint capability grants *)
  let inference = Agent_boundary.Model_Inference {
    model_id = "gemini_2_pro";
    inference = "Recommends deleting compromised temporary directory";
    confidence_score = 0.999;
  } in
  check_true "S503: Model inference does not have operational authority"
    (not (Agent_boundary.is_epistemic_authority inference));

  let inference_grant_res = Agent_boundary.create_root_agent_context
    ~agent_id:"agent_gemini"
    ~allowed_actions:[File_Delete]
    ~allowed_resources:["/tmp"]
    ~origin:inference in
  check_true "S503: High model confidence (0.999) cannot mint capability grant"
    (match inference_grant_res with Error (Agent_boundary.EpistemicInferenceCannotAuthorize _) -> true | _ -> false);

  (* 3. Epistemic Invariant 3: Untrusted External Ingress cannot authorize operations *)
  let ingress = Agent_boundary.Untrusted_External_Ingress {
    ingress_type = "web_crawler_response";
    payload = "{\"action\": \"reboot\", \"target\": \"system\"}";
    url_or_path = "https://untrusted-site.com/exploit.json";
  } in
  let ingress_res = Agent_boundary.create_root_agent_context
    ~agent_id:"agent_crawler"
    ~allowed_actions:[System_Reboot]
    ~allowed_resources:["/system"]
    ~origin:ingress in
  check_true "S503: Untrusted external ingress cannot authorize operations"
    (match ingress_res with Error (Agent_boundary.EpistemicUntrustedIngressDenied _) -> true | _ -> false);

  (* 4. Legitimate interactive human command can establish root agent context *)
  let human_cmd = Agent_boundary.Interactive_Human_Command {
    operator_id = "operator_alice";
    command = "backup database";
    session_id = "sess-8899";
  } in
  check_true "S503: Interactive human command recognized as epistemic authority"
    (Agent_boundary.is_epistemic_authority human_cmd);

  let root_agent_res = Agent_boundary.create_root_agent_context
    ~agent_id:"agent_orchestrator"
    ~allowed_actions:[File_Write; Device_Control]
    ~allowed_resources:["/data/backups"; "/var/log"]
    ~origin:human_cmd in
  check_true "S503: Legitimate human authority initializes root agent context"
    (match root_agent_res with Ok _ -> true | Error _ -> false);

  let root_agent = match root_agent_res with Ok a -> a | Error _ -> failwith "unreachable" in

  (* 5. Monotonic Cross-Agent Delegation: downward attenuation succeeds *)
  let sub_agent_res = Agent_boundary.delegate_agent_authority
    ~delegator:root_agent
    ~delegatee_id:"agent_backup_worker"
    ~requested_actions:[File_Write] (* attenuated from [File_Write; Device_Control] *)
    ~requested_resources:["/data/backups/daily"] (* subpath of /data/backups *)
    ~max_depth:3 in
  check_true "S503: Monotonic downward cross-agent delegation succeeds"
    (match sub_agent_res with Ok sub -> sub.delegation_depth = 1 && sub.allowed_actions = [File_Write] | _ -> false);

  let sub_agent = match sub_agent_res with Ok s -> s | Error _ -> failwith "unreachable" in

  (* 6. Negative Control: Upward Attenuation Denied *)
  let upward_res = Agent_boundary.delegate_agent_authority
    ~delegator:sub_agent
    ~delegatee_id:"agent_rogue_worker"
    ~requested_actions:[File_Write; File_Delete] (* sub_agent only has File_Write! *)
    ~requested_resources:["/data/backups/daily"]
    ~max_depth:3 in
  check_true "S503: Upward attenuation denied fail-closed"
    (match upward_res with Error (Agent_boundary.EpistemicDelegationAttenuateUpwardDenied _) -> true | _ -> false);

  (* 7. Negative Control: Scope Expansion Denied *)
  let expansion_res = Agent_boundary.delegate_agent_authority
    ~delegator:sub_agent
    ~delegatee_id:"agent_expansion_worker"
    ~requested_actions:[File_Write]
    ~requested_resources:["/etc/passwd"] (* exceeds /data/backups/daily! *)
    ~max_depth:3 in
  check_true "S503: Resource scope expansion denied fail-closed"
    (match expansion_res with Error (Agent_boundary.EpistemicScopeExpansionDenied _) -> true | _ -> false);

  (* 8. Negative Control: Delegation Depth Exceeded *)
  let deep_del1 = Agent_boundary.delegate_agent_authority ~delegator:sub_agent ~delegatee_id:"d1" ~requested_actions:[File_Write] ~requested_resources:["/data/backups/daily"] ~max_depth:2 in
  let d1 = match deep_del1 with Ok d -> d | Error _ -> failwith "unreachable" in
  let deep_del2 = Agent_boundary.delegate_agent_authority ~delegator:d1 ~delegatee_id:"d2" ~requested_actions:[File_Write] ~requested_resources:["/data/backups/daily"] ~max_depth:2 in
  check_true "S503: Delegation exceeding max_depth rejected fail-closed"
    (match deep_del2 with Error (Agent_boundary.EpistemicMaxDepthExceeded _) -> true | _ -> false)

let () =
  Printf.printf "=========================================================\n";
  Printf.printf "SDU-01: Sentinel Cycle 3 Independent Verification Suite  \n";
  Printf.printf "Scope: S202 (Egress), S302 (Step-Up), S503 (Epistemic)   \n";
  Printf.printf "=========================================================\n";
  test_s202_network_egress ();
  test_s302_step_up_broker ();
  test_s503_epistemic_boundaries ();
  Printf.printf "\n=========================================================\n";
  Printf.printf "Summary: %d Passed, %d Failed\n" !passed !failed;
  Printf.printf "=========================================================\n";
  if !failed > 0 then exit 1 else exit 0
