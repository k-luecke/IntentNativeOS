(* Code Audit Checklist and Security Test Evidence Registry (A014, S103) *)
open Types

let audit_registry : audit_finding list = [
  {
    id = "SEC-FIND-001";
    cwe = "CWE-287";
    title = "Improper Authentication in Cross-Device Approvals";
    severity = "Critical";
    status = "Mitigated";
    mitigation = "Hardware-backed phone assertion with Ed25519 signature and out-of-band SAS key agreement.";
  };
  {
    id = "SEC-FIND-002";
    cwe = "CWE-367";
    title = "Time-of-Check Time-of-Use (TOCTOU) Grant Race";
    severity = "High";
    status = "Mitigated";
    mitigation = "Atomic single-use token consumption with monotonic nonce in authorization broker.";
  };
  {
    id = "SEC-FIND-003";
    cwe = "CWE-294";
    title = "Authentication Bypass by Capture-Replay";
    severity = "High";
    status = "Mitigated";
    mitigation = "Strict monotonic counter enforcement and short-lived TTL on authorization tokens.";
  };
  {
    id = "SEC-FIND-004";
    cwe = "CWE-757";
    title = "Cryptographic Suite Downgrade Under Adversarial Negotiation";
    severity = "High";
    status = "Mitigated";
    mitigation = "Fail-closed security profiles prohibiting non-PQ suites under Post_Quantum_Strict.";
  };
  {
    id = "SEC-FIND-005";
    cwe = "CWE-22";
    title = "Improper Limitation of a Pathname to a Restricted Directory (Path Traversal)";
    severity = "Critical";
    status = "Mitigated";
    mitigation = "Strict path sanitization rejecting null bytes, relative dots, and path escaping in guest broker.";
  };
]

let verify_audit_checklist () : (int * int) =
  let total = List.length audit_registry in
  let mitigated = List.fold_left (fun acc f -> if f.status = "Mitigated" then acc + 1 else acc) 0 audit_registry in
  (total, mitigated)

(* ========================================================================= *)
(* S103: Tamper-Evident Hash-Chained Security Event Audit Log Subsystem      *)
(* ========================================================================= *)

type event_severity =
  | Severity_Info
  | Severity_Notice
  | Severity_Warning
  | Severity_Critical
  | Severity_Alert

let string_of_severity = function
  | Severity_Info -> "INFO"
  | Severity_Notice -> "NOTICE"
  | Severity_Warning -> "WARNING"
  | Severity_Critical -> "CRITICAL"
  | Severity_Alert -> "ALERT"

type event_kind =
  | Auth_Grant_Minted
  | Auth_Grant_Consumed
  | Auth_Grant_Denied
  | Capability_Delegated
  | Capability_Revoked
  | Syscall_Permission_Denied
  | Firewall_Packet_Dropped
  | Integrity_Tamper_Detected
  | Checkpoint_Committed
  | Recovery_Ceremony_Executed
  | Step_Up_Requested
  | Step_Up_Completed
  | Step_Up_Rejected
  | Epistemic_Boundary_Violation
  | Cross_Agent_Delegation

let string_of_kind = function
  | Auth_Grant_Minted -> "AUTH_GRANT_MINTED"
  | Auth_Grant_Consumed -> "AUTH_GRANT_CONSUMED"
  | Auth_Grant_Denied -> "AUTH_GRANT_DENIED"
  | Capability_Delegated -> "CAPABILITY_DELEGATED"
  | Capability_Revoked -> "CAPABILITY_REVOKED"
  | Syscall_Permission_Denied -> "SYSCALL_PERMISSION_DENIED"
  | Firewall_Packet_Dropped -> "FIREWALL_PACKET_DROPPED"
  | Integrity_Tamper_Detected -> "INTEGRITY_TAMPER_DETECTED"
  | Checkpoint_Committed -> "CHECKPOINT_COMMITTED"
  | Recovery_Ceremony_Executed -> "RECOVERY_CEREMONY_EXECUTED"
  | Step_Up_Requested -> "STEP_UP_REQUESTED"
  | Step_Up_Completed -> "STEP_UP_COMPLETED"
  | Step_Up_Rejected -> "STEP_UP_REJECTED"
  | Epistemic_Boundary_Violation -> "EPISTEMIC_BOUNDARY_VIOLATION"
  | Cross_Agent_Delegation -> "CROSS_AGENT_DELEGATION"

type audit_event = {
  sequence_num : int64;
  timestamp : int64;
  event_id : string;
  severity : event_severity;
  kind : event_kind;
  principal_id : string;
  target_resource : string;
  attributes : (string * string) list;
  prev_hash : string;
  event_hash : string;
}

type authenticated_checkpoint = {
  checkpoint_seq : int64;
  timestamp : int64;
  last_event_seq : int64;
  state_digest : string;
  signed_anchor : string;
}

type audit_journal = {
  max_capacity : int;
  mutable events : audit_event list; (* Stored in chronological order: oldest to newest *)
  mutable current_seq : int64;
  mutable latest_hash : string;
  mutable checkpoints : authenticated_checkpoint list;
  mutable rotation_count : int;
}

type integrity_violation =
  | SequenceGap of int64 * int64
  | HashMismatch of int64 * string * string
  | BrokenPrevPointer of int64 * string * string
  | TimestampRegression of int64 * int64
  | TruncatedWrite of int64
  | UnanchoredHistory of string

let genesis_hash = "0000000000000000000000000000000000000000000000000000000000000000"

