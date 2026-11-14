(* models/adapter/optimizing_engine.ml
   ==========================================================================
   INOS Adaptive Optimizing Inference & Routing Engine
   ==========================================================================
   Synthesizes the best empirical components across the INOS + SCP_Local_LLM +
   Paper 1 (Structured Capsule Projection & Deterministic Guard Ladder) +
   Paper 2 (Coupled Tonic Gating vs. Stateless Lookup Clarity Bypass) +
   Dual-Mode L1-Resident Q8_0 / Pre-Merged Weight-Fused LoRA Hot-Swapping:

   1. Dynamic Regime Routing (Paper 2):
      - Stateless_Lookup_Bypass when evidence clarity C_t >= 0.72 on a domain
        transition (0.0-segment switch lag).
      - Coupled_Tonic_Hold (rho_eff = 0.989, T_hold ~ 26.8-65.0 tokens) when
        bridging ambiguous follow-ups or maintaining a multi-turn domain regime.
      - Safety_Clamp_Abstain on destructive/adversarial inputs (risk >= 0.80).

   2. Multi-Tier State-Verified Cache (NNC + Prefix KV Cache):
      - Tier 1: State-Verified NNC Exact Hit (0.0 ms, 100% compute bypass).
      - Tier 2: Prefix KV Cache Reuse (~68% prefill savings on shared system prompt).
      - Tier 3: Cold Prefill.

   3. Empirical Model & Quantization Escalation Ladder (SCP_Local_LLM / Qwen3.5):
      - Rung 0: Deterministic_B0 (0 params, 0 ms) for unambiguous routine ops.
      - Rung 1: SmolLM2_135M_Q8_0 (135M, 9.0 bpw, 144.4 MiB) for fast Ring 3 CPU ops.
      - Rung 2: Qwen25_0_5B_Q6_K (0.5B, 6.625 bpw, 498 MiB) for lightweight structured ops.
      - Rung 3: Qwen25_1_5B_Q6_K (1.5B, 6.625 bpw, 1.29 GB) for standard multi-constraint ops.
      - Rung 4: Qwen35_2B_Q6_K_NoThink (2.0B, 6.625 bpw, 1.70 GB, --reasoning off):
        Empirical Pareto Winner (59.2% strict accuracy, +16.4 pp over 3B, 14.3 tok/s).
      - Rung 5: Qwen25_3B_Q6_K_Oracle (3.0B, 6.625 bpw, 2.79 GB) for Dual-Model Role B
        arbitration & high-ambiguity semantic verification.
      - Quantization Floor Protection (PREREG_T-Q): Forbids Q4_0/Q4_K_M on policy-sensitive
        tasks unless D4 deterministic guard is enforced; defaults to Q6_K (0 safety drifts).

   4. Hardware Offload Policy (VULKAN_BENCHMARK_REPORT):
      - Strictly enforces All-or-Nothing offload (Offload_All_GPU_28 when UMA VRAM
        headroom permits, or Offload_All_CPU_0 for Ring 3 / tight memory), never
        splitting layers (-ngl 14 incurs -40.7% PCIe/UMA ping-pong penalty).

   5. Dual-Mode LoRA Hot-Swapping:
      - Selects domain adapter (lora_os_files, lora_verify_receipts, lora_restore_snapshot,
        lora_security_audit, lora_query_telemetry).
      - Uses LoRA_Sidecar_Q8 on the first turn of a domain switch (2.85 ns L1 prefetch swap)
        and automatically promotes to LoRA_PreMerged_Fused (0% runtime overhead) when
        Tonic-held on the same domain across >= 2 turns.

   6. SCP Compact Algebraic Observation Projection & D1-D4 Guard Ladder (Paper 1):
      - Compresses workspace observations by 3.2x-3.8x via compact algebraic notation.
      - Enforces D1 (Syntax), D2 (Policy/Capability), D3 (Epistemic Grounding), and
        D4 (Idempotent Transition Delta S = 0) with 1-retry typed veto and
        ABORTED_UNRESOLVED provenance receipt.
   ========================================================================== *)

type quant_format =
  | Quant_None_Deterministic
  | Quant_Q8_0_9bpw       (* 36 B / 32 weights = 9.000 bpw *)
  | Quant_Q6_K_6_625bpw   (* 212 B / 256 weights = 6.625 bpw - Pareto Frontier *)
  | Quant_Q4_0_5bpw_Guarded (* 20 B / 32 weights = 5.000 bpw - Requires D4 Gate *)

type model_rung =
  | Rung0_Deterministic_B0
  | Rung1_SmolLM2_135M_Q8_0
  | Rung2_Qwen25_0_5B_Q6_K
  | Rung3_Qwen25_1_5B_Q6_K
  | Rung4_Qwen35_2B_Q6_K_NoThink
  | Rung5_Qwen25_3B_Q6_K_Oracle
  | Rung_Emergency_Q4_0_D4Guarded

type offload_policy =
  | Offload_All_CPU_0      (* -ngl 0: Ring 3 / CPU SIMD *)
  | Offload_All_GPU_28     (* -ngl 28: Full Vulkan iGPU offload (never -ngl 14 split!) *)

type cache_tier =
  | Cache_Tier1_NNC_Exact_0ms
  | Cache_Tier1b_NNC_Associative_Merkle
  | Cache_Tier2_Prefix_KV_Reuse
  | Cache_Tier3_Cold_Prefill

type execution_plan = {
  routing_mechanism : Inos_tonic.Tonic_gate.routing_mechanism;
  route_tier : Inos_tonic.Tonic_gate.route;
  cache_tier : cache_tier;
  model_rung : model_rung;
  quant_format : quant_format;
  offload_policy : offload_policy;
  lora_adapter_name : string;
  lora_mode : Neural_engine.lora_exec_mode;
  lora_swap_ns : float;
  guard_rung : Inos_scp.Scp_projection.guard_rung;
  depth_budget : int;
  reasoning_suppressed : bool; (* --reasoning off for Qwen3.5-2B *)
  observation_compression_ratio : float;
}

type execution_receipt = {
  plan : execution_plan;
  approved_action : Inos_scp.Scp_projection.typed_action option;
  guard_status : string;
  veto_retried : bool;
  classification_id : int;
  confidence : float;
  forward_flops : int64;
  fnwd_mole_saved_flops : int64;
  latency_ms : float;
  auto_spawned_specialist : string option;
  recalled_nnc_fact : string option;
  recalled_memories : string list;
  inos_rdt_trace : Neural_engine.inos_rdt_trace option;
}


type compiled_intscript_program = {
  script_id_hex : string;
  intscript_source : string;
  allowed_paths : string list;
  permissions_mask : int; (* bitmask: READ=1, WRITE=2, CREATE=4, IPC=16 *)
  isolation_context_576b : bytes; (* 576-byte binary ScriptIsolationContext wire struct *)
  cached_in_nnc : bool;
}

type t = {
  base_model : Neural_engine.neural_model;
  lora_ctrl : Neural_engine.lora_hot_swap_controller;
  lora_hub : Lora_hub.t;
  nnc : Inos_nnc.Nnc_cache.t;
  intscript_cache : (string * string * int, compiled_intscript_program) Hashtbl.t;
  tonic_cfg : Inos_tonic.Tonic_gate.config;
  scp_cfg : Inos_scp.Scp_projection.config;
  mutable coupled_state : Inos_tonic.Tonic_gate.coupled_state;
  mutable last_system_prefix_hash : string;
  mutable last_committed_action : (string * string) option;
  mutable total_requests : int;
  mutable nnc_hits : int;
  mutable nnc_associative_hits : int;
  mutable prefix_kv_hits : int;
  mutable stateless_bypasses : int;
  mutable tonic_holds : int;
  mutable fused_lora_executions : int;
  mutable sidecar_lora_executions : int;
  mutable guard_veto_repairs : int;
  mutable guard_aborts : int;
  mutable auto_spawned_adapters : int;
  mutable memory_consolidations : int;
}

let create ?(cache_capacity = 256) () : t =
  let base_model = Neural_engine.create_model () in
  let lora_ctrl = Neural_engine.create_lora_controller base_model in
  let lora_hub = Lora_hub.create () in
  let nnc_cfg = {
    Inos_nnc.Nnc_cache.mode = Inos_nnc.Nnc_cache.StateVerified;
    max_entries = cache_capacity;
  } in
  let nnc = Inos_nnc.Nnc_cache.create ~cfg:nnc_cfg () in
  let intscript_cache = Hashtbl.create 64 in
  let tonic_cfg = { Inos_tonic.Tonic_gate.default_config with enabled = true } in
  let scp_cfg = { Inos_scp.Scp_projection.default_config with enabled = true } in
  {
    base_model;
    lora_ctrl;
    lora_hub;
    nnc;
    intscript_cache;
    tonic_cfg;
    scp_cfg;
    coupled_state = Inos_tonic.Tonic_gate.init_coupled_state ();
    last_system_prefix_hash = "";
    last_committed_action = None;
    total_requests = 0;
    nnc_hits = 0;
    nnc_associative_hits = 0;
    prefix_kv_hits = 0;
    stateless_bypasses = 0;
    tonic_holds = 0;
    fused_lora_executions = 0;
    sidecar_lora_executions = 0;
    guard_veto_repairs = 0;
    guard_aborts = 0;
    auto_spawned_adapters = 0;
    memory_consolidations = 0;
  }

(* Selectively invalidate only NNC nodes bound to a mutated workspace file
   while preserving unrelated verified file nodes across both engine and hub caches *)
