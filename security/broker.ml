(* Authorization Broker & Replay-Resistant Single-Action Grants (A002, A003, S302) *)
open Types

type fido2_flags = {
  user_present : bool;
  user_verified : bool;
}

type fido2_auth_data = {
  rp_id_hash : string;
  flags : fido2_flags;
  sign_count : int64;
}

type step_up_challenge = {
  challenge_id : string;
  op_digest : canonical_digest;
  target_op : privileged_op;
  rp_id : string;
  created_at : int64;
  ttl_seconds : int64;
  mutable consumed : bool;
}

type fido2_assertion = {
  challenge_id : string;
  rp_id : string;
  auth_data : fido2_auth_data;
  signature : string;
  credential_id : string;
}

type step_up_error =
  | ChallengeNotFound
  | ChallengeExpired of int64 * int64
  | ChallengeAlreadyConsumed
  | RpIdMismatch of string * string
  | UserPresenceRequired
  | UserVerificationRequired
  | InvalidSignature
  | UntrustedCredential
  | OperationDigestMismatch of string * string
  | StaleSignCount of int64 * int64

type broker_state = {
  mutable active_grants : (string, grant_token) Hashtbl.t;
  mutable consumed_grants : (string, int64) Hashtbl.t;
  mutable next_grant_seq : int64;
  mutable challenges : (string, step_up_challenge) Hashtbl.t;
  mutable consumed_challenges : (string, int64) Hashtbl.t;
  mutable enrolled_credentials : (string, string) Hashtbl.t; (* credential_id -> public_key *)
  mutable credential_sign_counts : (string, int64) Hashtbl.t; (* credential_id -> last_sign_count *)
}

let create_broker () : broker_state = {
  active_grants = Hashtbl.create 32;
  consumed_grants = Hashtbl.create 64;
  next_grant_seq = 1L;
  challenges = Hashtbl.create 32;
  consumed_challenges = Hashtbl.create 64;
  enrolled_credentials = Hashtbl.create 16;
  credential_sign_counts = Hashtbl.create 16;
}

let enroll_fido2_credential (broker : broker_state) ~(credential_id : string) ~(pubkey : string) : unit =
  Hashtbl.replace broker.enrolled_credentials credential_id pubkey;
  Hashtbl.replace broker.credential_sign_counts credential_id 0L

(* A002: Trust placement enforcement - Model or Guest VM cannot mint grants *)
let can_mint_grants (role : caller_role) : bool =
  match role with
  | Untrusted_Model -> false
  | Untrusted_Guest_VM -> false
  | System_Service -> false
  | Host_Administrator -> true

let mint_grant
    (broker : broker_state)
    ~(role : caller_role)
    ~(caller_id : string)
    ~(op : privileged_op)
    ~(current_time : int64)
    ~(ttl : int64)
    : broker_decision =
  if not (can_mint_grants role) then
    Denied (Printf.sprintf "Caller role cannot mint authorization grants (role=%s, caller=%s)"
              (match role with
               | Untrusted_Model -> "Untrusted_Model"
               | Untrusted_Guest_VM -> "Untrusted_Guest_VM"
               | System_Service -> "System_Service"
               | Host_Administrator -> "Host_Administrator")
              caller_id)
  else
    let seq = broker.next_grant_seq in
    broker.next_grant_seq <- Int64.add seq 1L;
    let digest = Canonical.compute_digest op in
    let grant_id = Printf.sprintf "grant-%Ld-%s" seq (String.sub digest 0 8) in
    let token = {
      grant_id;
      op_digest = digest;
      issued_at = current_time;
      ttl_seconds = ttl;
      consumed = false;
      authorized_by = caller_id;
    } in
    Hashtbl.replace broker.active_grants grant_id token;
    Granted token

(* A003: Validation check: Reject expired, altered, or replayed grants *)
type grant_validation_error =
  | GrantNotFound
  | GrantAlreadyConsumed
  | GrantExpired of int64 * int64
  | DigestMismatch of string * string

let validate_and_consume_grant
    (broker : broker_state)
    (grant_id : string)
    (current_op : privileged_op)
    (current_time : int64)
    : (grant_token, grant_validation_error) result =
  if Hashtbl.mem broker.consumed_grants grant_id then
    Error GrantAlreadyConsumed
  else
    match Hashtbl.find_opt broker.active_grants grant_id with
    | None -> Error GrantNotFound
    | Some token ->
        if token.consumed then
          Error GrantAlreadyConsumed
        else
          let expiry = Int64.add token.issued_at token.ttl_seconds in
          if current_time > expiry then
            Error (GrantExpired (current_time, expiry))
          else
            let current_digest = Canonical.compute_digest current_op in
            if not (String.equal token.op_digest current_digest) then
              Error (DigestMismatch (token.op_digest, current_digest))
            else begin
              token.consumed <- true;
              Hashtbl.replace broker.consumed_grants grant_id current_time;
              Hashtbl.remove broker.active_grants grant_id;
              Ok token
            end

(* ========================================================================= *)
(* S302: Replay-Resistant Step-Up Authentication Broker (FIDO2 / WebAuthn)    *)
(* Cardinal Invariant: Authentication <> Authorization                       *)
(* Valid FIDO2 assertion proves human authentication to the broker,          *)
(* but ONLY mints a single-action, non-expandable capability grant.          *)
(* ========================================================================= *)

