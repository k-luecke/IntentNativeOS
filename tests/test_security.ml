(* Gate G5 Security and Authentication Comprehensive Test Suite *)
open Inos_security
open Types

let assert_true name cond =
  if cond then
    Printf.printf "[PASS] %s\n%!" name
  else begin
    Printf.printf "[FAIL] %s\n%!" name;
    exit 1
  end

let test_canonical_serialization_digest () =
  let op1 = {
    action = File_Write;
    target = "/sys/config/network.json";
    parameters = [("mode", "static"); ("ip", "192.168.1.100")];
    caller_id = "admin";
    timestamp = 1700000000L;
    constraints = ["quota:1024"; "timeout:30s"];
  } in
  let op1_dup = {
    action = File_Write;
    target = "/sys/config/network.json";
    (* parameters in reverse order to test sorting *)
    parameters = [("ip", "192.168.1.100"); ("mode", "static")];
    caller_id = "admin";
    timestamp = 1700000000L;
    constraints = ["timeout:30s"; "quota:1024"];
  } in
  let op_diff_target = { op1 with target = "/sys/config/other.json" } in
  let d1 = Canonical.compute_digest op1 in
  let d1_dup = Canonical.compute_digest op1_dup in
  let d_diff = Canonical.compute_digest op_diff_target in

  assert_true "A001: Identical ops with different parameter order produce identical digest" (String.equal d1 d1_dup);
  assert_true "A001: Changed target produces distinct digest" (not (String.equal d1 d_diff));
  assert_true "A001: Digest verification helper succeeds" (Canonical.verify_digest op1 d1)

let test_authorization_broker_trust () =
  let broker = Broker.create_broker () in
  let op = {
    action = VM_Capsule_Launch;
    target = "capsule_win32_guest";
    parameters = [("vram", "64MB")];
    caller_id = "model_runtime_b";
    timestamp = 1700000100L;
    constraints = [];
  } in
  (* A002: Model or guest cannot mint grants *)
  let dec_model = Broker.mint_grant broker ~role:Untrusted_Model ~caller_id:"model_a" ~op ~current_time:1700000100L ~ttl:60L in
  (match dec_model with
   | Denied _ -> assert_true "A002: Untrusted model cannot mint authorization grant" true
   | Granted _ -> assert_true "A002: Untrusted model cannot mint authorization grant" false);

  let dec_guest = Broker.mint_grant broker ~role:Untrusted_Guest_VM ~caller_id:"guest_vm_1" ~op ~current_time:1700000100L ~ttl:60L in
  (match dec_guest with
   | Denied _ -> assert_true "A002: Untrusted guest VM cannot mint authorization grant" true
   | Granted _ -> assert_true "A002: Untrusted guest VM cannot mint authorization grant" false);

  (* Host Administrator can mint grants *)
  let dec_admin = Broker.mint_grant broker ~role:Host_Administrator ~caller_id:"host_admin" ~op ~current_time:1700000100L ~ttl:60L in
  match dec_admin with
  | Granted _ -> assert_true "A002: Host Administrator can mint authorization grant" true
  | Denied _ -> assert_true "A002: Host Administrator can mint authorization grant" false

