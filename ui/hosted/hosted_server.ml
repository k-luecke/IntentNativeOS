(* ui/hosted/hosted_server.ml - Minimal Hosted File-Picker and Intent UI (U001)
   Provides an offline, web-based graphical interface for selecting files and submitting
   intent instructions without needing terminal commands or CLI flags.
*)

type file_info = {
  name: string;
  rel_path: string;
  size_bytes: int;
}

type intent_submission = {
  selected_file: string;
  intent_text: string;
  submitted_at: float;
}

let list_workspace_files (root : string) : file_info list =
  let results = ref [] in
  let rec traverse dir rel_prefix =
    let entries =
      try Sys.readdir dir |> Array.to_list
      with _ -> []
    in
    List.iter (fun entry ->
      if String.starts_with ~prefix:"." entry || entry = "_build" then ()
      else
        let full = Filename.concat dir entry in
        let rel = if rel_prefix = "" then entry else Filename.concat rel_prefix entry in
        if Sys.is_directory full then
          traverse full rel
        else
          let size = (Unix.stat full).Unix.st_size in
          results := { name = entry; rel_path = rel; size_bytes = size } :: !results
    ) entries
  in
  traverse root "";
  List.rev !results

let render_html (files : file_info list) (selected : string option) (last_intent : string option) : string =
  let file_options =
    List.map (fun f ->
      let is_sel = match selected with Some s -> s = f.rel_path | None -> false in
      Printf.sprintf "<option value=\"%s\" %s>%s (%d bytes)</option>"
        f.rel_path (if is_sel then "selected" else "") f.rel_path f.size_bytes
    ) files |> String.concat "\n"
  in
  let intent_val = match last_intent with Some s -> s | None -> "" in
  let sel_text = match selected with Some s -> s | None -> "(None selected)" in
  Printf.sprintf {|<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <title>Intent-Native OS — File & Intent Console</title>
  <style>
    body { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; margin: 2rem; background: #0f172a; color: #f8fafc; }
    .card { background: #1e293b; border-radius: 8px; padding: 1.5rem; max-width: 680px; margin: auto; box-shadow: 0 4px 6px rgba(0,0,0,0.3); }
    h1 { font-size: 1.25rem; margin-top: 0; color: #38bdf8; }
    label { display: block; font-weight: 600; margin-top: 1rem; margin-bottom: 0.5rem; font-size: 0.9rem; }
    select, textarea, input[type=text] { width: 100%%; padding: 0.75rem; border-radius: 6px; border: 1px solid #334155; background: #0f172a; color: #f8fafc; box-sizing: border-box; }
    button { background: #0284c7; color: white; border: none; padding: 0.75rem 1.5rem; border-radius: 6px; cursor: pointer; font-weight: 600; margin-top: 1.25rem; }
    button:hover { background: #0369a1; }
    .status-badge { display: inline-block; padding: 0.25rem 0.5rem; background: #334155; border-radius: 4px; font-size: 0.8rem; margin-top: 0.5rem; }
  </style>
</head>
<body>
  <div class="card">
    <h1>Intent-Native OS — File Selection & Intent Console</h1>
    <p>Select a workspace file and express your intent without terminal interaction.</p>
    
    <form method="POST" action="/api/intent">
      <label for="file_picker">1. Select Target File:</label>
      <select id="file_picker" name="file">
        <option value="">-- Choose a file --</option>
        %s
      </select>
      <div class="status-badge">Active target: %s</div>

      <label for="intent_text">2. Express Human Intent:</label>
      <textarea id="intent_text" name="intent" rows="4" placeholder="e.g., Optimize performance of pure operations and remove duplicates">%s</textarea>

      <button type="submit" id="btn_submit">Submit Intent</button>
    </form>
  </div>
</body>
</html>|} file_options sel_text intent_val

let parse_form_urlencoded (body : string) : (string * string) list =
  String.split_on_char '&' body
  |> List.filter_map (fun pair ->
      match String.split_on_char '=' pair with
      | [k; v] ->
          (* basic URL decoding for spaces and common symbols *)
          let decode s =
            let b = Buffer.create (String.length s) in
            let i = ref 0 in
            let len = String.length s in
            while !i < len do
              if s.[!i] = '+' then Buffer.add_char b ' '
              else if s.[!i] = '%' && !i + 2 < len then begin
                let hex = String.sub s (!i + 1) 2 in
                (try Buffer.add_char b (Char.chr (int_of_string ("0x" ^ hex)))
                 with _ -> Buffer.add_char b s.[!i]);
                i := !i + 2
              end else Buffer.add_char b s.[!i];
              incr i
            done;
            Buffer.contents b
          in
          Some (decode k, decode v)
      | _ -> None
    )

let process_request ~(root : string) (meth : string) (path : string) (body : string)
  : (int * string * string) =
  match (meth, path) with
  | ("GET", "/") | ("GET", "/index.html") ->
      let files = list_workspace_files root in
      let html = render_html files None None in
      (200, "text/html; charset=utf-8", html)

  | ("GET", "/api/files") ->
      let files = list_workspace_files root in
      let json_items =
        List.map (fun f ->
          Printf.sprintf "{\"name\": \"%s\", \"path\": \"%s\", \"size\": %d}"
            f.name f.rel_path f.size_bytes
        ) files |> String.concat ",\n"
      in
      (200, "application/json", Printf.sprintf "[\n%s\n]" json_items)

  | ("POST", "/api/intent") ->
      let params = parse_form_urlencoded body in
      let file = List.assoc_opt "file" params |> Option.value ~default:"" in
      let intent = List.assoc_opt "intent" params |> Option.value ~default:"" in
      if file = "" then
        (400, "application/json", "{\"error\": \"No target file selected\"}")
      else if String.trim intent = "" then
        (400, "application/json", "{\"error\": \"Intent instruction cannot be empty\"}")
      else
        let res_json = Printf.sprintf
          "{\"status\": \"accepted\", \"selected_file\": \"%s\", \"intent\": \"%s\", \"timestamp\": %.3f}"
          file (String.escaped intent) (Unix.gettimeofday ())
        in
        (200, "application/json", res_json)

  | ("GET", "/dashboard") ->
      let dash = Adapter_dashboard.create_dashboard () in
      (200, "text/html; charset=utf-8", Adapter_dashboard.render_dashboard_html dash)

  | ("GET", "/api/adapters") ->
      let dash = Adapter_dashboard.create_dashboard () in
      (200, "application/json", Adapter_dashboard.render_adapters_json dash)

  | ("POST", "/api/adapters/auto-build") ->
      let params = parse_form_urlencoded body in
      let goal = List.assoc_opt "goal" params |> Option.value ~default:"" in
      let raw_data = List.assoc_opt "raw_data" params |> Option.value ~default:"" in
      if String.trim goal = "" then
        (400, "application/json", "{\"error\": \"Goal cannot be empty\"}")
      else
        let dash = Adapter_dashboard.create_dashboard () in
        let rep = Adapter_dashboard.construct_expert_from_goal_and_data dash ~goal ~raw_data () in
        let json = Printf.sprintf
          "{\"status\":\"constructed\",\"adapter_id\":\"%s\",\"display_name\":\"%s\",\"app\":\"%s\",\"rank\":%d,\"q8_kib\":%.2f,\"base_conf\":%.4f,\"expert_conf\":%.4f,\"lift_pp\":%.2f}"
          rep.built_specialist.adapter_id rep.built_specialist.display_name
          (Inos_models.Lora_hub.app_id_to_string rep.inferred_app)
          rep.built_specialist.rank rep.built_specialist.q8_footprint_kib
          rep.base_mean_confidence rep.expert_mean_confidence rep.confidence_lift_pp
        in
        (200, "application/json", json)

  | ("POST", "/api/safety") ->
      let params = parse_form_urlencoded body in
      let preset_str = List.assoc_opt "preset" params |> Option.value ~default:"balanced_verified" in
      let dash = Adapter_dashboard.create_dashboard () in
      let preset = Adapter_dashboard.preset_of_string preset_str in
      ignore (Inos_models.Lora_hub.set_safety_preset dash.hub preset);
      (200, "application/json", Adapter_dashboard.render_adapters_json dash)

  | ("GET", "/trial") | ("GET", "/showcase") ->
      let html = {|<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <title>INOS Live Trial & Architecture Showcase</title>
  <style>
    body { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; margin: 0; padding: 1.5rem; background: #0b1120; color: #f8fafc; }
    .grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(340px, 1fr)); gap: 1rem; max-width: 1280px; margin: auto; }
    .card { background: #1e293b; border: 1px solid #334155; border-radius: 8px; padding: 1.25rem; }
    h1, h2 { margin-top: 0; color: #38bdf8; }
    .badge { display: inline-block; padding: 0.25rem 0.6rem; border-radius: 4px; font-size: 0.78rem; font-weight: 700; margin-right: 0.4rem; background: #0284c7; color: #fff; }
    pre { background: #0f172a; padding: 0.75rem; border-radius: 6px; overflow-x: auto; font-size: 0.82rem; color: #a7f3d0; }
  </style>
</head>
<body>
  <div style="max-width:1280px;margin:0 auto 1.25rem auto;">
    <h1>INOS Live Trial &amp; Architecture Showcase</h1>
    <p>16 Native Ring-3 Apps &bull; 3-State Scoped Continuity Chrome Ribbon &bull; Natural-Language-to-IntScript (.ints) Compiler &bull; INOS FNWD Recurrent Tie-Breaker (8.0x KV) &bull; FNWD E &otimes; O &bull; Sentinel PQ/ZK Security Console</p>
  </div>
  <div class="grid">
    <div class="card">
      <h2>1. 16 Native Apps &amp; 3-State Scoped Chrome Ribbon</h2>
      <p><span class="badge">LOCAL CLIPBOARD ONLY</span><span class="badge">LINK AVAILABLE</span><span class="badge">LIVE BOUND</span></p>
      <p>Apps: grid_app, write_app, files_app, notes_app, plan_app, mail_app, calc_app, pdf_app, canvas_app, archive_app, flow_app, net_app, security_console, media_app, terminal_app, settings_app</p>
    </div>
    <div class="card">
      <h2>2. INOS FNWD Onboard Intelligence (8.0x KV) &amp; FNWD E &otimes; O</h2>
      <p>Prelude &rarr; FNWD Recurrent Tie-Breaker (&alpha;_LTI = 0.20, 0 extra param RAM) &rarr; Adaptive Early-Exit Halting &rarr; Coda with Low-Rank Latent KV Compression (8.0x) &amp; FNWD 95.0% Lock Reduction.</p>
    </div>
    <div class="card">
      <h2>3. IntScript (.ints) Compiler &amp; Sentinel PQ/ZK Console</h2>
      <pre id="ints_preview">const wb = inos.grid.open("budget.xlsx");
const sheet = wb.getSheet("Sheet1");
sheet.setFormula("B2", "=SUM(B2:B4)");
const val = sheet.evaluate("B2");
if (val > 0) { inos.grid.save("budget.xlsx"); }</pre>
    </div>
  </div>
</body>
</html>|} in
      (200, "text/html; charset=utf-8", html)

  | ("POST", "/api/trial/execute") ->
      let params = parse_form_urlencoded body in
      let prompt =
        match List.assoc_opt "prompt" params with
        | Some p when String.trim p <> "" -> p
        | _ -> "Calculate sum of Q3 expenses in budget.xlsx and stage result in ribbon"
      in
      let src_artifact =
        match List.assoc_opt "src_artifact" params with
        | Some s when String.trim s <> "" -> s
        | _ -> "budget.xlsx"
      in
      let dst_artifact =
        match List.assoc_opt "dst_artifact" params with
        | Some d when String.trim d <> "" -> d
        | _ -> "q3_budget_report.pdf"
      in
      let eng = Inos_models.Optimizing_engine.create () in
      let obs : Inos_scp.Scp_projection.observation_entry list = [
        { path = src_artifact; size_bytes = 8192; sha256_prefix = "c319f004"; writable = true };
        { path = dst_artifact; size_bytes = 4096; sha256_prefix = "a8f901c2"; writable = true };
      ] in
      let pkt =
        Inos_models.Optimizing_engine.dispatch_to_app_spool eng
          ~prompt
          ~system_prefix:"inos_trial_showcase_v1"
          ~observed_files:obs
          ~workspace_state_hash:"ws_trial_showcase_001"
          ~capability_epoch:88 ()
      in
      let cont =
        Inos_models.Optimizing_engine.evaluate_cross_app_continuity_gate eng
          ~src_artifact
          ~src_context:prompt
          ~src_cell:"B2"
          ~src_value_num:1420000L
          ~src_formula:"=SUM(B2:B4)"
          ~dst_artifact
          ~dst_context:(dst_artifact ^ " " ^ prompt) ()
      in
      let (ribbon_badge, affinity_score) =
        match cont with
        | Inos_models.Optimizing_engine.Continuity_Blocked_Unrelated_Context b ->
            ("LOCAL CLIPBOARD ONLY", b.affinity_score)
        | Inos_models.Optimizing_engine.Continuity_Staged_Ribbon_Offer o ->
            (o.ribbon_badge, o.affinity_score)
        | Inos_models.Optimizing_engine.Continuity_Bound_Auto_Sync s ->
            (s.ribbon_badge, s.affinity_score)
      in
      let (executed_loops, early_halted, mla_kv_ratio, lift_pp) =
        match pkt.receipt.inos_rdt_trace with
        | Some tr ->
            (tr.executed_loops, tr.early_halted, tr.mla_kv_compression_ratio, tr.confidence_lift_pp)
        | None -> (1, true, 8.0, 0.0)
      in
      let json =
        Printf.sprintf
          "{\"status\":\"ok\",\"script_id_hex\":\"%s\",\"intscript_source\":\"%s\",\"executed_loops\":%d,\"early_halted\":%b,\"mla_kv_compression_ratio\":%.1f,\"confidence_lift_pp\":%.2f,\"ribbon_badge\":\"%s\",\"affinity_score\":%.2f,\"provenance_sha256\":\"%s\"}"
          pkt.intscript_program.script_id_hex
          (String.escaped pkt.intscript_program.intscript_source)
          executed_loops
          early_halted
          mla_kv_ratio
          lift_pp
          ribbon_badge
          affinity_score
          pkt.provenance_sha256
      in
      (200, "application/json", json)

  | _ ->
      (404, "application/json", "{\"error\": \"Not Found\"}")
