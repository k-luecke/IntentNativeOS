(* compiler/arbitration/router.ml - Budget-Aware Collaboration Router (Task L206)
   Routes easy/routine intents through single-model or deterministic fast-paths,
   triggering expensive dual-model arbitration only when estimated risks or ambiguity warrant it.
*)

type consequence_level =
  | ReadOnly          (* Zero mutation: diff, inspect, view *)
  | ReversibleMutation (* Single file creation/update with snapshot undo *)
  | IrreversibleRisk  (* Deletion, disk format, batch overwrite, privilege change *)

type routing_decision =
  | RouteDeterministicOnly
  | RouteSingleModel of { model_tier : string }
  | RouteDualModelReview of {
      proposer : string;
      critic : string;
      reason : string;
    }

let classify_consequence (prompt : string) : consequence_level =
  let p_lower = String.lowercase_ascii prompt in
  let contains_any needles =
    List.exists (fun n ->
      let len_p = String.length p_lower in
      let len_n = String.length n in
      if len_n > len_p then false
      else
        let found = ref false in
        let i = ref 0 in
        while !i <= len_p - len_n && not !found do
          if String.sub p_lower !i len_n = n then found := true;
          incr i
        done;
        !found
    ) needles
  in

  if contains_any ["delete"; "wipe"; "clean up"; "remove"; "format"; "drop"; "overwrite mbr"] then
    IrreversibleRisk
  else if contains_any ["create"; "write"; "save"; "update"; "append"] then
    ReversibleMutation
  else
    ReadOnly

let route_intent ~(prompt : string) ~(requires_model : bool) : routing_decision =
  let consequence = classify_consequence prompt in
  if not requires_model then
    RouteDeterministicOnly
  else
    match consequence with
    | ReadOnly ->
        RouteSingleModel { model_tier = "compact-0.5B" }
    | ReversibleMutation ->
        (* Simple reversible changes use compact single model unless prompt is complex *)
        if String.length prompt > 120 || String.contains prompt ';' then
          RouteDualModelReview {
            proposer = "compact-0.5B";
            critic = "larger-3B";
            reason = "Compound multi-step mutation detected";
          }
        else
          RouteSingleModel { model_tier = "compact-0.5B" }
    | IrreversibleRisk ->
        (* ALL destructive or irreversible requests mandate dual-model review *)
        RouteDualModelReview {
          proposer = "compact-0.5B";
          critic = "larger-3B";
          reason = "High-consequence destructive operation mandates independent Model B audit";
        }

(* Stateful dynamical routing via Tonic Gate (L302) integrated with consequence classifier *)
let route_intent_tonic
    ~(tonic_cfg : Inos_tonic.Tonic_gate.config)
    (tonic_state : Inos_tonic.Tonic_gate.state)
    ~(prompt : string)
    ?(system_load = 0.1)
    () : Inos_tonic.Tonic_gate.state * routing_decision =
  let consequence = classify_consequence prompt in
  let (next_state, tonic_route) =
    Inos_tonic.Tonic_gate.route_prompt ~cfg:tonic_cfg tonic_state ~prompt ~system_load
  in
  let decision =
    match consequence, tonic_route with
    | IrreversibleRisk, _
    | _, Inos_tonic.Tonic_gate.RouteDualModelReview ->
        RouteDualModelReview {
          proposer = "compact-0.5B";
          critic = "larger-3B";
          reason = "Tonic gate high-tone or irreversible consequence triggered dual-model audit";
        }
    | _, Inos_tonic.Tonic_gate.RouteSingleModel ->
        RouteSingleModel { model_tier = "compact-0.5B" }
    | _, Inos_tonic.Tonic_gate.RouteDeterministic ->
        RouteDeterministicOnly
  in
  (next_state, decision)