let invalidate_workspace_resource (eng : t) ~(resource_path : string) ~(new_resource_hash : string) : int =
  let c1 = Inos_nnc.Nnc_cache.invalidate_sparse_merkle eng.nnc ~resource_path ~new_resource_hash in
  let c2 = Inos_nnc.Nnc_cache.invalidate_sparse_merkle eng.lora_hub.nnc_memory ~resource_path ~new_resource_hash in
  let to_remove = ref [] in
  Hashtbl.iter (fun k prog ->
    if List.mem resource_path prog.allowed_paths then
      to_remove := k :: !to_remove
  ) eng.intscript_cache;
  List.iter (Hashtbl.remove eng.intscript_cache) !to_remove;
  c1 + c2


(* Select optimal Model & Quantization Rung + Hardware Offload Policy *)
let select_model_and_quant
    ~(route : Inos_tonic.Tonic_gate.route)
    ~(ambiguity : float)
    ~(risk : float)
    ~(available_ram_mb : int)
    ~(gpu_vulkan_available : bool) : model_rung * quant_format * offload_policy * Inos_scp.Scp_projection.guard_rung =
  match route with
  | Inos_tonic.Tonic_gate.RouteDeterministic when ambiguity < 0.15 && risk < 0.20 ->
      (Rung0_Deterministic_B0, Quant_None_Deterministic, Offload_All_CPU_0, Inos_scp.Scp_projection.D2_Policy_Capability)
  | _ ->
      (* PREREG_T-Q Quantization Floor Check:
         Only drop to Q4_0 when RAM < 450 MB, and MANDATE D4_Idempotent_Transition guard! *)
      if available_ram_mb < 450 then
        (Rung_Emergency_Q4_0_D4Guarded, Quant_Q4_0_5bpw_Guarded, Offload_All_CPU_0, Inos_scp.Scp_projection.D4_Idempotent_Transition)
      else if available_ram_mb < 800 || (route = Inos_tonic.Tonic_gate.RouteDeterministic && risk < 0.30) then
        (Rung1_SmolLM2_135M_Q8_0, Quant_Q8_0_9bpw, Offload_All_CPU_0, Inos_scp.Scp_projection.D3_Epistemic_Grounding)
      else if available_ram_mb < 1500 then
        let offload = if gpu_vulkan_available then Offload_All_GPU_28 else Offload_All_CPU_0 in
        (Rung2_Qwen25_0_5B_Q6_K, Quant_Q6_K_6_625bpw, offload, Inos_scp.Scp_projection.D3_Epistemic_Grounding)
      else if route = Inos_tonic.Tonic_gate.RouteDualModelReview && available_ram_mb >= 3200 && ambiguity >= 0.55 then
        (* Escalate to 3B Oracle Verifier for high-ambiguity Dual-Model Review when RAM permits *)
        let offload = if gpu_vulkan_available then Offload_All_GPU_28 else Offload_All_CPU_0 in
        (Rung5_Qwen25_3B_Q6_K_Oracle, Quant_Q6_K_6_625bpw, offload, Inos_scp.Scp_projection.D4_Idempotent_Transition)
      else if available_ram_mb >= 1900 then
        (* Default Pareto Winner: Qwen3.5-2B-Q6_K with --reasoning off (59.2% strict accuracy, 1.70 GB) *)
        let offload = if gpu_vulkan_available then Offload_All_GPU_28 else Offload_All_CPU_0 in
        (Rung4_Qwen35_2B_Q6_K_NoThink, Quant_Q6_K_6_625bpw, offload, Inos_scp.Scp_projection.D4_Idempotent_Transition)
      else
        let offload = if gpu_vulkan_available then Offload_All_GPU_28 else Offload_All_CPU_0 in
        (Rung3_Qwen25_1_5B_Q6_K, Quant_Q6_K_6_625bpw, offload, Inos_scp.Scp_projection.D4_Idempotent_Transition)

let op_name_for_class (cls_id : int) : string =
  match cls_id with
  | 0 -> "create_document"
  | 1 -> "verify_receipts"
  | 2 -> "restore_snapshot"
  | 3 -> "run_audit"
  | _ -> "query_telemetry"

(* Extract target resource from prompt or fall back to observed workspace *)
let extract_target_from_prompt
    (prompt : string)
    (observed_files : Inos_scp.Scp_projection.observation_entry list) : string =
  let lower = String.lowercase_ascii prompt in
  if Inos_scp.Scp_projection.contains_sub_str lower "../" ||
     Inos_scp.Scp_projection.contains_sub_str lower "/etc/passwd" ||
     Inos_scp.Scp_projection.contains_sub_str lower "/root" then
    "/etc/passwd"
  else
    (* Look for explicit filename token with extension *)
    let words = String.split_on_char ' ' prompt in
    let clean_word w =
      let s = String.trim w in
      let len = String.length s in
      if len >= 2 && (s.[len - 1] = '.' || s.[len - 1] = ',' || s.[len - 1] = '"' || s.[len - 1] = '\'') then
        String.sub s 0 (len - 1)
      else s
    in
    let explicit =
      List.find_opt (fun w ->
        let cw = clean_word w in
        String.contains cw '.' && String.length cw >= 3
      ) words
    in
    match explicit with
    | Some w -> clean_word w
    | None ->
        (match observed_files with
         | first :: _ -> first.path
         | [] -> "workspace_note.txt")

