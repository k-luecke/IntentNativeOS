(* security/console.ml - Sentinel Native Security Console Engine (S601 / Gate 6)
   Implements:
   - REQ-G6-1: Accessible security console state & UI engine
   - REQ-G6-2: Clear, un-obscured visual threat states (Safe/Action Required/Critical Threat)
   - REQ-G6-3: Plain-language permission prompts with cryptographic digest preview & default deny
   - REQ-G6-4: Consumer-accessible network controls, live flow monitor & emergency killswitch
   - REQ-G6-5: Guided 24-word BIP-39 recovery enrollment, error-locating feedback, & challenge verification *)
(* ========================================================================= *)
(* REQ-G6-2: Visual Threat States & System Health Assessment                *)
(* ========================================================================= *)

type threat_level =
  | Threat_Safe
  | Threat_Action_Required of string
  | Threat_Critical of string

type subsystem_health = {
  firewall_active: bool;
  conntrack_count: int;
  max_conntrack: int;
  dnssec_strict: bool;
  tuf_update_fresh: bool;
  feed_age_seconds: int;
  anomalies_detected: int;
  unauthorized_attempts: int;
  recovery_enrolled: bool;
  killswitch_active: bool;
}

let evaluate_threat_state (health : subsystem_health) : threat_level =
  if health.killswitch_active then
    Threat_Critical "EMERGENCY_ISOLATION: Network killswitch engaged"
  else if health.anomalies_detected > 0 then
    Threat_Critical (Printf.sprintf "CRITICAL_THREAT: %d security anomalies detected" health.anomalies_detected)
  else if health.unauthorized_attempts > 0 then
    Threat_Critical (Printf.sprintf "SECURITY_BREACH: %d unauthorized access attempts intercepted" health.unauthorized_attempts)
  else if not health.firewall_active then
    Threat_Critical "FIREWALL_DOWN: Native firewall subsystem inactive"
  else if health.feed_age_seconds > 86400 then
    Threat_Action_Required "STALE_INTEL: Vulnerability advisory feed older than 24 hours"
  else if not health.recovery_enrolled then
    Threat_Action_Required "UNSECURED_DEVICE: Hardware recovery phrase not yet enrolled"
  else if not health.tuf_update_fresh then
    Threat_Action_Required "UPDATE_STAGED: Security update pending activation"
  else
    Threat_Safe

let threat_level_color = function
  | Threat_Safe -> 0x0026A65B       (* Vivid Green *)
  | Threat_Action_Required _ -> 0x00F1C40F (* Amber / Yellow *)
  | Threat_Critical _ -> 0x00E74C3C (* Red / Warning *)

let threat_level_title = function
  | Threat_Safe -> "SYSTEM SECURE"
  | Threat_Action_Required _ -> "ACTION REQUIRED"
  | Threat_Critical _ -> "CRITICAL THREAT"

(* Geometric non-occlusion validator:
   Ensures that status cards, banners, and modals do not overlap or occlude critical text *)
type rect = { x: int; y: int; w: int; h: int }

let rects_overlap (r1 : rect) (r2 : rect) : bool =
  not (r1.x + r1.w <= r2.x || r2.x + r2.w <= r1.x ||
       r1.y + r1.h <= r2.y || r2.y + r2.h <= r1.y)

let layout_is_unobscured (cards : rect list) : bool =
  let rec check = function
    | [] -> true
    | r :: rest ->
        if List.exists (rects_overlap r) rest then false
        else check rest
  in
  check cards

(* ========================================================================= *)
(* REQ-G6-3: Plain-Language Permission Prompts & Digest Preview             *)
(* ========================================================================= *)

type permission_request = {
  req_id: string;
  caller_pid: int;
  caller_name: string;
  binary_sha256: string;
  target_resource: string;
  requested_rights: string;
  timestamp: Int64.t;
}

type prompt_verdict =
  | Verdict_Approve of string (* session grant token *)
  | Verdict_Deny of string

let format_digest_preview (sha256_hex : string) : string =
  if String.length sha256_hex < 16 then sha256_hex
  else
    let head = String.sub sha256_hex 0 8 in
    let tail = String.sub sha256_hex (String.length sha256_hex - 8) 8 in
    head ^ "..." ^ tail

let generate_plain_language_prompt (req : permission_request) : string * string =
  let summary = Printf.sprintf "Application \"%s\" (PID %d) requests %s"
    req.caller_name req.caller_pid req.requested_rights in
  let digest_preview = format_digest_preview req.binary_sha256 in
  let details = Printf.sprintf
    "Target Resource: %s\nCaller Identity: %s (PID %d)\nBinary SHA-256:  %s\nRequest ID:       %s\nSecurity Policy:  Default Deny"
    req.target_resource req.caller_name req.caller_pid digest_preview req.req_id in
  (summary, details)

let resolve_permission_prompt (req : permission_request) (approved : bool) : prompt_verdict =
  if approved then
    (* Only interactive confirmation mints short-lived token *)
    let token = Printf.sprintf "GRANT-%s-%d-%s" req.req_id req.caller_pid (format_digest_preview req.binary_sha256) in
    Verdict_Approve token
  else
    Verdict_Deny (Printf.sprintf "User denied permission request %s for PID %d" req.req_id req.caller_pid)

(* ========================================================================= *)
(* REQ-G6-4: Consumer-Accessible Network Controls & Live Flow Monitor        *)
(* ========================================================================= *)

type proto_type = TCP | UDP | ICMP

