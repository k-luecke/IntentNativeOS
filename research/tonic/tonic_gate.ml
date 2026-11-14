(* Tonic Gating - Experimental Research Module
   Quarantined behind feature flag: INOS_ENABLE_TONIC_GATING (Default: OFF)
*)

type config = {
  enabled : bool;
  lambda : float;
  theta_fast : float;
  theta_arbitration : float;
  weight_ambiguity : float;
  weight_risk : float;
  weight_resource : float;
  bias : float;
}

let default_config = {
  enabled = (match Sys.getenv_opt "INOS_ENABLE_TONIC_GATING" with
             | Some "1" | Some "true" | Some "TRUE" -> true
             | _ -> false);
  lambda = 0.35;
  theta_fast = 0.25;
  theta_arbitration = 0.65;
  weight_ambiguity = 1.5;
  weight_risk = 2.0;
  weight_resource = 0.5;
  bias = 2.0;
}

type state = {
  tone : float;
  step_count : int;
  is_calibrated : bool;
}

let init_state () = {
  tone = 0.0;
  step_count = 0;
  is_calibrated = true;
}

type route =
  | RouteDeterministic
  | RouteSingleModel
  | RouteDualModelReview

let sigmoid (x : float) : float =
  1.0 /. (1.0 +. exp (-. x))

(* Compute instantaneous tonic drive *)
let compute_instantaneous_drive ~(cfg : config) ~ambiguity ~risk ~resource =
  (* Monotonic risk elevation safety invariant *)
  if risk >= 0.8 then 1.0
  else
    let z = (cfg.weight_ambiguity *. ambiguity) +.
            (cfg.weight_risk *. risk) +.
            (cfg.weight_resource *. resource) -.
            cfg.bias in
    sigmoid z

(* Specialized runner for high-frequency execution (avoids record indirection) *)
let make_runner ~(cfg : config) =
  if not cfg.enabled then
    fun (st : state) ~ambiguity:_ ~risk ~resource:_ ->
      let route =
        if risk >= 0.7 then RouteDualModelReview
        else if risk >= 0.3 then RouteSingleModel
        else RouteDeterministic in
      (st, route)
  else
    let w_a = cfg.weight_ambiguity in
    let w_ri = cfg.weight_risk in
    let w_re = cfg.weight_resource in
    let b = cfg.bias in
    let lambda = cfg.lambda in
    let inv_lambda = 1.0 -. lambda in
    let t_fast = cfg.theta_fast in
    let t_arb = cfg.theta_arbitration in
    
    fun (st : state) ~ambiguity ~risk ~resource ->
      if not st.is_calibrated then
        let new_st = { st with tone = 1.0; step_count = st.step_count + 1 } in
        (new_st, RouteDualModelReview)
      else
        let inst_drive =
          if risk >= 0.8 then 1.0
          else
            let z = (w_a *. ambiguity) +. (w_ri *. risk) +. (w_re *. resource) -. b in
            sigmoid z
        in
        let new_tone =
          if risk >= 0.8 then 1.0
          else (lambda *. st.tone) +. (inv_lambda *. inst_drive)
        in
        let new_st = { st with tone = new_tone; step_count = st.step_count + 1 } in
        let route =
          if new_tone < t_fast then RouteDeterministic
          else if new_tone < t_arb then RouteSingleModel
          else RouteDualModelReview
        in
        (new_st, route)

(* Update tonic state given sensory and system features *)
let step_gate ~(cfg : config) (st : state) ~ambiguity ~risk ~resource : state * route =
  if not cfg.enabled then
    (* Feature flag OFF: Default static risk routing *)
    let route =
      if risk >= 0.7 then RouteDualModelReview
      else if risk >= 0.3 then RouteSingleModel
      else RouteDeterministic in
    (st, route)
  else if not st.is_calibrated then
    (* Fail-closed invariant: Uncalibrated gate defaults to maximum caution *)
    let new_st = { st with tone = 1.0; step_count = st.step_count + 1 } in
    (new_st, RouteDualModelReview)
  else
    let inst_drive = compute_instantaneous_drive ~cfg ~ambiguity ~risk ~resource in
    let new_tone =
      if risk >= 0.8 then 1.0 (* Hard clamp on destructive commands *)
      else (cfg.lambda *. st.tone) +. ((1.0 -. cfg.lambda) *. inst_drive) in
    let new_st = { st with tone = new_tone; step_count = st.step_count + 1 } in
    let route =
      if new_tone < cfg.theta_fast then RouteDeterministic
      else if new_tone < cfg.theta_arbitration then RouteSingleModel
      else RouteDualModelReview in
    (new_st, route)

(* Helper: substring check without regex allocation *)
let contains_sub (haystack : string) (needle : string) : bool =
  let h_len = String.length haystack in
  let n_len = String.length needle in
  if n_len = 0 then true
  else if n_len > h_len then false
  else
    let rec loop i =
      if i > h_len - n_len then false
      else if String.sub haystack i n_len = needle then true
      else loop (i + 1)
    in
    loop 0

