(* security/agent_boundary.ml - SDU-01 Agent Trust Boundaries & Defense-in-Depth Sanitizer (S502, S503) *)
open Types

type instruction_origin =
  | Trusted_System_Instruction
  | User_Interactive_Prompt
  | Untrusted_External_Document
  | Untrusted_Web_Page
  | Untrusted_Tool_Output
  | Untrusted_Cross_Agent_Message

type taint_level =
  | Clean
  | Tainted of string list (* List of untrusted provenance sources *)

type prompt_segment = {
  origin : instruction_origin;
  raw_content : string;
  sanitized_content : string;
  taint : taint_level;
  detected_threats : string list;
}

type tool_invocation_request = {
  tool_id : string;
  target_resource : string;
  action : op_action;
  caller_agent_id : string;
  context_taint : taint_level;
  presented_grant : grant_token option;
  requested_at : int64;
}

type agent_boundary_decision =
  | Execution_Permitted of grant_token
  | Execution_Denied of string

(* Defense-in-depth: Regex / string pattern matcher for prompt injection vectors *)
let known_injection_signatures = [
  ("ignore previous instructions", "Prompt_Override_Injection");
  ("ignore all prior instructions", "Prompt_Override_Injection");
  ("system override", "System_Privilege_Injection");
  ("you are now in developer mode", "Jailbreak_Persona_Shift");
  ("you are now DAN", "Jailbreak_Persona_Shift");
  ("format disk", "Destructive_Command_Injection");
  ("delete all files", "Destructive_Command_Injection");
  ("escalate to root", "Privilege_Escalation_Injection");
  ("<|im_start|>", "Delimiter_Spoofing");
  ("[INST]", "Delimiter_Spoofing");
  ("<system>", "Delimiter_Spoofing");
]

let sanitize_prompt_text (text : string) : string * string list =
  let lower = String.lowercase_ascii text in
  let detected = ref [] in
  List.iter (fun (sig_text, threat_name) ->
    if String.length lower >= String.length sig_text then begin
      (* Check for substring match *)
      let rec check_sub i =
        if i + String.length sig_text <= String.length lower then
          if String.sub lower i (String.length sig_text) = sig_text then begin
            if not (List.mem threat_name !detected) then
              detected := threat_name :: !detected
          end else
            check_sub (i + 1)
      in
      check_sub 0
    end
  ) known_injection_signatures;

  (* Redact detected threats in sanitized text *)
  let sanitized = List.fold_left (fun acc (sig_text, _) ->
    let pat = sig_text in
    let rec replace_all str =
      match String.length str with
      | 0 -> ""
      | _ ->
          if String.length str >= String.length pat then
            if String.lowercase_ascii (String.sub str 0 (String.length pat)) = pat then
              "[REDACTED_INJECTION_VECTOR]" ^ replace_all (String.sub str (String.length pat) (String.length str - String.length pat))
            else
              String.sub str 0 1 ^ replace_all (String.sub str 1 (String.length str - 1))
          else str
    in
    replace_all acc
  ) text known_injection_signatures in

  (sanitized, !detected)

let ingest_segment
    ~(origin : instruction_origin)
    ~(content : string)
    ~(prior_taint : taint_level) : prompt_segment =
  let (sanitized, threats) = sanitize_prompt_text content in
  let new_taint = match origin with
    | Trusted_System_Instruction ->
        prior_taint
    | User_Interactive_Prompt ->
        (match threats with
         | [] -> prior_taint
         | _ -> match prior_taint with
                | Clean -> Tainted ["Adversarial_User_Input"]
                | Tainted srcs -> Tainted ("Adversarial_User_Input" :: srcs))
    | Untrusted_External_Document ->
        (match prior_taint with
         | Clean -> Tainted ["External_Document"]
         | Tainted srcs -> Tainted ("External_Document" :: srcs))
    | Untrusted_Web_Page ->
        (match prior_taint with
         | Clean -> Tainted ["Web_Page"]
         | Tainted srcs -> Tainted ("Web_Page" :: srcs))
    | Untrusted_Tool_Output ->
        (match prior_taint with
         | Clean -> Tainted ["Tool_Output"]
         | Tainted srcs -> Tainted ("Tool_Output" :: srcs))
    | Untrusted_Cross_Agent_Message ->
        (match prior_taint with
         | Clean -> Tainted ["Cross_Agent_Message"]
         | Tainted srcs -> Tainted ("Cross_Agent_Message" :: srcs))
  in
  {
    origin;
    raw_content = content;
    sanitized_content = sanitized;
    taint = new_taint;
    detected_threats = threats;
  }

