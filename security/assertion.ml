(* Phone Trusted Approval Display & Hardware-Backed Verifier (A005, A006) *)
open Types

(* A005: Independent display formatting on phone device *)
let render_phone_display (op : privileged_op) : phone_display_view =
  let digest = Canonical.compute_digest op in
  let action_str = Canonical.string_of_action op.action in
  let preview = String.sub digest 0 16 ^ "..." ^ String.sub digest 48 16 in
  {
    target_summary = op.target;
    action_name = action_str;
    digest_preview = preview;
    timestamp_str = Printf.sprintf "Epoch:%Ld" op.timestamp;
  }

let sign_assertion
    ~(device_privkey : string)
    ~(device_key_id : string)
    ~(op_digest : string)
    ~(counter : int64)
    : phone_assertion =
  let assertion_id = Printf.sprintf "assert-%s-%Ld" device_key_id counter in
  let sign_payload = Printf.sprintf "INOS_ASSERT:v1|ID:%s|DIGEST:%s|KEY:%s|CTR:%Ld"
                       assertion_id op_digest device_key_id counter in
  (* Simulated Ed25519 signature computation *)
  let sig_bytes = Sha256.digest_string (sign_payload ^ "|" ^ device_privkey) in
  {
    assertion_id;
    op_digest;
    device_pubkey_id = device_key_id;
    monotonic_counter = counter;
    signature = sig_bytes;
  }

(* A006: Hardware-backed approval verifier *)
type verifier_error =
  | UnknownDevice of string
  | DeviceRevoked of string
  | StaleCounter of int64 * int64
  | DigestMismatch of string * string
  | SignatureVerificationFailed

type verifier_state = {
  enrolled_devices : (string, enrolled_device) Hashtbl.t;
}

let create_verifier () : verifier_state = {
  enrolled_devices = Hashtbl.create 16;
}

let enroll_device
    (verifier : verifier_state)
    ~(key_id : string)
    ~(pubkey : string)
    ~(enrolled_at : int64) : unit =
  let dev = {
    key_id;
    public_key = pubkey;
    enrolled_at;
    revoked = false;
    last_counter = 0L;
  } in
  Hashtbl.replace verifier.enrolled_devices key_id dev

let verify_hardware_approval
    (verifier : verifier_state)
    (assertion : phone_assertion)
    (expected_op : privileged_op)
    : (enrolled_device, verifier_error) result =
  match Hashtbl.find_opt verifier.enrolled_devices assertion.device_pubkey_id with
  | None -> Error (UnknownDevice assertion.device_pubkey_id)
  | Some dev ->
      if dev.revoked then
        Error (DeviceRevoked dev.key_id)
      else if assertion.monotonic_counter <= dev.last_counter then
        Error (StaleCounter (assertion.monotonic_counter, dev.last_counter))
      else
        let expected_digest = Canonical.compute_digest expected_op in
        if not (String.equal assertion.op_digest expected_digest) then
          Error (DigestMismatch (assertion.op_digest, expected_digest))
        else
          (* Verify signature against registered public key *)
          let sign_payload = Printf.sprintf "INOS_ASSERT:v1|ID:%s|DIGEST:%s|KEY:%s|CTR:%Ld"
                               assertion.assertion_id assertion.op_digest dev.key_id assertion.monotonic_counter in
          (* Device private key simulates public key verification *)
          let simulated_expected_sig = Sha256.digest_string (sign_payload ^ "|priv_" ^ dev.public_key) in
          if not (String.equal assertion.signature simulated_expected_sig) then
            Error SignatureVerificationFailed
          else begin
            dev.last_counter <- assertion.monotonic_counter;
            Ok dev
          end

(* ========================================================================= *)
(* S301: FIDO2 / CTAP 2.1 Hardware Identity & Authenticator Binding          *)
(* ========================================================================= *)

type authenticator_flags = {
  user_present : bool;             (* UP: bit 0 *)
  user_verified : bool;            (* UV: bit 2 *)
  attested_credential_data : bool; (* AT: bit 6 *)
  extension_data : bool;           (* ED: bit 7 *)
}

type fido2_algorithm =
  | Classical_P256
  | Classical_Ed25519
  | PostQuantum_ML_DSA_65

type fido2_credential = {
  credential_id : string;
  rp_id : string;
  public_key : string;
  algorithm : fido2_algorithm;
  mutable sign_count : int64;
  mutable is_revoked : bool;
  enrolled_at : int64;
}

type ctap2_auth_data = {
  rp_id_hash : string;
  flags : authenticator_flags;
  sign_count : int64;
  raw_bytes : string;
}

type fido2_assertion = {
  credential_id : string;
  rp_id : string;
  client_data_json : string;
  client_data_hash : string;
  auth_data : ctap2_auth_data;
  signature : string;
}

type fido2_challenge_record = {
  challenge_id : string;
  rp_id : string;
  nonce : string;
  issued_at : int64;
  ttl_seconds : int64;
  mutable consumed : bool;
}

type fido2_verification_error =
  | UnknownCredential of string
  | CredentialRevoked of string
  | RelyingPartyMismatch of string * string
  | ChallengeExpired of int64 * int64
  | ChallengeReplayed of string
  | ChallengeNotFound
  | UserPresenceRequired
  | UserVerificationRequired
  | SignCountRollback of int64 * int64
  | Fido2SignatureInvalid
  | NonQuantumSafeInStrictPQMode

type fido2_store = {
  credentials : (string, fido2_credential) Hashtbl.t;
  active_challenges : (string, fido2_challenge_record) Hashtbl.t;
}

let create_fido2_store () : fido2_store = {
  credentials = Hashtbl.create 32;
  active_challenges = Hashtbl.create 32;
}