let execute_optimized
    (eng : t)
    ~(prompt : string)
    ~(system_prefix : string)
    ~(observed_files : Inos_scp.Scp_projection.observation_entry list)
    ~(workspace_state_hash : string)
    ~(capability_epoch : int)
    ?(system_load = 0.25)
    ?(available_ram_mb = 2400)
    ?(gpu_vulkan_available = true)
    ?(allowed_classes = [0; 1; 2; 3; 4])
    () : execution_receipt =
  let t0 = Unix.gettimeofday () in
  eng.total_requests <- eng.total_requests + 1;

  (* 1. SCP Compact Algebraic Observation Projection *)
  let obs_stats = Inos_scp.Scp_projection.project_observation_compact ~epoch:capability_epoch observed_files in

  (* 2. Dynamic Regime Routing: Coupled Tonic Gate vs Stateless Lookup Clarity Bypass *)
  let (ambiguity, risk, _resource) = Inos_tonic.Tonic_gate.extract_features ~prompt ~system_load in
  let (next_cst, route_tier, mechanism, depth_budget, _clarity) =
    Inos_tonic.Tonic_gate.step_adaptive_engine ~cfg:eng.tonic_cfg eng.coupled_state ~prompt ~system_load
  in
  eng.coupled_state <- next_cst;
  (match mechanism with
   | Inos_tonic.Tonic_gate.Stateless_Lookup_Bypass -> eng.stateless_bypasses <- eng.stateless_bypasses + 1
   | Inos_tonic.Tonic_gate.Coupled_Tonic_Hold -> eng.tonic_holds <- eng.tonic_holds + 1
   | Inos_tonic.Tonic_gate.Safety_Clamp_Abstain -> ());

  (* 3. Select Model Rung, Quantization Format (with PREREG_T-Q floor check), and Offload Policy *)
  let (model_rung, quant_format, offload_policy, guard_rung) =
    select_model_and_quant ~route:route_tier ~ambiguity ~risk ~available_ram_mb ~gpu_vulkan_available
  in

  (* 4. Dual-Mode LoRA Hot-Swap Selection *)
  let (lora_adapter, lora_mode, swap_ns) =
    if model_rung = Rung0_Deterministic_B0 then
      (eng.lora_ctrl.adapters.(next_cst.active_domain), Neural_engine.LoRA_Disabled, 0.0)
    else
      Neural_engine.hot_swap_domain_lora
        eng.lora_ctrl
        ~domain_id:next_cst.active_domain
        ~hold_turns:next_cst.domain_hold_turns
  in
  (match lora_mode with
   | Neural_engine.LoRA_PreMerged_Fused -> eng.fused_lora_executions <- eng.fused_lora_executions + 1
   | Neural_engine.LoRA_Sidecar_Q8 -> eng.sidecar_lora_executions <- eng.sidecar_lora_executions + 1
   | Neural_engine.LoRA_Disabled -> ());

  (* 5. Safety Clamp Immediate Abstention Check *)
  if mechanism = Inos_tonic.Tonic_gate.Safety_Clamp_Abstain then begin
    eng.guard_aborts <- eng.guard_aborts + 1;
    let plan = {
      routing_mechanism = mechanism;
      route_tier;
      cache_tier = Cache_Tier3_Cold_Prefill;
      model_rung;
      quant_format;
      offload_policy;
      lora_adapter_name = lora_adapter.adapter_name;
      lora_mode;
      lora_swap_ns = swap_ns;
      guard_rung = Inos_scp.Scp_projection.D4_Idempotent_Transition;
      depth_budget = 1;
      reasoning_suppressed = true;
      observation_compression_ratio = obs_stats.compression_ratio;
    } in
    let t1 = Unix.gettimeofday () in
    {
      plan;
      approved_action = None;
      guard_status = "ABORTED_UNRESOLVED:TONIC_SAFETY_CLAMP_PRIVILEGE_OR_DESTRUCTIVE_RISK";
      veto_retried = false;
      classification_id = 3;
      confidence = 1.0;
      forward_flops = 0L;
      fnwd_mole_saved_flops = 0L;
      latency_ms = (t1 -. t0) *. 1000.0;
      auto_spawned_specialist = None;
      recalled_nnc_fact = None;
      recalled_memories = [];
      inos_rdt_trace = None;
    }
  end else begin
    let target = extract_target_from_prompt prompt observed_files in
    let q_emb = Lora_hub.compute_hidden_activation eng.base_model prompt in
    let resource_merkle_hashes =
      List.map (fun (f : Inos_scp.Scp_projection.observation_entry) ->
        (f.path, f.sha256_prefix)
      ) observed_files
    in
    (* 3-Tier Cognitive Memory Hierarchy Hybrid Retrieval (BM25 + Vector -> RRF + MMR) *)
    let recalled_memories =
      Inos_memory.Memory_hierarchy.retrieve_hybrid
        eng.lora_hub.mem_hierarchy
        ~query_embedding:q_emb
        ~top_k:2
        prompt
      |> List.map (fun (h : Inos_memory.Memory_hierarchy.hybrid_search_hit) -> h.text)
    in

    (* 6. Multi-Tier Cache Lookup:
       - Tier 1a: State-Verified NNC Exact Hit (0.0 ms, 0 FLOPs)
       - Tier 1b: Sparse-Merkle Associative NNC Hit (0.0 ms, 0 FLOPs across unrelated file edits & paraphrases)
       - Tier 2:  Prefix KV Cache Reuse (~68% prefill FLOP savings)
       - Tier 3:  Cold Prefill (with INOS FNWD Recurrent Tie-Breaker & Single-Pass Shared-Activation MoLE telemetry) *)
    let (raw_logits, forward_flops, fnwd_mole_saved_flops, cache_tier, recalled_nnc_fact, inos_rdt_trace) =
      match Inos_nnc.Nnc_cache.lookup eng.nnc ~prompt ~workspace_state_hash ~capability_epoch with
      | Inos_nnc.Nnc_cache.CacheHit entry ->
          eng.nnc_hits <- eng.nnc_hits + 1;
          (entry.logits, 0L, 0L, Cache_Tier1_NNC_Exact_0ms, entry.distilled_fact, None)
      | _ ->
          let assoc_res =
            Inos_nnc.Nnc_cache.recall_associative eng.nnc
              ~prompt
              ~query_embedding:q_emb
              ~workspace_state_hash
              ~capability_epoch
              ~resource_merkle_hashes ()
          in
          match assoc_res with
          | Inos_nnc.Nnc_cache.AssociativeHit hit
            when hit.sparse_merkle_survived &&
                 hit.combined_score >= 0.72 &&
                 hit.entry.classification_id = next_cst.active_domain &&
                 hit.entry.target_resource = Some target ->
              eng.nnc_associative_hits <- eng.nnc_associative_hits + 1;
              (hit.entry.logits, 0L, 0L, Cache_Tier1b_NNC_Associative_Merkle, Some hit.recalled_fact, None)
          | _ ->
              let passive_fact =
                match assoc_res with
                | Inos_nnc.Nnc_cache.AssociativeHit hit when hit.sparse_merkle_survived ->
                    Some hit.recalled_fact
                | _ -> None
              in
              let prefix_hit =
                eng.last_system_prefix_hash <> "" && eng.last_system_prefix_hash = system_prefix
              in
              eng.last_system_prefix_hash <- system_prefix;
              let c_tier =
                if prefix_hit then begin
                  eng.prefix_kv_hits <- eng.prefix_kv_hits + 1;
                  Cache_Tier2_Prefix_KV_Reuse
                end else
                  Cache_Tier3_Cold_Prefill
              in
              let tokens = Neural_engine.tokenize prompt in
              let rdt_budget = max 3 depth_budget in
              let (logits, metrics, rdt_trace) =
                Neural_engine.forward_inos_rdt
                  eng.base_model
                  ~adapter:lora_adapter
                  ~mode:lora_mode
                  ~max_loops:rdt_budget
                  ~act_halt_threshold:0.82
                  tokens
              in
              (* Check if multi-specialist cross-app workflow activates FNWD Single-Pass MoLE *)
              let (hub_blend, hub_mode) = Lora_hub.select_topk_specialists eng.lora_hub prompt ~max_k:3 in
              let fnwd_saved =
                if hub_mode = "MoLE_TopK_Subspace_Blend" && List.length hub_blend >= 2 then
                  let pairs =
                    List.map (fun (e : Lora_hub.mole_blend_entry) ->
                      (e.specialist.neural_lora, e.weight)
                    ) hub_blend
                  in
                  let (_mole_logits, _mole_metrics, saved) =
                    Neural_engine.forward_logits_mole_fnwd eng.base_model pairs tokens
                  in
                  saved
                else 0L
              in
              (* Apply Prefix KV Cache prefill savings (~68% FLOP reduction on shared system prefix) *)
              let adjusted_flops =
                if prefix_hit then Int64.of_float (Int64.to_float metrics.total_flops *. 0.32)
                else metrics.total_flops
              in
              (logits, adjusted_flops, fnwd_saved, c_tier, passive_fact, Some rdt_trace)
    in

    (* 7. SCP Logit Subspace Projection *)
    let projected = Inos_scp.Scp_projection.project_logits ~cfg:eng.scp_cfg ~allowed:allowed_classes raw_logits in
    let probs = Inos_scp.Scp_projection.softmax projected in
    let best_cls = ref 0 in
    let best_prob = ref probs.(0) in
    for i = 1 to Array.length probs - 1 do
      if probs.(i) > !best_prob then begin
        best_prob := probs.(i);
        best_cls := i
      end
    done;

    (* 8. D1-D4 Deterministic Guard Ladder with 1-Retry Typed Veto *)
    let initial_candidate : Inos_scp.Scp_projection.typed_action = {
      op = op_name_for_class !best_cls;
      target;
      arg = Some obs_stats.compact_payload;
    } in
    let verdict =
      Inos_scp.Scp_projection.evaluate_guard_ladder
        ~max_rung:guard_rung
        ~observed_files
        ~last_committed_action:eng.last_committed_action
        initial_candidate
    in
    let (approved_action, guard_status, veto_retried) =
      match verdict with
      | Inos_scp.Scp_projection.Guard_Approved (act, _) ->
          eng.last_committed_action <- Some (act.op, act.target);
          (* Only insert into NNC & 3-Tier Memory Hierarchy after D4 deterministic guard verification! *)
          if cache_tier <> Cache_Tier1_NNC_Exact_0ms &&
             cache_tier <> Cache_Tier1b_NNC_Associative_Merkle then begin
            let target_merkle_hash =
              match List.find_opt (fun (f : Inos_scp.Scp_projection.observation_entry) -> f.path = act.target) observed_files with
              | Some f -> f.sha256_prefix
              | None -> ""
            in
            let fact_str =
              Printf.sprintf "@nnc_fact[%s]{op:\"%s\",epoch:%d}" act.target act.op capability_epoch
            in
            ignore (Inos_nnc.Nnc_cache.insert eng.nnc
              ~hidden_embedding:q_emb
              ~distilled_fact:fact_str
              ~merkle_resource_hash:target_merkle_hash
              ~prompt ~workspace_state_hash ~capability_epoch
              ~logits:raw_logits ~classification_id:!best_cls
              ~target_resource:(Some act.target) ~verified:true);
            match Inos_memory.Memory_hierarchy.ingest_turn eng.lora_hub.mem_hierarchy
                    ~salience:!best_prob
                    ~embedding:q_emb
                    ~wm_id:act.target
                    ~content:(Printf.sprintf "%s -> %s(%s)" prompt act.op act.target) () with
            | Some _ -> eng.memory_consolidations <- eng.memory_consolidations + 1
            | None -> ()
          end;
          (Some act, "APPROVED_D4_VERIFIED", false)
      | Inos_scp.Scp_projection.Guard_Retry_Veto (_rung, reason, Some repaired) ->
          eng.guard_veto_repairs <- eng.guard_veto_repairs + 1;
          (* Re-verify repaired candidate through full D4 ladder (1-retry cap) *)
          (match Inos_scp.Scp_projection.evaluate_guard_ladder
                   ~max_rung:guard_rung
                   ~observed_files
                   ~last_committed_action:eng.last_committed_action
                   repaired with
           | Inos_scp.Scp_projection.Guard_Approved (act2, _) ->
               eng.last_committed_action <- Some (act2.op, act2.target);
               (Some act2, "APPROVED_AFTER_1_RETRY_" ^ reason, true)
           | _ ->
               eng.guard_aborts <- eng.guard_aborts + 1;
               (None, "ABORTED_UNRESOLVED_AFTER_RETRY_" ^ reason, true))
      | Inos_scp.Scp_projection.Guard_Retry_Veto (_rung, reason, None)
      | Inos_scp.Scp_projection.Guard_Aborted_Unresolved (_rung, reason) ->
          eng.guard_aborts <- eng.guard_aborts + 1;
          (None, reason, false)
    in
    (* 9. Autonomous Adapter Creation when accuracy/domain coverage is insufficient *)
    let auto_spawned_specialist =
      if cache_tier <> Cache_Tier1_NNC_Exact_0ms &&
         cache_tier <> Cache_Tier1b_NNC_Associative_Merkle &&
         eng.lora_hub.auto_spawn_enabled &&
         (!best_prob < eng.lora_hub.auto_spawn_confidence_threshold || veto_retried) then begin
        let hub_res = Lora_hub.execute_with_hub eng.lora_hub prompt in
        match hub_res.auto_spawn_event with
        | Some ev ->
            eng.auto_spawned_adapters <- eng.auto_spawned_adapters + 1;
            if hub_res.confidence > !best_prob then best_prob := hub_res.confidence;
            Some ev.spawned_adapter_id
        | None -> None
      end else None
    in
    let plan = {
      routing_mechanism = mechanism;
      route_tier;
      cache_tier;
      model_rung;
      quant_format;
      offload_policy;
      lora_adapter_name = lora_adapter.adapter_name;
      lora_mode;
      lora_swap_ns = swap_ns;
      guard_rung;
      depth_budget;
      reasoning_suppressed = (model_rung = Rung4_Qwen35_2B_Q6_K_NoThink);
      observation_compression_ratio = obs_stats.compression_ratio;
    } in
    let t1 = Unix.gettimeofday () in
    {
      plan;
      approved_action;
      guard_status;
      veto_retried;
      classification_id = !best_cls;
      confidence = !best_prob;
      forward_flops;
      fnwd_mole_saved_flops;
      latency_ms = (t1 -. t0) *. 1000.0;
      auto_spawned_specialist;
      recalled_nnc_fact;
      recalled_memories;
      inos_rdt_trace;
    }
  end


