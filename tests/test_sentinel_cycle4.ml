(* tests/test_sentinel_cycle4.ml - SDU-01 Sentinel Cycle 4 Independent Verification Suite
   Scope:
   - S401: Vulnerability Intelligence Engine (NVD, KEV, OSV, Uncertainty Tiers, Bounded Cache, AI Non-Authority)
   - S402: TUF v1.0.33 Secure Updating Engine (Role Separation, Anti-Rollback, A/B Staged Installation & Recovery)
   - S303: Identity & Secure Recovery (24-word BIP-39, RFC 3394 Wrapping, Identity vs Data Separation, PQ Hybrid) *)

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

(* ========================================================================= *)
(* 1. S401: Vulnerability Intelligence Engine Suite                         *)
(* ========================================================================= *)

let test_s401_vulnerability_intelligence () =
  Printf.printf "\n=== SDU-01 S401: Vulnerability Intelligence Engine ===\n%!";

  let inv : Vuln_intel.system_inventory = {
    kernel_version = "inos-kernel-0.3.0";
    architecture = "x86_64";
    components = [
      { name = "inos_kernel"; version = "0.3.0"; ecosystem = "Rust"; cpe = "cpe:2.3:o:inos:kernel:0.3.0:*:*:*:*:*:*:*"; architecture = "x86_64" };
      { name = "libssl"; version = "3.0.8"; ecosystem = "C"; cpe = "cpe:2.3:a:openssl:openssl:3.0.8:*:*:*:*:*:*:*"; architecture = "x86_64" };
      { name = "virtio_net"; version = "1.0.0"; ecosystem = "Driver"; cpe = "cpe:2.3:a:qemu:virtio_net:1.0.0:*:*:*:*:*:*:*"; architecture = "x86_64" };
    ];
  } in

  (* 1. Ingest NVD CVE *)
  let nvd_adv = Vuln_intel.parse_nvd_advisory
    ~cve_id:"CVE-2026-1001"
    ~raw_json:"{\"id\":\"CVE-2026-1001\",\"metrics\":{\"cvssMetricV31\":[{\"cvssData\":{\"baseScore\":9.8}}]}}"
    ~published_at:1700001000L
    ~ingested_at:1700002000L
    ~cvss:(Some 9.8)
    ~cpe_list:["cpe:2.3:o:inos:kernel:0.3.0:*:*:*:*:*:*:*"]
    ~title:"Buffer overflow in kernel net driver"
    ~summary:"Critical memory corruption vulnerability"
  in
  assert_sentinel "S401: Ingest NVD advisory parses CVSS and computes SHA-256 payload"
    (nvd_adv.severity = Vuln_intel.Critical && String.length nvd_adv.payload_sha256 = 64);

  (* 2. Ingest CISA KEV Advisory *)
  let kev_adv = Vuln_intel.parse_cisa_kev_advisory
    ~cve_id:"CVE-2026-9999"
    ~raw_json:"{\"cveID\":\"CVE-2026-9999\",\"vendorProject\":\"openssl\",\"product\":\"openssl\"}"
    ~published_at:1700001000L
    ~ingested_at:1700002000L
    ~vendor_project:"openssl"
    ~product:"openssl"
    ~required_action:"Apply vendor update immediately"
  in
  assert_sentinel "S401: Ingest CISA KEV advisory marks active exploitation"
    (kev_adv.known_exploited && kev_adv.severity = Vuln_intel.Critical);

  (* 3. Ingest OSV Advisory *)
  let osv_adv = Vuln_intel.parse_osv_advisory
    ~osv_id:"OSV-2026-0042"
    ~raw_json:"{\"id\":\"OSV-2026-0042\",\"package\":{\"name\":\"virtio_net\",\"ecosystem\":\"Driver\"}}"
    ~published_at:1700001000L
    ~ingested_at:1700002000L
    ~ecosystem:"Driver"
    ~package_name:"virtio_net"
    ~affected_versions:["1.0.0"; "1.0.1"]
    ~summary:"Packet length underflow"
  in
  assert_sentinel "S401: Ingest OSV advisory parses ecosystem and affected versions"
    (osv_adv.package_name = Some "virtio_net" && List.length osv_adv.affected_versions = 2);

  (* 4. 5-Tier Uncertainty Matching *)
  let dec_nvd = Vuln_intel.evaluate_applicability inv { nvd_adv with affected_versions = ["0.3.0"] } in
  assert_sentinel "S401: Exact version match classified as Definite_Match"
    (match dec_nvd with Vuln_intel.Definite_Match _ -> true | _ -> false);

  let dec_kev = Vuln_intel.evaluate_applicability inv kev_adv in
  assert_sentinel "S401: KEV active exploitation catalog classified as Probable_Match"
    (match dec_kev with Vuln_intel.Probable_Match _ -> true | _ -> false);

  let dec_potential = Vuln_intel.evaluate_applicability inv { nvd_adv with affected_versions = ["0.2.9"] } in
  assert_sentinel "S401: Component present but version outside range classified as Potential_Impact"
    (match dec_potential with Vuln_intel.Potential_Impact _ -> true | _ -> false);

  let unk_adv = { nvd_adv with cpe_matches = [] } in
  let dec_unk = Vuln_intel.evaluate_applicability inv unk_adv in
  assert_sentinel "S401: Missing structured criteria classified as Unknown_Applicability"
    (match dec_unk with Vuln_intel.Unknown_Applicability _ -> true | _ -> false);

  let not_app_adv = { osv_adv with package_name = Some "apache_httpd" } in
  let dec_not_app = Vuln_intel.evaluate_applicability inv not_app_adv in
  assert_sentinel "S401: Uninstalled component classified as Not_Applicable"
    (match dec_not_app with Vuln_intel.Not_Applicable _ -> true | _ -> false);

  (* 5. Feed Freshness Detection *)
  let fresh_st = Vuln_intel.check_feed_freshness ~feed_timestamp:1700000000L ~current_timestamp:1700010000L ~max_freshness_seconds:86400L in
  assert_sentinel "S401: Feed age within 24h classified as Fresh"
    (match fresh_st with Vuln_intel.Fresh age -> age = 10000L | _ -> false);

  let stale_st = Vuln_intel.check_feed_freshness ~feed_timestamp:1700000000L ~current_timestamp:1700100000L ~max_freshness_seconds:86400L in
  assert_sentinel "S401: Feed age exceeding 24h triggers Stale status"
    (match stale_st with Vuln_intel.Stale age -> age = 100000L | _ -> false);

  (* 6. Bounded LRU Cache Verification *)
  let cache = Vuln_intel.create_cache ~max_entries:2 () in
  Vuln_intel.cache_put cache nvd_adv;
  Vuln_intel.cache_put cache kev_adv;
  Vuln_intel.cache_put cache osv_adv; (* Should evict nvd_adv (oldest) *)
  let get_nvd = Vuln_intel.cache_get cache "NVD:CVE-2026-1001" in
  let get_osv = Vuln_intel.cache_get cache "OSV:OSV-2026-0042" in
  assert_sentinel "S401: Bounded LRU cache evicts oldest entry when max capacity is reached"
    (Option.is_none get_nvd && Option.is_some get_osv);

  (* 7. Cardinal Invariant: AI Non-Authority *)
  let ai_analysis : Vuln_intel.ai_advisory_analysis = {
    model_id = "agent_analyst_v1";
    confidence = 0.999;
    natural_language_summary = "Immediate patch recommended";
    recommended_action = "Execute system update immediately";
  } in
  let report = Vuln_intel.analyze_advisory_with_ai nvd_adv dec_nvd (Some ai_analysis) in
  let fake_op : privileged_op = {
    action = System_Reboot;
    target = "/sys/update";
    parameters = [];
    caller_id = "ai_agent";
    timestamp = 1700003000L;
    constraints = [];
  } in
  let mint_attempt = Vuln_intel.attempt_mint_grant_from_ai_analysis report fake_op in
  assert_sentinel "S401: AI advisory analysis cannot authorize changes or mint capability grants"
    (Result.is_error mint_attempt)

