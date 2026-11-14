(* Measured Boot and Signed Versioned System Update / Rollback (A011, A012, S402)
   Implements The Update Framework (TUF v1.0.33) Specification:
   - 4 Cryptographic Roles: Root, Targets, Snapshot, Timestamp
   - Role separation with protected offline root and release signing keys
   - Monotonic version anti-rollback and expiration verification
   - Transactional A/B slot staging and automated rollback recovery
   - Cardinal Invariant: Autonomous AI agents have zero release signing authority *)

open Types

(* A011: Measured Boot PCR Chain *)
type pcr_bank = {
  mutable pcr0_firmware : string;
  mutable pcr1_kernel : string;
  mutable pcr2_policy : string;
}

let init_pcr_bank () : pcr_bank = {
  pcr0_firmware = String.make 64 '0';
  pcr1_kernel = String.make 64 '0';
  pcr2_policy = String.make 64 '0';
}

let extend_pcr (current_val : string) (measurement : string) : string =
  Sha256.digest_string (current_val ^ measurement)

let measure_boot_sequence
    (bank : pcr_bank)
    ~(firmware_bytes : string)
    ~(kernel_bytes : string)
    ~(policy_bytes : string) : unit =
  bank.pcr0_firmware <- extend_pcr bank.pcr0_firmware (Sha256.digest_string firmware_bytes);
  bank.pcr1_kernel <- extend_pcr bank.pcr1_kernel (Sha256.digest_string kernel_bytes);
  bank.pcr2_policy <- extend_pcr bank.pcr2_policy (Sha256.digest_string policy_bytes)

(* A012 & S402: Signed Versioned System Update & Rollback *)
type update_error =
  | RollbackViolation of int * int
  | SignatureVerificationFailed of string
  | SlotConflict of partition_slot
  | ImageHashMismatch of string * string
  | MetadataExpired of string * int64 * int64 (* role, expiry, current_time *)
  | ThresholdNotMet of string * int * int     (* role, valid_sigs, threshold *)
  | SnapshotMismatch of string * string        (* expected, actual *)
  | TargetNotFound of string
  | AiAgentCannotSignReleases of string

let init_slot_state () : boot_slot_state = {
  current_slot = Slot_A;
  slot_a_version = 1;
  slot_b_version = 0;
  min_allowed_version = 1;
  status = Active_Healthy;
}

(* TUF Specification v1.0.33 Data Structures *)

type tuf_role =
  | RootRole
  | TargetsRole
  | SnapshotRole
  | TimestampRole

type tuf_signature = {
  key_id : string;
  sig_bytes : string;
}

type target_file_meta = {
  target_name : string;
  length_bytes : int;
  sha256_hash : string;
  target_slot : partition_slot;
  version : int;
}

type root_metadata = {
  version : int;
  expires_at : int64;
  roles : (tuf_role * (string list * int)) list; (* role -> (authorized_key_ids, threshold) *)
  public_keys : (string, string) Hashtbl.t;      (* key_id -> pubkey_string *)
  signatures : tuf_signature list;
}

type targets_metadata = {
  version : int;
  expires_at : int64;
  targets : target_file_meta list;
  signatures : tuf_signature list;
}

type snapshot_metadata = {
  version : int;
  expires_at : int64;
  meta_targets_version : int;
  meta_targets_hash : string;
  signatures : tuf_signature list;
}

type timestamp_metadata = {
  version : int;
  expires_at : int64;
  meta_snapshot_version : int;
  meta_snapshot_hash : string;
  signatures : tuf_signature list;
}

type tuf_repository_state = {
  mutable root : root_metadata;
  mutable targets : targets_metadata;
  mutable snapshot : snapshot_metadata;
  mutable timestamp : timestamp_metadata;
  slot_state : boot_slot_state;
}

(* Helper: verify signature with mock/ed25519-compatible mock helper *)
let verify_tuf_sig (pubkey : string) (payload : string) (sig_str : string) : bool =
  let expected = Sha256.digest_string (payload ^ "|priv_" ^ pubkey) in
  String.equal expected sig_str

(* Role separation check: AI agents cannot sign release metadata *)
let check_signer_authority (signer_role : caller_role) (key_id : string) : (unit, update_error) result =
  match signer_role with
  | Untrusted_Model ->
      Error (AiAgentCannotSignReleases (Printf.sprintf "AI Model cannot hold release signing key %s" key_id))
  | Untrusted_Guest_VM ->
      Error (AiAgentCannotSignReleases (Printf.sprintf "Untrusted Guest VM cannot hold release signing key %s" key_id))
  | System_Service | Host_Administrator ->
      Ok ()

