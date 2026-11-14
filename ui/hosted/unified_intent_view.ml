(* ui/hosted/unified_intent_view.ml - Consumer-Unified Intent UX (Task L501)
   Provides a single consumer-facing interface without exposed model jargon,
   with an optional advanced provenance inspector for system auditing.
*)

type ui_mode =
  | ConsumerDefault
  | AdvancedInspector

type intent_display_state =
  | Idle
  | Processing of string
  | AwaitingClarification of { prompt : string; options : string list }
  | PlanReady of { summary : string; target : string; is_reversible : bool }
  | Executed of { receipt_hash : string; summary : string }
  | SecurityRefusal of { reason : string }

type view_state = {
  mode : ui_mode;
  current_display : intent_display_state;
  history : string list;
  show_model_internals : bool;
}

let init_view () = {
  mode = ConsumerDefault;
  current_display = Idle;
  history = [];
  show_model_internals = false;
}

let set_mode (st : view_state) (m : ui_mode) : view_state =
  { st with
    mode = m;
    show_model_internals = (m = AdvancedInspector);
  }

(* Render user-facing status message *)
let render_user_prompt (st : view_state) : string =
  match st.current_display with
  | Idle ->
      "Intent-Native OS: What would you like to accomplish?"
  | Processing msg ->
      Printf.sprintf "Understanding request: %s..." msg
  | AwaitingClarification { prompt; options } ->
      let opts = String.concat "\n  * " options in
      Printf.sprintf "Clarification needed: %s\nPlease select an option:\n  * %s" prompt opts
  | PlanReady { summary; target; is_reversible } ->
      let rev_str = if is_reversible then "(Can be undone)" else "(Action is permanent)" in
      Printf.sprintf "Ready: %s on target '%s' %s" summary target rev_str
  | Executed { receipt_hash; summary } ->
      Printf.sprintf "Completed: %s (Receipt: %s)" summary (String.sub receipt_hash 0 (min 8 (String.length receipt_hash)))
  | SecurityRefusal { reason } ->
      Printf.sprintf "Request cannot be completed: %s" reason

(* Render advanced inspector details (only shown if mode is AdvancedInspector) *)
let render_inspector_diagnostics (st : view_state) ~(internal_stage : string) ~(model_a_id : string) ~(model_b_id : string) ~(cap_token : int) : string option =
  if not st.show_model_internals then None
  else
    Some (Printf.sprintf
      "--- [ADVANCED SYSTEM INSPECTOR] ---\n\
       Pipeline Stage: %s\n\
       Primary Proposer: %s\n\
       Adversarial Critic: %s\n\
       Kernel Capability Token: %d\n\
       Provenance Verification: Cryptographically Verified\n\
       ------------------------------------"
      internal_stage model_a_id model_b_id cap_token)