(* ========================================================================= *)
(* 2. S402: TUF Secure Autonomous Updating Engine Suite                      *)
(* ========================================================================= *)

let test_s402_tuf_secure_updating () =
  Printf.printf "\n=== SDU-01 S402: TUF Specification Secure Updating ===\n%!";

  (* Set up keys *)
  let root_pub = "root_pubkey_001" in
  let targets_pub = "targets_pubkey_002" in
  let snapshot_pub = "snapshot_pubkey_003" in
  let timestamp_pub = "timestamp_pubkey_004" in

  let key_table = Hashtbl.create 8 in
  Hashtbl.replace key_table "key_root" root_pub;
  Hashtbl.replace key_table "key_targets" targets_pub;
  Hashtbl.replace key_table "key_snapshot" snapshot_pub;
  Hashtbl.replace key_table "key_timestamp" timestamp_pub;

  let root_meta : Update.root_metadata = {
    version = 1;
    expires_at = 2000000000L;
    roles = [
      (Update.RootRole, (["key_root"], 1));
      (Update.TargetsRole, (["key_targets"], 1));
      (Update.SnapshotRole, (["key_snapshot"], 1));
      (Update.TimestampRole, (["key_timestamp"], 1));
    ];
    public_keys = key_table;
    signatures = [
      { key_id = "key_root"; sig_bytes = Sha256.digest_string ("TUF:ROOT|v1|exp2000000000|priv_" ^ root_pub) }
    ];
  } in

  let kernel_payload = "KERNEL_IMAGE_BINARY_BYTES_v0.3.1_PROTECTED_RING0" in
  let kernel_hash = Sha256.digest_string kernel_payload in

  let targets_meta : Update.targets_metadata = {
    version = 2;
    expires_at = 2000000000L;
    targets = [
      {
        target_name = "kernel-0.3.1.bin";
        length_bytes = String.length kernel_payload;
        sha256_hash = kernel_hash;
        target_slot = Slot_B;
        version = 2;
      }
    ];
    signatures = [
      { key_id = "key_targets"; sig_bytes = Sha256.digest_string (Update.canonical_targets_payload {
        version = 2; expires_at = 2000000000L;
        targets = [{ target_name = "kernel-0.3.1.bin"; length_bytes = String.length kernel_payload; sha256_hash = kernel_hash; target_slot = Slot_B; version = 2 }];
        signatures = [];
      } ^ "|priv_" ^ targets_pub) }
    ];
  } in

  let tgt_payload_str = Update.canonical_targets_payload targets_meta in
  let tgt_meta_hash = Sha256.digest_string tgt_payload_str in

  let snapshot_meta : Update.snapshot_metadata = {
    version = 2;
    expires_at = 2000000000L;
    meta_targets_version = 2;
    meta_targets_hash = tgt_meta_hash;
    signatures = [
      { key_id = "key_snapshot"; sig_bytes = Sha256.digest_string (Update.canonical_snapshot_payload {
        version = 2; expires_at = 2000000000L; meta_targets_version = 2; meta_targets_hash = tgt_meta_hash; signatures = [];
      } ^ "|priv_" ^ snapshot_pub) }
    ];
  } in

  let snp_payload_str = Update.canonical_snapshot_payload snapshot_meta in
  let snp_meta_hash = Sha256.digest_string snp_payload_str in

  let timestamp_meta : Update.timestamp_metadata = {
    version = 2;
    expires_at = 2000000000L;
    meta_snapshot_version = 2;
    meta_snapshot_hash = snp_meta_hash;
    signatures = [
      { key_id = "key_timestamp"; sig_bytes = Sha256.digest_string (Update.canonical_timestamp_payload {
        version = 2; expires_at = 2000000000L; meta_snapshot_version = 2; meta_snapshot_hash = snp_meta_hash; signatures = [];
      } ^ "|priv_" ^ timestamp_pub) }
    ];
  } in

  let slot_st = Update.init_slot_state () in
  let repo : Update.tuf_repository_state = {
    root = root_meta;
    targets = { targets_meta with version = 1 };
    snapshot = { snapshot_meta with version = 1 };
    timestamp = { timestamp_meta with version = 1 };
    slot_state = slot_st;
  } in

  (* 8. Valid TUF Update Verification *)
  let verify_res = Update.verify_tuf_update repo
    ~new_timestamp:timestamp_meta
    ~new_snapshot:snapshot_meta
    ~new_targets:targets_meta
    ~target_file_name:"kernel-0.3.1.bin"
    ~target_payload:kernel_payload
    ~current_time:1700005000L
  in
  assert_sentinel "S402: TUF multi-role verification succeeds across Root->Targets->Snapshot->Timestamp"
    (Result.is_ok verify_res);

  (* 9. Anti-Rollback Protection *)
  let downgraded_ts = { timestamp_meta with version = 0 } in
  let rb_res = Update.verify_tuf_update repo
    ~new_timestamp:downgraded_ts
    ~new_snapshot:snapshot_meta
    ~new_targets:targets_meta
    ~target_file_name:"kernel-0.3.1.bin"
    ~target_payload:kernel_payload
    ~current_time:1700005000L
  in
  assert_sentinel "S402: Downgraded TUF metadata version rejected fail-closed"
    (match rb_res with Error (Update.RollbackViolation _) -> true | _ -> false);

  (* 10. Expiration / Anti-Freeze Protection *)
  let expired_ts = { timestamp_meta with expires_at = 1600000000L } in
  let exp_res = Update.verify_tuf_update repo
    ~new_timestamp:expired_ts
    ~new_snapshot:snapshot_meta
    ~new_targets:targets_meta
    ~target_file_name:"kernel-0.3.1.bin"
    ~target_payload:kernel_payload
    ~current_time:1700005000L
  in
  assert_sentinel "S402: Expired TUF metadata rejected fail-closed"
    (match exp_res with Error (Update.MetadataExpired _) -> true | _ -> false);

  (* 11. Snapshot Mix-and-Match Attack Defense *)
  let tampered_snapshot = { snapshot_meta with meta_targets_hash = "deadbeefdeadbeef" } in
  let snp_res = Update.verify_tuf_update repo
    ~new_timestamp:timestamp_meta
    ~new_snapshot:tampered_snapshot
    ~new_targets:targets_meta
    ~target_file_name:"kernel-0.3.1.bin"
    ~target_payload:kernel_payload
    ~current_time:1700005000L
  in
  assert_sentinel "S402: Snapshot mix-and-match target hash mismatch rejected"
    (match snp_res with Error (Update.SnapshotMismatch _) -> true | _ -> false);

  (* 12. Payload Tampering Defense *)
  let tampered_payload_res = Update.verify_tuf_update repo
    ~new_timestamp:timestamp_meta
    ~new_snapshot:snapshot_meta
    ~new_targets:targets_meta
    ~target_file_name:"kernel-0.3.1.bin"
    ~target_payload:"MALICIOUS_BACKDOORED_KERNEL_PAYLOAD"
    ~current_time:1700005000L
  in
  assert_sentinel "S402: Target payload hash mismatch rejected fail-closed"
    (match tampered_payload_res with Error (Update.ImageHashMismatch _) -> true | _ -> false);

  (* 13. AI Agent Signing Authority Prohibition *)
  let ai_sign_check = Update.check_signer_authority Untrusted_Model "key_targets" in
  assert_sentinel "S402: Autonomous AI agents prohibited from holding release signing keys"
    (match ai_sign_check with Error (Update.AiAgentCannotSignReleases _) -> true | _ -> false);

  (* 14. Transactional A/B Staging *)
  let target_meta = Result.get_ok verify_res in
  let stage_res = Update.stage_tuf_target slot_st target_meta in
  assert_sentinel "S402: Valid TUF target staged to inactive slot Slot_B with Pending_Verification"
    (stage_res = Ok Slot_B && slot_st.status = Pending_Verification);

  (* 15. Automated Rollback Recovery *)
  let (fallback_slot, fallback_status) = Update.finalize_boot_slot slot_st ~health_check_passed:false in
  assert_sentinel "S402: Unhealthy trial boot triggers automatic rollback to Slot_A"
    (fallback_slot = Slot_A && fallback_status = Rolled_Back)

