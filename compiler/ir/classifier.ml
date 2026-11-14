(* compiler/ir/classifier.ml - Deterministic Request Classifier for Direct Action vs Clarification *)

open Mode
open Contract
open Resolver

type classification_result =
  | DirectAction of { target : target_ref; action : string; mode : mode }
  | ClarificationRequired of { reason : string; blocking_ambiguities : string list }

let is_go_command s =
  let norm = String.lowercase_ascii (String.trim s) in
  norm = "go" || norm = "do it" || norm = "run" || norm = "execute" || norm = "apply"

(* Core Invariant / Task I007 & Spec Section 4:
   'go' with no uniquely approved pending target yields CLARIFICATION_REQUIRED.
   'go' with exactly one valid target dispatches direct action. *)
let classify_request
    ~(raw_input : string)
    ~(mode : mode)
    ~(workspace : workspace_context)
    ~(pending_targets : target_ref list) : classification_result =
  let is_go = is_go_command raw_input in
  if is_go then
    match pending_targets with
    | [] ->
        (* No pending targets in queue; check if workspace has a unique active selection *)
        (match workspace.selected_files with
         | [] ->
             ClarificationRequired {
               reason = "'go' with no selected target yields clarification";
               blocking_ambiguities = ["No pending target in contract and no workspace file selected"];
             }
         | [single_file] ->
             DirectAction {
               target = FileTarget single_file;
               action = "execute";
               mode = (if mode = Explore then Design else mode);
             }
         | files ->
             ClarificationRequired {
               reason = "'go' issued with multiple workspace files selected";
               blocking_ambiguities = files;
             })
    | [single_target] ->
        DirectAction {
          target = single_target;
          action = "execute";
          mode = (if Mode.is_consequential mode then mode else Execute);
        }
    | multiple ->
        ClarificationRequired {
          reason = "Multiple pending targets queued; explicit selection required";
          blocking_ambiguities = List.map Contract.string_of_target multiple;
        }
  else
    (* Non-go command: check if input references an ambiguous target *)
    match resolve_reference workspace raw_input with
    | Resolved t ->
        DirectAction { target = t; action = raw_input; mode }
    | Ambiguous { phrase; candidates; explanation } ->
        ClarificationRequired {
          reason = Printf.sprintf "Reference '%s' is ambiguous: %s" phrase explanation;
          blocking_ambiguities = candidates;
        }
    | Unresolved { phrase; explanation } ->
        ClarificationRequired {
          reason = Printf.sprintf "Reference '%s' unresolved: %s" phrase explanation;
          blocking_ambiguities = [phrase];
        }
