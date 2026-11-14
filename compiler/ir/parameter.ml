(* compiler/ir/parameter.ml - Experimental vs Binding Parameters and Mutation Authorization *)

open Mode
open Contract

type parameter_role =
  | Experimental
  | Binding

type effect =
  | ReadContext
  | GenerateDesignArtifact
  | ModifySolver
  | LaunchOptimization
  | FileWrite of string
  | FileDelete of string
  | ProcessSpawn of string

type parameter = {
  name : string;
  value : string;
  origin : value_origin;
  binding : binding;
  role : parameter_role;
}

let string_of_parameter_role = function
  | Experimental -> "EXPERIMENTAL"
  | Binding -> "BINDING"

let string_of_effect = function
  | ReadContext -> "READ_CONTEXT"
  | GenerateDesignArtifact -> "GENERATE_DESIGN_ARTIFACT"
  | ModifySolver -> "MODIFY_SOLVER"
  | LaunchOptimization -> "LAUNCH_OPTIMIZATION"
  | FileWrite p -> "FILE_WRITE:" ^ p
  | FileDelete p -> "FILE_DELETE:" ^ p
  | ProcessSpawn cmd -> "PROCESS_SPAWN:" ^ cmd

let classify_role = function
  | Example | Preference -> Experimental
  | Constraint | Authorized_action -> Binding

let make_parameter ~name ~value ~origin ~binding =
  let role = classify_role binding in
  { name; value; origin; binding; role }

let is_mutating_effect = function
  | ReadContext | GenerateDesignArtifact -> false
  | ModifySolver | LaunchOptimization | FileWrite _ | FileDelete _ | ProcessSpawn _ -> true

type effect_validation_error =
  | ExperimentalParameterCannotAuthorizeMutation of { parameter_name : string; forbidden_effect : string }
  | ConsequentialEffectInExploratoryMode of { mode : mode; effect : string }
  | UnauthorizedEffect of string

(* Core Invariant / Spec Section 3.2:
   A selected example must not be upgraded to a binding optimization objective.
   Latency-as-example test never authorizes solver edits or optimization runs. *)
let validate_requested_effects
    ~(mode : mode)
    ~(parameters : parameter list)
    ~(requested_effects : effect list) : (effect list, effect_validation_error) result =
  (* Check 1: If mode is non-consequential (Explore, Design, Simulate), mutating effects are forbidden *)
  if not (Mode.is_consequential mode) then
    let violating = List.find_opt is_mutating_effect requested_effects in
    match violating with
    | Some eff ->
        Error (ConsequentialEffectInExploratoryMode { mode; effect = string_of_effect eff })
    | None -> Ok requested_effects
  else
    (* Check 2: Even in consequential mode, if key operational parameters are only Experimental/Example,
       they cannot authorize mutations on their target *)
    let experimental_params = List.filter (fun p -> p.role = Experimental) parameters in
    let has_experimental = List.length experimental_params > 0 in
    if has_experimental then
      let mutating_opt = List.find_opt is_mutating_effect requested_effects in
      match mutating_opt with
      | Some eff ->
          let exp_p = List.hd experimental_params in
          Error (ExperimentalParameterCannotAuthorizeMutation {
            parameter_name = exp_p.name;
            forbidden_effect = string_of_effect eff;
          })
      | None -> Ok requested_effects
    else
      Ok requested_effects