(* Extract dynamical sensory features [H_lex(x), C_risk(x), U_system] from raw prompt *)
let extract_features ~(prompt : string) ~(system_load : float) : float * float * float =
  let lower = String.lowercase_ascii prompt in
  let ambig_terms = [
    "clean up"; "old"; "stuff"; "anything important"; "whatever";
    "maybe"; "everything"; "somehow"; "handle it"; "junk"; "mess"
  ] in
  let ambig_hits =
    List.fold_left (fun acc w -> if contains_sub lower w then acc + 1 else acc) 0 ambig_terms
  in
  let is_inductive_puzzle =
    contains_sub lower "solve_inductive_puzzle" || contains_sub lower "arc_regime_"
  in
  let ambiguity =
    if is_inductive_puzzle then
      if contains_sub lower "merkle_dist=0" then 0.22 else 0.50
    else
      min 1.0 (float_of_int ambig_hits *. 0.45)
  in
  let risk =
    if List.exists (contains_sub lower)
         ["delete"; "wipe"; "erase"; "format disk"; "format filesystem"; "format drive";
          "destroy"; "drop table"; "overwrite"; "rm -rf"; "grant root";
          "ignore previous"; "ignore prior"; "system override"; "exfiltrate"; "send password";
          "wire $"; "transfer $"; "/etc/"]
    then 0.90
    else if List.exists (contains_sub lower)
         ["modify"; "replace"; "rename"; "move"; "patch"; "update"]
    then 0.45
    else if List.exists (contains_sub lower)
         ["create"; "write"; "save"; "export"; "archive"; "copy"; "duplicate"]
    then 0.35
    else if is_inductive_puzzle then 0.25
    else 0.05
  in
  let resource = max 0.0 (min 1.0 system_load) in
  (ambiguity, risk, resource)

let route_prompt ~(cfg : config) (st : state) ~(prompt : string) ~(system_load : float) : state * route =
  let (ambiguity, risk, resource) = extract_features ~prompt ~system_load in
  step_gate ~cfg st ~ambiguity ~risk ~resource

(* ==========================================================================
   Paper 2 Adaptive Optimizing Engine Extensions:
   1. Coupled Multi-Expert Tonic Hum (h_{t+1} = (1 - lambda_c)(I + C) h_t + lambda_c a_0 1 + g_tonic u_t,
      effective spectral persistence rho_eff = 0.989, T_hold ~ 26.8-65.0 tokens)
      for bridging ambiguous evidence gaps (CSR_gap = 0.6525 vs 0.2987 1-step memory).
   2. Stateless Lookup Clarity Bypass:
      Paper 2 demonstrated that while tonic hold excels across ambiguous gaps, it incurs a
      0.42-segment switch lag on unambiguous regime shifts. When evidence clarity C_t >= theta_clarity
      and the dominant domain changes, the optimizing engine bypasses tonic inertia via
      Stateless_Lookup_Bypass for 0.0-segment switch lag.
   3. Adaptive Loop Depth Budgeter (T_star):
      Paper 2 proved that forcing deep recurrent loops (T=8..12) on easy/unambiguous tasks causes
      overthinking degradation (-34.2% at T=12 vs T_star=4). Caps depth T_star in {1, 2, 4, 8}.
   ========================================================================== *)

type routing_mechanism =
  | Stateless_Lookup_Bypass
  | Coupled_Tonic_Hold
  | Safety_Clamp_Abstain

type coupled_state = {
  expert_h : float array;     (* 5-domain tonic activation vector *)
  active_domain : int;        (* 0..4 *)
  domain_hold_turns : int;    (* consecutive turns held in active_domain *)
  rho_eff : float;            (* effective persistence eigenvalue ~ 0.989 *)
  last_mechanism : routing_mechanism;
  scalar_state : state;
}

let init_coupled_state () : coupled_state = {
  expert_h = Array.make 5 0.02;
  active_domain = 0;
  domain_hold_turns = 0;
  rho_eff = 0.989;
  last_mechanism = Stateless_Lookup_Bypass;
  scalar_state = init_state ();
}

(* Extract 5-domain sensory evidence vector u_t and evidence clarity C_t in [0.0, 1.0]
   Incorporates T11 ScoreAffinityProvider domain evidence for both OS and ARC regimes *)