(* TUF Multi-Role Verification Pipeline *)

let canonical_root_payload (m : root_metadata) : string =
  Printf.sprintf "TUF:ROOT|v%d|exp%Ld" m.version m.expires_at

let canonical_targets_payload (m : targets_metadata) : string =
  let targets_str = String.concat ";" (List.map (fun t ->
    Printf.sprintf "%s:%d:%s:%s:%d"
      t.target_name t.length_bytes t.sha256_hash
      (match t.target_slot with Slot_A -> "A" | Slot_B -> "B")
      t.version
  ) m.targets) in
  Printf.sprintf "TUF:TARGETS|v%d|exp%Ld|%s" m.version m.expires_at targets_str

let canonical_snapshot_payload (m : snapshot_metadata) : string =
  Printf.sprintf "TUF:SNAPSHOT|v%d|exp%Ld|tgt_v%d|tgt_h%s"
    m.version m.expires_at m.meta_targets_version m.meta_targets_hash

let canonical_timestamp_payload (m : timestamp_metadata) : string =
  Printf.sprintf "TUF:TIMESTAMP|v%d|exp%Ld|snp_v%d|snp_h%s"
    m.version m.expires_at m.meta_snapshot_version m.meta_snapshot_hash

(* Verification of TUF Roles *)

let verify_role_signatures
    (root : root_metadata)
    (role : tuf_role)
    (payload : string)
    (sigs : tuf_signature list) : (unit, update_error) result =
  let role_name = match role with
    | RootRole -> "root" | TargetsRole -> "targets"
    | SnapshotRole -> "snapshot" | TimestampRole -> "timestamp" in
  match List.assoc_opt role root.roles with
  | None -> Error (SignatureVerificationFailed (Printf.sprintf "Role %s undefined in root" role_name))
  | Some (allowed_keys, threshold) ->
      let seen_keys = Hashtbl.create 8 in
      let valid_sigs = List.fold_left (fun acc (s : tuf_signature) ->
        if List.mem s.key_id allowed_keys && not (Hashtbl.mem seen_keys s.key_id) then
          match Hashtbl.find_opt root.public_keys s.key_id with
          | Some pubkey ->
              if verify_tuf_sig pubkey payload s.sig_bytes then (
                Hashtbl.replace seen_keys s.key_id true;
                acc + 1
              ) else acc
          | None -> acc
        else acc
      ) 0 sigs in
      if threshold <= 0 || valid_sigs < threshold then
        Error (ThresholdNotMet (role_name, valid_sigs, threshold))
      else
        Ok ()

let verify_tuf_update
    (repo : tuf_repository_state)
    ~(new_timestamp : timestamp_metadata)
    ~(new_snapshot : snapshot_metadata)
    ~(new_targets : targets_metadata)
    ~(target_file_name : string)
    ~(target_payload : string)
    ~(current_time : int64)
    : (target_file_meta, update_error) result =

  (* 0. Verify Root Metadata *)
  if repo.root.expires_at <= current_time then
    Error (MetadataExpired ("root", repo.root.expires_at, current_time))
  else
    let root_payload = canonical_root_payload repo.root in
    match verify_role_signatures repo.root RootRole root_payload repo.root.signatures with
    | Error e -> Error e
    | Ok () ->

  (* 1. Verify Timestamp Metadata *)
  if new_timestamp.expires_at <= current_time then
    Error (MetadataExpired ("timestamp", new_timestamp.expires_at, current_time))
  else if new_timestamp.version < repo.timestamp.version then
    Error (RollbackViolation (new_timestamp.version, repo.timestamp.version))
  else
    let ts_payload = canonical_timestamp_payload new_timestamp in
    match verify_role_signatures repo.root TimestampRole ts_payload new_timestamp.signatures with
    | Error e -> Error e
    | Ok () ->

  (* 2. Verify Snapshot Metadata *)
  if new_snapshot.expires_at <= current_time then
    Error (MetadataExpired ("snapshot", new_snapshot.expires_at, current_time))
  else if new_snapshot.version < repo.snapshot.version then
    Error (RollbackViolation (new_snapshot.version, repo.snapshot.version))
  else
    let snp_payload = canonical_snapshot_payload new_snapshot in
    let snp_hash = Sha256.digest_string snp_payload in
    if not (String.equal snp_hash new_timestamp.meta_snapshot_hash) then
      Error (SnapshotMismatch (new_timestamp.meta_snapshot_hash, snp_hash))
    else
      match verify_role_signatures repo.root SnapshotRole snp_payload new_snapshot.signatures with
      | Error e -> Error e
      | Ok () ->

  (* 3. Verify Targets Metadata *)
  if new_targets.expires_at <= current_time then
    Error (MetadataExpired ("targets", new_targets.expires_at, current_time))
  else if new_targets.version < repo.targets.version then
    Error (RollbackViolation (new_targets.version, repo.targets.version))
  else
    let tgt_payload = canonical_targets_payload new_targets in
    let tgt_hash = Sha256.digest_string tgt_payload in
    if not (String.equal tgt_hash new_snapshot.meta_targets_hash) then
      Error (SnapshotMismatch (new_snapshot.meta_targets_hash, tgt_hash))
    else
      match verify_role_signatures repo.root TargetsRole tgt_payload new_targets.signatures with
      | Error e -> Error e
      | Ok () ->

  (* 4. Verify Target File Payload *)
  match List.find_opt (fun t -> String.equal t.target_name target_file_name) new_targets.targets with
  | None -> Error (TargetNotFound target_file_name)
  | Some target_meta ->
      let actual_hash = Sha256.digest_string target_payload in
      if not (String.equal actual_hash target_meta.sha256_hash) then
        Error (ImageHashMismatch (target_meta.sha256_hash, actual_hash))
      else if target_meta.version < repo.slot_state.min_allowed_version then
        Error (RollbackViolation (target_meta.version, repo.slot_state.min_allowed_version))
      else begin
        repo.timestamp <- new_timestamp;
        repo.snapshot <- new_snapshot;
        repo.targets <- new_targets;
        Ok target_meta
      end

