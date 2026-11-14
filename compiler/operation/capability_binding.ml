(* compiler/operation/capability_binding.ml - Capability-to-Operation Binding Contracts *)

open Operation_ir

type capability_grant = {
  grant_id : string;
  target_operation_id : string;
  target_resource : string;
  granted_capabilities : string list;
  permitted_effects : string list;
  expires_at : float;
}

type binding_error =
  | UndeclaredCapabilityBlocked of {
      operation_id : string;
      undeclared_cap : string;
      granted_caps : string list;
    }
  | EffectExceedsCapabilityGrant of {
      operation_id : string;
      forbidden_effect : string;
      permitted_effects : string list;
    }
  | TargetResourceMismatch of {
      operation_id : string;
      expected_target : string;
      attempted_target : string;
    }
  | GrantExpired of { grant_id : string; expired_at : float }
  | SandboxEscapeAttempted of string

let is_within_sandbox ~sandbox_root path =
  let norm = if String.ends_with ~suffix:"/" sandbox_root then sandbox_root else sandbox_root ^ "/" in
  String.starts_with ~prefix:norm path && not (
    let parts = String.split_on_char '/' path in
    List.mem ".." parts
  )

let execute_sandbox_operation
    ~(sandbox_root : string)
    ~(op_name : string)
    ~(target_path : string)
    ?(content = "")
    () : (string, binding_error) result =
  if not (is_within_sandbox ~sandbox_root target_path) then
    Error (SandboxEscapeAttempted target_path)
  else
    match op_name with
    | "read" -> Ok (Printf.sprintf "READ_FIXTURE:%s" target_path)
    | "hash" -> Ok (Inos_ipc.Sha256.digest_string (target_path ^ content))
    | "copy" -> Ok (Printf.sprintf "COPIED_INTO_SANDBOX:%s (bytes=%d)" target_path (String.length content))
    | other -> Error (EffectExceedsCapabilityGrant {
        operation_id = other;
        forbidden_effect = other;
        permitted_effects = ["read"; "hash"; "copy"];
      })

let string_of_effect = function
  | Pure -> "PURE"
  | ReadFs p -> "READ_FS:" ^ p
  | WriteFs p -> "WRITE_FS:" ^ p
  | MutateFs p -> "MUTATE_FS:" ^ p
  | ExecuteBinary p -> "EXECUTE:" ^ p
  | AllocateMemory b -> Printf.sprintf "ALLOCATE:%d" b

let extract_effect_resource = function
  | ReadFs p | WriteFs p | MutateFs p | ExecuteBinary p -> Some p
  | Pure | AllocateMemory _ -> None

(* Core Invariant / Task C004:
   Operation requesting undeclared capability blocked.
   Grants bind strictly to specific operations, targets, and effects. *)
let verify_operation_binding
    ?(now = Unix.gettimeofday ())
    (grant : capability_grant)
    (node : operation_node) : (unit, binding_error) result =
  if now > grant.expires_at then
    Error (GrantExpired { grant_id = grant.grant_id; expired_at = grant.expires_at })
  else if grant.target_operation_id <> node.node_id then
    Error (TargetResourceMismatch {
      operation_id = node.node_id;
      expected_target = grant.target_operation_id;
      attempted_target = node.node_id;
    })
  else
    (* Check 1: All required capabilities must be granted *)
    let undeclared_opt = List.find_opt (fun cap ->
      not (List.mem cap grant.granted_capabilities)
    ) node.required_capabilities in
    match undeclared_opt with
    | Some undeclared ->
        Error (UndeclaredCapabilityBlocked {
          operation_id = node.node_id;
          undeclared_cap = undeclared;
          granted_caps = grant.granted_capabilities;
        })
    | None ->
        (* Check 2: All effects must be permitted *)
        let forbidden_effect_opt = List.find_opt (fun eff ->
          let eff_str = string_of_effect eff in
          not (List.mem eff_str grant.permitted_effects || List.mem "ALL_EFFECTS" grant.permitted_effects)
        ) node.effects in
        match forbidden_effect_opt with
        | Some eff ->
            Error (EffectExceedsCapabilityGrant {
              operation_id = node.node_id;
              forbidden_effect = string_of_effect eff;
              permitted_effects = grant.permitted_effects;
            })
        | None ->
            (* Check 3: All resource-targeting effects must match grant.target_resource *)
            let mismatched_resource_opt = List.find_map (fun eff ->
              match extract_effect_resource eff with
              | Some res when res <> grant.target_resource && not (String.starts_with ~prefix:(grant.target_resource ^ "/") res) ->
                  Some res
              | _ -> None
            ) node.effects in
            match mismatched_resource_opt with
            | Some bad_target ->
                Error (TargetResourceMismatch {
                  operation_id = node.node_id;
                  expected_target = grant.target_resource;
                  attempted_target = bad_target;
                })
            | None ->
                Ok ()
