(* compiler/ir/mode.ml - Typed Intent Execution Modes *)

type mode =
  | Explore
  | Design
  | Simulate
  | Execute
  | Maintain

type transition_error =
  | SilentEscalationForbidden of { from_mode : mode; to_mode : mode }
  | InvalidTransition of string

let to_string = function
  | Explore -> "EXPLORE"
  | Design -> "DESIGN"
  | Simulate -> "SIMULATE"
  | Execute -> "EXECUTE"
  | Maintain -> "MAINTAIN"

let of_string str =
  match String.uppercase_ascii (String.trim str) with
  | "EXPLORE" -> Ok Explore
  | "DESIGN" -> Ok Design
  | "SIMULATE" -> Ok Simulate
  | "EXECUTE" -> Ok Execute
  | "MAINTAIN" -> Ok Maintain
  | s -> Error (Printf.sprintf "Invalid intent mode: '%s'. Expected EXPLORE, DESIGN, SIMULATE, EXECUTE, or MAINTAIN." s)

let is_consequential = function
  | Execute | Maintain -> true
  | Explore | Design | Simulate -> false

(* Invariant I-02: EXPLORE, DESIGN, and SIMULATE MUST NOT silently transition to EXECUTE or MAINTAIN *)
let validate_transition ~from_mode ~to_mode ~authorized =
  match (from_mode, to_mode) with
  | ((Explore | Design | Simulate), (Explore | Design | Simulate)) ->
      Ok () (* Exploratory transitions permitted without elevation *)
  | ((Explore | Design | Simulate), (Execute | Maintain)) ->
      if authorized then Ok ()
      else Error (SilentEscalationForbidden { from_mode; to_mode })
  | ((Execute | Maintain), (Explore | Design | Simulate)) ->
      Ok () (* De-escalation always safe *)
  | ((Execute | Maintain), (Execute | Maintain)) ->
      Ok () (* Lateral transitions within consequential modes *)