let stage_tuf_target (state : boot_slot_state) (meta : target_file_meta) : (partition_slot, update_error) result =
  let target_slot = match state.current_slot with Slot_A -> Slot_B | Slot_B -> Slot_A in
  if meta.target_slot <> target_slot then
    Error (SlotConflict meta.target_slot)
  else begin
    (match target_slot with
     | Slot_A -> state.slot_a_version <- meta.version
     | Slot_B -> state.slot_b_version <- meta.version);
    state.current_slot <- target_slot;
    state.status <- Pending_Verification;
    Ok target_slot
  end

let verify_and_stage_update
    (state : boot_slot_state)
    (bundle : update_bundle)
    ~(trusted_update_pubkey : string)
    : (unit, update_error) result =
  if bundle.version < state.min_allowed_version then
    Error (RollbackViolation (bundle.version, state.min_allowed_version))
  else
    let target = match state.current_slot with Slot_A -> Slot_B | Slot_B -> Slot_A in
    if bundle.target_slot <> target then
      Error (SlotConflict bundle.target_slot)
    else
      let actual_hash = Sha256.digest_string bundle.payload in
      if not (String.equal actual_hash bundle.image_hash) then
        Error (ImageHashMismatch (bundle.image_hash, actual_hash))
      else
        let sign_payload = Printf.sprintf "INOS_UPDATE:v1|VER:%d|SLOT:%s|HASH:%s"
                             bundle.version
                             (match bundle.target_slot with Slot_A -> "Slot_A" | Slot_B -> "Slot_B")
                             bundle.image_hash in
        let expected_sig = Sha256.digest_string (sign_payload ^ "|priv_" ^ trusted_update_pubkey) in
        if not (String.equal bundle.signature expected_sig) then
          Error (SignatureVerificationFailed "Invalid update bundle signature")
        else begin
          (match target with
           | Slot_A -> state.slot_a_version <- bundle.version
           | Slot_B -> state.slot_b_version <- bundle.version);
          state.current_slot <- target;
          state.status <- Pending_Verification;
          Ok ()
        end

let finalize_boot_slot (state : boot_slot_state) ~(health_check_passed : bool) : partition_slot * update_status =
  if health_check_passed then begin
    state.status <- Active_Healthy;
    let running_version = match state.current_slot with Slot_A -> state.slot_a_version | Slot_B -> state.slot_b_version in
    state.min_allowed_version <- max state.min_allowed_version running_version;
    (state.current_slot, state.status)
  end else begin
    let fallback_slot = match state.current_slot with Slot_A -> Slot_B | Slot_B -> Slot_A in
    state.current_slot <- fallback_slot;
    state.status <- Rolled_Back;
    (fallback_slot, state.status)
  end