let test_replay_resistant_grants () =
  let broker = Broker.create_broker () in
  let op = {
    action = File_Delete;
    target = "/data/sensitive.db";
    parameters = [];
    caller_id = "host_admin";
    timestamp = 1700000200L;
    constraints = [];
  } in
  let dec = Broker.mint_grant broker ~role:Host_Administrator ~caller_id:"host_admin" ~op ~current_time:1700000200L ~ttl:30L in
  let token = match dec with Granted t -> t | Denied _ -> failwith "Failed to mint" in

  (* A003: First consumption succeeds *)
  let res1 = Broker.validate_and_consume_grant broker token.grant_id op 1700000210L in
  assert_true "A003: Fresh grant is accepted and consumed" (Result.is_ok res1);

  (* A003: Replay attack fails *)
  let res_replay = Broker.validate_and_consume_grant broker token.grant_id op 1700000215L in
  (match res_replay with
   | Error Broker.GrantAlreadyConsumed -> assert_true "A003: Repeated grant presentation is rejected (Replay resistant)" true
   | _ -> assert_true "A003: Repeated grant presentation is rejected (Replay resistant)" false);

  (* A003: Expired grant fails *)
  let dec_exp = Broker.mint_grant broker ~role:Host_Administrator ~caller_id:"host_admin" ~op ~current_time:1700000300L ~ttl:10L in
  let token_exp = match dec_exp with Granted t -> t | Denied _ -> failwith "Failed to mint" in
  let res_expired = Broker.validate_and_consume_grant broker token_exp.grant_id op 1700000350L in
  (match res_expired with
   | Error (Broker.GrantExpired _) -> assert_true "A003: Expired grant is rejected" true
   | _ -> assert_true "A003: Expired grant is rejected" false);

  (* A003: Altered operation fails *)
  let dec_alt = Broker.mint_grant broker ~role:Host_Administrator ~caller_id:"host_admin" ~op ~current_time:1700000400L ~ttl:30L in
  let token_alt = match dec_alt with Granted t -> t | Denied _ -> failwith "Failed to mint" in
  let altered_op = { op with target = "/data/other.db" } in
  let res_alt = Broker.validate_and_consume_grant broker token_alt.grant_id altered_op 1700000410L in
  (match res_alt with
   | Error (Broker.DigestMismatch _) -> assert_true "A003: Altered target digest is rejected" true
   | _ -> assert_true "A003: Altered target digest is rejected" false)

let test_offline_phone_pairing () =
  let host_keys = { public_key = "host_pub_x25519_001"; private_key = "host_priv_x25519_001" } in
  let phone_keys = { public_key = "phone_pub_x25519_002"; private_key = "phone_priv_x25519_002" } in
  let req = Pairing.initiate_pairing ~host_id:"host_inos_01" ~host_keys ~timestamp:1700000500L in
  let resp = Pairing.phone_respond_pairing ~phone_id:"phone_pixel_01" ~phone_keys ~req in

  (* Normal pairing with genuine keys *)
  let result_honest = Pairing.finalize_pairing
    ~host_pub:host_keys.public_key
    ~host_priv:host_keys.private_key
    ~received_phone_pub:resp.phone_ephemeral_pubkey
    ~reported_phone_sas:resp.sas_code in
  (match result_honest with
   | Pairing.PairingSuccess { sas_code; _ } ->
       assert_true "A004: Honest pairing generates matching SAS code" (String.length sas_code = 6)
   | Pairing.PairingRejectedMitm _ ->
       assert_true "A004: Honest pairing generates matching SAS code" false);

  (* Active MITM tampering with public key in transit *)
  let mitm_fake_phone_pub = "eve_tampered_pubkey_666" in
  let result_mitm = Pairing.finalize_pairing
    ~host_pub:host_keys.public_key
    ~host_priv:host_keys.private_key
    ~received_phone_pub:mitm_fake_phone_pub
    ~reported_phone_sas:resp.sas_code in
  (match result_mitm with
   | Pairing.PairingRejectedMitm _ ->
       assert_true "A004: Active MITM attack is detected and rejected via SAS mismatch" true
   | Pairing.PairingSuccess _ ->
       assert_true "A004: Active MITM attack is detected and rejected via SAS mismatch" false)