type flow_status =
  | Flow_Active
  | Flow_Closing
  | Flow_Blocked
  | Flow_Isolated

type active_flow = {
  flow_id: int;
  owner_pid: int;
  app_name: string;
  proto: proto_type;
  src_addr: string;
  src_port: int;
  dst_addr: string;
  dst_port: int;
  status: flow_status;
}

type network_manager = {
  mutable killswitch: bool;
  mutable app_egress_rules: (int, bool) Hashtbl.t; (* pid -> allowed *)
  mutable flows: active_flow list;
}

let create_network_manager () : network_manager = {
  killswitch = false;
  app_egress_rules = Hashtbl.create 16;
  flows = [];
}

let set_killswitch (mgr : network_manager) (enable : bool) : unit =
  mgr.killswitch <- enable;
  if enable then
    mgr.flows <- List.map (fun f -> { f with status = Flow_Isolated }) mgr.flows

let set_app_egress_permission (mgr : network_manager) (pid : int) (allowed : bool) : unit =
  Hashtbl.replace mgr.app_egress_rules pid allowed;
  if not allowed then
    mgr.flows <- List.map (fun f ->
      if f.owner_pid = pid then { f with status = Flow_Blocked } else f
    ) mgr.flows

let check_egress_allowed (mgr : network_manager) (pid : int) (_dst_ip : string) (_dst_port : int) : bool =
  if mgr.killswitch then false
  else
    match Hashtbl.find_opt mgr.app_egress_rules pid with
    | Some false -> false
    | _ -> true (* Default allows if killswitch off and not specifically denied *)

let add_active_flow (mgr : network_manager) (flow : active_flow) : (int, string) result =
  if mgr.killswitch then
    Error "Network killswitch engaged; outbound flows rejected"
  else if not (check_egress_allowed mgr flow.owner_pid flow.dst_addr flow.dst_port) then
    Error (Printf.sprintf "PID %d egress permission revoked" flow.owner_pid)
  else begin
    mgr.flows <- flow :: mgr.flows;
    Ok flow.flow_id
  end

(* ========================================================================= *)
(* REQ-G6-5: Guided 24-Word Recovery Enrollment & Test Recovery              *)
(* ========================================================================= *)

(* Levenshtein string distance for error-locating typo feedback *)
let min3 a b c = min a (min b c)

let levenshtein s1 s2 =
  let len1 = String.length s1 in
  let len2 = String.length s2 in
  let matrix = Array.make_matrix (len1 + 1) (len2 + 1) 0 in
  for i = 0 to len1 do matrix.(i).(0) <- i done;
  for j = 0 to len2 do matrix.(0).(j) <- j done;
  for i = 1 to len1 do
    for j = 1 to len2 do
      let cost = if s1.[i - 1] = s2.[j - 1] then 0 else 1 in
      matrix.(i).(j) <- min3
        (matrix.(i - 1).(j) + 1)
        (matrix.(i).(j - 1) + 1)
        (matrix.(i - 1).(j - 1) + cost)
    done
  done;
  matrix.(len1).(len2)

let suggest_corrections (word : string) : string list =
  let dict = Recovery_v2.bip39_dictionary in
  let matches = ref [] in
  Array.iter (fun candidate ->
    let dist = levenshtein word candidate in
    if dist <= 2 then matches := (dist, candidate) :: !matches
  ) dict;
  let sorted = List.sort (fun (d1, _) (d2, _) -> compare d1 d2) !matches in
  List.map snd (List.filter (fun (d, _) -> d <= 2) sorted)

type recovery_validation_feedback =
  | Feedback_Valid
  | Feedback_Invalid_Length of int
  | Feedback_Typo of int * string * string list (* index, bad word, suggestions *)
  | Feedback_Checksum_Error of string

let validate_user_entered_phrase (words : string list) : recovery_validation_feedback =
  if List.length words <> 24 then
    Feedback_Invalid_Length (List.length words)
  else
    (* Check for typos *)
    let rec check_typos idx = function
      | [] -> None
      | w :: rest ->
          (match Recovery_v2.find_word_index w with
           | Some _ -> check_typos (idx + 1) rest
           | None ->
               let suggestions = suggest_corrections w in
               Some (Feedback_Typo (idx + 1, w, suggestions)))
    in
    match check_typos 0 words with
    | Some err -> err
    | None ->
        (match Recovery_v2.verify_and_decode_24_words words with
         | Ok _ -> Feedback_Valid
         | Error (Recovery_v2.ChecksumMismatch (exp, comp)) ->
             Feedback_Checksum_Error (Printf.sprintf "Checksum failed: expected 0x%02X, got 0x%02X" exp comp)
         | Error _ ->
             Feedback_Checksum_Error "Integrity check failed")

type recovery_challenge = {
  indices: int list; (* 1-indexed, e.g. [3; 11; 22] *)
}

let generate_test_challenge () : recovery_challenge =
  (* Fixed representative 3-word challenge indices for repeatable verification *)
  { indices = [3; 11; 22] }

let verify_test_challenge (full_phrase : string list) (challenge : recovery_challenge) (answers : (int * string) list) : bool =
  let phrase_arr = Array.of_list full_phrase in
  if Array.length phrase_arr <> 24 then false
  else
    List.for_all (fun req_idx ->
      match List.assoc_opt req_idx answers with
      | None -> false
      | Some entered_word ->
          if req_idx >= 1 && req_idx <= 24 then
            String.equal phrase_arr.(req_idx - 1) entered_word
          else false
    ) challenge.indices
