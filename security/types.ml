(* Security Subsystem Types (Gate G5: A001-A014, G5T) *)

type op_action =
  | File_Write
  | File_Delete
  | Kernel_Module_Load
  | VM_Capsule_Launch
  | System_Reboot
  | Key_Enroll
  | Device_Control

type privileged_op = {
  action : op_action;
  target : string;
  parameters : (string * string) list;
  caller_id : string;
  timestamp : int64;
  constraints : string list;
}

type canonical_digest = string

type caller_role =
  | Untrusted_Model
  | Untrusted_Guest_VM
  | System_Service
  | Host_Administrator

type grant_token = {
  grant_id : string;
  op_digest : canonical_digest;
  issued_at : int64;
  ttl_seconds : int64;
  mutable consumed : bool;
  authorized_by : string;
}

type broker_decision =
  | Granted of grant_token
  | Denied of string

type ephemeral_keypair = {
  public_key : string;
  private_key : string;
}

type pairing_request = {
  host_id : string;
  host_ephemeral_pubkey : string;
  timestamp : int64;
}

type pairing_response = {
  phone_id : string;
  phone_ephemeral_pubkey : string;
  sas_code : string;
}

type phone_display_view = {
  target_summary : string;
  action_name : string;
  digest_preview : string;
  timestamp_str : string;
}

type phone_assertion = {
  assertion_id : string;
  op_digest : canonical_digest;
  device_pubkey_id : string;
  monotonic_counter : int64;
  signature : string;
}

type enrolled_device = {
  key_id : string;
  public_key : string;
  enrolled_at : int64;
  mutable revoked : bool;
  mutable last_counter : int64;
}

type recovery_secret = {
  share_index : int;
  threshold : int;
  recovery_hash : string;
}

type crypto_suite_id =
  | Suite_Ed25519_Sha256
  | Suite_ML_DSA_65_Sha3
  | Suite_ML_KEM_768

type security_profile =
  | Legacy_Compatibility
  | Standard_Modern
  | Post_Quantum_Strict

type proof_backend_kind =
  | SignatureProofBackend
  | MockZkBackend
  | SuccinctSnarkBackend

type proof_token = {
  backend : proof_backend_kind;
  statement : string;
  proof_payload : string;
}

type partition_slot =
  | Slot_A
  | Slot_B

type update_status =
  | Active_Healthy
  | Pending_Verification
  | Rolled_Back

type update_bundle = {
  version : int;
  target_slot : partition_slot;
  image_hash : string;
  signature : string;
  payload : string;
}

type boot_slot_state = {
  mutable current_slot : partition_slot;
  mutable slot_a_version : int;
  mutable slot_b_version : int;
  mutable min_allowed_version : int;
  mutable status : update_status;
}

type fuzz_mutation =
  | BitFlip
  | Truncation
  | PathTraversalInjection
  | IntegerOverflow
  | NullByteInjection

type audit_finding = {
  id : string;
  cwe : string;
  title : string;
  severity : string;
  status : string;
  mitigation : string;
}