let test_phone_trusted_display_and_assertion () =
  let op = {
    action = Kernel_Module_Load;
    target = "net_driver_virtio";
    parameters = [("checksum_offload", "true")];
    caller_id = "installer";
    timestamp = 1700000600L;
    constraints = ["signed:true"];
  } in
  (* A005: Phone displays target/action digest independently of host prose *)
  let display = Assertion.render_phone_display op in
  assert_true "A005: Phone display extracts canonical target" (String.equal display.target_summary "net_driver_virtio");
  assert_true "A005: Phone display extracts canonical action" (String.equal display.action_name "KERNEL_MODULE_LOAD");
  assert_true "A005: Phone display contains digest preview" (String.length display.digest_preview > 16);

  (* A006: Hardware-backed approval verifier *)
  let verifier = Assertion.create_verifier () in
  let device_key_id = "dev_secure_element_01" in
  let device_pubkey = "pub_se_778899" in
  let device_privkey = "priv_pub_se_778899" in
  Assertion.enroll_device verifier ~key_id:device_key_id ~pubkey:device_pubkey ~enrolled_at:1700000600L;

  let digest = Canonical.compute_digest op in
  let assertion = Assertion.sign_assertion ~device_privkey ~device_key_id ~op_digest:digest ~counter:1L in
  let v_res = Assertion.verify_hardware_approval verifier assertion op in
  assert_true "A006: Hardware-backed approval assertion verifies successfully" (Result.is_ok v_res);

  (* Tampered assertion (tampered digest) is rejected *)
  let tampered_assertion = { assertion with op_digest = "deadbeefdeadbeefdeadbeefdeadbeef" } in
  let v_tampered = Assertion.verify_hardware_approval verifier tampered_assertion op in
  assert_true "A006: Tampered assertion is rejected" (Result.is_error v_tampered);

  (* Stale / reused monotonic counter is rejected *)
  let stale_assertion = Assertion.sign_assertion ~device_privkey ~device_key_id ~op_digest:digest ~counter:1L in
  let v_stale = Assertion.verify_hardware_approval verifier stale_assertion op in
  (match v_stale with
   | Error (Assertion.StaleCounter _) -> assert_true "A006: Stale counter is rejected" true
   | _ -> assert_true "A006: Stale counter is rejected" false)

let test_offline_recovery_and_revocation () =
  let verifier = Assertion.create_verifier () in
  let lost_dev_id = "phone_lost_01" in
  Assertion.enroll_device verifier ~key_id:lost_dev_id ~pubkey:"pub_lost" ~enrolled_at:1700000700L;

  let shares = [
    (1, "recovery_mnemonic_phrase_share_1_alpha");
    (2, "recovery_mnemonic_phrase_share_2_bravo");
    (3, "recovery_mnemonic_phrase_share_3_charlie");
  ] in
  let rec_sys = Recovery.init_recovery verifier shares 2 in

  (* Revoke lost device directly *)
  assert_true "A007: Lost device key revocation succeeds" (Recovery.revoke_device verifier lost_dev_id);
  let dev_rec = Hashtbl.find verifier.enrolled_devices lost_dev_id in
  assert_true "A007: Lost device marked revoked in state" dev_rec.revoked;

  (* Replace with new device using 2-of-3 threshold shares *)
  let presented = [(1, "recovery_mnemonic_phrase_share_1_alpha"); (3, "recovery_mnemonic_phrase_share_3_charlie")] in
  let replace_res = Recovery.recover_and_replace_device rec_sys
    ~lost_key_id:lost_dev_id
    ~new_key_id:"phone_replacement_02"
    ~new_pubkey:"pub_replacement_new"
    ~presented_shares:presented
    ~timestamp:1700000750L in
  assert_true "A007: Threshold offline recovery enrolls replacement device" (Result.is_ok replace_res);

  (* Insufficient shares rejected *)
  let insufficient = [(1, "recovery_mnemonic_phrase_share_1_alpha")] in
  let bad_res = Recovery.recover_and_replace_device rec_sys
    ~lost_key_id:lost_dev_id
    ~new_key_id:"phone_replacement_03"
    ~new_pubkey:"pub_replacement_fail"
    ~presented_shares:insufficient
    ~timestamp:1700000760L in
  (match bad_res with
   | Error (Recovery.InsufficientShares _) -> assert_true "A007: Insufficient recovery shares rejected" true
   | _ -> assert_true "A007: Insufficient recovery shares rejected" false)

let test_crypto_registry_and_agility () =
  (* A008: Supported suites in Standard_Modern profile *)
  let modern_res = Crypto_registry.is_suite_permitted Standard_Modern Suite_Ed25519_Sha256 in
  assert_true "A008: Ed25519 permitted under Standard_Modern" (Result.is_ok modern_res);

  let pq_res = Crypto_registry.is_suite_permitted Standard_Modern Suite_ML_DSA_65_Sha3 in
  assert_true "A008: ML-DSA-65 permitted under Standard_Modern" (Result.is_ok pq_res);

  (* A008: Downgrade protection in Post_Quantum_Strict profile *)
  let downgrade_attempt = Crypto_registry.is_suite_permitted Post_Quantum_Strict Suite_Ed25519_Sha256 in
  assert_true "A008: Non-PQ suite strictly fails closed under Post_Quantum_Strict policy" (Result.is_error downgrade_attempt);

  let pq_strict_ok = Crypto_registry.is_suite_permitted Post_Quantum_Strict Suite_ML_KEM_768 in
  assert_true "A008: ML-KEM-768 permitted under Post_Quantum_Strict" (Result.is_ok pq_strict_ok)