let compute_event_hash
    (seq : int64)
    (time : int64)
    (sev : event_severity)
    (k : event_kind)
    (principal : string)
    (target : string)
    (attrs : (string * string) list)
    (prev : string) : string =
  let sorted_attrs = List.sort (fun (k1, _) (k2, _) -> String.compare k1 k2) attrs in
  let attr_str = List.fold_left (fun acc (k, v) -> acc ^ "|" ^ k ^ "=" ^ v) "" sorted_attrs in
  let raw = Printf.sprintf "SEQ:%Ld|TS:%Ld|SEV:%s|KIND:%s|PRIN:%s|TGT:%s|ATTR:%s|PREV:%s"
              seq time (string_of_severity sev) (string_of_kind k) principal target attr_str prev in
  Sha256.digest_string raw

let create_journal ?(max_capacity = 1000) () : audit_journal = {
  max_capacity;
  events = [];
  current_seq = 0L;
  latest_hash = genesis_hash;
  checkpoints = [];
  rotation_count = 0;
}

let append_event
    (journal : audit_journal)
    ~(severity : event_severity)
    ~(kind : event_kind)
    ~(principal_id : string)
    ~(target_resource : string)
    ~(attributes : (string * string) list)
    ~(timestamp : int64) : audit_event =
  let next_seq = Int64.add journal.current_seq 1L in
  let prev = journal.latest_hash in
  let hash = compute_event_hash next_seq timestamp severity kind principal_id target_resource attributes prev in
  let event_id = Printf.sprintf "EVT-%Ld-%s" next_seq (String.sub hash 0 8) in
  let event = {
    sequence_num = next_seq;
    timestamp;
    event_id;
    severity;
    kind;
    principal_id;
    target_resource;
    attributes;
    prev_hash = prev;
    event_hash = hash;
  } in

  journal.current_seq <- next_seq;
  journal.latest_hash <- hash;
  journal.events <- journal.events @ [event];

  (* Bounded log rotation handling *)
  if List.length journal.events > journal.max_capacity then begin
    journal.rotation_count <- journal.rotation_count + 1;
    let retained = List.tl journal.events in
    journal.events <- retained
  end;

  event

let create_checkpoint
    (journal : audit_journal)
    ~(timestamp : int64)
    ~(anchor_secret : string) : authenticated_checkpoint =
  let chk_seq = Int64.add (Int64.of_int (List.length journal.checkpoints)) 1L in
  let last_seq = journal.current_seq in
  let state_digest = journal.latest_hash in
  let anchor_payload = Printf.sprintf "CHK:%Ld|TS:%Ld|LAST:%Ld|DIGEST:%s|SECRET:%s"
                         chk_seq timestamp last_seq state_digest anchor_secret in
  let signed_anchor = Sha256.digest_string anchor_payload in
  let chk = {
    checkpoint_seq = chk_seq;
    timestamp;
    last_event_seq = last_seq;
    state_digest;
    signed_anchor;
  } in
  journal.checkpoints <- journal.checkpoints @ [chk];
  chk

let verify_checkpoint
    (chk : authenticated_checkpoint)
    ~(anchor_secret : string) : bool =
  let expected_payload = Printf.sprintf "CHK:%Ld|TS:%Ld|LAST:%Ld|DIGEST:%s|SECRET:%s"
                           chk.checkpoint_seq chk.timestamp chk.last_event_seq chk.state_digest anchor_secret in
  let expected_anchor = Sha256.digest_string expected_payload in
  String.equal chk.signed_anchor expected_anchor

let verify_journal_integrity
    (journal : audit_journal) : (int * string, integrity_violation) result =
  let rec check_chain events expected_prev prev_ts count =
    match events with
    | [] -> Ok (count, expected_prev)
    | ev :: rest ->
        if not (String.equal ev.prev_hash expected_prev) then
          Error (BrokenPrevPointer (ev.sequence_num, ev.prev_hash, expected_prev))
        else if ev.timestamp < prev_ts then
          Error (TimestampRegression (ev.sequence_num, ev.timestamp))
        else
          let computed = compute_event_hash
            ev.sequence_num ev.timestamp ev.severity ev.kind
            ev.principal_id ev.target_resource ev.attributes ev.prev_hash in
          if not (String.equal ev.event_hash computed) then
            Error (HashMismatch (ev.sequence_num, ev.event_hash, computed))
          else
            check_chain rest ev.event_hash ev.timestamp (count + 1)
  in
  match journal.events with
  | [] -> Ok (0, genesis_hash)
  | first :: _ ->
      let initial_prev = first.prev_hash in
      check_chain journal.events initial_prev 0L 0

let handle_crash_and_recover
    (journal : audit_journal) : int =
  (* Prune any malformed or half-written events from the tail until the chain is integral *)
  let valid_events = ref [] in
  let current_prev = ref (match journal.events with [] -> genesis_hash | h :: _ -> h.prev_hash) in
  let last_valid_seq = ref 0L in
  let count = ref 0 in

  List.iter (fun ev ->
    let computed = compute_event_hash
      ev.sequence_num ev.timestamp ev.severity ev.kind
      ev.principal_id ev.target_resource ev.attributes !current_prev in
    if String.equal ev.prev_hash !current_prev && String.equal ev.event_hash computed then begin
      valid_events := !valid_events @ [ev];
      current_prev := ev.event_hash;
      last_valid_seq := ev.sequence_num;
      incr count
    end
  ) journal.events;

  journal.events <- !valid_events;
  journal.current_seq <- !last_valid_seq;
  journal.latest_hash <- !current_prev;
  !count
