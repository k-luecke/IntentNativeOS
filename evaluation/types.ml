(* Evaluation & Usability Subsystem Types (Gate G6: Q001-Q010) *)

type task_script = {
  task_id : string;
  title : string;
  target_persona : string;
  instructions : string list;
  expected_steps : int;
  max_allowed_clarifications : int;
}

type observational_scorecard = {
  script_id : string;
  evaluator_id : string;
  coaching_interventions : int;
  completion_time_sec : float;
  clarifications_requested : int;
  error_count : int;
  task_succeeded : bool;
}

type ir_trace_entry = {
  step_index : int;
  phase : string;
  artifact_name : string;
  canonical_hash : string;
  verified : bool;
}

type attention_metric = {
  task_name : string;
  manual_baseline_minutes : float;
  automated_intent_minutes : float;
  prompt_time_minutes : float;
  supervision_time_minutes : float;
  review_time_minutes : float;
  correction_time_minutes : float;
}

type adversarial_test_case = {
  case_id : string;
  prompt : string;
  injection_type : string;
  expected_outcome : [`Reject_Malicious | `Request_Clarification];
  actual_outcome : [`Reject_Malicious | `Request_Clarification | `Accepted_Vulnerable];
}

type memory_allocation_breakdown = {
  total_hardware_ram_mb : int;
  model_a_weights_mb : int;
  model_b_weights_mb : int;
  kv_cache_context_mb : int;
  kernel_and_paging_mb : int;
  gui_compositor_mb : int;
  guest_capsule_reserve_mb : int;
  free_headroom_mb : int;
}

type ablation_entry = {
  component_name : string;
  baseline_accuracy_pct : float;
  enhanced_accuracy_pct : float;
  latency_overhead_ms : float;
  verdict : [`Keep_Benefit_Proven | `Abandon_Ineffective];
}

type telemetry_thresholds = {
  max_boot_time_sec : float;
  target_framerate_fps : int;
  max_input_latency_ms : float;
  target_crash_free_cycles : int;
}

type run_telemetry = {
  observed_boot_time_sec : float;
  observed_framerate_fps : int;
  observed_input_latency_ms : float;
  completed_cycles_without_crash : int;
}

type app_compat_record = {
  app_name : string;
  binary_format : string;
  compatibility_level : [`Native | `Ephemeral_Capsule | `Unsupported];
  tested_version : string;
  known_limitations : string list;
  fallback_strategy : string;
}

type first_run_state =
  | Uninitialized
  | PhonePairingPrompt
  | StorageKeyProvisioned
  | ReadyNormal
  | SafeModeTriggered
  | EmergencyRecoveryActive

type distribution_manifest = {
  image_name : string;
  target_arch : string;
  sha256_hash : string;
  file_size_bytes : int;
  build_reproducibility : string;
  known_hardware_risks : string list;
}