let issue_challenge
    (store : fido2_store)
    ~(rp_id : string)
    ~(current_time : int64)
    ~(ttl : int64) : fido2_challenge_record =
  let nonce = Sha256.digest_string (Printf.sprintf "FIDO2_CHALLENGE|%s|%Ld|%d" rp_id current_time (Random.bits ())) in
  let challenge_id = "fido2-chk-" ^ String.sub nonce 0 12 in
  let rec_ = {
    challenge_id;
    rp_id;
    nonce;
    issued_at = current_time;
    ttl_seconds = ttl;
    consumed = false;
  } in
  Hashtbl.replace store.active_challenges challenge_id rec_;
  rec_

let enroll_fido2_credential
    (store : fido2_store)
    ~(cred_id : string)
    ~(rp_id : string)
    ~(pubkey : string)
    ~(algo : fido2_algorithm)
    ~(current_time : int64) : fido2_credential =
  let cred = {
    credential_id = cred_id;
    rp_id;
    public_key = pubkey;
    algorithm = algo;
    sign_count = 0L;
    is_revoked = false;
    enrolled_at = current_time;
  } in
  Hashtbl.replace store.credentials cred_id cred;
  cred

let revoke_fido2_credential
    (store : fido2_store)
    ~(cred_id : string) : (unit, string) result =
  match Hashtbl.find_opt store.credentials cred_id with
  | None -> Error "Credential not found"
  | Some cred ->
      cred.is_revoked <- true;
      Ok ()

let sign_fido2_assertion
    ~(cred : fido2_credential)
    ~(challenge : fido2_challenge_record)
    ~(sign_count : int64)
    ~(user_present : bool)
    ~(user_verified : bool)
    ~(device_privkey : string) : fido2_assertion =
  let rp_id_hash = Sha256.digest_string cred.rp_id in
  let flags = {
    user_present;
    user_verified;
    attested_credential_data = false;
    extension_data = false;
  } in
  let auth_data_raw = Printf.sprintf "RP:%s|FLAGS:UP=%b,UV=%b|CTR:%Ld" rp_id_hash user_present user_verified sign_count in
  let auth_data = {
    rp_id_hash;
    flags;
    sign_count;
    raw_bytes = auth_data_raw;
  } in
  let client_data_json = Printf.sprintf "{\"type\":\"webauthn.get\",\"challenge\":\"%s\",\"origin\":\"https://%s\"}"
                           challenge.nonce cred.rp_id in
  let client_data_hash = Sha256.digest_string client_data_json in
  let sign_payload = auth_data_raw ^ "|" ^ client_data_hash ^ "|priv_" ^ device_privkey in
  let signature = Sha256.digest_string sign_payload in
  {
    credential_id = cred.credential_id;
    rp_id = cred.rp_id;
    client_data_json;
    client_data_hash;
    auth_data;
    signature;
  }

let verify_fido2_assertion
    (store : fido2_store)
    (assertion : fido2_assertion)
    ~(challenge_id : string)
    ~(current_time : int64)
    ~(require_user_verification : bool)
    ~(policy : Types.security_profile)
    : (fido2_credential, fido2_verification_error) result =
  (* 1. Verify credential enrollment and revocation status *)
  match Hashtbl.find_opt store.credentials assertion.credential_id with
  | None -> Error (UnknownCredential assertion.credential_id)
  | Some cred ->
      if cred.is_revoked then
        Error (CredentialRevoked cred.credential_id)
      else if not (String.equal cred.rp_id assertion.rp_id) then
        Error (RelyingPartyMismatch (cred.rp_id, assertion.rp_id))
      else
        (* 2. Verify Post-Quantum policy conformance (Do not equate classical FIDO2 with PQ) *)
        let is_pq_safe = match cred.algorithm with
          | PostQuantum_ML_DSA_65 -> true
          | Classical_P256 | Classical_Ed25519 -> false
        in
        if policy = Types.Post_Quantum_Strict && not is_pq_safe then
          Error NonQuantumSafeInStrictPQMode
        else
          (* 3. Verify Challenge Freshness & Single-Use Consumption *)
          match Hashtbl.find_opt store.active_challenges challenge_id with
          | None -> Error ChallengeNotFound
          | Some chk ->
              if chk.consumed then
                Error (ChallengeReplayed challenge_id)
              else
                let expiry = Int64.add chk.issued_at chk.ttl_seconds in
                if current_time > expiry then
                  Error (ChallengeExpired (current_time, expiry))
                else if not (String.equal chk.rp_id assertion.rp_id) then
                  Error (RelyingPartyMismatch (chk.rp_id, assertion.rp_id))
                else
                  (* 4. Verify Authenticator Flags: User Presence (UP) and User Verification (UV) *)
                  if not assertion.auth_data.flags.user_present then
                    Error UserPresenceRequired
                  else if require_user_verification && not assertion.auth_data.flags.user_verified then
                    Error UserVerificationRequired
                  else
                    (* 5. Verify Monotonic Sign Counter (Anti-Cloning Defense) *)
                    if assertion.auth_data.sign_count <= cred.sign_count then
                      Error (SignCountRollback (assertion.auth_data.sign_count, cred.sign_count))
                    else
                      (* 6. Verify Cryptographic Signature *)
                      let sign_payload = assertion.auth_data.raw_bytes ^ "|" ^ assertion.client_data_hash ^ "|priv_" ^ cred.public_key in
                      let expected_sig = Sha256.digest_string sign_payload in
                      if not (String.equal assertion.signature expected_sig) then
                        Error Fido2SignatureInvalid
                      else begin
                        (* Mark challenge consumed to prevent replay *)
                        chk.consumed <- true;
                        cred.sign_count <- assertion.auth_data.sign_count;
                        Ok cred
                      end