let compute_fido2_signature_payload
    ~(rp_id : string)
    ~(challenge_id : string)
    ~(op_digest : string)
    ~(flags : fido2_flags)
    ~(sign_count : int64) : string =
  Printf.sprintf "INOS_FIDO2_ASSERTION:v1|RP:%s|CHAL:%s|DIGEST:%s|UP:%b|UV:%b|CTR:%Ld"
    rp_id challenge_id op_digest flags.user_present flags.user_verified sign_count

let sign_fido2_assertion
    ~(privkey : string)
    ~(credential_id : string)
    ~(rp_id : string)
    ~(challenge_id : string)
    ~(op_digest : string)
    ~(user_present : bool)
    ~(user_verified : bool)
    ~(sign_count : int64) : fido2_assertion =
  let flags = { user_present; user_verified } in
  let rp_id_hash = Sha256.digest_string rp_id in
  let payload = compute_fido2_signature_payload ~rp_id ~challenge_id ~op_digest ~flags ~sign_count in
  let signature = Sha256.digest_string (payload ^ "|" ^ privkey) in
  {
    challenge_id;
    rp_id;
    auth_data = {
      rp_id_hash;
      flags;
      sign_count;
    };
    signature;
    credential_id;
  }

let request_step_up
    (broker : broker_state)
    ~(op : privileged_op)
    ~(rp_id : string)
    ~(current_time : int64)
    ~(ttl : int64)
    : step_up_challenge =
  let seq = broker.next_grant_seq in
  broker.next_grant_seq <- Int64.add seq 1L;
  let op_digest = Canonical.compute_digest op in
  let challenge_nonce = Sha256.digest_string (Printf.sprintf "%Ld|%s|%Ld" seq op_digest current_time) in
  let challenge_id = Printf.sprintf "chal-%Ld-%s" seq (String.sub challenge_nonce 0 16) in
  let challenge = {
    challenge_id;
    op_digest;
    target_op = op;
    rp_id;
    created_at = current_time;
    ttl_seconds = ttl;
    consumed = false;
  } in
  Hashtbl.replace broker.challenges challenge_id challenge;
  challenge

let verify_and_authorize_step_up
    (broker : broker_state)
    ~(assertion : fido2_assertion)
    ~(current_time : int64)
    ~(require_user_verification : bool)
    : (grant_token, step_up_error) result =
  (* 1. Check if challenge was already consumed (replay defense) *)
  if Hashtbl.mem broker.consumed_challenges assertion.challenge_id then
    Error ChallengeAlreadyConsumed
  else
    match Hashtbl.find_opt broker.challenges assertion.challenge_id with
    | None -> Error ChallengeNotFound
    | Some chal ->
        if chal.consumed then
          Error ChallengeAlreadyConsumed
        else
          (* 2. Check TTL *)
          let expiry = Int64.add chal.created_at chal.ttl_seconds in
          if current_time > expiry then
            Error (ChallengeExpired (current_time, expiry))
          (* 3. Relying-Party binding check *)
          else if not (String.equal assertion.rp_id chal.rp_id) then
            Error (RpIdMismatch (chal.rp_id, assertion.rp_id))
          (* 4. User Presence check (UP flag) *)
          else if not assertion.auth_data.flags.user_present then
            Error UserPresenceRequired
          (* 5. User Verification check (UV flag) for high-privilege operations *)
          else if require_user_verification && not assertion.auth_data.flags.user_verified then
            Error UserVerificationRequired
          else
            (* 6. Credential trust check *)
            match Hashtbl.find_opt broker.enrolled_credentials assertion.credential_id with
            | None -> Error UntrustedCredential
            | Some enrolled_pubkey ->
                (* 7. Signature verification *)
                let expected_payload = compute_fido2_signature_payload
                  ~rp_id:assertion.rp_id
                  ~challenge_id:chal.challenge_id
                  ~op_digest:chal.op_digest
                  ~flags:assertion.auth_data.flags
                  ~sign_count:assertion.auth_data.sign_count in
                let expected_sig = Sha256.digest_string (expected_payload ^ "|" ^ enrolled_pubkey) in
                if not (String.equal assertion.signature expected_sig) then
                  Error InvalidSignature
                else
                  (* 8. Monotonic sign_count verification (anti-cloning / replay defense) *)
                  let last_count =
                    match Hashtbl.find_opt broker.credential_sign_counts assertion.credential_id with
                    | Some c -> c
                    | None -> 0L
                  in
                  if assertion.auth_data.sign_count <= last_count then
                    Error (StaleSignCount (assertion.auth_data.sign_count, last_count))
                  else begin
                    Hashtbl.replace broker.credential_sign_counts assertion.credential_id assertion.auth_data.sign_count;
                    (* Invariant: Authentication != Authorization.
                       Valid FIDO2 assertion authenticates the user, but only authorizes
                       minting an explicit, single-use, strictly-bound grant token. *)
                    chal.consumed <- true;
                    Hashtbl.replace broker.consumed_challenges chal.challenge_id current_time;
                    Hashtbl.remove broker.challenges chal.challenge_id;

                    let seq = broker.next_grant_seq in
                    broker.next_grant_seq <- Int64.add seq 1L;
                    let grant_id = Printf.sprintf "stepup-grant-%Ld-%s" seq (String.sub chal.op_digest 0 8) in
                    let token = {
                      grant_id;
                      op_digest = chal.op_digest;
                      issued_at = current_time;
                      ttl_seconds = 60L; (* Strict 60s TTL for step-up grants *)
                      consumed = false;
                      authorized_by = Printf.sprintf "fido2:%s" assertion.credential_id;
                    } in
                    Hashtbl.replace broker.active_grants grant_id token;
                    Ok token
                  end
