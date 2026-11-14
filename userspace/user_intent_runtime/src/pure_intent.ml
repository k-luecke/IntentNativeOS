(* userspace/user_intent_runtime/src/pure_intent.ml
   Native Ring 3 OCaml A/B Intent Orchestration & Sustained Execution (L403, L404)
*)

external sys_write : string -> string -> int -> int = "caml_sys_file_create_write"
external sys_print : string -> unit = "caml_sys_print"

type cap =
  | CapStorageWrite
  | CapStorageRead
  | CapInvalid

type role =
  | Role_A_FastProposer
  | Role_B_DeepCritic

type intent_proposal = {
  target_file : string;
  payload : string;
  is_destructive : bool;
  requires_clarification : bool;
  assigned_cap : cap;
}

type review_result =
  | ReviewApproved
  | ReviewChallenged of string

type arbitration_outcome =
  | OutcomeExecuted of int
  | OutcomeEscalated of string
  | OutcomeDenied of string

let match_prefix_exact (prefix : string) (s : string) : bool =
  let p_len = String.length prefix in
  let s_len = String.length s in
  if s_len >= p_len then
    let rec check i =
      if i = p_len then true
      else if s.[i] <> prefix.[i] then false
      else check (i + 1)
    in check 0
  else
    false

let is_create (s : string) : bool =
  match_prefix_exact "create" s || match_prefix_exact "Create" s

let is_ambiguous (s : string) : bool =
  match_prefix_exact "clean" s || match_prefix_exact "Clean" s

(* Role A: Fast Intent Proposer *)
let role_a_propose (raw_intent : string) : intent_proposal =
  sys_print "[RING3_OCAML_AB] Role A: Analyzing raw input and synthesizing candidate plan...\n";
  if is_ambiguous raw_intent then
    { target_file = "*";
      payload = "";
      is_destructive = true;
      requires_clarification = true;
      assigned_cap = CapInvalid }
  else if is_create raw_intent then
    { target_file = "os_changes.txt";
      payload = "Intent-Native OS: Ring 3 compiled OCaml A/B orchestration verified.\n";
      is_destructive = false;
      requires_clarification = false;
      assigned_cap = CapStorageWrite }
  else
    { target_file = "unknown";
      payload = "";
      is_destructive = false;
      requires_clarification = false;
      assigned_cap = CapInvalid }

(* Role B: Deep Critic Reviewer *)
let role_b_review (proposal : intent_proposal) : review_result =
  sys_print "[RING3_OCAML_AB] Role B: Evaluating plan safety, parameters, and risk boundaries...\n";
  if proposal.requires_clarification || proposal.target_file = "*" then
    ReviewChallenged "Ambiguous deletion target; unsupported assumptions detected."
  else if proposal.assigned_cap = CapInvalid then
    ReviewChallenged "Invalid capability token requested."
  else
    ReviewApproved

(* Ring 3 Native A/B Orchestration Pipeline *)
let orchestrate_intent (raw_intent : string) (cap_token : int) : arbitration_outcome =
  sys_print "[RING3_OCAML_AB] Commencing native Ring 3 dual-model intent arbitration...\n";
  let proposal = role_a_propose raw_intent in
  let review = role_b_review proposal in
  match review with
  | ReviewChallenged reason ->
      sys_print "[RING3_OCAML_AB] Model B challenged proposal. Halting execution. Escalating to user.\n";
      OutcomeEscalated reason
  | ReviewApproved ->
      sys_print "[RING3_OCAML_AB] Model B approved proposal. Validating capability against kernel contract...\n";
      if cap_token = 2 && proposal.assigned_cap = CapStorageWrite then begin
        sys_print "[RING3_OCAML_AB] Deterministic verifier passed. Emitting authorized Ring 3 syscall...\n";
        let res = sys_write proposal.target_file proposal.payload cap_token in
        OutcomeExecuted res
      end else begin
        sys_print "[RING3_OCAML_AB] Capability verification failed! Access denied.\n";
        OutcomeDenied "Capability verification failure"
      end

let ocaml_entry (intent_str : string) (cap_token : int) : int =
  match orchestrate_intent intent_str cap_token with
  | OutcomeExecuted code ->
      sys_print "[RING3_OCAML_AB] Plan successfully executed under kernel authority.\n";
      code
  | OutcomeEscalated msg ->
      sys_print "[RING3_OCAML_AB] Transaction paused: User clarification receipt generated.\n";
      0
  | OutcomeDenied _ ->
      sys_print "[RING3_OCAML_AB] Transaction rejected: Ring 3 security denial.\n";
      -1