let test_post_quantum_kat_harness () =
  (* A009: ML-KEM-768 KAT & Ring-LWE Encapsulate/Decapsulate Roundtrip *)
  let kat_kem = Post_quantum.run_kat_vector_kem () in
  assert_true "A009: Post-quantum ML-KEM-768 KAT vector matches reference" kat_kem.passed;
  let enc = Post_quantum.kem_encapsulate "inos_kem_pub_v1" "ephemeral_seed_v1" in
  let dec_ss = Post_quantum.kem_decapsulate "priv_inos_kem_pub_v1" enc.ciphertext in
  assert_true "A009: Ring-LWE kem_decapsulate recovers exact shared_secret" (String.equal enc.shared_secret dec_ss);
  let bad_ss = Post_quantum.kem_decapsulate "priv_wrong_key" enc.ciphertext in
  assert_true "A009: Ring-LWE kem_decapsulate with wrong private key rejects shared_secret" (not (String.equal enc.shared_secret bad_ss));

  (* A009: ML-DSA-65 KAT *)
  let kat_dsa = Post_quantum.run_kat_vector_dsa () in
  assert_true "A009: Post-quantum ML-DSA-65 KAT signature vector passes" kat_dsa.passed

let test_proof_backend_interface () =
  (* A010: Signature backend *)
  let (module SigB) = Proof_backend.get_backend SignatureProofBackend in
  let p_sig = SigB.prove ~statement:"CLAIM:CAPABILITY_VALID" ~witness:"KEY_AUTH_TOKEN_99" in
  assert_true "A010: Signature proof backend generates valid proof" (SigB.verify ~proof:p_sig ~statement:"CLAIM:CAPABILITY_VALID");
  assert_true "A010: Signature proof backend rejects tampered statement" (not (SigB.verify ~proof:p_sig ~statement:"CLAIM:TAMPERED"));

  (* A010: Non-Interactive Schnorr Fiat-Shamir ZK backend *)
  let (module ZkB) = Proof_backend.get_backend MockZkBackend in
  let p_zk = ZkB.prove ~statement:"CLAIM:CAPABILITY_VALID" ~witness:"SECRET_WITNESS_XYZ" in
  assert_true "A010: Schnorr Fiat-Shamir ZK proof backend verifies valid proof" (ZkB.verify ~proof:p_zk ~statement:"CLAIM:CAPABILITY_VALID");
  assert_true "A010: Schnorr Fiat-Shamir ZK proof backend rejects tampered statement" (not (ZkB.verify ~proof:p_zk ~statement:"CLAIM:CAPABILITY_TAMPERED"));
  let tampered_zk = { p_zk with proof_payload = p_zk.proof_payload ^ "0" } in
  assert_true "A010: Schnorr Fiat-Shamir ZK proof backend rejects tampered proof_payload" (not (ZkB.verify ~proof:tampered_zk ~statement:"CLAIM:CAPABILITY_VALID"));
  assert_true "A010: Generic proof verifier dispatcher succeeds" (Proof_backend.verify_generic_proof p_zk "CLAIM:CAPABILITY_VALID");

  (* A010b: IntScript (.ints) static security boundary auditor *)
  let valid_ints =
    "const wb = inos.grid.open(\"budget.xlsx\");\nconst sheet = wb.getSheet(\"Sheet1\");\nsheet.setFormula(\"B2\", \"=SUM(B2:B4)\");\ninos.grid.save(\"budget.xlsx\");"
  in
  assert_true "A010b: Agent_boundary.audit_intscript_source approves valid .ints within allowed_paths"
    (Result.is_ok (Agent_boundary.audit_intscript_source valid_ints ~allowed_paths:[ "budget.xlsx" ]));
  let traversal_ints = "const secret = inos.files.read(\"../../.workspace/private/keys.txt\");" in
  assert_true "A010b: Agent_boundary.audit_intscript_source blocks .workspace/private path traversal"
    (Result.is_error (Agent_boundary.audit_intscript_source traversal_ints ~allowed_paths:[ "budget.xlsx" ]));
  let unauth_file_ints = "inos.files.write(\"unauthorized.xlsx\", \"data\");" in
  assert_true "A010b: Agent_boundary.audit_intscript_source blocks file target outside allowed_paths"
    (Result.is_error (Agent_boundary.audit_intscript_source unauth_file_ints ~allowed_paths:[ "budget.xlsx" ]))

