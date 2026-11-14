(* compiler/ir/contract.ml - Typed Intent Contract and Qualified Values *)

open Mode

type value_origin =
  | User_explicit
  | User_selected_example
  | Workspace_observed
  | Model_inferred
  | Policy_default

type binding =
  | Example
  | Preference
  | Constraint
  | Authorized_action

type 'a qualified = {
  value : 'a;
  origin : value_origin;
  binding : binding;
  evidence : string list;
}

type target_ref =
  | FileTarget of string
  | DirectoryTarget of string
  | ProjectTarget of string
  | UnresolvedTarget of string

type material_ambiguity = {
  field_path : string;
  alternatives : string list;
  consequence : string;
}

type intent_contract = {
  id : string;
  revision : int;
  mode : mode;
  objective : string qualified;
  targets : target_ref qualified list;
  constraints : string qualified list;
  ambiguities : material_ambiguity list;
  verification_criteria : string list;
  authority_scope : string list;
  parent_revision_hash : string option;
  content_hash : string;
}

let string_of_origin = function
  | User_explicit -> "USER_EXPLICIT"
  | User_selected_example -> "USER_SELECTED_EXAMPLE"
  | Workspace_observed -> "WORKSPACE_OBSERVED"
  | Model_inferred -> "MODEL_INFERRED"
  | Policy_default -> "POLICY_DEFAULT"

let origin_of_string = function
  | "USER_EXPLICIT" -> Ok User_explicit
  | "USER_SELECTED_EXAMPLE" -> Ok User_selected_example
  | "WORKSPACE_OBSERVED" -> Ok Workspace_observed
  | "MODEL_INFERRED" -> Ok Model_inferred
  | "POLICY_DEFAULT" -> Ok Policy_default
  | s -> Error (Printf.sprintf "Invalid origin: '%s'" s)

let string_of_binding = function
  | Example -> "EXAMPLE"
  | Preference -> "PREFERENCE"
  | Constraint -> "CONSTRAINT"
  | Authorized_action -> "AUTHORIZED_ACTION"

let binding_of_string = function
  | "EXAMPLE" -> Ok Example
  | "PREFERENCE" -> Ok Preference
  | "CONSTRAINT" -> Ok Constraint
  | "AUTHORIZED_ACTION" -> Ok Authorized_action
  | s -> Error (Printf.sprintf "Invalid binding: '%s'" s)

let string_of_target = function
  | FileTarget p -> "file:" ^ p
  | DirectoryTarget p -> "dir:" ^ p
  | ProjectTarget p -> "project:" ^ p
  | UnresolvedTarget p -> "unresolved:" ^ p

let target_of_string str =
  if String.starts_with ~prefix:"file:" str then
    FileTarget (String.sub str 5 (String.length str - 5))
  else if String.starts_with ~prefix:"dir:" str then
    DirectoryTarget (String.sub str 4 (String.length str - 4))
  else if String.starts_with ~prefix:"project:" str then
    ProjectTarget (String.sub str 8 (String.length str - 8))
  else if String.starts_with ~prefix:"unresolved:" str then
    UnresolvedTarget (String.sub str 11 (String.length str - 11))
  else
    UnresolvedTarget str

let compute_intent_hash
    ?parent_revision_hash
    ?(targets = [])
    ?(constraints = [])
    ?(ambiguities = [])
    ?(verification_criteria = [])
    ?(authority_scope = [])
    id revision mode objective =
  let targets_str = String.concat ";" (List.map (fun (t : target_ref qualified) ->
    Printf.sprintf "%s:%s:%s:%s"
      (string_of_target t.value)
      (string_of_origin t.origin)
      (string_of_binding t.binding)
      (String.concat "," t.evidence)
  ) targets) in
  let constraints_str = String.concat ";" (List.map (fun (c : string qualified) ->
    Printf.sprintf "%s:%s:%s:%s"
      c.value
      (string_of_origin c.origin)
      (string_of_binding c.binding)
      (String.concat "," c.evidence)
  ) constraints) in
  let ambiguities_str = String.concat ";" (List.map (fun (a : material_ambiguity) ->
    Printf.sprintf "%s:[%s]:%s"
      a.field_path
      (String.concat "," a.alternatives)
      a.consequence
  ) ambiguities) in
  let criteria_str = String.concat ";" verification_criteria in
  let scope_str = String.concat ";" authority_scope in
  let parent_str = match parent_revision_hash with None -> "" | Some h -> h in
  let obj_ev_str = String.concat "," objective.evidence in
  let content = Printf.sprintf "%s|%d|%s|%s|%s|%s|[%s]|[%s]|[%s]|[%s]|[%s]|[%s]|%s"
    id revision (Mode.to_string mode)
    objective.value (string_of_origin objective.origin) (string_of_binding objective.binding) obj_ev_str
    targets_str constraints_str ambiguities_str criteria_str scope_str parent_str
  in
  Inos_ipc.Sha256.digest_string content

let make ?parent_revision_hash ~id ~revision ~mode ~objective ~targets ~constraints ~ambiguities ~verification_criteria ~authority_scope () =
  let content_hash = compute_intent_hash
    ?parent_revision_hash
    ~targets
    ~constraints
    ~ambiguities
    ~verification_criteria
    ~authority_scope
    id revision mode objective
  in
  {
    id;
    revision;
    mode;
    objective;
    targets;
    constraints;
    ambiguities;
    verification_criteria;
    authority_scope;
    parent_revision_hash;
    content_hash;
  }

type validation_error =
  | MaterialAmbiguityUnresolved of string
  | ConsequentialModeWithNoTargets
  | InvalidHash of string * string

let validate (contract : intent_contract) =
  if Mode.is_consequential contract.mode && List.length contract.ambiguities > 0 then
    Error (MaterialAmbiguityUnresolved (Printf.sprintf "Contract has %d material ambiguities blocking consequential execution" (List.length contract.ambiguities)))
  else if Mode.is_consequential contract.mode && List.length contract.targets = 0 then
    Error ConsequentialModeWithNoTargets
  else
    let expected_hash = compute_intent_hash
      ?parent_revision_hash:contract.parent_revision_hash
      ~targets:contract.targets
      ~constraints:contract.constraints
      ~ambiguities:contract.ambiguities
      ~verification_criteria:contract.verification_criteria
      ~authority_scope:contract.authority_scope
      contract.id contract.revision contract.mode contract.objective
    in
    if expected_hash <> contract.content_hash then
      Error (InvalidHash (contract.content_hash, expected_hash))
    else
      Ok ()
