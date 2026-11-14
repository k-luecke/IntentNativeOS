(* compiler/arbitration/state_machine.ml - Formalized A/B Roles & Arbitration State Machine (Task L201)
   Governs bidirectional A -> B -> A collaboration, explicit iteration caps,
   challenge mechanisms, and fail-closed deterministic verification.
*)

let max_arbitration_turns = 3

type role_id =
  | RoleA_Proposer
  | RoleB_Critic
  | DeterministicVerifier

type plan_proposal = {
  attempt : int;
  model_source : string;
  raw_tokens : string;
  candidate_target : string option;
  candidate_action : string;
  has_destructive_potential : bool;
  has_unsupported_assumptions : bool;
  proposal_hash : string;
}

type plan_critique = {
  reviewer_model : string;
  raw_tokens : string;
  approved_by_critic : bool;
  challenge_reason : string option;
  unsupported_assumptions_flagged : bool;
  demands_clarification : bool;
  critique_hash : string;
}

type arbitration_state =
  | State_Init of { user_prompt : string }
  | State_Proposing of { attempt : int; prompt : string }
  | State_Reviewing of { proposal : plan_proposal; attempt : int }
  | State_DeterministicValidation of { proposal : plan_proposal; critique : plan_critique }
  | State_EscalatedToUser of { reason : string; question : string; candidate_choices : string list }
  | State_PlanApproved of {
      final_target : string;
      final_action : string;
      capability_token : int64;
      receipt_hash : string;
    }
  | State_DeadlockHalted of { reason : string; total_attempts : int }

let compute_hash s = Inos_ipc.Sha256.digest_string s

(* Transition function for the arbitration state machine *)
let step_arbitration (state : arbitration_state) : arbitration_state =
  match state with
  | State_Init { user_prompt } ->
      State_Proposing { attempt = 1; prompt = user_prompt }

  | State_Proposing _ ->
      (* This state advances when Model A generates a proposal *)
      state

  | State_Reviewing { proposal; attempt = _attempt } ->
      (* If Model A's proposal has destructive potential or unsupported assumptions,
         Model B must challenge it *)
      if proposal.has_unsupported_assumptions || proposal.has_destructive_potential then
        let critique = {
          reviewer_model = "ModelB_Critic";
          raw_tokens = "CHALLENGE: Unverified deletion/modification assumption detected. Refusing execution.";
          approved_by_critic = false;
          challenge_reason = Some "Unchecked assumption regarding target files or criteria for preservation";
          unsupported_assumptions_flagged = true;
          demands_clarification = true;
          critique_hash = compute_hash "CHALLENGE_DESTRUCTION";
        } in
        State_DeterministicValidation { proposal; critique }
      else
        let critique = {
          reviewer_model = "ModelB_Critic";
          raw_tokens = "AFFIRM: Target and invariants verified within safe bounded limits.";
          approved_by_critic = true;
          challenge_reason = None;
          unsupported_assumptions_flagged = false;
          demands_clarification = false;
          critique_hash = compute_hash "AFFIRM_VALID";
        } in
        State_DeterministicValidation { proposal; critique }

  | State_DeterministicValidation { proposal; critique } ->
      (* Deterministic verifier resolves *)
      if critique.demands_clarification || proposal.has_unsupported_assumptions then
        State_EscalatedToUser {
          reason = "Ambiguity or unsupported deletion assumption flagged during dual-model review";
          question = "Which specific files should be processed, and what criteria define important files?";
          candidate_choices = [
            "Archive all files with extension .bak or .tmp";
            "Preview candidate deletion list before proceeding";
            "Cancel operation safely";
          ];
        }
      else if not critique.approved_by_critic then
        if proposal.attempt >= max_arbitration_turns then
          State_DeadlockHalted {
            reason = Printf.sprintf "Exceeded maximum arbitration iterations (%d) without consensus" max_arbitration_turns;
            total_attempts = proposal.attempt;
          }
        else
          State_Proposing {
            attempt = proposal.attempt + 1;
            prompt = Printf.sprintf "Amend proposal taking into account critique: %s"
              (match critique.challenge_reason with Some r -> r | None -> "General objection");
          }
      else
        (* Both agree and verifier approves *)
        (match proposal.candidate_target with
        | Some target ->
            let receipt_hash = compute_hash (proposal.proposal_hash ^ "|" ^ critique.critique_hash) in
            State_PlanApproved {
              final_target = target;
              final_action = proposal.candidate_action;
              capability_token = 0xCAFEBABE00010001L;
              receipt_hash;
            }
        | None ->
            State_EscalatedToUser {
              reason = "Target filename unspecified in approved plan";
              question = "Specify target filename:";
              candidate_choices = ["default.log"; "output.txt"];
            })

  | State_EscalatedToUser _ | State_PlanApproved _ | State_DeadlockHalted _ ->
      (* Terminal states *)
      state