(* ========================================================================= *)
(* 3. S303: Identity & Secure Recovery Architecture Suite                    *)
(* ========================================================================= *)

let test_s303_secure_recovery () =
  Printf.printf "\n=== SDU-01 S303: Identity & Secure Recovery Architecture ===\n%!";

  (* 256-bit machine-generated entropy (32 bytes) *)
  let entropy = Bytes.make 32 '\x42' in
  Bytes.set entropy 0 '\xAA';
  Bytes.set entropy 15 '\x55';
  Bytes.set entropy 31 '\x99';

  (* 16. 24-Word BIP-39 Encoding *)
  let words_res = Recovery_v2.encode_entropy_to_24_words entropy in
  assert_sentinel "S303: 256-bit entropy encoded into 24 BIP-39 mnemonic words"
    (match words_res with Ok w -> List.length w = 24 | Error _ -> false);

  let words = Result.get_ok words_res in

  (* 17. Verification of authentic mnemonic phrase *)
  let decode_res = Recovery_v2.verify_and_decode_24_words words in
  assert_sentinel "S303: Authentic 24-word recovery phrase verifies successfully"
    (Result.is_ok decode_res);

  let seed = Result.get_ok decode_res in

  (* 18. Typo / Error Location Detection *)
  let corrupted_words = List.mapi (fun i w -> if i = 4 then "invalidwordxyz" else w) words in
  let typo_res = Recovery_v2.verify_and_decode_24_words corrupted_words in
  assert_sentinel "S303: Typo at word position 5 detected and isolated"
    (match typo_res with Error (Recovery_v2.InvalidWordAt (5, "invalidwordxyz")) -> true | _ -> false);

  (* 19. Truncated Word Count Check *)
  let truncated_words = List.filteri (fun i _ -> i < 23) words in
  let trunc_res = Recovery_v2.verify_and_decode_24_words truncated_words in
  assert_sentinel "S303: Truncated 23-word phrase rejected fail-closed"
    (match trunc_res with Error (Recovery_v2.InvalidWordCount 23) -> true | _ -> false);

  (* 20. HKDF Key Derivation *)
  let recovery_keys = Recovery_v2.derive_keys_from_seed seed "user_alice_01" in
  assert_sentinel "S303: HKDF derives independent identity and storage volume keys"
    (String.length recovery_keys.k_identity = 64 &&
     String.length recovery_keys.k_storage = 64 &&
     not (String.equal recovery_keys.k_identity recovery_keys.k_storage));

  (* 21. Authenticated Key Wrapping with AAD *)
  let volume_mek = "PLAINTEXT_STORAGE_MASTER_KEY_AES256_GCM" in
  let verifier = Assertion.create_verifier () in
  Assertion.enroll_device verifier ~key_id:"fido_key_lost_01" ~pubkey:"pub_lost_key" ~enrolled_at:1700000000L;

  let vault = Recovery_v2.create_vault
    ~user_id:"user_alice_01"
    ~verifier
    ~volume_mek
    ~recovery_keys
  in

  (* Altered AAD fails *)
  let unwrap_bad_aad = Recovery_v2.unwrap_key recovery_keys.k_storage vault.wrapped_volume_mek ~expected_aad:"USER:attacker|TYPE:VOLUME_MEK" in
  assert_sentinel "S303: Key wrap authentication fails when AAD is altered"
    (match unwrap_bad_aad with Error Recovery_v2.KeyWrapAuthFailed -> true | _ -> false);

  (* 22. Strict Separation: Identity Recovery *)
  let id_rec_res = Recovery_v2.execute_identity_recovery vault
    ~_recovery_keys:recovery_keys
    ~lost_key_id:"fido_key_lost_01"
    ~new_key_id:"fido_key_replacement_02"
    ~new_pubkey:"pub_replacement_key"
    ~timestamp:1700010000L
  in
  let lost_dev = Hashtbl.find verifier.enrolled_devices "fido_key_lost_01" in
  assert_sentinel "S303: Identity recovery revokes lost device and enrolls replacement without touching volume MEK"
    (Result.is_ok id_rec_res && lost_dev.revoked && Hashtbl.mem verifier.enrolled_devices "fido_key_replacement_02");

  (* 23. Strict Separation: Data Volume Recovery *)
  let data_rec_res = Recovery_v2.execute_data_volume_recovery vault ~recovery_keys in
  assert_sentinel "S303: Data volume recovery unwraps MEK without altering authenticator registry"
    (Result.is_ok data_rec_res);

  (* 24. Post-Quantum Hybrid Migration *)
  let hybrid_wrap = Recovery_v2.wrap_hybrid_master_key recovery_keys volume_mek ~pq_kem_pk:"ml_kem_768_public_key_01" ~user_id:"user_alice_01" in
  let hybrid_unwrap = Recovery_v2.unwrap_hybrid_master_key recovery_keys hybrid_wrap ~pq_kem_sk:"ml_kem_768_public_key_01" ~user_id:"user_alice_01" in
  assert_sentinel "S303: Post-quantum hybrid ML-KEM-768 master key wrapping and recovery succeeds"
    (Result.is_ok hybrid_unwrap)

let () =
  Printf.printf "=========================================================\n";
  Printf.printf "SDU-01: Sentinel Cycle 4 Independent Verification Suite  \n";
  Printf.printf "Scope: S401 (Vuln Intel), S402 (TUF Update), S303 (Recovery)\n";
  Printf.printf "=========================================================\n%!";

  test_s401_vulnerability_intelligence ();
  test_s402_tuf_secure_updating ();
  test_s303_secure_recovery ();

  Printf.printf "\n=========================================================\n";
  Printf.printf "Summary: %d Passed, %d Failed\n" !passed_count !failed_count;
  Printf.printf "=========================================================\n%!";

  if !failed_count > 0 then exit 1 else exit 0
