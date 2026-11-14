(* tests/test_sentinel_cycle4_integration.ml - SDU-01 S4-INT Cross-Subsystem Integration Gate
   End-to-End workflow:
   1. Vulnerability Advisory Ingest (S401)
   2. Deterministic Component Applicability Match (S401)
   3. Update Trigger & TUF v1.0.33 Metadata Chain Verification (S402)
   4. Transactional A/B Slot Staging (S402)
   5. Step-Up Hardware Approval for Final Activation (S302)
   6. Trial Boot Health Confirmation & Slot Promotion (S402)
   7. Replay Attack Detection Fail-Closed (S402)
   8. Complete Audit Journal Hash Chaining & Checkpoint Verification (S103) *)

open Inos_security
open Types

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

let run_s4_integration_gate () =
  Printf.printf "=========================================================\n";
  Printf.printf "SDU-01: Sentinel S4-INT Cross-Subsystem Integration Gate \n";
  Printf.printf "=========================================================\n\n%!";

  let journal = Audit.create_journal () in

  (* 1. Ingest Vulnerability Advisory *)
  let nvd_adv = Vuln_intel.parse_nvd_advisory
    ~cve_id:"CVE-2026-8888"
    ~raw_json:"{\"id\":\"CVE-2026-8888\",\"metrics\":{\"cvssMetricV31\":[{\"cvssData\":{\"baseScore\":9.5}}]}}"
    ~published_at:1700000000L
    ~ingested_at:1700000100L
    ~cvss:(Some 9.5)
    ~cpe_list:["cpe:2.3:o:inos:kernel:0.3.0:*:*:*:*:*:*:*"]
    ~title:"Remote execution vulnerability in network stack"
    ~summary:"Critical vulnerability requiring security update"
  in
  let inv : Vuln_intel.system_inventory = {
    kernel_version = "inos-kernel-0.3.0";
    architecture = "x86_64";
    components = [
      { name = "inos_kernel"; version = "0.3.0"; ecosystem = "Rust"; cpe = "cpe:2.3:o:inos:kernel:0.3.0:*:*:*:*:*:*:*"; architecture = "x86_64" }
    ];
  } in
  let app_dec = Vuln_intel.evaluate_applicability inv { nvd_adv with affected_versions = ["0.3.0"] } in
  let is_definite = match app_dec with Vuln_intel.Definite_Match _ -> true | _ -> false in
  assert_sentinel "S4-INT.1: Critical vulnerability ingested and mapped as Definite_Match" is_definite;

  ignore (Audit.append_event journal
    ~severity:Audit.Severity_Warning
    ~kind:Audit.Firewall_Packet_Dropped
    ~principal_id:"vuln_intel"
    ~target_resource:"CVE-2026-8888"
    ~attributes:[("action", "advisory_ingest"); ("severity", "critical")]
    ~timestamp:1700000100L);

  (* 2. TUF v1.0.33 Update Staging *)
  let root_pub = "root_key_01" in
  let targets_pub = "targets_key_01" in
  let snapshot_pub = "snapshot_key_01" in
  let timestamp_pub = "timestamp_key_01" in

  let key_tbl = Hashtbl.create 4 in
  Hashtbl.replace key_tbl "root" root_pub;
  Hashtbl.replace key_tbl "targets" targets_pub;
  Hashtbl.replace key_tbl "snapshot" snapshot_pub;
  Hashtbl.replace key_tbl "timestamp" timestamp_pub;

  let root_meta : Update.root_metadata = {
    version = 1; expires_at = 2000000000L;
    roles = [
      (Update.RootRole, (["root"], 1));
      (Update.TargetsRole, (["targets"], 1));
      (Update.SnapshotRole, (["snapshot"], 1));
      (Update.TimestampRole, (["timestamp"], 1));
    ];
    public_keys = key_tbl;
    signatures = [{ key_id = "root"; sig_bytes = Sha256.digest_string ("TUF:ROOT|v1|exp2000000000|priv_" ^ root_pub) }];
  } in

  let update_payload = "PATCHED_KERNEL_0.3.1_SECURITY_IMAGE" in
  let payload_hash = Sha256.digest_string update_payload in

  let targets_meta : Update.targets_metadata = {
    version = 2; expires_at = 2000000000L;
    targets = [{ target_name = "kernel-0.3.1.bin"; length_bytes = String.length update_payload; sha256_hash = payload_hash; target_slot = Slot_B; version = 2 }];
    signatures = [{ key_id = "targets"; sig_bytes = Sha256.digest_string (Update.canonical_targets_payload {
      version = 2; expires_at = 2000000000L;
      targets = [{ target_name = "kernel-0.3.1.bin"; length_bytes = String.length update_payload; sha256_hash = payload_hash; target_slot = Slot_B; version = 2 }];
      signatures = [];
    } ^ "|priv_" ^ targets_pub) }];
  } in

  let tgt_hash = Sha256.digest_string (Update.canonical_targets_payload targets_meta) in
  let snapshot_meta : Update.snapshot_metadata = {
    version = 2; expires_at = 2000000000L; meta_targets_version = 2; meta_targets_hash = tgt_hash;
    signatures = [{ key_id = "snapshot"; sig_bytes = Sha256.digest_string (Update.canonical_snapshot_payload {
      version = 2; expires_at = 2000000000L; meta_targets_version = 2; meta_targets_hash = tgt_hash; signatures = [];
    } ^ "|priv_" ^ snapshot_pub) }];
  } in

  let snp_hash = Sha256.digest_string (Update.canonical_snapshot_payload snapshot_meta) in
  let timestamp_meta : Update.timestamp_metadata = {
    version = 2; expires_at = 2000000000L; meta_snapshot_version = 2; meta_snapshot_hash = snp_hash;
    signatures = [{ key_id = "timestamp"; sig_bytes = Sha256.digest_string (Update.canonical_timestamp_payload {
      version = 2; expires_at = 2000000000L; meta_snapshot_version = 2; meta_snapshot_hash = snp_hash; signatures = [];
    } ^ "|priv_" ^ timestamp_pub) }];
  } in

  let slot_st = Update.init_slot_state () in
  let repo : Update.tuf_repository_state = {
    root = root_meta;
    targets = { targets_meta with version = 1 };
    snapshot = { snapshot_meta with version = 1 };
    timestamp = { timestamp_meta with version = 1 };
    slot_state = slot_st;
  } in

  let verify_res = Update.verify_tuf_update repo
    ~new_timestamp:timestamp_meta
    ~new_snapshot:snapshot_meta
    ~new_targets:targets_meta
    ~target_file_name:"kernel-0.3.1.bin"
    ~target_payload:update_payload
    ~current_time:1700000200L
  in
  assert_sentinel "S4-INT.2: TUF multi-role verification succeeds for security patch" (Result.is_ok verify_res);

  let target_meta = Result.get_ok verify_res in
  let stage_res = Update.stage_tuf_target slot_st target_meta in
  assert_sentinel "S4-INT.3: Update staged to partition Slot_B in Pending_Verification state"
    (stage_res = Ok Slot_B && slot_st.status = Pending_Verification);

  ignore (Audit.append_event journal
    ~severity:Audit.Severity_Info
    ~kind:Audit.Auth_Grant_Minted
    ~principal_id:"tuf_update_engine"
    ~target_resource:"Slot_B"
    ~attributes:[("version", "2"); ("image_hash", payload_hash)]
    ~timestamp:1700000300L);

  (* 3. Step-Up Hardware Approval for Trial Activation *)
  let broker = Broker.create_broker () in
  let cred_id = "cred_admin_fido2" in
  let pubkey = "fido2_admin_ec_pubkey_777" in
  Broker.enroll_fido2_credential broker ~credential_id:cred_id ~pubkey;
  let activation_op : privileged_op = {
    action = System_Reboot;
    target = "/boot/slot_b";
    parameters = [("action", "trial_boot"); ("target_slot", "Slot_B")];
    caller_id = "admin";
    timestamp = 1700000400L;
    constraints = ["timeout:60s"];
  } in
  let rp_id = "inos.system.update" in
  let ch = Broker.request_step_up broker ~op:activation_op ~rp_id ~current_time:1700000400L ~ttl:60L in
  let fido_assertion = Broker.sign_fido2_assertion
    ~privkey:pubkey
    ~credential_id:cred_id
    ~rp_id
    ~challenge_id:ch.challenge_id
    ~op_digest:ch.op_digest
    ~user_present:true
    ~user_verified:true
    ~sign_count:100L in
  let auth_res = Broker.verify_and_authorize_step_up broker
    ~assertion:fido_assertion
    ~current_time:1700000405L
    ~require_user_verification:true
  in
  assert_sentinel "S4-INT.4: Hardware-backed FIDO2 assertion authorizes trial boot activation" (Result.is_ok auth_res);

  (* 4. Successful Trial Boot & Permanent Promotion *)
  let (final_slot, final_status) = Update.finalize_boot_slot slot_st ~health_check_passed:true in
  assert_sentinel "S4-INT.5: Watchdog health check passes; Slot_B promoted to Active_Healthy"
    (final_slot = Slot_B && final_status = Active_Healthy && slot_st.min_allowed_version = 2);

  ignore (Audit.append_event journal
    ~severity:Audit.Severity_Info
    ~kind:Audit.Step_Up_Completed
    ~principal_id:"admin"
    ~target_resource:"Slot_B"
    ~attributes:[("status", "Active_Healthy"); ("min_version", "2")]
    ~timestamp:1700000500L);

  (* 5. Anti-Rollback Attack Attempt Rejected *)
  let downgraded_meta = { targets_meta with version = 1 } in
  let rb_attempt = Update.verify_tuf_update repo
    ~new_timestamp:timestamp_meta
    ~new_snapshot:snapshot_meta
    ~new_targets:downgraded_meta
    ~target_file_name:"kernel-0.3.1.bin"
    ~target_payload:update_payload
    ~current_time:1700000600L
  in
  assert_sentinel "S4-INT.6: Replay attack presenting downgraded version 1 rejected fail-closed"
    (match rb_attempt with Error (Update.RollbackViolation _) -> true | _ -> false);

  (* 6. Audit Trail Hash Chain Verification *)
  let chain_valid = match Audit.verify_journal_integrity journal with Ok _ -> true | Error _ -> false in
  assert_sentinel "S4-INT.7: Complete integration audit journal preserved with intact hash chain" chain_valid;

  let checkpoint = Audit.create_checkpoint journal ~timestamp:1700000600L ~anchor_secret:"sentinel_anchor_secret" in
  let checkpoint_valid = Audit.verify_checkpoint checkpoint ~anchor_secret:"sentinel_anchor_secret" in
  assert_sentinel "S4-INT.8: Authenticated checkpoint cryptographically anchors integration state" checkpoint_valid

let () =
  run_s4_integration_gate ();
  Printf.printf "\n=========================================================\n";
  Printf.printf "Summary: %d Passed, %d Failed\n" !passed_count !failed_count;
  Printf.printf "=========================================================\n%!";
  if !failed_count > 0 then exit 1 else exit 0