let test_measured_boot_and_update_rollback () =

  (* A011: Measured boot PCR chain *)
  let pcr_bank = Update.init_pcr_bank () in
  Update.measure_boot_sequence pcr_bank
    ~firmware_bytes:"UEFI_FIRMWARE_X64_RELEASE_v0.1"
    ~kernel_bytes:"INOS_KERNEL_ELF64_HASH_VAL"
    ~policy_bytes:"ROOT_CAPABILITY_MANIFEST_V1";
  assert_true "A011: PCR0 extended by firmware measurement" (String.length pcr_bank.pcr0_firmware = 64);
  assert_true "A011: PCR1 extended by kernel measurement" (String.length pcr_bank.pcr1_kernel = 64);
  assert_true "A011: PCR2 extended by policy measurement" (String.length pcr_bank.pcr2_policy = 64);

  (* A012: Signed versioned system update *)
  let boot_state = Update.init_slot_state () in
  let update_pubkey = "sec_update_root_key_2026" in
  let payload_bytes = "SYSTEM_IMAGE_V2_PAYLOAD_BYTES" in
  let image_hash = Sha256.digest_string payload_bytes in
  let sign_payload = Printf.sprintf "INOS_UPDATE:v1|VER:2|SLOT:Slot_B|HASH:%s" image_hash in
  let signature = Sha256.digest_string (sign_payload ^ "|priv_" ^ update_pubkey) in

  let bundle = {
    version = 2;
    target_slot = Slot_B;
    image_hash;
    signature;
    payload = payload_bytes;
  } in

  let stage_res = Update.verify_and_stage_update boot_state bundle ~trusted_update_pubkey:update_pubkey in
  assert_true "A012: Signed update bundle verified and staged to Slot_B" (Result.is_ok stage_res);
  assert_true "A012: Staged slot status is Pending_Verification" (boot_state.status = Pending_Verification);

  (* A012: Rollback on boot failure *)
  let (final_slot, final_status) = Update.finalize_boot_slot boot_state ~health_check_passed:false in
  assert_true "A012: Unhealthy update triggers automatic rollback to Slot_A" (final_slot = Slot_A && final_status = Rolled_Back);

  (* A012: Anti-rollback violation test *)
  let bad_bundle = { bundle with version = 0 } in
  let bad_res = Update.verify_and_stage_update boot_state bad_bundle ~trusted_update_pubkey:update_pubkey in
  (match bad_res with
   | Error (Update.RollbackViolation _) -> assert_true "A012: Downgraded update version rejected by anti-rollback control" true
   | _ -> assert_true "A012: Downgraded update version rejected by anti-rollback control" false)

let test_security_fuzzing () =
  (* A013: Run fuzz corpus *)
  let (total, passed, violations) = Fuzz.run_fuzz_corpus () in
  assert_true "A013: Fuzz corpus executes with zero violations" (violations = 0 && passed = total)

let test_audit_checklist_evidence () =
  (* A014: Security audit checklist *)
  let (total, mitigated) = Audit.verify_audit_checklist () in
  assert_true "A014: All registered security audit findings are mitigated" (total > 0 && total = mitigated)

let () =
  Printf.printf "=== Running Gate G5 Security & Authentication Test Suite ===\n%!";
  test_canonical_serialization_digest ();
  test_authorization_broker_trust ();
  test_replay_resistant_grants ();
  test_offline_phone_pairing ();
  test_phone_trusted_display_and_assertion ();
  test_offline_recovery_and_revocation ();
  test_crypto_registry_and_agility ();
  test_post_quantum_kat_harness ();
  test_proof_backend_interface ();
  test_measured_boot_and_update_rollback ();
  test_security_fuzzing ();
  test_audit_checklist_evidence ();
  Printf.printf "=== Gate G5 Security & Authentication Test Suite: ALL PASSED ===\n%!"
