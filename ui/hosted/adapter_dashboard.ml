(* ui/hosted/adapter_dashboard.ml
   ==========================================================================
   INOS Specialization, LoRA Hub & Safety Customization Dashboard
   ==========================================================================
   Provides an interactive user control plane and HTML dashboard for:
   1. Browsing, searching, pinning, and toggling the 16 App-Specialist Q8_0
      LoRA adapters (plus user-forged custom specialists).
   2. Forging new custom LoRA specialists on the fly (custom trigger vocabulary,
      target app binding, rank r in {4, 8, 16}, alpha scaling, format schema).
   3. Customizing Safety & Autonomy Posture:
      - Fortress_Strict (Full D1-D4 + SCP + Dual-Model Review)
      - Balanced_Verified (Default Pareto Frontier)
      - PowerUser_Permissive (Relaxes D4 idempotent block for batch/script loops)
      - Unrestricted_Raw ("Lack-of-Safety / Raw Frontier Mode" — bypasses SCP,
        Tonic Clamps, and D2-D4 Guards with optional pre-exec WAL snapshot).
   4. Live Top-K Mixture-of-LoRA-Experts (MoLE) workflow previewer.
   ========================================================================== *)

open Inos_models

type dashboard_state = {
  hub : Lora_hub.t;
  opt_engine : Optimizing_engine.t;
  mutable last_query : string;
  mutable last_result : Lora_hub.hub_execution_result option;
  mutable last_build_report : Lora_hub.expert_build_report option;
}

let create_dashboard () : dashboard_state = {
  hub = Lora_hub.create ();
  opt_engine = Optimizing_engine.create ();
  last_query = "";
  last_result = None;
  last_build_report = None;
}

let preset_to_string = function
  | Lora_hub.Safety_Fortress_Strict -> "fortress_strict"
  | Lora_hub.Safety_Balanced_Verified -> "balanced_verified"
  | Lora_hub.Safety_PowerUser_Permissive -> "power_user_permissive"
  | Lora_hub.Safety_Unrestricted_Raw -> "unrestricted_raw"

let preset_of_string = function
  | "fortress_strict" -> Lora_hub.Safety_Fortress_Strict
  | "power_user_permissive" -> Lora_hub.Safety_PowerUser_Permissive
  | "unrestricted_raw" -> Lora_hub.Safety_Unrestricted_Raw
  | _ -> Lora_hub.Safety_Balanced_Verified

let app_id_of_string = function
  | "files_app" -> Lora_hub.App_Files
  | "grid_app" -> Lora_hub.App_Grid
  | "plan_app" -> Lora_hub.App_Plan
  | "mail_app" -> Lora_hub.App_Mail
  | "write_app" -> Lora_hub.App_Write
  | "notes_app" -> Lora_hub.App_Notes
  | "calc_app" -> Lora_hub.App_Calc
  | "pdf_app" -> Lora_hub.App_Pdf
  | "canvas_app" -> Lora_hub.App_Canvas
  | "archive_app" -> Lora_hub.App_Archive
  | "flow_app" -> Lora_hub.App_Flow
  | "script_app" -> Lora_hub.App_Script
  | "intscript_app" -> Lora_hub.App_IntScript
  | "net_app" -> Lora_hub.App_Net
  | "web_render" -> Lora_hub.App_WebRender
  | "security_console" -> Lora_hub.App_SecurityConsole
  | custom -> Lora_hub.App_Custom custom

(* Search specialists by keyword, app name, or capability description *)
let search_specialists (st : dashboard_state) (query : string) : Lora_hub.specialist_adapter list =
  let q = String.trim (String.lowercase_ascii query) in
  if q = "" then st.hub.specialists
  else
    List.filter (fun (s : Lora_hub.specialist_adapter) ->
      Lora_hub.contains_sub (String.lowercase_ascii s.adapter_id) q ||
      Lora_hub.contains_sub (String.lowercase_ascii s.display_name) q ||
      Lora_hub.contains_sub (String.lowercase_ascii (Lora_hub.app_id_to_string s.target_app)) q ||
      List.exists (fun kw -> Lora_hub.contains_sub (String.lowercase_ascii kw) q) s.trigger_keywords ||
      List.exists (fun cap -> Lora_hub.contains_sub (String.lowercase_ascii cap) q) s.capabilities_unlocked
    ) st.hub.specialists

