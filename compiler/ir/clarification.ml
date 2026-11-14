(* compiler/ir/clarification.ml - Clarification Question Selection by Consequence *)

type action_consequence =
  | LowRiskKnownMove of { source : string; destination : string }
  | LowRiskRead of { target : string }
  | AmbiguousDelete of { candidate_targets : string list }
  | DestructiveMutation of { target : string; operation : string }
  | UnresolvedTargetSelection of { context_description : string; candidates : string list }

type clarification_question = {
  question_id : string;
  prompt : string;
  target_candidates : string list;
  severity : string; (* "CRITICAL" | "WARNING" | "INFO" *)
  recommended_action : string option;
}

(* Core Invariant / Task I008:
   Low-risk known move asks zero questions (returns None).
   Ambiguous delete asks a specific targeted question (returns Some question). *)
let select_clarification_question (action : action_consequence) : clarification_question option =
  match action with
  | LowRiskKnownMove _ | LowRiskRead _ ->
      (* Low-risk known moves require zero confirmation dialogs or questions *)
      None

  | AmbiguousDelete { candidate_targets } ->
      let candidates_str = String.concat ", " candidate_targets in
      Some {
        question_id = "q-ambiguous-delete";
        prompt = Printf.sprintf "Delete requested without unique target (%s). Which exact path should be deleted?" candidates_str;
        target_candidates = candidate_targets;
        severity = "CRITICAL";
        recommended_action = None;
      }

  | DestructiveMutation { target; operation } ->
      Some {
        question_id = "q-destructive-mutation";
        prompt = Printf.sprintf "Operation '%s' will overwrite or alter target '%s'. Proceed with staging snapshot?" operation target;
        target_candidates = [target];
        severity = "WARNING";
        recommended_action = Some "Create staging snapshot and review diff";
      }

  | UnresolvedTargetSelection { context_description; candidates } ->
      Some {
        question_id = "q-unresolved-target";
        prompt = Printf.sprintf "Multiple candidates available for %s: please select target." context_description;
        target_candidates = candidates;
        severity = "INFO";
        recommended_action = (match candidates with [first; _] -> Some first | _ -> None);
      }