(* Primary authorization boundary: Tool invocations require valid kernel capability grants *)
let evaluate_tool_invocation
    (broker : Broker.broker_state)
    (req : tool_invocation_request)
    : agent_boundary_decision =
  (* Rule 1: Tool call must present a valid grant token *)
  match req.presented_grant with
  | None ->
      Execution_Denied "Denied: Tool call missing kernel-enforced authorization grant token"
  | Some grant ->
      (* Rule 2: Reconstruct expected privileged operation *)
      let op = {
        action = req.action;
        target = req.target_resource;
        parameters = [("tool", req.tool_id); ("caller", req.caller_agent_id)];
        caller_id = req.caller_agent_id;
        timestamp = req.requested_at;
        constraints = [];
      } in

      (* Rule 3: Tainted context cannot self-authorize high-privilege operations *)
      (match req.context_taint with
       | Tainted srcs when req.action = File_Delete || req.action = VM_Capsule_Launch || req.action = Kernel_Module_Load ->
           let src_str = String.concat "," srcs in
           Execution_Denied (Printf.sprintf "Denied: Tainted execution context (%s) prohibited from high-privilege tool execution without interactive step-up verification" src_str)
       | _ ->
           (* Rule 4: Validate grant in broker (checking replay, TTL, canonical digest) *)
           match Broker.validate_and_consume_grant broker grant.grant_id op req.requested_at with
           | Ok consumed_token ->
               Execution_Permitted consumed_token
           | Error Broker.GrantNotFound ->
               Execution_Denied "Denied: Forged or non-existent capability grant"
           | Error Broker.GrantAlreadyConsumed ->
               Execution_Denied "Denied: Capability grant already consumed (Replay attack blocked)"
           | Error (Broker.GrantExpired (curr, exp)) ->
               Execution_Denied (Printf.sprintf "Denied: Capability grant expired (current=%Ld, exp=%Ld)" curr exp)
           | Error (Broker.DigestMismatch (tok_d, curr_d)) ->
               Execution_Denied (Printf.sprintf "Denied: Scope expansion mismatch (token digest %s != operation digest %s)" tok_d curr_d))

(* ========================================================================= *)
(* S503: AI Epistemic Authority Boundaries                                   *)
(* Cardinal Invariant: Verified_System_Fact <> Execution Authority           *)
(* Facts and inferences cannot synthesize or mint capability grants.         *)
(* Monotonic cross-agent delegation strictly attenuates rights downwards.   *)
(* ========================================================================= *)

type epistemic_status =
  | Verified_System_Fact of {
      source : string;
      measurement : string;
      verified_at : int64;
    }
  | Interactive_Human_Command of {
      operator_id : string;
      command : string;
      session_id : string;
    }
  | Model_Inference of {
      model_id : string;
      inference : string;
      confidence_score : float;
    }
  | Untrusted_External_Ingress of {
      ingress_type : string;
      payload : string;
      url_or_path : string;
    }

type epistemic_error =
  | EpistemicFactCannotAuthorize of string
  | EpistemicInferenceCannotAuthorize of string
  | EpistemicUntrustedIngressDenied of string
  | EpistemicDelegationAttenuateUpwardDenied of string
  | EpistemicScopeExpansionDenied of string
  | EpistemicMaxDepthExceeded of int * int

let is_epistemic_authority (status : epistemic_status) : bool =
  match status with
  | Interactive_Human_Command _ -> true
  | Verified_System_Fact _ -> false
  | Model_Inference _ -> false
  | Untrusted_External_Ingress _ -> false