(* Execute a prompt through the dashboard's LoRA Hub + Safety Customizer *)
let run_dashboard_intent (st : dashboard_state) (prompt : string) : Lora_hub.hub_execution_result =
  st.last_query <- prompt;
  let res = Lora_hub.execute_with_hub st.hub ~max_k:3 prompt in
  st.last_result <- Some res;
  res

(* "Make Your Own Adapter": Given a user's Goal and raw multi-line Data/Examples,
   automatically mines vocabulary, infers the target INOS app & schema, calibrates
   the low-rank Q8_0 matrices A and B over the hidden activations of the data samples,
   and registers the new specialist in the live hub. *)
let construct_expert_from_goal_and_data
    (st : dashboard_state)
    ?(custom_id : string option)
    ?(custom_name : string option)
    ?(apply_recommended_safety = false)
    ~(goal : string)
    ~(raw_data : string) () : Lora_hub.expert_build_report =
  let lines =
    String.split_on_char '\n' raw_data
    |> List.map String.trim
    |> List.filter (fun s -> s <> "")
  in
  let report =
    Lora_hub.build_expert_from_goal_and_data
      st.hub ?custom_id ?custom_name ~goal ~data_samples:lines ()
  in
  if apply_recommended_safety then
    ignore (Lora_hub.set_safety_preset st.hub report.recommended_safety);
  st.last_build_report <- Some report;
  report

(* Render JSON summary of all specialists + active safety configuration *)
let render_adapters_json (st : dashboard_state) : string =
  let items =
    List.map (fun (s : Lora_hub.specialist_adapter) ->
      Printf.sprintf
        "{\"adapter_id\":\"%s\",\"display_name\":\"%s\",\"app\":\"%s\",\"rank\":%d,\"q8_kib\":%.2f,\"custom\":%b,\"enabled\":%b,\"pinned\":%b}"
        s.adapter_id s.display_name (Lora_hub.app_id_to_string s.target_app)
        s.rank s.q8_footprint_kib s.is_custom_user_forged s.enabled s.pinned
    ) st.hub.specialists
    |> String.concat ",\n  "
  in
  let sf = st.hub.safety in
  Printf.sprintf
    "{\n  \"safety_preset\": \"%s\",\n  \"scp_grammar_enforced\": %b,\n  \"guard_d1_syntax\": %b,\n  \"guard_d2_policy_cap\": %b,\n  \"guard_d3_grounding\": %b,\n  \"guard_d4_idempotent\": %b,\n  \"tonic_safety_clamp\": %b,\n  \"allow_destructive_ops\": %b,\n  \"auto_snapshot_on_unrestricted\": %b,\n  \"auto_spawn_enabled\": %b,\n  \"auto_spawn_confidence_threshold\": %.2f,\n  \"auto_spawned_count\": %d,\n  \"specialists_count\": %d,\n  \"specialists\": [\n  %s\n  ]\n}"
    (preset_to_string sf.preset)
    sf.scp_grammar_enforced sf.guard_d1_syntax sf.guard_d2_policy_cap
    sf.guard_d3_grounding sf.guard_d4_idempotent sf.tonic_safety_clamp
    sf.allow_destructive_ops sf.auto_snapshot_on_unrestricted
    st.hub.auto_spawn_enabled st.hub.auto_spawn_confidence_threshold
    (List.length st.hub.auto_spawn_events)
    (List.length st.hub.specialists) items

(* Render full interactive HTML Specialization & Safety Customization Dashboard *)
let render_dashboard_html (st : dashboard_state) : string =
  let sf = st.hub.safety in
  let total_kib =
    List.fold_left (fun acc (s : Lora_hub.specialist_adapter) ->
      if s.enabled then acc +. s.q8_footprint_kib else acc
    ) 0.0 st.hub.specialists
  in
  let specialist_rows =
    List.map (fun (s : Lora_hub.specialist_adapter) ->
      let badge =
        if s.is_custom_user_forged then "<span class=\"badge badge-custom\">USER FORGED</span>"
        else "<span class=\"badge badge-builtin\">BUILT-IN Q8_0</span>"
      in
      let pin_badge =
        if s.pinned then "<span class=\"badge badge-pinned\">PINNED</span>" else ""
      in
      let kw_str = String.concat ", " s.trigger_keywords in
      let cap_str = String.concat " &bull; " s.capabilities_unlocked in
      Printf.sprintf
        "<tr>\n\
           <td><strong>%s</strong><br/><small>%s</small> %s %s</td>\n\
           <td><code>%s</code></td>\n\
           <td>r=%d (%.1f KiB)</td>\n\
           <td><small>%s</small><br/><em>%s</em></td>\n\
           <td><code>%s</code></td>\n\
         </tr>"
        s.display_name s.adapter_id badge pin_badge
        (Lora_hub.app_id_to_string s.target_app)
        s.rank s.q8_footprint_kib
        cap_str kw_str s.native_format_schema
    ) st.hub.specialists
    |> String.concat "\n"
  in
  let last_exec_html =
    match st.last_result with
    | None -> "<p>No workflow executed yet. Test a single-app or cross-app prompt below.</p>"
    | Some res ->
        let blend_str =
          List.map (fun (b : Lora_hub.mole_blend_entry) ->
            Printf.sprintf "%s (w=%.2f)" b.specialist.adapter_id b.weight
          ) res.active_blend
          |> String.concat " + "
        in
        let actions_str =
          List.map (fun (a : Lora_hub.app_usability_action) ->
            Printf.sprintf "<li><strong>%s</strong> [<code>%s</code>] &rarr; Artifact: <code>%s</code><br/>Payload: <code>%s</code></li>"
              (Lora_hub.app_id_to_string a.app) a.operation_verb a.target_artifact a.native_ipc_or_format_payload
          ) res.app_actions
          |> String.concat "\n"
        in
        Printf.sprintf
          "<div class=\"result-box\">\n\
             <div><strong>Execution Mode:</strong> <code>%s</code> | <strong>Active LoRA Blend:</strong> <code>%s</code></div>\n\
             <div><strong>Safety Status:</strong> %s | <strong>Pre-Exec Snapshot:</strong> %b | <strong>FLOPs:</strong> %Ld</div>\n\
             <ul>%s</ul>\n\
           </div>"
          res.blend_mode blend_str res.status_note res.snapshot_taken res.total_flops actions_str
  in
  let last_build_html =
    match st.last_build_report with
    | None -> ""
    | Some rep ->
        Printf.sprintf
          "<div class=\"result-box\">\n\
             <div><strong>Constructed Expert:</strong> <code>%s</code> (%s) &rarr; Bound to <code>%s</code> (r=%d, %.2f KiB Q8_0)</div>\n\
             <div><strong>Empirical Calibration Lift (%d samples):</strong> Base Confidence <code>%.2f%%</code> &rarr; Trained Expert <code>%.2f%%</code> (<strong>+%.2f pp lift</strong>)</div>\n\
             <div><strong>Mined Trigger Keywords:</strong> <em>%s</em> | <strong>Recommended Safety:</strong> <code>%s</code></div>\n\
           </div>"
          rep.built_specialist.adapter_id rep.built_specialist.display_name
          (Lora_hub.app_id_to_string rep.inferred_app)
          rep.built_specialist.rank rep.built_specialist.q8_footprint_kib
          rep.samples_count (rep.base_mean_confidence *. 100.0) (rep.expert_mean_confidence *. 100.0)
          rep.confidence_lift_pp
          (String.concat ", " rep.extracted_keywords)
          (preset_to_string rep.recommended_safety)
  in
  Printf.sprintf {|<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <title>INOS — LoRA Specialization Hub & Safety Customizer</title>
  <style>
    body { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; margin: 1.5rem; background: #0b1120; color: #f1f5f9; }
    .grid { display: grid; grid-template-columns: 1fr 1fr; gap: 1.25rem; max-width: 1400px; margin: auto; }
    .card { background: #1e293b; border-radius: 10px; padding: 1.25rem; border: 1px solid #334155; }
    .card-full { grid-column: 1 / -1; }
    h1, h2 { margin-top: 0; color: #38bdf8; }
    table { width: 100%%; border-collapse: collapse; font-size: 0.85rem; margin-top: 0.75rem; }
    th, td { border-bottom: 1px solid #334155; padding: 0.6rem; text-align: left; vertical-align: top; }
    th { color: #94a3b8; font-weight: 600; }
    .badge { display: inline-block; padding: 0.15rem 0.45rem; border-radius: 4px; font-size: 0.7rem; font-weight: 700; }
    .badge-builtin { background: #0369a1; color: #e0f2fe; }
    .badge-custom { background: #7c3aed; color: #f3e8ff; }
    .badge-pinned { background: #d97706; color: #fef3c7; }
    input[type=text], select, textarea { width: 100%%; padding: 0.6rem; border-radius: 6px; border: 1px solid #475569; background: #0f172a; color: #f8fafc; box-sizing: border-box; margin-top: 0.35rem; margin-bottom: 0.75rem; }
    button { background: #0284c7; color: white; border: none; padding: 0.65rem 1.25rem; border-radius: 6px; cursor: pointer; font-weight: 600; }
    button.danger { background: #e11d48; }
    button.wizard { background: #7c3aed; }
    .result-box { background: #0f172a; border-left: 4px solid #38bdf8; padding: 1rem; border-radius: 6px; margin-top: 0.75rem; font-size: 0.88rem; }
  </style>
</head>
<body>
  <div class="grid">
    <div class="card card-full">
      <h1>INOS — Multi-LoRA Specialization Hub & User Safety Customizer</h1>
      <p>Active Specialists: <strong>%d</strong> (Total L1/L2 Q8_0 Cache Footprint: <strong>%.1f KiB</strong>) | Active Safety Preset: <strong><code>%s</code></strong> | Hot-Swap Generation: <strong>%d</strong></p>
      %s
      %s
    </div>

    <div class="card card-full">
      <h2>Make Your Own Adapter — Guided Expert Constructor (Goal + Data &rarr; Calibrated Q8_0 LoRA)</h2>
      <p>Provide a high-level goal and paste sample domain data or examples. INOS automatically mines trigger vocabulary, selects the optimal native app &amp; rank, calibrates the low-rank Q8_0 matrices <em>A</em> and <em>B</em> over the hidden representations of your data, and hot-swaps the new expert into the live hub.</p>
      <form method="POST" action="/api/adapters/auto-build">
        <label>1. What is your Goal for this Expert?</label>
        <input type="text" name="goal" placeholder="e.g., Analyze clinical trial biomarker CSV tables and compute statistical cohort summaries" required />
        <label>2. Paste Sample Domain Data / Example Inputs (one per line):</label>
        <textarea name="raw_data" rows="4" placeholder="cohort_a,biomarker_il6,mean_delta=-42.5,p_value=0.0012&#10;evaluate spreadsheet =AVERAGE(C2:C120) for trial_phase3.xlsx&#10;summarize dose-response table and export xlsx" required></textarea>
        <button type="submit" class="wizard">Construct &amp; Calibrate New Q8_0 Expert from Goal + Data</button>
      </form>
    </div>

    <div class="card">
      <h2>1. Customize Safety, Guards & Autonomy Posture</h2>
      <form method="POST" action="/api/safety">
        <label>Select Safety / Autonomy Preset:</label>
        <select name="preset">
          <option value="fortress_strict" %s>Fortress Strict (Full D1-D4 + SCP + Dual-Model Review)</option>
          <option value="balanced_verified" %s>Balanced Verified (Default Pareto: Qwen3.5-2B-Q6_K + D1-D4 + SCP)</option>
          <option value="power_user_permissive" %s>Power-User Permissive (Relax D4 Idempotent Block for Batch/Scripts)</option>
          <option value="unrestricted_raw" %s>Unrestricted Raw Mode (Bypass SCP, Tonic Clamp & D2-D4 Guards + Auto-Snapshot)</option>
        </select>
        <p><small>Current Flags: SCP=%b | D1=%b | D2=%b | D3=%b | D4=%b | TonicClamp=%b | DestructiveAllowed=%b | AutoSnapshot=%b</small></p>
        <button type="submit" class="danger">Apply Safety & Guard Profile</button>
      </form>
    </div>

    <div class="card">
      <h2>2. Advanced Manual Q8_0 LoRA Forge</h2>
      <form method="POST" action="/api/adapters/forge">
        <label>Adapter ID &amp; Display Name:</label>
        <input type="text" name="adapter_id" placeholder="e.g., lora_quant_finance" required />
        <input type="text" name="display_name" placeholder="e.g., Quantitative Finance & Portfolio Specialist" required />
        <label>Bind to Native INOS Application:</label>
        <input type="text" name="target_app" placeholder="grid_app, calc_app, files_app, script_app..." value="grid_app" />
        <label>Trigger Keywords (comma-separated):</label>
        <input type="text" name="keywords" placeholder="sharpe, portfolio, volatility, black-scholes, hedge" required />
        <label>Native Format / Command Schema:</label>
        <input type="text" name="schema" placeholder="OPENXML_XLSX_QUANT_V1: =STDEV(B2:B100)*SQRT(252)" required />
        <button type="submit">Forge &amp; Hot-Swap Q8_0 Specialist</button>
      </form>
    </div>

    <div class="card card-full">
      <h2>3. 16-App Specialist LoRA Catalog &amp; Top-K MoLE Subspace Blending Matrix</h2>
      <table>
        <thead>
          <tr>
            <th>Specialist Adapter</th>
            <th>Target INOS App</th>
            <th>Q8_0 Rank &amp; Size</th>
            <th>Usability Capabilities &amp; Trigger Vocabulary</th>
            <th>Native Format / IPC Schema</th>
          </tr>
        </thead>
        <tbody>
          %s
        </tbody>
      </table>
    </div>
  </div>
</body>
</html>|}
    (List.length st.hub.specialists) total_kib (preset_to_string sf.preset) st.hub.swap_count
    last_exec_html last_build_html
    (if sf.preset = Lora_hub.Safety_Fortress_Strict then "selected" else "")
    (if sf.preset = Lora_hub.Safety_Balanced_Verified then "selected" else "")
    (if sf.preset = Lora_hub.Safety_PowerUser_Permissive then "selected" else "")
    (if sf.preset = Lora_hub.Safety_Unrestricted_Raw then "selected" else "")
    sf.scp_grammar_enforced sf.guard_d1_syntax sf.guard_d2_policy_cap
    sf.guard_d3_grounding sf.guard_d4_idempotent sf.tonic_safety_clamp
    sf.allow_destructive_ops sf.auto_snapshot_on_unrestricted
    specialist_rows
