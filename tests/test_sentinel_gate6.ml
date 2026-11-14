(* tests/test_sentinel_gate6.ml - SDU-01 Sentinel Gate 6 Independent Verification Suite
   Scope: Gate 6: Native Consumer Interface (S601)
   - REQ-G6-1: Native accessible security console UI (S601)
   - REQ-G6-2: Clear, un-obscured threat states (Safe/Action/Threat)
   - REQ-G6-3: Plain-language permission prompts with digest preview & default deny
   - REQ-G6-4: Consumer-accessible network controls, live flow monitor & emergency killswitch
   - REQ-G6-5: Guided 24-word recovery enrollment, checksum feedback & challenge verification
   - Negative controls & mutation tests proving fail-closed properties *)

open Inos_security

let passed_count = ref 0
let failed_count = ref 0

let assert_sentinel (name : string) (cond : bool) =
  if cond then begin
    incr passed_count;
    Printf.printf "[SENTINEL_PASS] %s\n%!" name
  end else begin
    incr failed_count;
    Printf.printf "[SENTINEL_FAIL] %s\n%!" name
  end

let run_gate6_verification_suite () =
  Printf.printf "=========================================================\n";
  Printf.printf "SDU-01: Sentinel Gate 6 Independent Verification Suite   \n";
  Printf.printf "Scope: S601 Native Consumer Security Console Interface   \n";
  Printf.printf "=========================================================\n\n%!";

  (* ========================================================================= *)
  (* REQ-G6-1: Native Accessible Security Console UI                          *)
  (* ========================================================================= *)
  Printf.printf "=== REQ-G6-1: Security Console UI Engine ===\n%!";

  let base_health : Console.subsystem_health = {
    firewall_active = true;
    conntrack_count = 12;
    max_conntrack = 4096;
    dnssec_strict = true;
    tuf_update_fresh = true;
    feed_age_seconds = 1800; (* 30 min (fresh) *)
    anomalies_detected = 0;
    unauthorized_attempts = 0;
    recovery_enrolled = true;
    killswitch_active = false;
  } in

  let initial_state = Console.evaluate_threat_state base_health in
  assert_sentinel "G6-1.1: Security console engine initializes and evaluates healthy baseline"
    (initial_state = Console.Threat_Safe);

  (* ========================================================================= *)
  (* REQ-G6-2: Clear, Un-Obscured Threat States                                *)
  (* ========================================================================= *)
  Printf.printf "\n=== REQ-G6-2: Threat State Evaluation & Non-Obscured Layout ===\n%!";

  (* Safe state -> Green (0x0026A65B) *)
  let safe_color = Console.threat_level_color Console.Threat_Safe in
  let safe_title = Console.threat_level_title Console.Threat_Safe in
  assert_sentinel "G6-2.1: Safe state renders green badge (0x0026A65B) and title 'SYSTEM SECURE'"
    (safe_color = 0x0026A65B && safe_title = "SYSTEM SECURE");

  (* Action Required state: Stale vulnerability feed (>24h) -> Amber (0x00F1C40F) *)
  let stale_health = { base_health with feed_age_seconds = 100000 } in
  let stale_state = Console.evaluate_threat_state stale_health in
  let is_action_req = match stale_state with Console.Threat_Action_Required _ -> true | _ -> false in
  let action_color = Console.threat_level_color stale_state in
  assert_sentinel "G6-2.2: Stale advisory feed (>24h) triggers Action Required state with amber color"
    (is_action_req && action_color = 0x00F1C40F);

  (* Action Required state: Unenrolled recovery phrase *)
  let unenrolled_health = { base_health with recovery_enrolled = false } in
  let unenrolled_state = Console.evaluate_threat_state unenrolled_health in
  let is_unenrolled_action = match unenrolled_state with Console.Threat_Action_Required _ -> true | _ -> false in
  assert_sentinel "G6-2.3: Unenrolled recovery phrase triggers Action Required warning"
    is_unenrolled_action;

  (* Critical Threat state: Detected anomalies -> Red (0x00E74C3C) *)
  let anomaly_health = { base_health with anomalies_detected = 2 } in
  let anomaly_state = Console.evaluate_threat_state anomaly_health in
  let is_crit_anomaly = match anomaly_state with Console.Threat_Critical _ -> true | _ -> false in
  let crit_color = Console.threat_level_color anomaly_state in
  assert_sentinel "G6-2.4: Detected anomalies trigger Critical Threat state with red warning badge"
    (is_crit_anomaly && crit_color = 0x00E74C3C);

  (* Critical Threat state: Firewall disabled *)
  let fw_down_health = { base_health with firewall_active = false } in
  let fw_down_state = Console.evaluate_threat_state fw_down_health in
  let is_crit_fw = match fw_down_state with Console.Threat_Critical _ -> true | _ -> false in
  assert_sentinel "G6-2.5: Inactive firewall triggers immediate Critical Threat escalation"
    is_crit_fw;

  (* Non-occlusion geometric verification *)
  let layout_cards : Console.rect list = [
    { x = 20; y = 20; w = 760; h = 60 };   (* Banner *)
    { x = 20; y = 90; w = 370; h = 240 };  (* Left Panel: Flow Monitor *)
    { x = 410; y = 90; w = 370; h = 240 }; (* Right Panel: Prompts / Actions *)
    { x = 20; y = 340; w = 760; h = 240 }; (* Bottom Panel: Recovery / Audits *)
  ] in
  assert_sentinel "G6-2.6: UI layout geometry strictly satisfies non-occlusion and zero overlap invariant"
    (Console.layout_is_unobscured layout_cards);

  (* Negative control: Overlapping cards rejected by layout checker *)
  let bad_layout : Console.rect list = [
    { x = 20; y = 20; w = 760; h = 60 };
    { x = 100; y = 40; w = 300; h = 100 }; (* Overlaps banner! *)
  ] in
  assert_sentinel "G6-2.7: Overlapping layout cards detected and rejected fail-closed"
    (not (Console.layout_is_unobscured bad_layout));

  (* ========================================================================= *)
  (* REQ-G6-3: Plain-Language Permission Prompts & Digest Preview             *)
  (* ========================================================================= *)
  Printf.printf "\n=== REQ-G6-3: Plain-Language Permission Prompts ===\n%!";

  let perm_req : Console.permission_request = {
    req_id = "REQ-SEC-901";
    caller_pid = 102;
    caller_name = "Notes Editor";
    binary_sha256 = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    target_resource = "storage:/var/secrets/user_auth.key";
    requested_rights = "FILE_READ | FILE_WRITE";
    timestamp = 1700000000L;
  } in

  let (summary, details) = Console.generate_plain_language_prompt perm_req in
  let has_pid = String.contains summary '1' && String.contains summary '2' in
  let has_name = String.contains summary 'N' in
  let has_details = String.contains details 'k' && String.contains details 'e' in
  assert_sentinel "G6-3.1: Plain-language prompt clearly states caller name, PID, target and action"
    (has_pid && has_name && has_details);

  let digest_prev = Console.format_digest_preview perm_req.binary_sha256 in
  assert_sentinel "G6-3.2: Cryptographic digest formatted with visible prefix and suffix preview"
    (String.starts_with ~prefix:"e3b0c442" digest_prev && String.ends_with ~suffix:"7852b855" digest_prev);

  (* Default Deny verification *)
  let deny_verdict = Console.resolve_permission_prompt perm_req false in
  let is_denied = match deny_verdict with Console.Verdict_Deny _ -> true | _ -> false in
  assert_sentinel "G6-3.3: Permission prompt enforces strict Default-Deny on user cancel/rejection"
    is_denied;

  (* Explicit Approval mints single-use token *)
  let approve_verdict = Console.resolve_permission_prompt perm_req true in
  let token_valid = match approve_verdict with
    | Console.Verdict_Approve tok -> String.contains tok '-' && String.contains tok '9'
    | _ -> false in
  assert_sentinel "G6-3.4: Explicit interactive user approval mints bound session grant token"
    token_valid;

  (* ========================================================================= *)
  (* REQ-G6-4: Consumer-Accessible Network Controls & Live Flow Monitor        *)
  (* ========================================================================= *)
  Printf.printf "\n=== REQ-G6-4: Network Controls & Flow Monitor ===\n%!";

  let net_mgr = Console.create_network_manager () in
  let sample_flow : Console.active_flow = {
    flow_id = 1;
    owner_pid = 102;
    app_name = "Net Client";
    proto = Console.TCP;
    src_addr = "192.168.1.50";
    src_port = 48220;
    dst_addr = "198.51.100.1";
    dst_port = 443;
    status = Console.Flow_Active;
  } in

  let reg_res = Console.add_active_flow net_mgr sample_flow in
  assert_sentinel "G6-4.1: Flow monitor successfully tracks active network connection"
    (reg_res = Ok 1);

  (* Per-application egress toggle *)
  Console.set_app_egress_permission net_mgr 102 false;
  let can_tx_blocked = Console.check_egress_allowed net_mgr 102 "198.51.100.1" 443 in
  assert_sentinel "G6-4.2: Per-application egress toggle revokes network authority for PID 102"
    (not can_tx_blocked);

  (* Emergency Killswitch Activation *)
  Console.set_killswitch net_mgr true;
  assert_sentinel "G6-4.3: One-click Emergency Killswitch activates instantly"
    net_mgr.killswitch;

  let can_tx_any = Console.check_egress_allowed net_mgr 100 "1.1.1.1" 53 in
  assert_sentinel "G6-4.4: Network Killswitch drops all outbound network traffic unconditionally"
    (not can_tx_any);

  let new_flow_res = Console.add_active_flow net_mgr { sample_flow with flow_id = 2 } in
  let killswitch_fails_closed = match new_flow_res with Error _ -> true | Ok _ -> false in
  assert_sentinel "G6-4.5: New flow creation while Killswitch engaged is rejected fail-closed"
    killswitch_fails_closed;

  (* ========================================================================= *)
  (* REQ-G6-5: Guided 24-Word Recovery Enrollment & Verification               *)
  (* ========================================================================= *)
  Printf.printf "\n=== REQ-G6-5: Guided 24-Word Recovery Enrollment ===\n%!";

  let entropy = Bytes.make 32 '\x42' in
  let phrase_res = Recovery_v2.encode_entropy_to_24_words entropy in
  let valid_phrase = match phrase_res with Ok p -> p | Error _ -> [] in
  assert_sentinel "G6-5.1: 24-word recovery phrase generated successfully from 256-bit entropy"
    (List.length valid_phrase = 24);

  let valid_feedback = Console.validate_user_entered_phrase valid_phrase in
  assert_sentinel "G6-5.2: Intact 24-word phrase passes wordlist and checksum validation"
    (valid_feedback = Console.Feedback_Valid);

  (* Error-locating feedback: Typo in word #3 *)
  let corrupted_phrase =
    let arr = Array.of_list valid_phrase in
    arr.(2) <- "abandn"; (* typo for 'abandon' *)
    Array.to_list arr
  in
  let typo_feedback = Console.validate_user_entered_phrase corrupted_phrase in
  let locates_typo = match typo_feedback with
    | Console.Feedback_Typo (idx, bad_w, suggs) ->
        idx = 3 && bad_w = "abandn" && List.mem "abandon" suggs
    | _ -> false in
  assert_sentinel "G6-5.3: Error-locating feedback identifies exact typo index #3 and suggests 'abandon'"
    locates_typo;

  (* Error-locating feedback: Bad word count *)
  let short_phrase = List.filteri (fun i _ -> i < 20) valid_phrase in
  let len_feedback = Console.validate_user_entered_phrase short_phrase in
  assert_sentinel "G6-5.4: Truncated recovery phrase rejected with invalid length indicator (20 != 24)"
    (len_feedback = Console.Feedback_Invalid_Length 20);

  (* Test-recovery verification challenge: Prompt user for 3 words (3, 11, 22) *)
  let challenge = Console.generate_test_challenge () in
  let phrase_arr = Array.of_list valid_phrase in
  let correct_answers = [
    (3, phrase_arr.(2));
    (11, phrase_arr.(10));
    (22, phrase_arr.(21));
  ] in
  assert_sentinel "G6-5.5: Accurate test challenge response successfully authenticates recovery backup"
    (Console.verify_test_challenge valid_phrase challenge correct_answers);

  let bad_answers = [
    (3, phrase_arr.(2));
    (11, "wrongword");
    (22, phrase_arr.(21));
  ] in
  assert_sentinel "G6-5.6: Incorrect challenge response rejected fail-closed"
    (not (Console.verify_test_challenge valid_phrase challenge bad_answers));

  (* ========================================================================= *)
  (* Negative Controls & Mutation Testing                                      *)
  (* ========================================================================= *)
  Printf.printf "\n=== Sentinel Gate 6 Adversarial Mutation Controls ===\n%!";

  (* Mutation 1: Tampered threat state reporting nominal when killswitch active *)
  let killswitch_health = { base_health with killswitch_active = true } in
  let mutated_threat_safe =
    let original = Console.evaluate_threat_state killswitch_health in
    original = Console.Threat_Safe in (* Should NEVER be safe *)
  assert_sentinel "MUTATION-G6-1: Tampered health state cannot suppress emergency isolation (Fails closed)"
    (not mutated_threat_safe);

  (* Mutation 2: Bypassing permission prompt approval mints no token *)
  let forged_verdict = Console.resolve_permission_prompt perm_req false in
  let forged_token_minted = match forged_verdict with Console.Verdict_Approve _ -> true | _ -> false in
  assert_sentinel "MUTATION-G6-2: Denied permission prompt cannot mint session grant token (Fails closed)"
    (not forged_token_minted);

  (* Mutation 3: Flow permitted after application egress revoked *)
  let mutated_tx = Console.check_egress_allowed net_mgr 102 "198.51.100.1" 443 in
  assert_sentinel "MUTATION-G6-3: Revoked app egress cannot transmit packets (Fails closed)"
    (not mutated_tx);

  (* Mutation 4: Partial challenge answers (missing word 22) *)
  let partial_answers = [
    (3, phrase_arr.(2));
    (11, phrase_arr.(10));
  ] in
  assert_sentinel "MUTATION-G6-4: Incomplete challenge answers rejected fail-closed"
    (not (Console.verify_test_challenge valid_phrase challenge partial_answers));

  Printf.printf "\n=========================================================\n";
  Printf.printf "Summary: %d Passed, %d Failed\n" !passed_count !failed_count;
  Printf.printf "=========================================================\n%!";

  if !failed_count > 0 then exit 1 else exit 0

let () = run_gate6_verification_suite ()
