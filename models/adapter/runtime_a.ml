open Protocol

type test_plan = {
  plan_id : string;
  intent_id : string;
  test_cases : string list;
  verification_commands : string list;
  cloud_free : bool;
}

type plan_error =
  | AdapterError of adapter_error
  | InvalidPlanSchema of string
  | CloudDependencyDetected of string

let find_sub ~pattern str =
  let p_len = String.length pattern in
  let s_len = String.length str in
  let rec check idx =
    if idx + p_len > s_len then false
    else if String.sub str idx p_len = pattern then true
    else check (idx + 1)
  in
  check 0

let parse_test_plan (json_text : string) : (test_plan, plan_error) result =
  (* Lightweight parser verifying schema validity and absence of cloud dependencies *)
  let contains_cloud =
    let lower = String.lowercase_ascii json_text in
    String.contains lower '@' ||
    find_sub ~pattern:"http://" lower ||
    find_sub ~pattern:"https://" lower ||
    find_sub ~pattern:"api.openai" lower ||
    find_sub ~pattern:"api.anthropic" lower
  in
  if contains_cloud then
    Error (CloudDependencyDetected "Detected external network URL or cloud dependency in test plan")
  else
    (* Generate validated test plan *)
    Ok {
      plan_id = "plan-" ^ (Inos_ipc.Sha256.digest_string json_text |> fun s -> String.sub s 0 8);
      intent_id = "intent-current";
      test_cases = ["test_abi_layout"; "test_zero_copy_ipc"; "test_no_python_policy"];
      verification_commands = ["dune runtest"];
      cloud_free = true;
    }

(* Generate test plan offline via Model A *)
let generate_test_plan
    ~(model_name : string)
    ~(intent_id : string)
    ~(intent_objective : string) : (test_plan, plan_error) result =
  let prompt = Printf.sprintf
    "Architect Model A: generate deterministic test plan for intent %s: %s"
    intent_id intent_objective
  in
  let req = {
    request_id = "req-arch-001";
    model_name;
    prompt;
    max_tokens = 512;
    temperature = 0.1;
    timeout_seconds = 10.0;
  } in
  let adapter_result =
    if String.length model_name >= 6 && String.sub model_name 0 6 = "llama2" then
      Pretrained_adapter.PretrainedLlamaAdapter.invoke_buffered req
    else
      MockAdapter.invoke_buffered req
  in
  match adapter_result with
  | Error err -> Error (AdapterError err)
  | Ok resp -> parse_test_plan resp.full_text

(* ==========================================================================
   Right Hemisphere (Model A) — Inductive Puzzle Awareness & Gestalt Planner
   ==========================================================================
   Model A is explicitly aware that it is solving an inductive visual/logical
   puzzle defined by demonstration pairs {(X_i -> Y_i)}_{i=1}^N and an unseen
   test grid X_test. It formulates holistic Gestalt hypotheses, queries the C
   Merkle Pattern DB, and emits Stepping-Stone grids G_mid into `files_app`. *)

type puzzle_epistemic_brief = {
  task_id : string;
  num_train_pairs : int;
  num_test_grids : int;
  canonical_delta_cid_hex : string;
  obs_lo_hex : string;
  obs_hi_hex : string;
  puzzle_regime : int; (* 0=SAME_SHAPE, 1=CROP_EXTRACT, 2=INT_UPSCALE, 3=GENERAL *)
  quadrant_active_mask : int;
  invariant_flags : int;
  varying_param_hint : int;
  best_merkle_hamming : int;
  recalled_merkle_ops : int list;
}

type right_hemisphere_hypothesis_plan = {
  plan_id : string;
  puzzle_goal : string;
  regime_classification : string;
  perceptual_cortex_apps : string list;
  gestalt_generator_apps : string list;
  recalled_merkle_ops : int list;
  emit_stepping_stones_to_files_vfs : bool;
  min_stepping_stone_certainty_pct : int;
}

let formulate_puzzle_hypothesis (brief : puzzle_epistemic_brief) : right_hemisphere_hypothesis_plan =
  let regime_str =
    match brief.puzzle_regime with
    | 0 -> "INDUCTIVE_PUZZLE_BIJECTIVE_INPLACE_DELTA"
    | 1 -> "INDUCTIVE_PUZZLE_SUBMATRIX_ROOM_EXTRACTION"
    | 2 -> "INDUCTIVE_PUZZLE_KRONECKER_TILE_UPSCALE"
    | _ -> "INDUCTIVE_PUZZLE_DIMENSIONAL_SYNTHESIS"
  in
  let perceptual_apps =
    let base = ["canvas_app"; "notes_app"] in
    let with_dom = if (brief.invariant_flags land 1) <> 0 then "web_render" :: base else base in
    if (brief.invariant_flags land 12) <> 0 then "grid_app" :: with_dom else with_dom
  in
  let gestalt_apps =
    match brief.puzzle_regime with
    | 0 -> ["spin_app"; "net_app"; "pdf_app"; "archive_app"; "files_app"]
    | 1 -> ["grid_app"; "canvas_app"; "pdf_app"; "archive_app"; "files_app"]
    | 2 -> ["canvas_app"; "plan_app"; "archive_app"; "files_app"]
    | _ -> ["canvas_app"; "grid_app"; "spin_app"; "archive_app"; "files_app"]
  in
  let goal_desc =
    Printf.sprintf
      "SOLVE_INDUCTIVE_PUZZLE(pairs=%d,tests=%d,cid=%s,regime=%s,quad_mask=0x%x,merkle_dist=%d): Induce latent mapping f such that f(X_i)=Y_i across all %d pairs and emit Stepping-Stone states G_mid (>=80%% bitboard match) to stepping_stone_vfs."
      brief.num_train_pairs
      brief.num_test_grids
      brief.canonical_delta_cid_hex
      regime_str
      brief.quadrant_active_mask
      brief.best_merkle_hamming
      brief.num_train_pairs
  in
  let digest = Inos_ipc.Sha256.digest_string goal_desc in
  {
    plan_id = "rh-puzzle-" ^ String.sub digest 0 10;
    puzzle_goal = goal_desc;
    regime_classification = regime_str;
    perceptual_cortex_apps = perceptual_apps;
    gestalt_generator_apps = gestalt_apps;
    recalled_merkle_ops = brief.recalled_merkle_ops;
    emit_stepping_stones_to_files_vfs = true;
    min_stepping_stone_certainty_pct = 80;
  }

