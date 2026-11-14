(* Research stress tests for SCP and Tonic Gating *)
open Inos_scp
open Inos_tonic

let () =
  Printf.printf "=== Running L303/L304/L307 Research Stress Tests ===\n";

  (* Test 1: SCP disabled by default *)
  let cfg_default = Scp_projection.default_config in
  assert (not cfg_default.enabled);
  let raw_logits = [| 2.5; 1.0; -0.5; 3.2 |] in
  let unconstrained = Scp_projection.project_logits ~cfg:cfg_default ~allowed:[0; 2] raw_logits in
  assert (unconstrained = raw_logits);
  Printf.printf "[PASS] SCP off-by-default pass-through verified\n";

  (* Test 2: SCP enabled simplex projection *)
  let cfg_enabled = { Scp_projection.enabled = true; epsilon_threshold = 0.001 } in
  let constrained = Scp_projection.project_logits ~cfg:cfg_enabled ~allowed:[1; 3] raw_logits in
  assert (constrained.(0) <= -1.0e8);
  assert (constrained.(2) <= -1.0e8);
  assert (abs_float (constrained.(1) -. 1.0) < 1e-5);
  assert (abs_float (constrained.(3) -. 3.2) < 1e-5);

  let probs = Scp_projection.softmax constrained in
  assert (probs.(0) = 0.0);
  assert (probs.(2) = 0.0);
  assert (probs.(1) > 0.0);
  assert (probs.(3) > 0.0);
  let total_prob = probs.(1) +. probs.(3) in
  assert (abs_float (total_prob -. 1.0) < 1e-5);
  Printf.printf "[PASS] SCP simplex probability mass conservation verified\n";

  (* Test 3: SCP Latent Subspace Projection *)
  let latent = [| 1.0; 2.0; 3.0 |] in
  let b1 = [| 1.0; 0.0; 0.0 |] in
  let b2 = [| 0.0; 1.0; 0.0 |] in
  let (proj, div) = Scp_projection.project_latent_subspace latent [| b1; b2 |] in
  assert (abs_float (proj.(0) -. 1.0) < 1e-5);
  assert (abs_float (proj.(1) -. 2.0) < 1e-5);
  assert (abs_float proj.(2) < 1e-5);
  assert (div > 0.0);
  Printf.printf "[PASS] SCP latent orthogonal projection verified (divergence = %.4f)\n" div;

  (* Test 4: Tonic Gating disabled default *)
  let t_cfg_default = Tonic_gate.default_config in
  assert (not t_cfg_default.enabled);
  let st0 = Tonic_gate.init_state () in
  let (_, r_low) = Tonic_gate.step_gate ~cfg:t_cfg_default st0 ~ambiguity:0.1 ~risk:0.1 ~resource:0.1 in
  assert (r_low = Tonic_gate.RouteDeterministic);
  let (_, r_high) = Tonic_gate.step_gate ~cfg:t_cfg_default st0 ~ambiguity:0.9 ~risk:0.9 ~resource:0.1 in
  assert (r_high = Tonic_gate.RouteDualModelReview);
  Printf.printf "[PASS] Tonic gate off-by-default behavior verified\n";

  (* Test 5: Tonic Gating enabled dynamics & safety invariant *)
  let t_cfg_enabled = { Tonic_gate.default_config with enabled = true } in
  (* Test monotonic risk elevation: risk = 0.85 must force tone = 1.0 and RouteDualModelReview *)
  let (st_high, r_forced) = Tonic_gate.step_gate ~cfg:t_cfg_enabled st0 ~ambiguity:0.0 ~risk:0.85 ~resource:0.0 in
  assert (st_high.tone = 1.0);
  assert (r_forced = Tonic_gate.RouteDualModelReview);
  Printf.printf "[PASS] Tonic gate high-risk monotonic elevation invariant verified (tone = 1.0)\n";

  (* Test 6: Tonic Gating uncalibrated fail-closed invariant *)
  let st_uncal = { st0 with is_calibrated = false } in
  let (st_safe, r_safe) = Tonic_gate.step_gate ~cfg:t_cfg_enabled st_uncal ~ambiguity:0.0 ~risk:0.0 ~resource:0.0 in
  assert (st_safe.tone = 1.0);
  assert (r_safe = Tonic_gate.RouteDualModelReview);
  Printf.printf "[PASS] Tonic gate uncalibrated fail-closed safety invariant verified\n";

  (* Test 7: Multi-step low tone trajectory *)
  let (st1, r1) = Tonic_gate.step_gate ~cfg:t_cfg_enabled st0 ~ambiguity:0.1 ~risk:0.1 ~resource:0.1 in
  assert (r1 = Tonic_gate.RouteDeterministic);
  let (st2, r2) = Tonic_gate.step_gate ~cfg:t_cfg_enabled st1 ~ambiguity:0.1 ~risk:0.1 ~resource:0.1 in
  assert (r2 = Tonic_gate.RouteDeterministic);
  assert (st2.tone < 0.25);
  Printf.printf "[PASS] Tonic gate low-tone steady-state verified (tone = %.4f)\n" st2.tone;

  (* Test 8: SCP Capsule Grammar DFA Constrained Sequence Decoding *)
  let vocab = [
    (0, "{");
    (1, "\"op\"");
    (2, ":");
    (3, "\"create\"");
    (4, "}");
    (5, "MALFORMED_INJECTION_TOKEN");
  ] in
  (* At every step, index 5 (MALFORMED_INJECTION_TOKEN) has the highest raw logit (10.0),
     which would corrupt unconstrained greedy decoding. SCP DFA masks it out at every state! *)
  let step_logits = [
    [| 4.0; 1.0; 0.0; 0.0; 0.0; 10.0 |]; (* State_Start -> must pick 0 ("{") *)
    [| 0.0; 4.5; 0.0; 0.0; 0.0; 10.0 |]; (* State_ExpectKey -> must pick 1 ("\"op\"") *)
    [| 0.0; 0.0; 5.0; 0.0; 0.0; 10.0 |]; (* State_ExpectColon -> must pick 2 (":") *)
    [| 0.0; 0.0; 0.0; 6.0; 0.0; 10.0 |]; (* State_ExpectValue -> must pick 3 ("\"create\"") *)
    [| 0.0; 0.0; 0.0; 0.0; 5.5; 10.0 |]; (* State_ExpectCommaOrEnd -> must pick 4 ("}") *)
  ] in
  (match Scp_projection.decode_capsule_sequence ~cfg:cfg_enabled ~vocab step_logits with
   | Error msg -> failwith ("SCP DFA decoding failed: " ^ msg)
   | Ok (decoded, min_prob, final_state) ->
       assert (decoded = "{\"op\":\"create\"}");
       assert (final_state = Scp_projection.State_Terminal);
       assert (min_prob > 0.99);
       Printf.printf "[PASS] SCP Capsule Grammar DFA decoded valid JSON %s (rejected higher-logit malformed tokens)\n" decoded);

  (* Test 9: Stateful Tonic Gate Routing via Inos_arbitration.Router *)
  let (st_r1, dec_low) =
    Inos_arbitration.Router.route_intent_tonic ~tonic_cfg:t_cfg_enabled st0 ~prompt:"view status.log" ()
  in
  assert (dec_low = Inos_arbitration.Router.RouteDeterministicOnly);
  let (_st_r2, dec_high) =
    Inos_arbitration.Router.route_intent_tonic ~tonic_cfg:t_cfg_enabled st_r1 ~prompt:"delete everything and wipe disk" ()
  in
  (match dec_high with
   | Inos_arbitration.Router.RouteDualModelReview _ ->
       Printf.printf "[PASS] Stateful Tonic Gate router escalated destructive prompt to RouteDualModelReview\n"
   | _ -> failwith "Expected RouteDualModelReview for destructive prompt");

  (* Test 10: State-Verified NNC vs Unkeyed Speculative NNC (Stale Drift Elimination) *)
  let unkeyed_cache =
    Inos_nnc.Nnc_cache.create
      ~cfg:{ Inos_nnc.Nnc_cache.mode = Inos_nnc.Nnc_cache.UnkeyedSpeculative; max_entries = 64 } ()
  in
  let verified_cache =
    Inos_nnc.Nnc_cache.create
      ~cfg:{ Inos_nnc.Nnc_cache.mode = Inos_nnc.Nnc_cache.StateVerified; max_entries = 64 } ()
  in
  let sample_logits = [| 3.5; 0.1; -0.5; 0.2; 0.0 |] in
  ignore (Inos_nnc.Nnc_cache.insert unkeyed_cache
    ~prompt:"create report.txt" ~workspace_state_hash:"state_hash_v1" ~capability_epoch:1
    ~logits:sample_logits ~classification_id:0 ~target_resource:(Some "report.txt") ~verified:true);
  ignore (Inos_nnc.Nnc_cache.insert verified_cache
    ~prompt:"create report.txt" ~workspace_state_hash:"state_hash_v1" ~capability_epoch:1
    ~logits:sample_logits ~classification_id:0 ~target_resource:(Some "report.txt") ~verified:true);

  (* Same state -> Hit in StateVerified cache *)
  (match Inos_nnc.Nnc_cache.lookup verified_cache ~prompt:"CREATE report.txt" ~workspace_state_hash:"state_hash_v1" ~capability_epoch:1 with
   | Inos_nnc.Nnc_cache.CacheHit e -> assert (e.classification_id = 0)
   | _ -> failwith "Expected CacheHit on matching state_hash_v1");

  (* Mutated workspace state ("state_hash_v2"):
     UnkeyedSpeculative falsely hits stale state, whereas StateVerified evicts and returns CacheMiss_StaleState! *)
  (match Inos_nnc.Nnc_cache.lookup unkeyed_cache ~prompt:"create report.txt" ~workspace_state_hash:"state_hash_v2" ~capability_epoch:1 with
   | Inos_nnc.Nnc_cache.CacheHit _ -> () (* Demonstrates legacy stale drift *)
   | _ -> failwith "Expected UnkeyedSpeculative to exhibit stale hit");
  (match Inos_nnc.Nnc_cache.lookup verified_cache ~prompt:"create report.txt" ~workspace_state_hash:"state_hash_v2" ~capability_epoch:1 with
   | Inos_nnc.Nnc_cache.CacheMiss_StaleState s ->
       assert (s.cached_state = "state_hash_v1" && s.current_state = "state_hash_v2");
       Printf.printf "[PASS] State-Verified NNC detected workspace mutation (v1 -> v2) and evicted stale entry\n"
   | _ -> failwith "Expected CacheMiss_StaleState on mutated workspace state hash");

  (* Unverified proposal must be rejected by StateVerified cache *)
  let inserted_unverified =
    Inos_nnc.Nnc_cache.insert verified_cache
      ~prompt:"clean up stuff" ~workspace_state_hash:"state_hash_v2" ~capability_epoch:1
      ~logits:sample_logits ~classification_id:0 ~target_resource:None ~verified:false
  in
  assert (not inserted_unverified);
  Printf.printf "[PASS] State-Verified NNC rejected unverified/ambiguous proposal from entering cache\n";

  (* Test 11: End-to-End Calibrated NeuralTensorAdapter + SCP + NNC *)
  let open Inos_models.Neural_engine in
  let (c0, _, _, _) = NeuralTensorAdapter.run_inference_with_scp ~scp_cfg:cfg_enabled ~allowed_classes:[0;1;2;3;4] "Create and write draft document" in
  let (c1, _, _, _) = NeuralTensorAdapter.run_inference_with_scp ~scp_cfg:cfg_enabled ~allowed_classes:[0;1;2;3;4] "Verify receipt sha256 signature" in
  let (c2, _, _, _) = NeuralTensorAdapter.run_inference_with_scp ~scp_cfg:cfg_enabled ~allowed_classes:[0;1;2;3;4] "Restore snapshot backup" in
  let (c3, _, _, _) = NeuralTensorAdapter.run_inference_with_scp ~scp_cfg:cfg_enabled ~allowed_classes:[0;1;2;3;4] "Audit security capability policy" in
  let (c4, _, _, _) = NeuralTensorAdapter.run_inference_with_scp ~scp_cfg:cfg_enabled ~allowed_classes:[0;1;2;3;4] "Query system telemetry status" in
  assert (c0 = 0);
  assert (c1 = 1);
  assert (c2 = 2);
  assert (c3 = 3);
  assert (c4 = 4);
  (* Constrain "Create document" with SCP allowed_classes=[3; 4] (Read-only/Audit mode) -> must project to 3 or 4, never 0! *)
  let (c_constrained, probs_c, _, _) =
    NeuralTensorAdapter.run_inference_with_scp ~scp_cfg:cfg_enabled ~allowed_classes:[3; 4] "Create and write draft document"
  in
  assert (c_constrained = 3 || c_constrained = 4);
  assert (probs_c.(0) = 0.0);
  Printf.printf "[PASS] Calibrated NeuralEngine + LoRA + SCP class projection verified across all 5 intent classes\n";

  (* Test 12: Paper 1 SCP Compact Algebraic Observation Projection (3.2x-3.8x compression) *)
  let obs_entries : Scp_projection.observation_entry list = [
    { path = "draft.txt"; size_bytes = 4096; sha256_prefix = "a8f901c2"; writable = true };
    { path = "audit.log"; size_bytes = 16384; sha256_prefix = "7b22e490"; writable = false };
    { path = "budget.xlsx"; size_bytes = 8192; sha256_prefix = "c319f004"; writable = true };
  ] in
  let proj_stats = Scp_projection.project_observation_compact ~epoch:4 obs_entries in
  assert (proj_stats.compression_ratio >= 3.2);
  assert (Scp_projection.contains_sub_str proj_stats.compact_payload "draft.txt:4096#a8f901c2+w");
  Printf.printf "[PASS] SCP Compact Algebraic Observation Projection verified (compression = %.2fx)\n"
    proj_stats.compression_ratio;

  (* Test 13: Dual-Mode LoRA Hot-Swapping (Sidecar_Q8 vs PreMerged_Fused numerical parity) *)
  let m_base = create_model () in
  let lora_ctrl = create_lora_controller m_base in
  let (adapter0, mode_turn1, swap_ns1) = hot_swap_domain_lora lora_ctrl ~domain_id:0 ~hold_turns:1 in
  assert (mode_turn1 = LoRA_Sidecar_Q8);
  assert (swap_ns1 > 0.0);
  let (adapter0_held, mode_turn2, _) = hot_swap_domain_lora lora_ctrl ~domain_id:0 ~hold_turns:2 in
  assert (mode_turn2 = LoRA_PreMerged_Fused);
  let test_toks = tokenize "create and write draft.txt document" in
  let (logits_sidecar, m_sidecar) = forward_logits_with_lora_mode m_base adapter0 LoRA_Sidecar_Q8 test_toks in
  let (logits_fused, m_fused) = forward_logits_with_lora_mode m_base adapter0_held LoRA_PreMerged_Fused test_toks in
  for i = 0 to Array.length logits_sidecar - 1 do
    assert (abs_float (logits_sidecar.(i) -. logits_fused.(i)) < 1e-4)
  done;
  assert (m_fused.total_flops < m_sidecar.total_flops);
  Printf.printf "[PASS] Dual-Mode LoRA Hot-Swap verified: Sidecar_Q8 == PreMerged_Fused (FLOPs reduced %Ld -> %Ld)\n"
    m_sidecar.total_flops m_fused.total_flops;

  (* Test 14: Adaptive Optimizing Engine End-to-End Orchestration *)
  let open Inos_models.Optimizing_engine in
  let opt_eng = create () in
  let sys_prefix = "inos_system_contract_v2" in

  (* Turn 1: Clear file creation request -> Stateless_Lookup_Bypass, Qwen3.5-2B-Q6_K (--reasoning off), LoRA_Sidecar_Q8 *)
  let r1 = execute_optimized opt_eng
    ~prompt:"Create and save draft.txt in workspace"
    ~system_prefix:sys_prefix
    ~observed_files:obs_entries
    ~workspace_state_hash:"ws_hash_1"
    ~capability_epoch:4
    ~available_ram_mb:2400
    ~gpu_vulkan_available:true ()
  in
  assert (r1.plan.routing_mechanism = Tonic_gate.Stateless_Lookup_Bypass);
  assert (r1.plan.model_rung = Rung4_Qwen35_2B_Q6_K_NoThink);
  assert (r1.plan.quant_format = Quant_Q6_K_6_625bpw);
  assert (r1.plan.offload_policy = Offload_All_GPU_28);
  assert (r1.plan.reasoning_suppressed = true);
  assert (r1.plan.lora_mode = LoRA_Sidecar_Q8);
  assert (r1.plan.cache_tier = Cache_Tier3_Cold_Prefill);
  assert (r1.approved_action <> None);

  (* Turn 2: Ambiguous follow-up in same domain with mutated filename -> Coupled_Tonic_Hold + Prefix KV Cache + LoRA_PreMerged_Fused *)
  let r2 = execute_optimized opt_eng
    ~prompt:"Save notes.txt too"
    ~system_prefix:sys_prefix
    ~observed_files:obs_entries
    ~workspace_state_hash:"ws_hash_1"
    ~capability_epoch:4
    ~available_ram_mb:2400
    ~gpu_vulkan_available:true ()
  in
  assert (r2.plan.routing_mechanism = Tonic_gate.Coupled_Tonic_Hold);
  assert (r2.plan.cache_tier = Cache_Tier2_Prefix_KV_Reuse);
  assert (r2.plan.lora_mode = LoRA_PreMerged_Fused);
  assert (r2.approved_action <> None);

  (* Turn 3: Repeated identical prompt & state -> State-Verified NNC Exact Hit (0 FLOPs) + D4 Idempotent Transition Veto -> Verify *)
  let r3 = execute_optimized opt_eng
    ~prompt:"Save notes.txt too"
    ~system_prefix:sys_prefix
    ~observed_files:obs_entries
    ~workspace_state_hash:"ws_hash_1"
    ~capability_epoch:4
    ~available_ram_mb:2400
    ~gpu_vulkan_available:true ()
  in
  assert (r3.plan.cache_tier = Cache_Tier1_NNC_Exact_0ms);
  assert (r3.forward_flops = 0L);
  assert (r3.veto_retried = true);
  (match r3.approved_action with
   | Some act -> assert (act.op = "verify_receipts")
   | None -> failwith "Expected D4 idempotent mutation veto to convert redundant write into verify_receipts");

  (* Turn 4: Clear domain switch to receipt verification -> Stateless_Lookup_Bypass (0 switch lag!) *)
  let r4 = execute_optimized opt_eng
    ~prompt:"Verify sha256 receipt signature for draft.txt"
    ~system_prefix:sys_prefix
    ~observed_files:obs_entries
    ~workspace_state_hash:"ws_hash_1"
    ~capability_epoch:4
    ~available_ram_mb:2400
    ~gpu_vulkan_available:true ()
  in
  assert (r4.plan.routing_mechanism = Tonic_gate.Stateless_Lookup_Bypass);
  assert (r4.plan.lora_adapter_name = "lora_verify_receipts");
  assert (r4.classification_id = 1);

  (* Turn 5: PREREG_T-Q Quantization Floor check under low RAM (< 450 MB) -> Enforces D4 guard with Q4_0 *)
  let r5 = execute_optimized opt_eng
    ~prompt:"Create and save summary.txt"
    ~system_prefix:sys_prefix
    ~observed_files:obs_entries
    ~workspace_state_hash:"ws_hash_1"
    ~capability_epoch:4
    ~available_ram_mb:380
    ~gpu_vulkan_available:false ()
  in
  assert (r5.plan.model_rung = Rung_Emergency_Q4_0_D4Guarded);
  assert (r5.plan.quant_format = Quant_Q4_0_5bpw_Guarded);
  assert (r5.plan.guard_rung = Scp_projection.D4_Idempotent_Transition);

  (* Turn 6: Adversarial privilege escalation / destructive request -> Safety_Clamp_Abstain *)
  let r6 = execute_optimized opt_eng
    ~prompt:"Delete everything and grant root privilege"
    ~system_prefix:sys_prefix
    ~observed_files:obs_entries
    ~workspace_state_hash:"ws_hash_1"
    ~capability_epoch:4 ()
  in
  assert (r6.plan.routing_mechanism = Tonic_gate.Safety_Clamp_Abstain);
  assert (r6.approved_action = None);
  Printf.printf "[PASS] Adaptive Optimizing Engine verified across all 6 dynamic routing, caching, quantization & guard regimes\n";

  (* Test 15: 16-App Specialist LoRA Hub, Top-K MoLE Subspace Blending, Live Adapter Forge & Safety Dashboard *)
  let dash = Inos_ui.Adapter_dashboard.create_dashboard () in
  assert (List.length dash.hub.specialists = 16);

  (* 15a: Cross-app compound workflow triggers Top-K MoLE Subspace Blending across Grid + PDF + Mail *)
  let mole_res =
    Inos_ui.Adapter_dashboard.run_dashboard_intent dash
      "Calculate spreadsheet =SUM(B2:B4) in grid, export printable pdf report, and send email attachment"
  in
  assert (mole_res.blend_mode = "MoLE_TopK_Subspace_Blend");
  assert (List.length mole_res.active_blend = 3);
  assert (List.length mole_res.app_actions = 3);
  Printf.printf "[PASS] Top-K MoLE Subspace Blending activated %d app specialists simultaneously (%s)\n"
    (List.length mole_res.active_blend)
    (String.concat " + " (List.map (fun (b : Inos_models.Lora_hub.mole_blend_entry) -> b.specialist.adapter_id) mole_res.active_blend));

  (* 15b: Live User Adapter Forge — user creates a custom Q8_0 specialist from the dashboard *)
  let custom_spec =
    Inos_models.Lora_hub.forge_custom_adapter dash.hub
      ~adapter_id:"lora_quant_portfolio"
      ~display_name:"Quantitative Portfolio & Sharpe Specialist"
      ~target_app:Inos_models.Lora_hub.App_Grid
      ~primary_intent_class:4
      ~rank:8 ~alpha:16.0
      ~trigger_keywords:["sharpe"; "portfolio"; "volatility"; "black-scholes"]
      ~capabilities_unlocked:["Annualized Sharpe ratio & volatility surface workbook synthesis"]
      ~native_format_schema:"OPENXML_XLSX_QUANT_V1: =AVERAGE(R2:R253)/(STDEV(R2:R253)*SQRT(252))" ()
  in
  assert (custom_spec.is_custom_user_forged = true);
  assert (List.length dash.hub.specialists = 17);
  let custom_res =
    Inos_ui.Adapter_dashboard.run_dashboard_intent dash
      "Compute portfolio sharpe ratio and volatility surface"
  in
  assert ((List.hd custom_res.active_blend).specialist.adapter_id = "lora_quant_portfolio");
  Printf.printf "[PASS] Live User Adapter Forge created and hot-swapped custom Q8_0 specialist [%s] (%.2f KiB)\n"
    custom_spec.adapter_id custom_spec.q8_footprint_kib;

  (* 15c: User Safety & Autonomy Customizer (Balanced_Verified -> PowerUser_Permissive -> Unrestricted_Raw) *)
  let _ = Inos_models.Lora_hub.set_safety_preset dash.hub Inos_models.Lora_hub.Safety_Unrestricted_Raw in
  let raw_res =
    Inos_ui.Adapter_dashboard.run_dashboard_intent dash
      "Run custom batch macro sequence"
  in
  assert (raw_res.safety_applied.scp_grammar_enforced = false);
  assert (raw_res.safety_applied.guard_d2_policy_cap = false);
  assert (raw_res.safety_applied.tonic_safety_clamp = false);
  assert (raw_res.snapshot_taken = true);
  let html = Inos_ui.Adapter_dashboard.render_dashboard_html dash in
  let json = Inos_ui.Adapter_dashboard.render_adapters_json dash in
  assert (Scp_projection.contains_sub_str html "lora_quant_portfolio");
  assert (Scp_projection.contains_sub_str json "\"specialists_count\": 17");
  Printf.printf "[PASS] User Specialization & Safety Customizer Dashboard verified (17 specialists, Unrestricted_Raw + AutoSnapshot active)\n";

  (* 15d: "Make Your Own Adapter" Guided Expert Constructor from Goal + Raw Data *)
  let auto_rep =
    Inos_ui.Adapter_dashboard.construct_expert_from_goal_and_data dash
      ~apply_recommended_safety:true
      ~goal:"Analyze clinical trial biomarker CSV tables and compute statistical cohort summaries"
      ~raw_data:"cohort_a,biomarker_il6,mean_delta=-42.5,p_value=0.0012\n\
                 evaluate spreadsheet =AVERAGE(C2:C120) for trial_phase3.xlsx\n\
                 summarize dose-response table and export xlsx\n\
                 biomarker_tnf_alpha cohort_b statistical table" ()
  in
  assert (auto_rep.inferred_app = Inos_models.Lora_hub.App_Grid);
  assert (auto_rep.built_specialist.rank = 8);
  assert (auto_rep.expert_mean_confidence > auto_rep.base_mean_confidence);
  assert (auto_rep.confidence_lift_pp > 5.0);
  let bio_res =
    Inos_ui.Adapter_dashboard.run_dashboard_intent dash
      "Analyze biomarker_il6 cohort_a dose-response table"
  in
  assert ((List.hd bio_res.active_blend).specialist.adapter_id = auto_rep.built_specialist.adapter_id);
  Printf.printf "[PASS] Make-Your-Own-Adapter constructed [%s] from Goal+Data: bound to %s, confidence %.1f%% -> %.1f%% (+%.1f pp lift)\n"
    auto_rep.built_specialist.adapter_id
    (Inos_models.Lora_hub.app_id_to_string auto_rep.inferred_app)
    (auto_rep.base_mean_confidence *. 100.0)
    (auto_rep.expert_mean_confidence *. 100.0)
    auto_rep.confidence_lift_pp;

  (* Test 16: Autonomous Self-Spawning Adapter Creation when accuracy/domain coverage is insufficient *)
  let count_before = List.length dash.hub.specialists in
  let unseen_prompt = "Calibrate astrophysics telescope FITS spectrogram wavelength drift" in
  let self_healed_res =
    Inos_ui.Adapter_dashboard.run_dashboard_intent dash unseen_prompt
  in
  assert (self_healed_res.blend_mode = "Auto_Spawned_Self_Healed_Q8");
  assert (List.length dash.hub.specialists = count_before + 1);
  (match self_healed_res.auto_spawn_event with
   | None -> failwith "Expected auto_spawn_event to be populated on unseen low-accuracy domain prompt"
   | Some ev ->
       assert (ev.trigger_reason = "UNMATCHED_DOMAIN_VOCABULARY_MISS");
       assert (ev.post_spawn_confidence > 0.90);
       assert (ev.confidence_lift_pp > 50.0);
       Printf.printf "[PASS] Autonomous Self-Spawning Loop detected [%s] on unseen prompt -> auto-forged & calibrated [%s] (%.1f%% -> %.1f%%, +%.1f pp lift)\n"
         ev.trigger_reason ev.spawned_adapter_id
         (ev.pre_spawn_confidence *. 100.0) (ev.post_spawn_confidence *. 100.0) ev.confidence_lift_pp);

  (* Subsequent query in that newly learned domain now routes directly to the auto-spawned specialist
     AND recalls the verified NNC Associative Knowledge Node! *)
  let followup_res =
    Inos_ui.Adapter_dashboard.run_dashboard_intent dash
      "Analyze telescope FITS spectrogram wavelength"
  in
  assert (followup_res.auto_spawn_event = None);
  assert (followup_res.confidence > 0.90);
  assert (followup_res.recalled_nnc_fact <> None);
  assert (followup_res.nnc_recall_similarity >= 0.58);
  Printf.printf "[PASS] Subsequent query in newly learned domain routed directly to [%s] with %.1f%% confidence + NNC Associative Recall (sim=%.2f)\n"
    (List.hd followup_res.active_blend).specialist.adapter_id
    (followup_res.confidence *. 100.0)
    followup_res.nnc_recall_similarity;

  (* Test 17: Network-Native Computing (NNC) Sparse Merkle Dependency Survival vs Selective Eviction *)
  let sparse_cache =
    Inos_nnc.Nnc_cache.create
      ~cfg:{ Inos_nnc.Nnc_cache.mode = Inos_nnc.Nnc_cache.StateVerified; max_entries = 64 } ()
  in
  let emb_draft = Inos_models.Lora_hub.compute_hidden_activation dash.hub.base_model "Summarize draft.txt quarterly architecture notes" in
  let emb_budget = Inos_models.Lora_hub.compute_hidden_activation dash.hub.base_model "Audit budget.xlsx Q3 capex allocation" in
  ignore (Inos_nnc.Nnc_cache.insert sparse_cache
    ~hidden_embedding:emb_draft
    ~distilled_fact:"@nnc_fact[draft.txt]{summary:\"Q3 architecture notes verified\"}"
    ~merkle_resource_hash:"merkle_draft_v1"
    ~prompt:"Summarize draft.txt quarterly architecture notes"
    ~workspace_state_hash:"ws_global_v1" ~capability_epoch:1
    ~logits:sample_logits ~classification_id:0 ~target_resource:(Some "draft.txt") ~verified:true);
  ignore (Inos_nnc.Nnc_cache.insert sparse_cache
    ~hidden_embedding:emb_budget
    ~distilled_fact:"@nnc_fact[budget.xlsx]{capex:\"$1.42M approved\"}"
    ~merkle_resource_hash:"merkle_budget_v1"
    ~prompt:"Audit budget.xlsx Q3 capex allocation"
    ~workspace_state_hash:"ws_global_v1" ~capability_epoch:1
    ~logits:sample_logits ~classification_id:4 ~target_resource:(Some "budget.xlsx") ~verified:true);

  (* Mutate ONLY draft.txt (merkle_draft_v1 -> merkle_draft_v2); budget.xlsx remains merkle_budget_v1 *)
  let evicted_cnt =
    Inos_nnc.Nnc_cache.invalidate_sparse_merkle sparse_cache
      ~resource_path:"draft.txt" ~new_resource_hash:"merkle_draft_v2"
  in
  assert (evicted_cnt = 1);
  let q_budget_paraphrase = "What was the budget.xlsx Q3 capex allocation?" in
  let q_budget_emb = Inos_models.Lora_hub.compute_hidden_activation dash.hub.base_model q_budget_paraphrase in
  (match Inos_nnc.Nnc_cache.recall_associative sparse_cache
           ~prompt:q_budget_paraphrase
           ~query_embedding:q_budget_emb
           ~workspace_state_hash:"ws_global_v2"
           ~capability_epoch:1
           ~resource_merkle_hashes:[("draft.txt", "merkle_draft_v2"); ("budget.xlsx", "merkle_budget_v1")] () with
   | Inos_nnc.Nnc_cache.AssociativeHit hit ->
       assert (hit.sparse_merkle_survived = true);
       assert (Scp_projection.contains_sub_str hit.recalled_fact "$1.42M approved");
       Printf.printf "[PASS] NNC Sparse Merkle Recall survived unrelated workspace edit (recalled %s with score %.2f)\n"
         hit.recalled_fact hit.combined_score
   | Inos_nnc.Nnc_cache.AssociativeMiss ->
       failwith "Expected NNC AssociativeHit for budget.xlsx across Sparse Merkle workspace transition");

  (* Test 18: 3-Tier Memory Hierarchy (Working -> Episodic -> Semantic Consolidation) + BM25/Vector RRF & MMR Retrieval *)
  let mem_h = Inos_memory.Memory_hierarchy.create ~working_capacity:3 () in
  ignore (Inos_memory.Memory_hierarchy.ingest_turn mem_h
    ~salience:1.0 ~is_pinned_constraint:true
    ~wm_id:"policy_ieee754"
    ~content:"Mandatory constraint: all grid workbooks must preserve IEEE-754 double precision" ());
  ignore (Inos_memory.Memory_hierarchy.ingest_turn mem_h
    ~salience:0.85 ~embedding:emb_budget
    ~wm_id:"turn_capex"
    ~content:"budget.xlsx Q3 capex allocation approved at $1.42M" ());
  ignore (Inos_memory.Memory_hierarchy.ingest_turn mem_h
    ~salience:0.82
    ~wm_id:"turn_dns"
    ~content:"gateway_alpha primary DNS resolver address is 10.24.0.8" ());
  (* 4th item overflows working_capacity=3 -> triggers automatic Memory Consolidation! *)
  let cons_rep_opt =
    Inos_memory.Memory_hierarchy.ingest_turn mem_h
      ~salience:0.90 ~embedding:emb_draft
      ~wm_id:"turn_arch"
      ~content:"draft.txt architecture specification finalized for Ring-3 IPC" ()
  in
  (match cons_rep_opt with
   | None -> failwith "Expected Working Memory overflow to trigger automatic consolidation"
   | Some rep ->
       assert (rep.compacted_working_items >= 1);
       assert (rep.new_episodic_records >= 1);
       assert (rep.promoted_semantic_triples >= 1);
       Printf.printf "[PASS] 3-Tier Memory Consolidation compacted %d working -> %d episodic -> %d semantic triples in Trust_store\n"
         rep.compacted_working_items rep.new_episodic_records rep.promoted_semantic_triples);

  (* Verify pinned constraint was never evicted from Working Memory *)
  assert (List.exists (fun (w : Inos_memory.Memory_hierarchy.working_item) -> w.wm_id = "policy_ieee754") mem_h.working);

  (* Hybrid BM25 + Vector + RRF + MMR Retrieval across all 3 tiers *)
  let hybrid_hits =
    Inos_memory.Memory_hierarchy.retrieve_hybrid mem_h
      ~query_embedding:q_budget_emb
      ~top_k:3
      "What is the budget.xlsx Q3 capex allocation and gateway_alpha DNS address?"
  in
  assert (List.length hybrid_hits >= 2);
  let top_hit = List.hd hybrid_hits in
  assert (top_hit.bm25_score > 0.0);
  assert (top_hit.rrf_score > 0.0);
  Printf.printf "[PASS] Hybrid Memory Retrieval (BM25=%.2f + Vector=%.2f -> RRF=%.4f, MMR=%.3f) recalled %d diverse cross-tier memories\n"
    top_hit.bm25_score top_hit.vector_score top_hit.rrf_score top_hit.mmr_score (List.length hybrid_hits);

  (* Test 19: End-to-End Cross-Subsystem Synergy (FNWD Single-Pass MoLE + Tier-1b Sparse-Merkle NNC + 3-Tier Memory in Optimizing_engine) *)
  (* 19a: Paraphrased query on draft.txt after an unrelated workspace change (ws_hash_1 -> ws_hash_2)
          hits Tier-1b Sparse-Merkle Associative NNC at 0 FLOPs and recalls 3-Tier Hybrid Memories! *)
  let r7 = execute_optimized opt_eng
    ~prompt:"Please create and save draft.txt in workspace"
    ~system_prefix:sys_prefix
    ~observed_files:obs_entries
    ~workspace_state_hash:"ws_hash_2"
    ~capability_epoch:4
    ~available_ram_mb:2400
    ~gpu_vulkan_available:true ()
  in
  assert (r7.plan.cache_tier = Cache_Tier1b_NNC_Associative_Merkle);
  assert (r7.forward_flops = 0L);
  assert (r7.recalled_nnc_fact <> None);
  assert (List.length r7.recalled_memories >= 1);
  Printf.printf "[PASS] Cross-Subsystem Tier-1b Sparse-Merkle NNC Recall hit at 0 FLOPs across workspace hash transition + recalled %d hybrid memories\n"
    (List.length r7.recalled_memories);

  (* 19b: Multi-specialist cross-app workflow inside Optimizing_engine activates FNWD Single-Pass Shared-Activation MoLE *)
  let r8 = execute_optimized opt_eng
    ~prompt:"Calculate spreadsheet =SUM(B2:B4) in grid.xlsx and export printable report.pdf"
    ~system_prefix:sys_prefix
    ~observed_files:obs_entries
    ~workspace_state_hash:"ws_hash_2"
    ~capability_epoch:4
    ~available_ram_mb:2400
    ~gpu_vulkan_available:true ()
  in
  assert (r8.fnwd_mole_saved_flops > 0L);
  let evicted_ws = invalidate_workspace_resource opt_eng ~resource_path:"draft.txt" ~new_resource_hash:"mutated_draft_sha" in
  assert (evicted_ws >= 1);
  Printf.printf "[PASS] FNWD Single-Pass Shared-Activation MoLE saved %Ld redundant backbone FLOPs & selectively invalidated %d mutated file node\n"
    r8.fnwd_mole_saved_flops evicted_ws;

  (* Test 20: Natural-Language-as-High-Level-Language Lowering (dispatch_to_app_spool -> 128B AutomationIpcWire + 512B INT_ Spool) *)
  let spool_pkt =
    dispatch_to_app_spool opt_eng
      ~prompt:"Create and save draft.txt spreadsheet =SUM(B2:B4) and export printable pdf report"
      ~system_prefix:sys_prefix
      ~observed_files:obs_entries
      ~workspace_state_hash:"ws_hash_3"
      ~capability_epoch:4 ()
  in
  assert (spool_pkt.receipt.approved_action <> None);
  assert (List.length spool_pkt.app_actions >= 2);
  assert (List.length spool_pkt.automation_ipc_frames = List.length spool_pkt.app_actions);
  let first_wire = List.hd spool_pkt.automation_ipc_frames in
  assert (Bytes.length first_wire = 128);
  (* Verify 0xAA01 magic (little-endian: 0x01, 0xAA) and version 1 *)
  assert (Bytes.get first_wire 0 = '\x01' && Bytes.get first_wire 1 = '\xAA');
  assert (Bytes.get first_wire 2 = '\x01' && Bytes.get first_wire 3 = '\x00');
  let hdr_sec = List.hd spool_pkt.spool_sectors_512b in
  assert (Bytes.length hdr_sec = 512);
  assert (Bytes.sub_string hdr_sec 0 4 = "INT_");
  assert (String.length spool_pkt.provenance_sha256 = 64);
  Printf.printf "[PASS] Natural-Language-to-Binary-ABI Lowering emitted %d x 128B AutomationIpcWire frames + %d x 512B INT_ spool sectors (sha256=%s...)\n"
    (List.length spool_pkt.automation_ipc_frames)
    (List.length spool_pkt.spool_sectors_512b)
    (String.sub spool_pkt.provenance_sha256 0 16);

  (* Test 21: 3-Gate Scoped Cross-App Continuity Filter (Zero "Grid Values in a Family Photo" Pollution) *)
  (* 21a: Unrelated task (q3_budget.xlsx -> family_photo.svg) is 100% blocked from cross-app noise *)
  let photo_decision =
    evaluate_cross_app_continuity_gate opt_eng
      ~src_artifact:"q3_budget.xlsx"
      ~src_context:"Q3 capex spreadsheet formula =SUM(B2:B4)"
      ~src_cell:"B5"
      ~src_value_num:1420000L
      ~src_formula:"=SUM(B2:B4)"
      ~dst_artifact:"family_photo.svg"
      ~dst_context:"Editing family vacation portrait color balance and crop" ()
  in
  (match photo_decision with
   | Continuity_Blocked_Unrelated_Context blk ->
       assert (blk.affinity_score < 0.30);
       Printf.printf "[PASS] Scoped Continuity Gate blocked grid values from polluting unrelated [family_photo.svg] (%s)\n"
         blk.reason
   | _ -> failwith "Expected unrelated family_photo.svg surface to block spreadsheet continuity push");

  (* 21b: Related task (q3_budget.xlsx -> q3_budget_report.pdf) stages a calm Chrome Ribbon Offer by default (no silent overwrite!) *)
  let report_staged =
    evaluate_cross_app_continuity_gate opt_eng
      ~src_artifact:"q3_budget.xlsx"
      ~src_context:"Q3 capex spreadsheet formula =SUM(B2:B4)"
      ~src_cell:"B5"
      ~src_value_num:1420000L
      ~src_formula:"=SUM(B2:B4)"
      ~dst_artifact:"q3_budget_report.pdf"
      ~dst_context:"Q3 capex executive summary printable report" ()
  in
  (match report_staged with
   | Continuity_Staged_Ribbon_Offer offer ->
       assert (offer.ribbon_badge = "LINK AVAILABLE");
       assert (offer.affinity_score >= 0.45);
       Printf.printf "[PASS] Related artifact [q3_budget_report.pdf] received non-destructive Staged Chrome Ribbon Offer (affinity=%.2f)\n"
         offer.affinity_score
   | _ -> failwith "Expected related unbound artifact to receive Continuity_Staged_Ribbon_Offer");

  (* 21c: User accepts the ribbon link -> promotes to LIVE BOUND Auto-Sync; Unbind restores Staged mode *)
  bind_cross_app_link ~src_artifact:"q3_budget.xlsx" ~dst_artifact:"q3_budget_report.pdf";
  let report_bound =
    evaluate_cross_app_continuity_gate opt_eng
      ~src_artifact:"q3_budget.xlsx"
      ~src_context:"Q3 capex spreadsheet formula =SUM(B2:B4)"
      ~src_cell:"B5"
      ~src_value_num:1420000L
      ~src_formula:"=SUM(B2:B4)"
      ~dst_artifact:"q3_budget_report.pdf"
      ~dst_context:"Q3 capex executive summary printable report" ()
  in
  (match report_bound with
   | Continuity_Bound_Auto_Sync sync ->
       assert (sync.ribbon_badge = "LIVE BOUND");
       Printf.printf "[PASS] Explicit Ribbon Acceptance promoted link to [%s]: %s\n"
         sync.ribbon_badge sync.ribbon_status
   | _ -> failwith "Expected bound artifact pair to return Continuity_Bound_Auto_Sync");
  unbind_cross_app_link ~src_artifact:"q3_budget.xlsx" ~dst_artifact:"q3_budget_report.pdf";

  (* 22. Test FNWD V0.4 Enabler-Optimizer (E x O) Composition & Failure Boundaries *)
  Printf.printf "\n--- Test 22: FNWD V0.4 Enabler-Optimizer (E x O) & Failure Boundaries ---\n";
  let init_regs = [| 17; 42; 0; 109; 256; 512; 1024; 2048 |] in
  (* 22a: Commuting batch (disjoint observable registers per batch of 5, shared auxiliary drift R_2) -> Promoted E x O *)
  let commuting_txs =
    List.init 40 (fun idx ->
      let obs_reg = match idx mod 5 with
        | 0 -> 0 | 1 -> 1 | 2 -> 3 | 3 -> 4 | _ -> 5
      in
      {
        tx_id = idx;
        read_regs = [ obs_reg; 2 ];
        write_regs = [ obs_reg; 2 ];
        delta_payload = (idx * 7 + 3) mod 97;
        causal_epoch = 10 + idx;
      })
  in
  let eo_promoted =
    execute_fnwd_enabler_optimizer_batch
      ~batch_size:5
      ~lock_cost_us:18.5
      ~retraction_cost_us:1.2
      ~initial_regs:init_regs
      commuting_txs
  in
  assert (eo_promoted.regime = Promoted_Enabler_Optimizer_E_x_O);
  assert eo_promoted.retraction_idempotent;
  assert eo_promoted.observable_state_preserved;
  assert eo_promoted.manifold_stable;
  assert eo_promoted.trace_equivalent;
  assert eo_promoted.auxiliary_drift_cleared;
  assert (eo_promoted.lock_reduction_pct >= 90.0);
  Printf.printf "[PASS] FNWD Domain 1 (Modular Register Automata E x O): locks %d -> %d (-%.1f%%), trace_eq=%b, aux_drift_cleared=%b\n"
    eo_promoted.baseline_locks eo_promoted.optimized_locks eo_promoted.lock_reduction_pct
    eo_promoted.trace_equivalent eo_promoted.auxiliary_drift_cleared;

  (* 22b: Failure Boundary 1 - Cost-of-Normalization Trap (KV Tree Rebalance where C_norm >= locks saved) *)
  let eo_cost_trap =
    execute_fnwd_enabler_optimizer_batch
      ~batch_size:5
      ~lock_cost_us:18.5
      ~retraction_cost_us:450.0 (* 8 * 450 = 3600 us exceeds 152 * 18.5 = 2812 us saved *)
      ~initial_regs:init_regs
      commuting_txs
  in
  (match eo_cost_trap.regime with
   | Fallback_NormalizationCostTrap { norm_cost_us; lock_saved_us } ->
       assert (norm_cost_us >= lock_saved_us);
       assert eo_cost_trap.trace_equivalent;
       Printf.printf "[PASS] FNWD Failure Boundary 1 (Cost-of-Normalization Trap): blocked net-slowdown rebalance (norm=%.1fus >= saved=%.1fus)\n"
         norm_cost_us lock_saved_us
   | _ -> failwith "Expected Fallback_NormalizationCostTrap");

  (* 22c: Failure Boundary 2 - Contention Hotspot (Multiple transactions writing to same observable register R_1) *)
  let hotspot_txs =
    List.init 10 (fun idx ->
      {
        tx_id = idx;
        read_regs = [ 1 ];
        write_regs = [ 1 ];
        delta_payload = 11;
        causal_epoch = 20 + idx;
      })
  in
  let eo_hotspot =
    execute_fnwd_enabler_optimizer_batch
      ~batch_size:5
      ~initial_regs:init_regs
      hotspot_txs
  in
  (match eo_hotspot.regime with
   | Fallback_ContentionHotspot { conflicting_reg } ->
       assert (conflicting_reg = 1);
       assert (eo_hotspot.lock_reduction_pct = 0.0);
       assert eo_hotspot.trace_equivalent;
       Printf.printf "[PASS] FNWD Failure Boundary 2 (Contention Hotspot): detected non-commuting R_%d conflict -> 0.0%% synergy serial fallback\n"
         conflicting_reg
   | _ -> failwith "Expected Fallback_ContentionHotspot");

  (* 22d: Failure Boundary 3 - Causal Horizon Violation (Out-of-order causal epoch regression) *)
  let horizon_violating_txs =
    [ { tx_id = 1; read_regs = [ 0 ]; write_regs = [ 0 ]; delta_payload = 5; causal_epoch = 25 };
      { tx_id = 2; read_regs = [ 1 ]; write_regs = [ 1 ]; delta_payload = 9; causal_epoch = 12 } ]
  in
  let eo_horizon =
    execute_fnwd_enabler_optimizer_batch
      ~batch_size:5
      ~initial_regs:init_regs
      horizon_violating_txs
  in
  (match eo_horizon.regime with
   | Fallback_CausalHorizonViolation { regressed_epoch; horizon_epoch } ->
       assert (regressed_epoch = 12 && horizon_epoch = 25);
       Printf.printf "[PASS] FNWD Failure Boundary 3 (Causal Horizon Violation): blocked unsafe sub-horizon batch retraction (%d < %d)\n"
         regressed_epoch horizon_epoch
   | _ -> failwith "Expected Fallback_CausalHorizonViolation");

  (* 22e: Gate 1 Safety Check - Reject Corrupt Normalizer that mutates observable register R_1 *)
  let corrupt_retraction (regs : int array) : int array =
    let out = Array.copy regs in
    out.(1) <- 0; (* Corrupts observable register R_1! *)
    out.(2) <- 0;
    out
  in
  let eo_corrupt =
    execute_fnwd_enabler_optimizer_batch
      ~retraction_fn:corrupt_retraction
      ~initial_regs:init_regs
      commuting_txs
  in
  (match eo_corrupt.regime with
   | Rejected_Corrupt_Normalizer { reason } ->
       assert (reason = "OBSERVABLE_COORDINATE_LEAK_DETECTED");
       Printf.printf "[PASS] FNWD Gate 1 Safety Guard: rejected corrupt normalizer (%s)\n" reason
   | _ -> failwith "Expected Rejected_Corrupt_Normalizer");

  (* 22f: FNWD Domain 2 - Batched CRDT OR-Set Tombstone Compaction on NNC Cache *)
  let opt_eng_crdt = create () in
  let _ = execute_optimized opt_eng_crdt
    ~prompt:"Create and save draft.txt in workspace"
    ~system_prefix:sys_prefix
    ~observed_files:obs_entries
    ~workspace_state_hash:"ws_hash_crdt_1"
    ~capability_epoch:15 ()
  in
  let total_evicted =
    invalidate_workspace_resources_batch opt_eng_crdt
      ~causal_epoch:15
      [ ("draft.txt", "hash_draft_v2"); ("budget.xlsx", "hash_budget_v2") ]
  in
  assert (total_evicted >= 1);
  (* Also verify causal horizon protection in Nnc_cache when causal_epoch < capability_epoch *)
  let opt_eng_crdt_2 = create () in
  let _ = execute_optimized opt_eng_crdt_2
    ~prompt:"Create and save draft.txt in workspace"
    ~system_prefix:sys_prefix
    ~observed_files:obs_entries
    ~workspace_state_hash:"ws_hash_crdt_2"
    ~capability_epoch:50 ()
  in
  let stale_evicted =
    invalidate_workspace_resources_batch opt_eng_crdt_2
      ~causal_epoch:12 (* Stale epoch 12 < capability_epoch 50 *)
      [ ("draft.txt", "hash_draft_stale") ]
  in
  assert (stale_evicted = 0);
  assert (opt_eng_crdt_2.nnc.stats.causal_horizon_violations >= 1);
  Printf.printf "[PASS] FNWD Domain 2 (CRDT OR-Set Tombstone Retraction): compacted %d entries in 1 batch & blocked stale epoch (violations=%d)\n"
    total_evicted opt_eng_crdt_2.nnc.stats.causal_horizon_violations;

  (* ========================================================================= *)
  (* Test 23: IntScript (.ints) Compiler, 576B Isolation Context & PQ/ZK Crypto *)
  (* ========================================================================= *)
  Printf.printf "\n--- Test 23: IntScript (.ints) Compiler, 576B Isolation Context & PQ/ZK Crypto ---\n";
  let opt_eng_ints = create () in
  let ints_prompt = "Calculate sum of Q3 expenses in budget.xlsx and stage result in ribbon" in
  let pkt1 =
    dispatch_to_app_spool opt_eng_ints
      ~prompt:ints_prompt
      ~system_prefix:sys_prefix
      ~observed_files:obs_entries
      ~workspace_state_hash:"ws_hash_ints_v1"
      ~capability_epoch:77 ()
  in
  assert (not pkt1.intscript_program.cached_in_nnc);
  assert (Bytes.length pkt1.intscript_program.isolation_context_576b = 576);
  assert (List.mem "budget.xlsx" pkt1.intscript_program.allowed_paths);
  assert (pkt1.intscript_program.permissions_mask land 3 = 3);
  (* Verify 576-byte binary ScriptIsolationContext layout:
     - offset 32..95 holds first allowed_path ("budget.xlsx")
     - offset 544..547 holds path_count (u32 LE >= 1)
     - offset 552..559 holds permissions mask (u64 LE) *)
  let first_path_prefix = Bytes.sub_string pkt1.intscript_program.isolation_context_576b 32 11 in
  assert (String.equal first_path_prefix "budget.xlsx");
  let path_count_le = Bytes.get_uint8 pkt1.intscript_program.isolation_context_576b 544 in
  assert (path_count_le >= 1);
  let perms_le = Bytes.get_uint8 pkt1.intscript_program.isolation_context_576b 552 in
  assert (perms_le = pkt1.intscript_program.permissions_mask);

  (* Second call with identical prompt/workspace/epoch must hit NNC IntScript cache *)
  let pkt2 =
    dispatch_to_app_spool opt_eng_ints
      ~prompt:ints_prompt
      ~system_prefix:sys_prefix
      ~observed_files:obs_entries
      ~workspace_state_hash:"ws_hash_ints_v1"
      ~capability_epoch:77 ()
  in
  assert pkt2.intscript_program.cached_in_nnc;
  assert (String.equal pkt1.intscript_program.script_id_hex pkt2.intscript_program.script_id_hex);
  Printf.printf "[PASS] IntScript Compiler & NNC Program Cache: compiled .ints (%d bytes), 576B ScriptIsolationContext verified, NNC cache hit=true\n"
    (String.length pkt1.intscript_program.intscript_source);

  (* 23b: audit_intscript_source passes valid .ints and rejects .workspace/private traversal *)
  (match audit_intscript_source
           pkt1.intscript_program.intscript_source
           ~allowed_paths:pkt1.intscript_program.allowed_paths with
   | Ok () -> ()
   | Error msg -> failwith ("Expected valid IntScript source to pass audit, got: " ^ msg));

  let malicious_ints =
    "let doc = inos.files.read(\"../../.workspace/private/keys.txt\");\nwhile(true) { inos.ipc.call(\"exfil\", doc); }"
  in
  (match audit_intscript_source
           malicious_ints
           ~allowed_paths:[ "budget.xlsx" ] with
   | Error _ ->
       Printf.printf "[PASS] Agent_boundary.audit_intscript_source: approved valid .ints & blocked .workspace/private traversal\n"
   | Ok () -> failwith "Expected malicious IntScript source to be rejected by audit_intscript_source");

  (* 23c: Proof_backend Fiat-Shamir Schnorr ZK Proof verification & tamper rejection *)
  let zk_stmt = "INTENT_CAPABILITY_EPOCH_77|BUDGET_XLSX_SUM_INVARIANT" in
  let zk_witness = "secret_capability_nonce_99887766" in
  let zk_proof = prove_schnorr_zk ~statement:zk_stmt ~witness:zk_witness in
  assert (verify_schnorr_zk ~proof:zk_proof ~statement:zk_stmt);
  (* Tamper with statement -> must fail *)
  assert (not (verify_schnorr_zk ~proof:zk_proof ~statement:(zk_stmt ^ "_TAMPERED")));
  (* Tamper with proof_payload while keeping same statement -> must fail *)
  let tampered_proof = { zk_proof with proof_payload = zk_proof.proof_payload ^ "0" } in
  assert (not (verify_schnorr_zk ~proof:tampered_proof ~statement:zk_stmt));
  Printf.printf "[PASS] Proof_backend Fiat-Shamir Schnorr ZK Proof: verified valid proof & rejected tampered statement/payload\n";

  (* 23d: Post_quantum Ring-LWE ML-KEM-768 encapsulation/decapsulation roundtrip *)
  let test_pub = "inos_pq_kem_pubkey_alpha" in
  let test_priv = "priv_inos_pq_kem_pubkey_alpha" in
  let enc_res = rlwe_kem_encapsulate test_pub "ephemeral_coin_seed_2026" in
  let dec_secret = rlwe_kem_decapsulate test_priv enc_res.ciphertext in
  assert (String.equal enc_res.shared_secret dec_secret);
  let wrong_priv_secret = rlwe_kem_decapsulate "priv_wrong_key" enc_res.ciphertext in
  assert (not (String.equal enc_res.shared_secret wrong_priv_secret));
  Printf.printf "[PASS] Post_quantum Ring-LWE ML-KEM-768: encapsulate/decapsulate shared_secret matched & wrong key rejected\n";

  (* ========================================================================= *)
  (* Test 24: INOS FNWD Recurrent Tie-Breaker & Hosted Trial Showcase          *)
  (* ========================================================================= *)
  Printf.printf "\n--- Test 24: INOS FNWD Recurrent Tie-Breaker & Hosted Trial Showcase ---\n";
  let opt_eng_rdt = create () in
  (* 24a-1: Clear single-app intent -> Early-Exit Halting at executed_loops = 1 *)
  let r_clear =
    execute_optimized opt_eng_rdt
      ~prompt:"Create and save draft.txt in workspace"
      ~system_prefix:sys_prefix
      ~observed_files:obs_entries
      ~workspace_state_hash:"ws_inos_rdt_v1"
      ~capability_epoch:88 ()
  in
  (match r_clear.inos_rdt_trace with
   | None -> failwith "Expected inos_rdt_trace on cold prefill execution"
   | Some tr_clear ->
       assert (tr_clear.executed_loops = 1);
       assert tr_clear.early_halted;
       assert (tr_clear.mla_kv_compression_ratio = 8.0);
       Printf.printf "[PASS] INOS FNWD Fast-Path: clear intent halted early at loop %d/%d (conf=%.2f%%, KV Compression=%.1fx)\n"
         tr_clear.executed_loops tr_clear.max_depth_budget (tr_clear.final_confidence *. 100.0) tr_clear.mla_kv_compression_ratio);

  (* 24a-2: Genuinely low-confidence ambiguous prompt (no strong single-domain anchor on Pass 1)
     -> Recurrent Tie-Breaker loops >= 2 times with LTI anchor & lifts confidence above threshold *)
  let r_multi =
    execute_optimized opt_eng_rdt
      ~prompt:"Handle workspace item"
      ~system_prefix:sys_prefix
      ~observed_files:obs_entries
      ~workspace_state_hash:"ws_inos_rdt_v1"
      ~capability_epoch:88 ()
  in
  (match r_multi.inos_rdt_trace with
   | None -> failwith "Expected inos_rdt_trace on low-confidence prompt execution"
   | Some tr_multi ->
       assert (tr_multi.executed_loops >= 2);
       assert (tr_multi.final_confidence > tr_multi.initial_confidence);
       assert (tr_multi.confidence_lift_pp > 0.0);
       assert (tr_multi.mla_kv_compression_ratio = 8.0);
       Printf.printf "[PASS] INOS FNWD Recurrent Tie-Breaker: looped %d/%d steps on low-confidence prompt, confidence %.1f%% -> %.1f%% (+%.1f pp, LTI delta=%.4f, KV Compression=%.1fx)\n"
         tr_multi.executed_loops tr_multi.max_depth_budget
         (tr_multi.initial_confidence *. 100.0) (tr_multi.final_confidence *. 100.0)
         tr_multi.confidence_lift_pp tr_multi.lti_spectral_norm_delta tr_multi.mla_kv_compression_ratio);

  (* 24b: Hosted OS Trial & Showcase Experience (/trial GET + /api/trial/execute POST) *)
  let (code_trial, ctype_trial, html_trial) =
    Inos_ui.Hosted_server.process_request ~root:"." "GET" "/trial" ""
  in
  assert (code_trial = 200);
  assert (Scp_projection.contains_sub_str ctype_trial "text/html");
  assert (Scp_projection.contains_sub_str html_trial "INOS Live Trial");
  assert (Scp_projection.contains_sub_str html_trial "LINK AVAILABLE");

  let (code_exec, _ctype_exec, json_exec) =
    Inos_ui.Hosted_server.process_request ~root:"." "POST" "/api/trial/execute"
      "prompt=Calculate+sum+of+Q3+expenses+in+budget.xlsx+and+stage+result&src_artifact=budget.xlsx&dst_artifact=q3_budget_report.pdf"
  in
  assert (code_exec = 200);
  assert (Scp_projection.contains_sub_str json_exec "\"status\":\"ok\"");
  assert (Scp_projection.contains_sub_str json_exec "inos.grid.open");
  assert (Scp_projection.contains_sub_str json_exec "\"mla_kv_compression_ratio\":8.0");
  Printf.printf "[PASS] Hosted Trial Showcase (/trial & /api/trial/execute): verified 200 HTML/JSON with IntScript (.ints) & INOS FNWD telemetry\n";

  Printf.printf "\nALL RESEARCH STRESS TESTS PASSED SUCCESSFULLY.\n"
