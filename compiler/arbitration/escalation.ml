(* compiler/arbitration/escalation.ml - Disagreement Escalation and Preservation (Task L205)
   Preserves role disagreements without fabricated consensus, routing unresolved
   disputes to deterministic verification or contextual user clarification.
*)

type disagreement_cause =
  | TargetMismatch of { a_target : string; b_target : string }
  | SafetyRejection of { challenge : string }
  | UnsupportedAssumption of { assumption : string }
  | IterationExhaustion of { turns : int }

type disagreement_receipt = {
  receipt_id : string;
  timestamp : float;
  cause : disagreement_cause;
  model_a_proposal_hash : string;
  model_b_critique_hash : string;
  escalation_action : [`RequestClarification | `HaltDeadlock | `DeterministicDenial];
  clarification_prompt : string option;
  receipt_signature : string;
}

let compute_hash s = Inos_ipc.Sha256.digest_string s

let escalate_disagreement
    ~(cause : disagreement_cause)
    ~(a_proposal_hash : string)
    ~(b_critique_hash : string) : disagreement_receipt =
  let (action, prompt_opt) =
    match cause with
    | SafetyRejection { challenge } ->
        (`RequestClarification,
         Some (Printf.sprintf "Operation halted: Model B identified safety objection: %s. Confirm intent before proceeding." challenge))
    | UnsupportedAssumption { assumption } ->
        (`RequestClarification,
         Some (Printf.sprintf "Ambiguity detected: %s. Please clarify exact criteria and target files." assumption))
    | TargetMismatch { a_target; b_target } ->
        (`RequestClarification,
         Some (Printf.sprintf "Model A proposed target '%s' while Model B argued for '%s'. Which is correct?" a_target b_target))
    | IterationExhaustion { turns } ->
        (`HaltDeadlock,
         Some (Printf.sprintf "Arbitration failed to converge after %d turns. Execution denied for safety." turns))
  in

  let receipt_id = Printf.sprintf "esc-%f-%d" (Unix.gettimeofday ()) (Random.int 100000) in
  let sig_payload = Printf.sprintf "%s|%s|%s" receipt_id a_proposal_hash b_critique_hash in

  {
    receipt_id;
    timestamp = Unix.gettimeofday ();
    cause;
    model_a_proposal_hash = a_proposal_hash;
    model_b_critique_hash = b_critique_hash;
    escalation_action = action;
    clarification_prompt = prompt_opt;
    receipt_signature = compute_hash sig_payload;
  }