let extract_domain_evidence (prompt : string) : float array * int * float =
  let lower = String.lowercase_ascii prompt in
  let u = Array.make 5 0.05 in
  let add_hits dom terms weight =
    List.iter (fun w ->
      if contains_sub lower w then u.(dom) <- u.(dom) +. weight
    ) terms
  in
  add_hits 0 ["create"; "write"; "save"; "draft"; "note"; "document"; "file"; "copy"; "duplicate"; "archive"; "rename"; "path"; "arc_regime_same"; "inplace_delta"] 0.45;
  add_hits 1 ["verify"; "receipt"; "receipts"; "hash"; "signature"; "digest"; "provenance"; "sha256"; "bijective_conservation"; "symmetry_verify"] 0.45;
  add_hits 2 ["restore"; "undo"; "rollback"; "snapshot"; "revert"; "recover"; "backup"; "arc_regime_crop"; "room_extraction"] 0.45;
  add_hits 3 ["audit"; "security"; "policy"; "capability"; "firewall"; "permissions"; "privilege"; "cellular_closure"; "pocket_fill"] 0.45;
  add_hits 4 ["query"; "telemetry"; "status"; "inspect"; "view"; "list"; "read"; "show"; "calculate"; "eval"; "sum"; "time"; "date"; "arc_regime_scale"; "tile_upscale"] 0.45;
  let best_dom = ref 0 in
  let max_v = ref u.(0) in
  let second_v = ref 0.0 in
  for i = 1 to 4 do
    if u.(i) > !max_v then begin
      second_v := !max_v;
      max_v := u.(i);
      best_dom := i
    end else if u.(i) > !second_v then
      second_v := u.(i)
  done;
  let margin = !max_v -. !second_v in
  let clarity = max 0.0 (min 1.0 (margin /. 0.45)) in
  (u, !best_dom, clarity)

(* Compute optimal recurrent/verification depth budget T_star to prevent Paper 2 overthinking *)
let compute_optimal_depth_budget ~(ambiguity : float) ~(risk : float) ~(clarity : float) : int =
  if risk >= 0.8 then 1 (* Immediate safety clamp / abstain *)
  else if clarity >= 0.75 && ambiguity < 0.20 && risk < 0.20 then 1 (* Routine deterministic *)
  else if ambiguity < 0.45 && risk < 0.65 then 4 (* Paper 2 sweet spot T_star=4, avoids T=12 overthinking *)
  else 8 (* High ambiguity or elevated risk requires deeper multi-step verification *)

let step_adaptive_engine
    ~(cfg : config)
    (cst : coupled_state)
    ~(prompt : string)
    ~(system_load : float) : coupled_state * route * routing_mechanism * int * float =
  let (ambiguity, risk, resource) = extract_features ~prompt ~system_load in
  let (u, instant_dom, clarity) = extract_domain_evidence prompt in
  let (next_scalar, base_route) = step_gate ~cfg cst.scalar_state ~ambiguity ~risk ~resource in
  let depth_budget = compute_optimal_depth_budget ~ambiguity ~risk ~clarity in
  if risk >= 0.8 then
    let clamped_h = Array.make 5 0.0 in
    clamped_h.(3) <- 1.0; (* Lock onto security/audit domain *)
    let next_cst = {
      expert_h = clamped_h;
      active_domain = 3;
      domain_hold_turns = 1;
      rho_eff = cst.rho_eff;
      last_mechanism = Safety_Clamp_Abstain;
      scalar_state = { next_scalar with tone = 1.0 };
    } in
    (next_cst, RouteDualModelReview, Safety_Clamp_Abstain, 1, clarity)
  else if clarity >= 0.72 && (instant_dom <> cst.active_domain || cst.scalar_state.step_count = 0) then
    (* Stateless Lookup Clarity Bypass: Clear evidence of a new domain -> zero switch lag *)
    let new_h = Array.init 5 (fun i -> if i = instant_dom then u.(i) else 0.02) in
    let hold_turns = if instant_dom = cst.active_domain && cst.scalar_state.step_count > 0 then cst.domain_hold_turns + 1 else 1 in
    let next_cst = {
      expert_h = new_h;
      active_domain = instant_dom;
      domain_hold_turns = hold_turns;
      rho_eff = cst.rho_eff;
      last_mechanism = Stateless_Lookup_Bypass;
      scalar_state = next_scalar;
    } in
    (next_cst, base_route, Stateless_Lookup_Bypass, depth_budget, clarity)
  else
    (* Coupled Tonic Hold: Bridge ambiguous follow-up or maintain current domain regime
       via h_{t+1} = rho_eff * h_t + (1 - rho_eff) * a_0 + g_tonic * u_t *)
    let rho = cst.rho_eff in
    let a0 = 0.02 in
    let g_tonic = 0.35 in
    let new_h = Array.init 5 (fun i ->
      let lateral =
        if i = cst.active_domain then 0.015 *. cst.expert_h.(i)
        else -0.005 *. cst.expert_h.(cst.active_domain)
      in
      (rho *. cst.expert_h.(i)) +. lateral +. ((1.0 -. rho) *. a0) +. (g_tonic *. u.(i))
    ) in
    let best_dom = ref cst.active_domain in
    let best_val = ref new_h.(!best_dom) in
    for i = 0 to 4 do
      if new_h.(i) > !best_val then begin
        best_val := new_h.(i);
        best_dom := i
      end
    done;
    let hold_turns =
      if !best_dom = cst.active_domain then cst.domain_hold_turns + 1 else 1
    in
    let next_cst = {
      expert_h = new_h;
      active_domain = !best_dom;
      domain_hold_turns = hold_turns;
      rho_eff = cst.rho_eff;
      last_mechanism = Coupled_Tonic_Hold;
      scalar_state = next_scalar;
    } in
    (next_cst, base_route, Coupled_Tonic_Hold, depth_budget, clarity)


