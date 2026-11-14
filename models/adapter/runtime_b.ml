(* models/adapter/runtime_b.ml - Local Model B (Implementer / Challenger) Adapter *)

open Protocol

type implementation_proposal = {
  proposal_id : string;
  plan_ref : string;
  proposed_modules : string list;
  proposed_diff : string;
  offline_verified : bool;
}

type proposal_error =
  | AdapterError of adapter_error
  | InvalidProposalSchema of string
  | CloudDependencyInProposal of string

let find_sub ~pattern str =
  let p_len = String.length pattern in
  let s_len = String.length str in
  let rec check idx =
    if idx + p_len > s_len then false
    else if String.sub str idx p_len = pattern then true
    else check (idx + 1)
  in
  check 0

let parse_proposal ~(plan_ref : string) (raw_text : string) : (implementation_proposal, proposal_error) result =
  let lower = String.lowercase_ascii raw_text in
  if find_sub ~pattern:"http://" lower || find_sub ~pattern:"https://" lower || find_sub ~pattern:"cloud" lower then
    Error (CloudDependencyInProposal "External cloud network reference detected in proposal")
  else
    Ok {
      proposal_id = "prop-" ^ (Inos_ipc.Sha256.digest_string raw_text |> fun s -> String.sub s 0 8);
      plan_ref;
      proposed_modules = ["kernel/memory/paging.rs"; "core/ipc/shm.ml"];
      proposed_diff = "--- a/core/ipc/shm.ml\n+++ b/core/ipc/shm.ml\n@@ -1 +1 @@\n+(* zero copy shm *)\n";
      offline_verified = true;
    }

let generate_implementation_proposal
    ~(model_name : string)
    ~(plan_ref : string)
    ~(plan_description : string) : (implementation_proposal, proposal_error) result =
  let prompt = Printf.sprintf
    "Implementer Model B: generate pure offline implementation proposal for plan %s: %s"
    plan_ref plan_description
  in
  let req = {
    request_id = "req-impl-001";
    model_name;
    prompt;
    max_tokens = 1024;
    temperature = 0.2;
    timeout_seconds = 15.0;
  } in
  let adapter_result =
    if String.length model_name >= 6 && String.sub model_name 0 6 = "llama2" then
      Pretrained_adapter.PretrainedLlamaAdapter.invoke_buffered req
    else
      MockAdapter.invoke_buffered req
  in
  match adapter_result with
  | Error err -> Error (AdapterError err)
  | Ok resp -> parse_proposal ~plan_ref resp.full_text

(* ==========================================================================
   Left Hemisphere (Model B) — Analytic Critic, Residual Solver & No-Abstain
   ==========================================================================
   Model B receives Model A's Gestalt hypothesis plan over the Callosal Bus
   (`mail_app` / `Envelope`) and knows its cognitive role in solving the puzzle:
   1. Bind cross-pair varying parameters (`varying_param_hint`: minority marker,
      dominant color, border color, or object count).
   2. Prune algebraically redundant operator pairs (`T^2=T`, `AoB=A`, `AB=BA`)
      via the C `RH` catalog (`inos_merkle_should_prune_composition`).
   3. Repair Model A's Stepping-Stone grids `G_mid` by solving the residual
      sub-puzzle `partial(G_mid, Y_i)` (`P_patch o P_gestalt`).
   4. Enforce the No-Abstain Best-Guess Policy: never fall back to "I don't know"
      when a high-certainty Stepping Stone (e.g. 90-99% training match) exists;
      always emit the top 2 distinct predictions (`attempt_1 != attempt_2`). *)

type left_hemisphere_refinement_plan = {
  proposal_id : string;
  plan_ref : string;
  analytic_apps : string list;
  parameter_binding_rule : string;
  residual_repair_strategy : string;
  enforce_rh_algebraic_pruning : bool;
  require_orthogonal_pass2 : bool;
  never_abstain_use_best_stepping_stone : bool;
}

let synthesize_bicameral_puzzle_refinement
    ~(rh_plan : Runtime_a.right_hemisphere_hypothesis_plan)
    ~(varying_param_hint : int)
    ~(specialist_blend_csv : string) : left_hemisphere_refinement_plan =
  let param_rule =
    match varying_param_hint with
    | 1 -> "BIND_OUTPUT_COLOR_TO_INPUT_DOMINANT_FG"
    | 2 -> "BIND_OUTPUT_COLOR_TO_INPUT_MINORITY_MARKER_DOT"
    | 3 -> "BIND_OUTPUT_PARAM_TO_UNIQUE_COLOR_OR_OBJECT_COUNT"
    | 4 -> "BIND_OUTPUT_COLOR_TO_BORDER_FRAME_COLOR"
    | _ -> "INVARIANT_FIXED_OR_BIJECTIVE_PALETTE_LAW"
  in
  let analytic_apps = ["script_app"; "calc_app"; "write_app"; "intscript_app"; "plan_app"; "security_console"] in
  let strategy =
    Printf.sprintf
      "CALLOSAL_REFINEMENT(rh_plan=%s,param_law=%s,mole=%s): (1) Replay C-Merkle Top-16 via script_app; (2) Prune T^2=T & AB=BA via RH matrix; (3) Solve residual sub-puzzle partial(G_mid,Y) via P_patch o P_gestalt; (4) Emit Top-2 orthogonal answers (never abstain on high-certainty Stepping Stones)."
      rh_plan.plan_id
      param_rule
      specialist_blend_csv
  in
  let digest = Inos_ipc.Sha256.digest_string strategy in
  {
    proposal_id = "lh-puzzle-" ^ String.sub digest 0 10;
    plan_ref = rh_plan.plan_id;
    analytic_apps;
    parameter_binding_rule = param_rule;
    residual_repair_strategy = strategy;
    enforce_rh_algebraic_pruning = true;
    require_orthogonal_pass2 = true;
    never_abstain_use_best_stepping_stone = true;
  }