let validate_epistemic_authorization_source (status : epistemic_status) : (unit, epistemic_error) result =
  match status with
  | Interactive_Human_Command _ -> Ok ()
  | Verified_System_Fact { source; measurement; _ } ->
      Error (EpistemicFactCannotAuthorize
        (Printf.sprintf "Denied: Epistemic invariant violated. Verified fact from '%s' (%s) is informative data, NOT execution authority."
          source measurement))
  | Model_Inference { model_id; confidence_score; _ } ->
      Error (EpistemicInferenceCannotAuthorize
        (Printf.sprintf "Denied: Epistemic invariant violated. Model inference from '%s' (confidence=%.2f) cannot self-mint authorization grants."
          model_id confidence_score))
  | Untrusted_External_Ingress { ingress_type; url_or_path; _ } ->
      Error (EpistemicUntrustedIngressDenied
        (Printf.sprintf "Denied: Untrusted ingress of type '%s' from '%s' cannot authorize operations."
          ingress_type url_or_path))

type delegated_agent_context = {
  agent_id : string;
  parent_agent_id : string option;
  delegation_depth : int;
  allowed_actions : op_action list;
  allowed_resources : string list;
  epistemic_origin : epistemic_status;
}

let create_root_agent_context
    ~(agent_id : string)
    ~(allowed_actions : op_action list)
    ~(allowed_resources : string list)
    ~(origin : epistemic_status) : (delegated_agent_context, epistemic_error) result =
  match validate_epistemic_authorization_source origin with
  | Error err -> Error err
  | Ok () ->
      Ok {
        agent_id;
        parent_agent_id = None;
        delegation_depth = 0;
        allowed_actions;
        allowed_resources;
        epistemic_origin = origin;
      }

let delegate_agent_authority
    ~(delegator : delegated_agent_context)
    ~(delegatee_id : string)
    ~(requested_actions : op_action list)
    ~(requested_resources : string list)
    ~(max_depth : int)
    : (delegated_agent_context, epistemic_error) result =
  (* 1. Check max delegation depth *)
  if delegator.delegation_depth >= max_depth then
    Error (EpistemicMaxDepthExceeded (delegator.delegation_depth, max_depth))
  else
    (* 2. Monotonic attenuation check for actions: requested_actions must be a subset of delegator.allowed_actions *)
    let unheld_action = List.find_opt (fun act -> not (List.mem act delegator.allowed_actions)) requested_actions in
    match unheld_action with
    | Some act ->
        Error (EpistemicDelegationAttenuateUpwardDenied
          (Printf.sprintf "Denied: Cannot delegate action '%s' not held by delegator '%s'"
            (Canonical.string_of_action act) delegator.agent_id))
    | None ->
        (* 3. Scope expansion check: requested resources must match or be subpaths of delegator resources *)
        let unheld_resource = List.find_opt (fun res ->
          not (List.exists (fun allowed ->
            String.equal res allowed ||
            (String.length res > String.length allowed &&
             String.sub res 0 (String.length allowed) = allowed)
          ) delegator.allowed_resources)
        ) requested_resources in
        match unheld_resource with
        | Some res ->
            Error (EpistemicScopeExpansionDenied
              (Printf.sprintf "Denied: Scope expansion attack. Resource '%s' exceeds delegator's allowed boundaries" res))
        | None ->
            Ok {
              agent_id = delegatee_id;
              parent_agent_id = Some delegator.agent_id;
              delegation_depth = delegator.delegation_depth + 1;
              allowed_actions = requested_actions;
              allowed_resources = requested_resources;
              epistemic_origin = delegator.epistemic_origin;
            }

