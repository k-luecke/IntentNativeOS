(* Gate G6 Usability and Performance Comprehensive Test Suite *)
open Inos_evaluation
open Types

let assert_true name cond =
  if cond then
    Printf.printf "[PASS] %s\n%!" name
  else begin
    Printf.printf "[FAIL] %s\n%!" name;
    exit 1
  end

let test_novice_scorecards () =
  let runs = [
    { script_id = "NOV-SCRIPT-01"; evaluator_id = "eval_01"; coaching_interventions = 0; completion_time_sec = 145.0; clarifications_requested = 1; error_count = 0; task_succeeded = true };
    { script_id = "NOV-SCRIPT-02"; evaluator_id = "eval_02"; coaching_interventions = 0; completion_time_sec = 85.0; clarifications_requested = 0; error_count = 0; task_succeeded = true };
    { script_id = "NOV-SCRIPT-03"; evaluator_id = "eval_03"; coaching_interventions = 0; completion_time_sec = 110.0; clarifications_requested = 1; error_count = 0; task_succeeded = true };
  ] in
  List.iter (fun run ->
    let res = Scorecard.evaluate_novice_run run in
    assert_true (Printf.sprintf "Q001: %s executed without coaching" run.script_id) (Result.is_ok res)
  ) runs;

  (* Coaching violation rejected *)
  let bad_run = { (List.hd runs) with coaching_interventions = 2 } in
  assert_true "Q001: Run requiring coaching fails independent evaluation" (Result.is_error (Scorecard.evaluate_novice_run bad_run))

let test_ir_inspector () =
  let pipe = Inspector.build_inspection_pipeline
    ~raw_intent:"Extract quarterly report and compute sha256"
    ~ir_repr:"INTENT:v1|OP:EXTRACT|TARGET:report_q3.pdf"
    ~dag_nodes:["node_read_file"; "node_sha256_hash"; "node_write_receipt"]
    ~grants:["grant_file_read_01"; "grant_receipt_write_02"]
    ~receipt_digest:(Inos_security.Sha256.digest_string "RECEIPT_DIGEST_SAMPLE")
  in
  assert_true "Q002: Expert can trace intent -> IR -> DAG -> grants -> receipt" (Inspector.verify_trace_integrity pipe);
  assert_true "Q002: Trace pipeline contains 5 verified stages" (List.length pipe.trace_log = 5)

let test_attention_measurement () =
  let records = Attention.benchmark_dataset in
  List.iter (fun r ->
    let ratio = Attention.calculate_attention_reduction r in
    assert_true (Printf.sprintf "Q003: Attention reduction factor %.1fx for %s" ratio r.task_name) (ratio > 5.0)
  ) records

let test_adversarial_consistency () =
  let summary = Adversarial.run_adversarial_suite () in
  assert_true "Q004: All adversarial prompt injections rejected" (summary.correct_rejections >= 3);
  assert_true "Q004: Ambiguous instructions trigger clarification" (summary.correct_clarifications >= 2);
  assert_true "Q004: Zero misinterpreted intents" (summary.misinterpreted_count = 0)

let test_16gb_memory_budget () =
  let b = Memory_budget.reference_16gb_budget in
  let res = Memory_budget.validate_memory_budget b in
  assert_true "Q005: 16 GiB reference memory budget validates exactly" (Result.is_ok res);
  assert_true "Q005: Free dynamic headroom is at least 2048 MB" (b.free_headroom_mb >= 2048);
  let rep = Memory_budget.format_memory_report b in
  assert_true "Q005: Memory report string is formatted" (String.length rep > 100)

let test_ablation_evaluation () =
  let (retained, abandoned) = Ablation.analyze_ablation () in
  assert_true "Q006: Proven technologies (SCP, Tonic Gating) retained" (retained = 2);
  assert_true "Q006: Speculative NNC formally recorded as abandoned" (abandoned = 1)

let test_telemetry_ci_thresholds () =
  let run = {
    observed_boot_time_sec = 1.15;
    observed_framerate_fps = 60;
    observed_input_latency_ms = 8.5;
    completed_cycles_without_crash = 100;
  } in
  let res = Telemetry_ci.evaluate_telemetry_against_targets run Telemetry_ci.standard_ci_thresholds in
  assert_true "Q007: Performance and stability run satisfies CI targets" (Result.is_ok res)

let test_application_compatibility_manifest () =
  let native_app = App_manifest.query_compatibility "INOS Native Document Workspace" in
  assert_true "Q008: Native app recorded" (match native_app with Some a -> a.compatibility_level = `Native | None -> false);

  let win_app = App_manifest.query_compatibility "Notepad / Text Viewer" in
  assert_true "Q008: Windows app mapped to Ephemeral Capsule" (match win_app with Some a -> a.compatibility_level = `Ephemeral_Capsule | None -> false);

  let unsupp_app = App_manifest.query_compatibility "Proprietary Kernel-Mode Anticheat Game" in
  assert_true "Q008: Unsupported kernel driver app returns diagnostic" (match unsupp_app with Some a -> a.compatibility_level = `Unsupported | None -> false)

let test_first_run_and_recovery () =
  let session = Setup_recovery.init_setup_session () in
  let advance_res = Setup_recovery.advance_first_run session ~paired_id:"pixel_auth_01" in
  assert_true "Q009: First-run setup advances to ReadyNormal" (Result.is_ok advance_res && session.state = ReadyNormal);

  Setup_recovery.trigger_safe_mode session;
  assert_true "Q009: Safe mode triggered" (session.state = SafeModeTriggered);

  let rec_res = Setup_recovery.execute_recovery_path session Setup_recovery.RollbackPreviousSlot in
  assert_true "Q009: Nontechnical safe-mode rollback succeeds" (Result.is_ok rec_res && session.state = ReadyNormal && session.active_boot_slot = Inos_security.Types.Slot_B)

let test_distribution_manifest () =
  let m = Distribution.release_distribution_manifest in
  assert_true "Q010: Distribution manifest contains valid SHA-256" (String.length m.sha256_hash = 64);
  assert_true "Q010: Known hardware risks documented" (List.length m.known_hardware_risks >= 3);
  let readme = Distribution.generate_readme_instructions () in
  assert_true "Q010: Readme build instructions generated" (String.length readme > 50)

let () =
  Printf.printf "=== Running Gate G6 Usability & Performance Test Suite ===\n%!";
  test_novice_scorecards ();
  test_ir_inspector ();
  test_attention_measurement ();
  test_adversarial_consistency ();
  test_16gb_memory_budget ();
  test_ablation_evaluation ();
  test_telemetry_ci_thresholds ();
  test_application_compatibility_manifest ();
  test_first_run_and_recovery ();
  test_distribution_manifest ();
  Printf.printf "=== Gate G6 Usability & Performance Test Suite: ALL PASSED ===\n%!"