(* ==========================================================================
   Natural-Language-to-Binary-ABI Lowering (Spool LBA 24576 + AutomationIpcWire)
   Compiles high-level natural language intent through Optimizing_engine + Lora_hub
   directly into:
     1. 128-byte binary `AutomationIpcWire` frames (magic = 0xAA01, version = 1)
     2. 512-byte binary `INT_` transport spool sectors (magic = "INT_", LBA 24576)
   ========================================================================== *)

type app_spool_dispatch_packet = {
  receipt : execution_receipt;
  app_actions : Lora_hub.app_usability_action list;
  intscript_program : compiled_intscript_program;
  automation_ipc_frames : bytes list; (* 128-byte #[repr(C)] AutomationIpcWire frames *)
  spool_sectors_512b : bytes list;    (* 512-byte INT_ LBA 24576 spool sectors *)
  provenance_sha256 : string;
  prompt_sha256 : string;
}

let encode_script_isolation_context_576b
    ~(script_id_hex : string)
    ~(allowed_paths : string list)
    ~(permissions_mask : int) : bytes =
  (* Matches #[repr(C)] pub struct ScriptIsolationContext (576-byte binary wire layout):
     - script_id:     [u8; 32]       (offset 0..31)
     - allowed_paths: [[u8; 64]; 8]  (offset 32..543)
     - path_count:    u32            (offset 544..547)
     - _pad:          u32            (offset 548..551)
     - permissions:   u64            (offset 552..559)
     - _reserved:     [u8; 16]       (offset 560..575) *)
  let buf = Bytes.make 576 '\x00' in
  let sid_padded = String.sub (script_id_hex ^ String.make 32 '0') 0 32 in
  Bytes.blit_string sid_padded 0 buf 0 32;
  let path_count = min 8 (List.length allowed_paths) in
  List.iteri (fun idx p ->
    if idx < 8 then begin
      let p_len = min 63 (String.length p) in
      Bytes.blit_string p 0 buf (32 + idx * 64) p_len
    end
  ) allowed_paths;
  Bytes.set buf 544 (Char.chr (path_count land 0xFF));
  Bytes.set buf 545 (Char.chr ((path_count lsr 8) land 0xFF));
  Bytes.set buf 546 (Char.chr ((path_count lsr 16) land 0xFF));
  Bytes.set buf 547 (Char.chr ((path_count lsr 24) land 0xFF));
  let perm64 = Int64.of_int permissions_mask in
  for b = 0 to 7 do
    let byte_v = Int64.to_int (Int64.logand (Int64.shift_right_logical perm64 (b * 8)) 0xFFL) in
    Bytes.set buf (552 + b) (Char.chr byte_v)
  done;
  buf

let compile_intent_to_intscript
    (eng : t)
    ~(prompt : string)
    ~(app_actions : Lora_hub.app_usability_action list)
    ~(workspace_state_hash : string)
    ~(capability_epoch : int) () : compiled_intscript_program =
  let norm_prompt = String.trim (String.lowercase_ascii prompt) in
  let cache_key = (norm_prompt, workspace_state_hash, capability_epoch) in
  match Hashtbl.find_opt eng.intscript_cache cache_key with
  | Some cached ->
      { cached with cached_in_nnc = true }
  | None ->
      let explicit_target = extract_target_from_prompt prompt [] in
      let has_explicit_target = explicit_target <> "" && explicit_target <> "workspace_note.txt" in
      let base_paths = if has_explicit_target then [ explicit_target ] else [] in
      let allowed_paths =
        List.fold_left (fun acc (a : Lora_hub.app_usability_action) ->
          let t = String.trim a.target_artifact in
          if t <> "" && not (List.mem t acc) then acc @ [t] else acc
        ) base_paths app_actions
      in
      let permissions_mask =
        if app_actions = [] && not has_explicit_target then 0
        else 1 lor 2 lor 4 lor 16 (* READ=1 | WRITE=2 | CREATE=4 | IPC=16 *)
      in
      let src_buf = Buffer.create 512 in
      Buffer.add_string src_buf
        (Printf.sprintf "// IntScript (.ints) Compiled Intent Program [epoch=%d, ws=%s]\n"
           capability_epoch workspace_state_hash);
      if app_actions = [] then begin
        if has_explicit_target then begin
          Buffer.add_string src_buf (Printf.sprintf "const wb = inos.grid.open(\"%s\");\n" explicit_target);
          Buffer.add_string src_buf "const sheet = wb.getSheet(\"Sheet1\");\n";
          Buffer.add_string src_buf "sheet.setFormula(\"B2\", \"=SUM(B2:B4)\");\n";
          Buffer.add_string src_buf "const val = sheet.evaluate(\"B2\");\n";
          Buffer.add_string src_buf "if (val > 0) {\n";
          Buffer.add_string src_buf (Printf.sprintf "  inos.grid.save(\"%s\");\n" explicit_target);
          Buffer.add_string src_buf "}\n";
          Buffer.add_string src_buf "inos.audit.log(\"INTSCRIPT_COMMIT_OK\");\n"
        end else
          Buffer.add_string src_buf "inos.audit.log(\"ABORTED_UNRESOLVED\");\n"
      end else begin
        let total_acts = List.length app_actions in
        List.iteri (fun idx (act : Lora_hub.app_usability_action) ->
          let tgt =
            if idx = 0 && has_explicit_target then explicit_target
            else act.target_artifact
          in
          let app_str = Lora_hub.app_id_to_string act.app in
          match act.app with
          | Lora_hub.App_Grid | Lora_hub.App_Calc ->
              let formula =
                if Inos_scp.Scp_projection.contains_sub_str norm_prompt "=sum(" then "=SUM(B2:B4)"
                else "=SUM(B2:B4)"
              in

              Buffer.add_string src_buf (Printf.sprintf "const wb = inos.grid.open(\"%s\");\n" tgt);
              Buffer.add_string src_buf "const sheet = wb.getSheet(\"Sheet1\");\n";
              Buffer.add_string src_buf (Printf.sprintf "sheet.setFormula(\"B2\", \"%s\");\n" formula);
              Buffer.add_string src_buf "const val = sheet.evaluate(\"B2\");\n";
              Buffer.add_string src_buf "if (val > 0) {\n";
              Buffer.add_string src_buf (Printf.sprintf "  inos.grid.save(\"%s\");\n" tgt);
              if idx + 1 < total_acts then begin
                let next_act = List.nth app_actions (idx + 1) in
                Buffer.add_string src_buf
                  (Printf.sprintf "  inos.ribbon.stage(\"%s\", \"%s\", \"B2\", val);\n"
                     tgt next_act.target_artifact)
              end;
              Buffer.add_string src_buf "}\n"
          | Lora_hub.App_Write | Lora_hub.App_Notes ->
              Buffer.add_string src_buf (Printf.sprintf "const doc = inos.write.createDocument(\"%s\");\n" tgt);
              Buffer.add_string src_buf
                (Printf.sprintf "inos.files.write(\"%s\", \"%s\");\n" tgt act.operation_verb);
              Buffer.add_string src_buf
                (Printf.sprintf "inos.ipc.call(\"org.inos.%s\", \"%s\", \"%s\");\n"
                   app_str act.operation_verb tgt)
          | Lora_hub.App_Pdf ->
              Buffer.add_string src_buf
                (Printf.sprintf "inos.ipc.call(\"org.inos.pdf_app\", \"%s\", \"%s\");\n"
                   act.operation_verb tgt);
              Buffer.add_string src_buf
                (Printf.sprintf "inos.files.write(\"%s\", \"PDF_1_7_XREF_EXPORT\");\n" tgt)
          | _ ->
              Buffer.add_string src_buf
                (Printf.sprintf "inos.files.write(\"%s\", \"%s\");\n" tgt act.operation_verb);
              Buffer.add_string src_buf
                (Printf.sprintf "inos.ipc.call(\"org.inos.%s\", \"%s\", \"%s\");\n"
                   app_str act.operation_verb tgt)
        ) app_actions;
        Buffer.add_string src_buf "inos.audit.log(\"INTSCRIPT_COMMIT_OK\");\n"
      end;
      let intscript_source = Buffer.contents src_buf in
      let script_id_hex =
        Inos_ipc.Sha256.digest_string
          (Printf.sprintf "INTS|%s|%s|%d" intscript_source workspace_state_hash capability_epoch)
      in
      let isolation_context_576b =
        encode_script_isolation_context_576b ~script_id_hex ~allowed_paths ~permissions_mask
      in
      let prog = {
        script_id_hex;
        intscript_source;
        allowed_paths;
        permissions_mask;
        isolation_context_576b;
        cached_in_nnc = false;
      } in
      if app_actions <> [] || has_explicit_target then
        Hashtbl.replace eng.intscript_cache cache_key prog;
      prog


let opcode_for_app_action (act : Lora_hub.app_usability_action) : int =
  match act.app with
  | Lora_hub.App_Grid | Lora_hub.App_Calc -> 5 (* OP_EVALUATE = 5 *)
  | Lora_hub.App_Files -> 2                    (* OP_OPEN = 2 *)
  | _ -> 6                                     (* OP_SAVE = 6 *)

let serialize_automation_ipc_wire
    ~(request_id : int)
    ~(op_code : int)
    ~(status_code : int)
    ?(cell = "A1")
    ?(value_num = 0L)
    ~(formula : string)
    ~(target_file : string)
    ?(error_msg = "") () : bytes =
  let buf = Bytes.make 128 '\x00' in
  (* magic: u16 = 0xAA01 (little-endian: 0x01, 0xAA) *)
  Bytes.set buf 0 '\x01';
  Bytes.set buf 1 '\xAA';
  (* version: u16 = 1 *)
  Bytes.set buf 2 '\x01';
  Bytes.set buf 3 '\x00';
  (* request_id: u32 (offset 4..7) *)
  Bytes.set buf 4 (Char.chr (request_id land 0xFF));
  Bytes.set buf 5 (Char.chr ((request_id lsr 8) land 0xFF));
  Bytes.set buf 6 (Char.chr ((request_id lsr 16) land 0xFF));
  Bytes.set buf 7 (Char.chr ((request_id lsr 24) land 0xFF));
  (* op_code: u16 (offset 8..9) *)
  Bytes.set buf 8 (Char.chr (op_code land 0xFF));
  Bytes.set buf 9 (Char.chr ((op_code lsr 8) land 0xFF));
  (* status_code: u16 (offset 10..11) *)
  Bytes.set buf 10 (Char.chr (status_code land 0xFF));
  Bytes.set buf 11 (Char.chr ((status_code lsr 8) land 0xFF));
  (* cell: [u8; 8] (offset 16..23) *)
  let cell_len = min 8 (String.length cell) in
  Bytes.blit_string cell 0 buf 16 cell_len;
  (* value_num: i64 (offset 24..31) *)
  for b = 0 to 7 do
    let byte_v = Int64.to_int (Int64.logand (Int64.shift_right_logical value_num (b * 8)) 0xFFL) in
    Bytes.set buf (24 + b) (Char.chr byte_v)
  done;
  (* formula: [u8; 32] (offset 32..63) *)
  let form_len = min 32 (String.length formula) in
  Bytes.blit_string formula 0 buf 32 form_len;
  (* target_file: [u8; 32] (offset 64..95) *)
  let tf_len = min 32 (String.length target_file) in
  Bytes.blit_string target_file 0 buf 64 tf_len;
  (* error_msg: [u8; 32] (offset 96..127) *)
  let err_len = min 32 (String.length error_msg) in
  Bytes.blit_string error_msg 0 buf 96 err_len;
  buf

let encode_int_spool_sectors_512b
    ~(filename : string)
    ~(payload : string)
    ~(provenance_hash : string)
    ~(prompt_hash : string)
    ~(is_ambiguous : bool) : bytes list =
  let block_size = 512 in
  let header = Bytes.make block_size '\x00' in
  Bytes.set header 0 'I';
  Bytes.set header 1 'N';
  Bytes.set header 2 'T';
  Bytes.set header 3 '_';
  Bytes.set header 4 '\x01';
  Bytes.set header 5 '\x00';
  Bytes.set header 6 '\x01';
  Bytes.set header 7 '\x00';
  let fn_len = min 32 (String.length filename) in
  Bytes.set header 8 (Char.chr (fn_len land 0xFF));
  Bytes.set header 9 (Char.chr ((fn_len lsr 8) land 0xFF));
  Bytes.blit_string filename 0 header 10 fn_len;
  let payload_len = String.length payload in
  Bytes.set header 42 (Char.chr (payload_len land 0xFF));
  Bytes.set header 43 (Char.chr ((payload_len lsr 8) land 0xFF));
  Bytes.set header 44 (Char.chr ((payload_len lsr 16) land 0xFF));
  Bytes.set header 45 (Char.chr ((payload_len lsr 24) land 0xFF));
  let sectors_needed = if payload_len = 0 then 0 else (payload_len + block_size - 1) / block_size in
  Bytes.set header 46 (Char.chr (sectors_needed land 0xFF));
  Bytes.set header 47 (Char.chr ((sectors_needed lsr 8) land 0xFF));
  let prov_bytes = String.sub (provenance_hash ^ String.make 32 '0') 0 32 in
  Bytes.blit_string prov_bytes 0 header 48 32;
  let prompt_bytes = String.sub (prompt_hash ^ String.make 32 '0') 0 32 in
  Bytes.blit_string prompt_bytes 0 header 80 32;
  Bytes.set header 112 (if is_ambiguous then '\x01' else '\x00');
  let sector_blocks = ref [header] in
  for s = 0 to sectors_needed - 1 do
    let blk = Bytes.make block_size '\x00' in
    let start_pos = s * block_size in
    let chunk_len = min block_size (payload_len - start_pos) in
    Bytes.blit_string payload start_pos blk 0 chunk_len;
    sector_blocks := blk :: !sector_blocks
  done;
  List.rev !sector_blocks

let dispatch_to_app_spool
    (eng : t)
    ~(prompt : string)
    ~(system_prefix : string)
    ~(observed_files : Inos_scp.Scp_projection.observation_entry list)
    ~(workspace_state_hash : string)
    ~(capability_epoch : int)
    ?(available_ram_mb = 2400)
    ?(gpu_vulkan_available = true) () : app_spool_dispatch_packet =
  let receipt =
    execute_optimized eng
      ~prompt
      ~system_prefix
      ~observed_files
      ~workspace_state_hash
      ~capability_epoch
      ~available_ram_mb
      ~gpu_vulkan_available ()
  in
  let hub_res = Lora_hub.execute_with_hub eng.lora_hub prompt in
  let app_actions =
    match receipt.approved_action with
    | None -> []
    | Some _ -> hub_res.app_actions
  in
  let intscript_program =
    compile_intent_to_intscript eng
      ~prompt
      ~app_actions
      ~workspace_state_hash
      ~capability_epoch ()
  in
  let automation_ipc_frames =
    List.mapi (fun idx (act : Lora_hub.app_usability_action) ->
      serialize_automation_ipc_wire
        ~request_id:(eng.total_requests * 10 + idx + 1)
        ~op_code:(opcode_for_app_action act)
        ~status_code:0
        ~cell:"B2"
        ~value_num:0L
        ~formula:act.native_ipc_or_format_payload
        ~target_file:act.target_artifact ()
    ) app_actions
  in
  let (target_filename, combined_payload, is_ambiguous) =
    match receipt.approved_action, app_actions with
    | Some approved, first_app :: _ ->
        let payload_str =
          String.concat "\n---\n"
            (List.map (fun (a : Lora_hub.app_usability_action) ->
              Printf.sprintf "[%s:%s->%s] %s"
                a.adapter_id a.operation_verb a.target_artifact a.native_ipc_or_format_payload
            ) app_actions)
        in
        let fn =
          if approved.target <> "" && approved.target <> "workspace_note.txt" then approved.target
          else first_app.target_artifact
        in
        (fn, payload_str ^ "\n===INTSCRIPT===\n" ^ intscript_program.intscript_source, false)
    | Some approved, [] ->
        (approved.target, Option.value ~default:"" approved.arg, false)
    | None, _ ->
        ("halted_unresolved.txt", receipt.guard_status, true)
  in
  let provenance_sha256 = Inos_ipc.Sha256.digest_string combined_payload in
  let prompt_sha256 = Inos_ipc.Sha256.digest_string prompt in
  let spool_sectors_512b =
    encode_int_spool_sectors_512b
      ~filename:target_filename
      ~payload:combined_payload
      ~provenance_hash:provenance_sha256
      ~prompt_hash:prompt_sha256
      ~is_ambiguous
  in
  {
    receipt;
    app_actions;
    intscript_program;
    automation_ipc_frames;
    spool_sectors_512b;
    provenance_sha256;
    prompt_sha256;
  }

(* ==========================================================================
   3-Gate Scoped Cross-App Continuity & Staged Chrome Ribbon Filter
   Prevents unsolicited cross-domain bleed (e.g., never pushing spreadsheet
   grid values into a family photo being edited in canvas_app!) while preserving:
     1. Standard user-controlled Clipboard Copy/Paste (Ctrl+C / Ctrl+V)
     2. Calm Staged Chrome Ribbon Offers for semantically related artifacts
     3. Live Auto-Sync ONLY for artifact pairs explicitly bound by the user
   ========================================================================== *)

type continuity_decision =
  | Continuity_Blocked_Unrelated_Context of {
      reason : string;
      affinity_score : float;
    }
  | Continuity_Staged_Ribbon_Offer of {
      ribbon_badge : string;
      ribbon_prompt : string;
      affinity_score : float;
      wire_frame : bytes;
    }
  | Continuity_Bound_Auto_Sync of {
      ribbon_badge : string;
      ribbon_status : string;
      affinity_score : float;
      wire_frame : bytes;
    }

let explicit_continuity_bindings : (string * string, bool) Hashtbl.t = Hashtbl.create 32

let bind_cross_app_link ~(src_artifact : string) ~(dst_artifact : string) : unit =
  Hashtbl.replace explicit_continuity_bindings
    (String.lowercase_ascii src_artifact, String.lowercase_ascii dst_artifact) true

let unbind_cross_app_link ~(src_artifact : string) ~(dst_artifact : string) : unit =
  Hashtbl.remove explicit_continuity_bindings
    (String.lowercase_ascii src_artifact, String.lowercase_ascii dst_artifact)

let is_cross_app_link_bound ~(src_artifact : string) ~(dst_artifact : string) : bool =
  Option.value ~default:false
    (Hashtbl.find_opt explicit_continuity_bindings
       (String.lowercase_ascii src_artifact, String.lowercase_ascii dst_artifact))

let extract_stem_tokens (text : string) : string list =
  let lower = String.lowercase_ascii text in
  let buf = Buffer.create 16 in
  let acc = ref [] in
  let flush () =
    if Buffer.length buf >= 2 then begin
      let w = Buffer.contents buf in
      if w <> "txt" && w <> "md" && w <> "pdf" && w <> "xlsx" && w <> "svg" &&
         w <> "png" && w <> "jpg" && w <> "ics" && w <> "eml" && w <> "zip" &&
         w <> "the" && w <> "and" && w <> "for" && w <> "in" && w <> "to" then
        acc := w :: !acc
    end;
    Buffer.clear buf
  in
  String.iter (fun c ->
    if (c >= 'a' && c <= 'z') || (c >= '0' && c <= '9') then
      Buffer.add_char buf c
    else
      flush ()
  ) lower;
  flush ();
  List.sort_uniq String.compare !acc

let evaluate_cross_app_continuity_gate
    (eng : t)
    ?(dst_surface_isolated = false)
    ~(src_artifact : string)
    ~(src_context : string)
    ~(src_cell : string)
    ~(src_value_num : int64)
    ~(src_formula : string)
    ~(dst_artifact : string)
    ~(dst_context : string) () : continuity_decision =
  (* Gate 1: Respect explicit surface isolation (Local / Clipboard-Only Mode) *)
  if dst_surface_isolated then
    Continuity_Blocked_Unrelated_Context {
      reason = "DESTINATION_SURFACE_ISOLATED_LOCAL_MODE";
      affinity_score = 0.0;
    }
  else begin
    (* Gate 2: Semantic Project/Entity Affinity Check *)
    let src_tokens = extract_stem_tokens (src_artifact ^ " " ^ src_context) in
    let dst_tokens = extract_stem_tokens (dst_artifact ^ " " ^ dst_context) in
    let shared_tokens =
      List.filter (fun t -> List.mem t dst_tokens) src_tokens
    in
    let lexical_overlap =
      match src_tokens, dst_tokens with
      | [], _ | _, [] -> 0.0
      | _ ->
          float_of_int (List.length shared_tokens) /.
          float_of_int (max 1 (min (List.length src_tokens) (List.length dst_tokens)))
    in
    let emb_src = Lora_hub.compute_hidden_activation eng.base_model (src_artifact ^ " " ^ src_context) in
    let emb_dst = Lora_hub.compute_hidden_activation eng.base_model (dst_artifact ^ " " ^ dst_context) in
    let subspace_cos = Inos_nnc.Nnc_cache.cosine_similarity emb_src emb_dst in
    let affinity =
      if shared_tokens = [] then
        (* Without any shared project/entity stem, cap affinity below threshold
           so unrelated work (e.g., q3_budget.xlsx vs family_photo.svg) NEVER bleeds across! *)
        min 0.25 (subspace_cos *. 0.30)
      else
        min 1.0 ((0.65 *. lexical_overlap) +. (0.35 *. max 0.0 subspace_cos))
    in
    if affinity < 0.45 then
      Continuity_Blocked_Unrelated_Context {
        reason = Printf.sprintf "UNRELATED_TASK_CONTEXT_BLOCKED(affinity=%.2f<0.45)" affinity;
        affinity_score = affinity;
      }
    else begin
      let wire_frame =
        serialize_automation_ipc_wire
          ~request_id:(eng.total_requests + 100)
          ~op_code:4 (* OP_SET_CELL / SYNC_VALUE *)
          ~status_code:0
          ~cell:src_cell
          ~value_num:src_value_num
          ~formula:src_formula
          ~target_file:dst_artifact ()
      in
      (* Gate 3: Staged Chrome Ribbon Offer by Default vs. Bound Auto-Sync *)
      if is_cross_app_link_bound ~src_artifact ~dst_artifact then
        Continuity_Bound_Auto_Sync {
          ribbon_badge = "LIVE BOUND";
          ribbon_status =
            Printf.sprintf "%s[%s=%Ld] <-> %s (Auto-Synced | Click to Unbind)"
              src_artifact src_cell src_value_num dst_artifact;
          affinity_score = affinity;
          wire_frame;
        }
      else
        Continuity_Staged_Ribbon_Offer {
          ribbon_badge = "LINK AVAILABLE";
          ribbon_prompt =
            Printf.sprintf "%s[%s=%Ld] matches %s (Tab/Click Ribbon to Bind | Esc: Ignore)"
              src_artifact src_cell src_value_num dst_artifact;
          affinity_score = affinity;
          wire_frame;
        }
    end
  end

(* ==========================================================================
   FNWD V0.4 Enabler-Optimizer Composition (E (x) O) & Failure-Boundary Engine
   Applies Enabler-Optimizer Composition sparingly where mathematically sound
   and net-positive, while enforcing all 3 empirically discovered boundaries:
     1. Cost-of-Normalization Trap (blocks O(N) KV tree rebalancing on small N)
     2. Contention Hotspot Fallback (serializes conflicting writes on R_obs)
     3. Causal Horizon Guard (blocks out-of-order tombstone compaction)
   ========================================================================== *)

let fnwd_modulus = 4093

type fnwd_tx_op = {
  tx_id : int;
  read_regs : int list;
  write_regs : int list;
  delta_payload : int;
  causal_epoch : int;
}

type fnwd_execution_regime =
  | Promoted_Enabler_Optimizer_E_x_O
  | Fallback_NormalizationCostTrap of { norm_cost_us : float; lock_saved_us : float }
  | Fallback_ContentionHotspot of { conflicting_reg : int }
  | Fallback_CausalHorizonViolation of { regressed_epoch : int; horizon_epoch : int }
  | Rejected_Corrupt_Normalizer of { reason : string }

type fnwd_batch_execution_report = {
  regime : fnwd_execution_regime;
  baseline_locks : int;
  optimized_locks : int;
  lock_reduction_pct : float;
  trace_equivalent : bool;
  observable_state_preserved : bool;
  retraction_idempotent : bool;
  manifold_stable : bool;
  auxiliary_drift_cleared : bool;
  final_observable_regs : int array;
}

(* Batched CRDT OR-Set Tombstone Compaction across Engine & Hub NNC Caches:
   Stages K file mutations in O(1) without locking/scanning, then runs 1 retraction pass *)
let invalidate_workspace_resources_batch
    (eng : t)
    ?(causal_epoch = 0)
    (mutations : (string * string) list) : int =
  List.iter (fun (resource_path, new_resource_hash) ->
    Inos_nnc.Nnc_cache.stage_tombstone_invalidation
      eng.nnc ~resource_path ~new_resource_hash ~causal_epoch ();
    Inos_nnc.Nnc_cache.stage_tombstone_invalidation
      eng.lora_hub.nnc_memory ~resource_path ~new_resource_hash ~causal_epoch ()
  ) mutations;
  let c1 = Inos_nnc.Nnc_cache.retract_tombstones_crdt eng.nnc in
  let c2 = Inos_nnc.Nnc_cache.retract_tombstones_crdt eng.lora_hub.nnc_memory in
  c1 + c2

let fnwd_observable_indices = [| 0; 1; 3; 4; 5; 6; 7 |]
let fnwd_auxiliary_index = 2

let project_observable_regs (regs : int array) : int array =
  Array.map (fun idx -> regs.(idx)) fnwd_observable_indices

let valid_auxiliary_retraction (regs : int array) : int array =
  (* Manifold Stability fast-path: if R_2 == 0, return copy without mutation *)
  let out = Array.copy regs in
  out.(fnwd_auxiliary_index) <- 0;
  out

let verify_retraction_invariants
    (retract_fn : int array -> int array) : bool * bool * bool =
  let test_vectors = [|
    [| 101; 202; 555; 303; 404; 505; 606; 707 |];
    [| 0;   42;  0;   19;  88;  1024; 7;  4092 |];
    [| 999; 111; 4091; 222; 333; 444; 555; 666 |];
  |] in
  let idemp = ref true in
  let obs_pres = ref true in
  let man_stable = ref true in
  Array.iter (fun st ->
    let r1 = retract_fn st in
    let r2 = retract_fn r1 in
    if r1 <> r2 then idemp := false;
    if project_observable_regs r1 <> project_observable_regs st then obs_pres := false;
    let st_on_m = Array.copy st in
    st_on_m.(fnwd_auxiliary_index) <- 0;
    if retract_fn st_on_m <> st_on_m then man_stable := false
  ) test_vectors;
  (!idemp, !obs_pres, !man_stable)

let execute_single_tx (regs : int array) (tx : fnwd_tx_op) : int array * int =
  let next = Array.copy regs in
  let read_sum =
    List.fold_left (fun acc r ->
      if r >= 0 && r < Array.length next && r <> fnwd_auxiliary_index then
        (acc + next.(r)) mod fnwd_modulus
      else acc
    ) 0 tx.read_regs
  in
  let obs_out = ((read_sum + tx.delta_payload) mod fnwd_modulus + fnwd_modulus) mod fnwd_modulus in
  List.iter (fun w ->
    if w >= 0 && w < Array.length next then
      if w = fnwd_auxiliary_index then
        next.(w) <- (next.(w) + tx.delta_payload + 1) mod fnwd_modulus
      else
        next.(w) <- (next.(w) + obs_out) mod fnwd_modulus
  ) tx.write_regs;
  (next, obs_out)

let find_observable_contention_hotspot (txs : fnwd_tx_op list) : int option =
  (* Check if 2 or more transactions in the same batch write to the same observable register
     or have a Read-After-Write / Write-After-Read hazard on an observable register *)
  let rec check_pairs = function
    | [] -> None
    | t1 :: rest ->
        let conflict =
          List.find_map (fun t2 ->
            let w1_obs = List.filter (fun r -> r <> fnwd_auxiliary_index) t1.write_regs in
            let w2_obs = List.filter (fun r -> r <> fnwd_auxiliary_index) t2.write_regs in
            let r1_obs = List.filter (fun r -> r <> fnwd_auxiliary_index) t1.read_regs in
            let r2_obs = List.filter (fun r -> r <> fnwd_auxiliary_index) t2.read_regs in
            let hazard =
              List.find_opt (fun r -> List.mem r w2_obs || List.mem r r2_obs) w1_obs
            in
            match hazard with
            | Some r -> Some r
            | None -> List.find_opt (fun r -> List.mem r r1_obs) w2_obs
          ) rest
        in
        (match conflict with
         | Some r -> Some r
         | None -> check_pairs rest)
  in
  check_pairs txs

let check_causal_horizon (txs : fnwd_tx_op list) : (int * int) option =
  let rec loop max_ep = function
    | [] -> None
    | tx :: rest ->
        if tx.causal_epoch < max_ep then Some (tx.causal_epoch, max_ep)
        else loop (max max_ep tx.causal_epoch) rest
  in
  loop 0 txs

let execute_fnwd_enabler_optimizer_batch
    ?(retraction_fn = valid_auxiliary_retraction)
    ?(batch_size = 5)
    ?(lock_cost_us = 18.5)
    ?(retraction_cost_us = 1.2)
    ~(initial_regs : int array)
    (transactions : fnwd_tx_op list) : fnwd_batch_execution_report =
  (* 1. Canonical Sequential Baseline Execution (2PL fine-grained register locks) *)
  let baseline_locks =
    List.fold_left (fun acc tx ->
      acc + max 1 (List.length tx.read_regs + List.length tx.write_regs)
    ) 0 transactions
  in
  let seq_regs = ref (Array.copy initial_regs) in
  let seq_outputs =
    List.map (fun tx ->
      let (st', y) = execute_single_tx !seq_regs tx in
      seq_regs := st';
      (tx.tx_id, y)
    ) transactions
  in
  let seq_obs = project_observable_regs !seq_regs in

  (* 2. Verify Gate 1 Machine-Checkable Retraction Invariants *)
  let (retraction_idempotent, obs_invar, manifold_stable) =
    verify_retraction_invariants retraction_fn
  in
  if not (retraction_idempotent && obs_invar && manifold_stable) then
    let reason =
      if not obs_invar then "OBSERVABLE_COORDINATE_LEAK_DETECTED"
      else if not retraction_idempotent then "NON_IDEMPOTENT_NORMALIZER_REJECTED"
      else "MANIFOLD_STABILITY_VIOLATION"
    in
    {
      regime = Rejected_Corrupt_Normalizer { reason };
      baseline_locks;
      optimized_locks = baseline_locks;
      lock_reduction_pct = 0.0;
      trace_equivalent = true;
      observable_state_preserved = true;
      retraction_idempotent;
      manifold_stable;
      auxiliary_drift_cleared = false;
      final_observable_regs = seq_obs;
    }
  else
    (* 3. Failure Boundary Guard #3: Causal Horizon Violation Check *)
    match check_causal_horizon transactions with
    | Some (regressed_epoch, horizon_epoch) ->
        {
          regime = Fallback_CausalHorizonViolation { regressed_epoch; horizon_epoch };
          baseline_locks;
          optimized_locks = baseline_locks;
          lock_reduction_pct = 0.0;
          trace_equivalent = true;
          observable_state_preserved = true;
          retraction_idempotent = true;
          manifold_stable = true;
          auxiliary_drift_cleared = false;
          final_observable_regs = seq_obs;
        }
    | None ->
        (* 4. Failure Boundary Guard #1: Cost-of-Normalization Crossover Trap Check *)
        let num_tx = List.length transactions in
        let b_size = max 1 batch_size in
        let num_batches = (num_tx + b_size - 1) / b_size in
        let locks_saved = max 0 (baseline_locks - num_batches) in
        let lock_saved_us = float_of_int locks_saved *. lock_cost_us in
        let total_norm_cost_us = float_of_int num_batches *. retraction_cost_us in
        if total_norm_cost_us >= lock_saved_us then
          {
            regime = Fallback_NormalizationCostTrap {
              norm_cost_us = total_norm_cost_us;
              lock_saved_us;
            };
            baseline_locks;
            optimized_locks = baseline_locks;
            lock_reduction_pct = 0.0;
            trace_equivalent = true;
            observable_state_preserved = true;
            retraction_idempotent = true;
            manifold_stable = true;
            auxiliary_drift_cleared = false;
            final_observable_regs = seq_obs;
          }
        else
          (* 5. Failure Boundary Guard #2: Contention Hotspot Check across Batch Windows *)
          let rec split_batches k lst =
            if lst = [] then []
            else
              let rec take n acc = function
                | [] -> (List.rev acc, [])
                | x :: xs ->
                    if n = 0 then (List.rev acc, x :: xs)
                    else take (n - 1) (x :: acc) xs
              in
              let (b, rest) = take k [] lst in
              b :: split_batches k rest
          in
          let batches = split_batches b_size transactions in
          let hotspot =
            List.find_map find_observable_contention_hotspot batches
          in
          match hotspot with
          | Some conflicting_reg ->
              {
                regime = Fallback_ContentionHotspot { conflicting_reg };
                baseline_locks;
                optimized_locks = baseline_locks;
                lock_reduction_pct = 0.0;
                trace_equivalent = true;
                observable_state_preserved = true;
                retraction_idempotent = true;
                manifold_stable = true;
                auxiliary_drift_cleared = false;
                final_observable_regs = seq_obs;
              }
          | None ->
              (* 6. Promoted E (x) O Execution: Coarse Batch Lock + Idempotent Auxiliary Retraction *)
              let spec_regs = ref (retraction_fn initial_regs) in
              let spec_outputs = ref [] in
              List.iter (fun batch ->
                (* Speculative batch execution (1 coarse batch barrier per batch) *)
                List.iter (fun tx ->
                  let (st', y) = execute_single_tx !spec_regs tx in
                  spec_regs := st';
                  spec_outputs := (tx.tx_id, y) :: !spec_outputs
                ) batch;
                (* Apply Enabler Idempotent Retraction Normalizer at batch boundary *)
                spec_regs := retraction_fn !spec_regs
              ) batches;
              let final_spec_outputs = List.rev !spec_outputs in
              let spec_obs = project_observable_regs !spec_regs in
              let trace_equivalent = (final_spec_outputs = seq_outputs) in
              let observable_state_preserved = (spec_obs = seq_obs) in
              let optimized_locks = max 1 num_batches in
              let lock_reduction_pct =
                if baseline_locks = 0 then 0.0
                else
                  100.0 *. float_of_int (baseline_locks - optimized_locks) /. float_of_int baseline_locks
              in
              {
                regime = Promoted_Enabler_Optimizer_E_x_O;
                baseline_locks;
                optimized_locks;
                lock_reduction_pct;
                trace_equivalent;
                observable_state_preserved;
                retraction_idempotent = true;
                manifold_stable = true;
                auxiliary_drift_cleared = (!spec_regs.(fnwd_auxiliary_index) = 0);
                final_observable_regs = spec_obs;
              }

(* ==========================================================================
   Control-Plane IntScript Static Auditor, Fiat-Shamir ZK & Ring-LWE KEM Gate
   Mirrors Sentinel security/ boundary checks for direct control-plane and
   stress-suite verification without cross-library circular dependencies.
   ========================================================================== *)

let extract_quoted_literals (src : string) : string list =
  let len = String.length src in
  let rec loop i in_quote start acc =
    if i >= len then List.rev acc
    else if src.[i] = '"' then
      if in_quote then
        let lit = String.sub src start (i - start) in
        loop (i + 1) false 0 (lit :: acc)
      else
        loop (i + 1) true (i + 1) acc
    else
      loop (i + 1) in_quote start acc
  in
  loop 0 false 0 []

let is_file_like_literal (lit : string) : bool =
  let file_exts =
    [ ".xlsx"; ".txt"; ".pdf"; ".md"; ".csv"; ".log"; ".svg";
      ".ics"; ".eml"; ".zip"; ".json"; ".ini"; ".dat"; ".note"; ".doc" ]
  in
  List.exists (fun ext ->
    let elen = String.length ext in
    let llen = String.length lit in
    llen > elen && String.sub lit (llen - elen) elen = ext &&
    not (Inos_scp.Scp_projection.contains_sub_str lit " ")
  ) file_exts

let audit_intscript_source (script_src : string) ~(allowed_paths : string list) : (unit, string) result =
  let lower_src = String.lowercase_ascii script_src in
  let forbidden_path_patterns =
    [ ".."; ".workspace"; "private"; "/etc/"; "/root"; "\\etc\\"; "c:\\" ]
  in
  match List.find_opt (fun pat -> Inos_scp.Scp_projection.contains_sub_str lower_src pat) forbidden_path_patterns with
  | Some pat ->
      Error (Printf.sprintf "IntScript Security Violation: forbidden path or traversal pattern '%s' detected" pat)
  | None ->
      let unbounded_loops = [ "while(true)"; "while (true)"; "for(;;)"; "for (;;)" ] in
      (match List.find_opt (fun lp -> Inos_scp.Scp_projection.contains_sub_str lower_src lp) unbounded_loops with
       | Some lp ->
           Error (Printf.sprintf "IntScript Security Violation: unbounded loop '%s' rejected by termination guard" lp)
       | None ->
           let literals = extract_quoted_literals script_src in
           let file_literals = List.filter is_file_like_literal literals in
           let unauthorized =
             List.find_opt (fun flit ->
               not (List.exists (fun allowed -> String.equal flit allowed) allowed_paths)
             ) file_literals
           in
           (match unauthorized with
            | Some bad_file ->
                Error
                  (Printf.sprintf
                     "IntScript Security Violation: file target '%s' is outside allowed ScriptIsolationContext paths [%s]"
                     bad_file (String.concat ", " allowed_paths))
            | None -> Ok ()))

type schnorr_zk_proof = {
  statement : string;
  proof_payload : string;
}

let zk_prime_p = 65537
let zk_order_q = 65536
let zk_gen_g = 3

let pow_mod (base : int) (exp : int) (m : int) : int =
  let rec loop b e acc =
    if e <= 0 then acc
    else
      let acc' = if e land 1 = 1 then (acc * b) mod m else acc in
      loop ((b * b) mod m) (e lsr 1) acc'
  in
  loop ((base mod m + m) mod m) exp 1

let hex_prefix_to_int (hex : string) : int =
  let n = min 7 (String.length hex) in
  let acc = ref 0 in
  for i = 0 to n - 1 do
    let c = hex.[i] in
    let v =
      if c >= '0' && c <= '9' then Char.code c - Char.code '0'
      else if c >= 'a' && c <= 'f' then Char.code c - Char.code 'a' + 10
      else if c >= 'A' && c <= 'F' then Char.code c - Char.code 'A' + 10
      else 0
    in
    acc := (!acc lsl 4) lor v
  done;
  !acc

let prove_schnorr_zk ~(statement : string) ~(witness : string) : schnorr_zk_proof =
  let w_hash = Inos_ipc.Sha256.digest_string ("ZK_WITNESS_SCALAR|" ^ witness) in
  let w = (hex_prefix_to_int w_hash mod (zk_order_q - 1)) + 1 in
  let y = pow_mod zk_gen_g w zk_prime_p in
  let r_hash = Inos_ipc.Sha256.digest_string ("ZK_EPHEMERAL_NONCE|" ^ witness ^ "|" ^ statement) in
  let r = (hex_prefix_to_int r_hash mod (zk_order_q - 1)) + 1 in
  let t = pow_mod zk_gen_g r zk_prime_p in
  let c_hash =
    Inos_ipc.Sha256.digest_string (Printf.sprintf "ZK_FS_CHAL|Y:%d|T:%d|STMT:%s" y t statement)
  in
  let c = (hex_prefix_to_int c_hash mod 32767) + 1 in
  let s = (r + c * w) mod zk_order_q in
  let bind_tag =
    Inos_ipc.Sha256.digest_string
      (Printf.sprintf "ZK_BIND|Y:%d|T:%d|S:%d|C:%d|STMT:%s" y t s c statement)
  in
  let proof_payload = Printf.sprintf "SCHNORR_ZK_V1|%d|%d|%d|%s" y t s bind_tag in
  { statement; proof_payload }

let verify_schnorr_zk ~(proof : schnorr_zk_proof) ~(statement : string) : bool =
  if not (String.equal proof.statement statement) then false
  else
    match String.split_on_char '|' proof.proof_payload with
    | [ "SCHNORR_ZK_V1"; y_str; t_str; s_str; bind_tag ] -> (
        try
          let y = int_of_string y_str in
          let t = int_of_string t_str in
          let s = int_of_string s_str in
          if y <= 0 || y >= zk_prime_p || t <= 0 || t >= zk_prime_p || s < 0 || s >= zk_order_q then
            false
          else
            let c_hash =
              Inos_ipc.Sha256.digest_string (Printf.sprintf "ZK_FS_CHAL|Y:%d|T:%d|STMT:%s" y t statement)
            in
            let c = (hex_prefix_to_int c_hash mod 32767) + 1 in
            let expected_bind =
              Inos_ipc.Sha256.digest_string
                (Printf.sprintf "ZK_BIND|Y:%d|T:%d|S:%d|C:%d|STMT:%s" y t s c statement)
            in
            if not (String.equal bind_tag expected_bind) then false
            else
              let lhs = pow_mod zk_gen_g s zk_prime_p in
              let rhs = (t * pow_mod y c zk_prime_p) mod zk_prime_p in
              lhs = rhs
        with Failure _ -> false)
    | _ -> false

type rlwe_kem_encapsulation = {
  ciphertext : string;
  shared_secret : string;
}

let rlwe_q = 3329
let rlwe_n = 16

let rlwe_mod_q (x : int) : int =
  let r = x mod rlwe_q in
  if r < 0 then r + rlwe_q else r

let rlwe_poly_add (a : int array) (b : int array) : int array =
  Array.init rlwe_n (fun i -> rlwe_mod_q (a.(i) + b.(i)))

let rlwe_poly_sub (a : int array) (b : int array) : int array =
  Array.init rlwe_n (fun i -> rlwe_mod_q (a.(i) - b.(i)))

let rlwe_poly_mul_negacyclic (a : int array) (b : int array) : int array =
  let res = Array.make rlwe_n 0 in
  for i = 0 to rlwe_n - 1 do
    for j = 0 to rlwe_n - 1 do
      let prod = a.(i) * b.(j) in
      if i + j < rlwe_n then
        res.(i + j) <- rlwe_mod_q (res.(i + j) + prod)
      else
        res.(i + j - rlwe_n) <- rlwe_mod_q (res.(i + j - rlwe_n) - prod)
    done
  done;
  res

let rlwe_hex_byte_at (hex : string) (idx : int) : int =
  let pos = (idx * 2) mod max 2 (String.length hex - 1) in
  let hex_val c =
    if c >= '0' && c <= '9' then Char.code c - Char.code '0'
    else if c >= 'a' && c <= 'f' then Char.code c - Char.code 'a' + 10
    else if c >= 'A' && c <= 'F' then Char.code c - Char.code 'A' + 10
    else 0
  in
  (hex_val hex.[pos] lsl 4) lor hex_val hex.[pos + 1]

let rlwe_canonical_key_id (key_str : string) : string =
  if String.length key_str > 5 && String.sub key_str 0 5 = "priv_" then
    String.sub key_str 5 (String.length key_str - 5)
  else
    key_str

let rlwe_derive_uniform_poly (domain : string) (seed : string) : int array =
  let h1 = Inos_ipc.Sha256.digest_string (domain ^ "|0|" ^ seed) in
  let h2 = Inos_ipc.Sha256.digest_string (domain ^ "|1|" ^ seed) in
  let combined = h1 ^ h2 in
  Array.init rlwe_n (fun i ->
    let b0 = rlwe_hex_byte_at combined (2 * i) in
    let b1 = rlwe_hex_byte_at combined (2 * i + 1) in
    rlwe_mod_q ((b0 lsl 8) lor b1))

let rlwe_derive_small_poly (domain : string) (seed : string) : int array =
  let h = Inos_ipc.Sha256.digest_string (domain ^ "|SMALL|" ^ seed) in
  Array.init rlwe_n (fun i ->
    let v = (rlwe_hex_byte_at h i mod 3) - 1 in
    rlwe_mod_q v)

let rlwe_derive_message_bits (seed : string) : int array =
  let h = Inos_ipc.Sha256.digest_string ("ML_KEM_768_MSG_BITS|" ^ seed) in
  Array.init rlwe_n (fun i -> rlwe_hex_byte_at h i land 1)

let rlwe_serialize_poly (p : int array) : string =
  String.concat "," (Array.to_list (Array.map string_of_int p))

let rlwe_deserialize_poly (s : string) : int array =
  let parts = String.split_on_char ',' s in
  let arr = Array.make rlwe_n 0 in
  List.iteri (fun i p_str ->
    if i < rlwe_n then
      try arr.(i) <- rlwe_mod_q (int_of_string p_str) with Failure _ -> ()
  ) parts;
  arr

let rlwe_bits_to_string (bits : int array) : string =
  String.init rlwe_n (fun i -> if bits.(i) <> 0 then '1' else '0')

let rlwe_kem_encapsulate (public_key : string) (seed : string) : rlwe_kem_encapsulation =
  let key_id = rlwe_canonical_key_id public_key in
  let a_poly = rlwe_derive_uniform_poly "RLWE_MAT_A" key_id in
  let s_poly = rlwe_derive_small_poly "RLWE_SEC_S" key_id in
  let e_poly = rlwe_derive_small_poly "RLWE_ERR_E" key_id in
  let b_poly = rlwe_poly_add (rlwe_poly_mul_negacyclic a_poly s_poly) e_poly in
  let r_poly = rlwe_derive_small_poly "RLWE_EPH_R" seed in
  let e1_poly = rlwe_derive_small_poly "RLWE_ERR_E1" seed in
  let e2_poly = rlwe_derive_small_poly "RLWE_ERR_E2" seed in
  let m_bits = rlwe_derive_message_bits seed in
  let m_scaled = Array.init rlwe_n (fun i -> m_bits.(i) * ((rlwe_q + 1) / 2)) in
  let u_poly = rlwe_poly_add (rlwe_poly_mul_negacyclic a_poly r_poly) e1_poly in
  let v_poly = rlwe_poly_add (rlwe_poly_add (rlwe_poly_mul_negacyclic b_poly r_poly) e2_poly) m_scaled in
  let ciphertext =
    Printf.sprintf "RLWE_CT_V1|%s|%s" (rlwe_serialize_poly u_poly) (rlwe_serialize_poly v_poly)
  in
  let m_str = rlwe_bits_to_string m_bits in
  let ct_hash = Inos_ipc.Sha256.digest_string ciphertext in
  let shared_secret =
    Inos_ipc.Sha256.digest_string (Printf.sprintf "ML_KEM_768_SS|MSG:%s|CT:%s" m_str ct_hash)
  in
  { ciphertext; shared_secret }

let rlwe_kem_decapsulate (private_key : string) (ciphertext : string) : string =
  let key_id = rlwe_canonical_key_id private_key in
  let s_poly = rlwe_derive_small_poly "RLWE_SEC_S" key_id in
  match String.split_on_char '|' ciphertext with
  | [ "RLWE_CT_V1"; u_str; v_str ] ->
      let u_poly = rlwe_deserialize_poly u_str in
      let v_poly = rlwe_deserialize_poly v_str in
      let d_poly = rlwe_poly_sub v_poly (rlwe_poly_mul_negacyclic u_poly s_poly) in
      let m_bits =
        Array.init rlwe_n (fun i ->
          let c = d_poly.(i) in
          if c >= 832 && c <= 2496 then 1 else 0)
      in
      let m_str = rlwe_bits_to_string m_bits in
      let ct_hash = Inos_ipc.Sha256.digest_string ciphertext in
      Inos_ipc.Sha256.digest_string (Printf.sprintf "ML_KEM_768_SS|MSG:%s|CT:%s" m_str ct_hash)
  | _ ->
      Inos_ipc.Sha256.digest_string ("ML_KEM_768_INVALID_CT|" ^ private_key ^ "|" ^ ciphertext)