let evaluate_agent_tool_invocation
    (broker : Broker.broker_state)
    (agent : delegated_agent_context)
    (req : tool_invocation_request)
    : agent_boundary_decision =
  (* Check 1: Action must be within agent's allowed delegated actions *)
  if not (List.mem req.action agent.allowed_actions) then
    Execution_Denied (Printf.sprintf "Denied: Action '%s' exceeds delegated authority for agent '%s'"
      (Canonical.string_of_action req.action) agent.agent_id)
  else
    (* Check 2: Target resource must be within agent's allowed resources *)
    let resource_allowed = List.exists (fun allowed ->
      String.equal req.target_resource allowed ||
      (String.length req.target_resource > String.length allowed &&
       String.sub req.target_resource 0 (String.length allowed) = allowed)
    ) agent.allowed_resources in
    if not resource_allowed then
      Execution_Denied (Printf.sprintf "Denied: Target resource '%s' exceeds delegated resource boundaries for agent '%s'"
        req.target_resource agent.agent_id)
    else
      (* Check 3: Hand off to kernel capability gate and broker validation *)
      evaluate_tool_invocation broker req

(* ========================================================================= *)
(* IntScript (.ints) Static Security & Capability Auditor                    *)
(* ========================================================================= *)

let contains_substr (haystack : string) (needle : string) : bool =
  let hlen = String.length haystack in
  let nlen = String.length needle in
  if nlen = 0 then true
  else if nlen > hlen then false
  else
    let rec loop i =
      if i > hlen - nlen then false
      else if String.sub haystack i nlen = needle then true
      else loop (i + 1)
    in
    loop 0

let extract_quoted_literals (src : string) : string list =
  let len = String.length src in
  let rec loop i in_quote start acc =
    if i >= len then List.rev acc
    else if src.[i] = '"' then
      if in_quote then
        let lit = String.sub src start (i - start) in
        loop (i + 1) false 0 (lit :: acc)
      else
        loop (i + 1) true (i + 1) acc
    else
      loop (i + 1) in_quote start acc
  in
  loop 0 false 0 []

let is_file_like_literal (lit : string) : bool =
  let file_exts =
    [ ".xlsx"; ".txt"; ".pdf"; ".md"; ".csv"; ".log"; ".svg";
      ".ics"; ".eml"; ".zip"; ".json"; ".ini"; ".dat"; ".note"; ".doc" ]
  in
  List.exists (fun ext ->
    let elen = String.length ext in
    let llen = String.length lit in
    llen > elen && String.sub lit (llen - elen) elen = ext && not (contains_substr lit " ")
  ) file_exts

let audit_intscript_source (script_src : string) ~(allowed_paths : string list) : (unit, string) result =
  let lower_src = String.lowercase_ascii script_src in
  (* 1. Reject path traversal or restricted system/workspace directory access *)
  let forbidden_path_patterns =
    [ ".."; ".workspace"; "private"; "/etc/"; "/root"; "\\etc\\"; "c:\\" ]
  in
  match List.find_opt (fun pat -> contains_substr lower_src pat) forbidden_path_patterns with
  | Some pat ->
      Error (Printf.sprintf "IntScript Security Violation: forbidden path or traversal pattern '%s' detected" pat)
  | None ->
      (* 2. Reject unbounded loop constructs *)
      let unbounded_loops =
        [ "while(true)"; "while (true)"; "for(;;)"; "for (;;)" ]
      in
      (match List.find_opt (fun lp -> contains_substr lower_src lp) unbounded_loops with
       | Some lp ->
           Error (Printf.sprintf "IntScript Security Violation: unbounded loop '%s' rejected by termination guard" lp)
       | None ->
           (* 3. Verify all quoted file-like literals are within allowed_paths *)
           let literals = extract_quoted_literals script_src in
           let file_literals = List.filter is_file_like_literal literals in
           let unauthorized =
             List.find_opt (fun flit ->
               not (List.exists (fun allowed -> String.equal flit allowed) allowed_paths)
             ) file_literals
           in
           (match unauthorized with
            | Some bad_file ->
                Error
                  (Printf.sprintf
                     "IntScript Security Violation: file target '%s' is outside allowed ScriptIsolationContext paths [%s]"
                     bad_file (String.concat ", " allowed_paths))
            | None -> Ok ()))

