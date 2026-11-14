(* models/adapter/envelope.ml - Strict Structured A-Plan / B-Result Handoff Envelopes *)

type role = Architect | Implementer

type intent_reference = {
  id : string;
  revision : int;
  hash : string;
}

type handoff_limits = {
  max_wall_seconds : int;
  max_iterations : int;
  max_memory_bytes : int64;
}

type handoff_envelope = {
  schema : string;
  handoff_id : string;
  intent_ref : intent_reference;
  plan_ref_hash : string;
  from_role : role;
  to_role : role;
  iteration : int;
  operation : string;
  allowed_effects : string list;
  capability_scope : string list;
  limits : handoff_limits;
  payload : string;
  envelope_hash : string;
}

type validation_error =
  | UnknownSchema of string
  | MissingRequiredField of string
  | UnknownEffectRejected of string
  | InvalidIterationCount of int
  | RoleMismatch of { expected : string; found : string }

let current_handoff_schema = "inos.handoff.v0.2"

let known_permitted_effects = [
  "READ_CONTEXT";
  "GENERATE_DESIGN_ARTIFACT";
  "STAGE_FILE";
  "RUN_VERIFICATION";
  "WRITE_STAGING";
  "EXECUTE_TEST_RUNNER";
]

let string_of_role = function
  | Architect -> "ARCHITECT"
  | Implementer -> "IMPLEMENTER"

let role_of_string = function
  | "ARCHITECT" -> Ok Architect
  | "IMPLEMENTER" -> Ok Implementer
  | s -> Error (Printf.sprintf "Invalid handoff role: '%s'" s)

let compute_envelope_hash handoff_id intent_ref iteration operation payload =
  let content = Printf.sprintf "%s|%s|%d|%d|%s|%s"
    handoff_id intent_ref.id intent_ref.revision iteration operation payload in
  Inos_ipc.Sha256.digest_string content

let make_envelope
    ~handoff_id
    ~intent_ref
    ~plan_ref_hash
    ~from_role
    ~to_role
    ~iteration
    ~operation
    ~allowed_effects
    ~capability_scope
    ?(limits = { max_wall_seconds = 600; max_iterations = 3; max_memory_bytes = 2147483648L })
    ~payload
    () : handoff_envelope =
  let envelope_hash = compute_envelope_hash handoff_id intent_ref iteration operation payload in
  {
    schema = current_handoff_schema;
    handoff_id;
    intent_ref;
    plan_ref_hash;
    from_role;
    to_role;
    iteration;
    operation;
    allowed_effects;
    capability_scope;
    limits;
    payload;
    envelope_hash;
  }

(* Core Invariant / Task R004:
   Strict validation: rejects invalid fields and unknown effects *)
let validate_envelope (env : handoff_envelope) : (unit, validation_error) result =
  if env.schema <> current_handoff_schema then
    Error (UnknownSchema env.schema)
  else if String.length env.handoff_id = 0 then
    Error (MissingRequiredField "handoff_id")
  else if String.length env.intent_ref.id = 0 then
    Error (MissingRequiredField "intent_ref.id")
  else if String.length env.intent_ref.hash = 0 then
    Error (MissingRequiredField "intent_ref.hash")
  else if env.iteration < 1 then
    Error (InvalidIterationCount env.iteration)
  else
    (* Check allowed effects against known permitted set *)
    let unknown_effect_opt = List.find_opt (fun eff ->
      not (List.mem eff known_permitted_effects)
    ) env.allowed_effects in
    match unknown_effect_opt with
    | Some unknown ->
        Error (UnknownEffectRejected unknown)
    | None ->
        Ok ()
