(* compiler/ir/state_machine.ml - Intent State Machine with Legal Transitions and Audit Trails *)

open Authority
open Clarification

type state =
  | Received
  | ContextResolved
  | IntentDrafted
  | ClarificationRequired of clarification_question
  | Ready
  | Exploration
  | Design
  | Simulation
  | AwaitingAuthorization of { requested_scope : string list }
  | Authorized of authority_token
  | Planning
  | Executing
  | Verifying
  | Completed of { result_hash : string }
  | RevisionRequired of { feedback : string }
  | Failed of { error : string }
  | Rejected of { reason : string }
  | Expired

type event =
  | ResolveContext
  | DraftIntent
  | RequireClarification of clarification_question
  | SubmitClarification of string
  | MarkReady
  | EnterExploration
  | EnterDesign
  | EnterSimulation
  | RequestAuthorization of string list
  | GrantAuthorization of authority_token
  | RejectAuthorization of string
  | Expire
  | StartPlanning
  | StartExecuting
  | StartVerifying
  | Complete of string
  | RequestRevision of string
  | Fail of string

type audit_record = {
  timestamp : float;
  from_state : string;
  attempted_event : string;
  legal : bool;
  reason : string;
}

type machine = {
  current_state : state;
  audit_trail : audit_record list;
}

let string_of_state = function
  | Received -> "RECEIVED"
  | ContextResolved -> "CONTEXT_RESOLVED"
  | IntentDrafted -> "INTENT_DRAFTED"
  | ClarificationRequired _ -> "CLARIFICATION_REQUIRED"
  | Ready -> "READY"
  | Exploration -> "EXPLORATION"
  | Design -> "DESIGN"
  | Simulation -> "SIMULATION"
  | AwaitingAuthorization _ -> "AWAITING_AUTHORIZATION"
  | Authorized _ -> "AUTHORIZED"
  | Planning -> "PLANNING"
  | Executing -> "EXECUTING"
  | Verifying -> "VERIFYING"
  | Completed _ -> "COMPLETED"
  | RevisionRequired _ -> "REVISION_REQUIRED"
  | Failed _ -> "FAILED"
  | Rejected _ -> "REJECTED"
  | Expired -> "EXPIRED"

let string_of_event = function
  | ResolveContext -> "ResolveContext"
  | DraftIntent -> "DraftIntent"
  | RequireClarification _ -> "RequireClarification"
  | SubmitClarification _ -> "SubmitClarification"
  | MarkReady -> "MarkReady"
  | EnterExploration -> "EnterExploration"
  | EnterDesign -> "EnterDesign"
  | EnterSimulation -> "EnterSimulation"
  | RequestAuthorization _ -> "RequestAuthorization"
  | GrantAuthorization _ -> "GrantAuthorization"
  | RejectAuthorization _ -> "RejectAuthorization"
  | Expire -> "Expire"
  | StartPlanning -> "StartPlanning"
  | StartExecuting -> "StartExecuting"
  | StartVerifying -> "StartVerifying"
  | Complete _ -> "Complete"
  | RequestRevision _ -> "RequestRevision"
  | Fail _ -> "Fail"

let create () = {
  current_state = Received;
  audit_trail = [];
}

type step_error =
  | IllegalTransition of {
      from_state : string;
      event : string;
      reason : string;
      audit : audit_record;
    }

(* Core Invariant / Task I009 & Spec Section 4:
   State machine enforces legal transitions according to the clarification gate spec.
   Invalid transitions fail closed and are audited. *)
let step ?(now = Unix.gettimeofday ()) (m : machine) (ev : event) : (machine, step_error * machine) result =
  let from_str = string_of_state m.current_state in
  let ev_str = string_of_event ev in
  let legal_next_opt =
    match (m.current_state, ev) with
    | (Received, ResolveContext) -> Some ContextResolved
    | (ContextResolved, DraftIntent) -> Some IntentDrafted
    | (IntentDrafted, RequireClarification q) -> Some (ClarificationRequired q)
    | (ClarificationRequired _, SubmitClarification _) -> Some IntentDrafted
    | (IntentDrafted, MarkReady) -> Some Ready
    | (Ready, EnterExploration) -> Some Exploration
    | (Ready, EnterDesign) -> Some Design
    | (Ready, EnterSimulation) -> Some Simulation
    | (Ready, RequestAuthorization scope) -> Some (AwaitingAuthorization { requested_scope = scope })
    | (Simulation, RequestAuthorization scope) -> Some (AwaitingAuthorization { requested_scope = scope })
    | (AwaitingAuthorization _, GrantAuthorization tok) -> Some (Authorized tok)
    | (AwaitingAuthorization _, RejectAuthorization r) -> Some (Rejected { reason = r })
    | (AwaitingAuthorization _, Expire) -> Some Expired
    | (Authorized _, StartPlanning) -> Some Planning
    | (Planning, StartExecuting) -> Some Executing
    | (Executing, StartVerifying) -> Some Verifying
    | (Verifying, Complete hash) -> Some (Completed { result_hash = hash })
    | (Verifying, RequestRevision fb) -> Some (RevisionRequired { feedback = fb })
    | (RevisionRequired _, DraftIntent) -> Some IntentDrafted
    | (Verifying, Fail err) -> Some (Failed { error = err })
    | (Executing, Fail err) -> Some (Failed { error = err })
    | (Planning, Fail err) -> Some (Failed { error = err })
    | _ -> None
  in
  match legal_next_opt with
  | Some next_state ->
      let audit = {
        timestamp = now;
        from_state = from_str;
        attempted_event = ev_str;
        legal = true;
        reason = "Legal state transition";
      } in
      Ok {
        current_state = next_state;
        audit_trail = audit :: m.audit_trail;
      }
  | None ->
      let audit = {
        timestamp = now;
        from_state = from_str;
        attempted_event = ev_str;
        legal = false;
        reason = Printf.sprintf "Illegal state transition from %s via %s" from_str ev_str;
      } in
      let updated_machine = { m with audit_trail = audit :: m.audit_trail } in
      Error (IllegalTransition {
        from_state = from_str;
        event = ev_str;
        reason = audit.reason;
        audit;
      }, updated_machine)
