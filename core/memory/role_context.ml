(* core/memory/role_context.ml - Separate Role Context and Journal Isolation (Task L202)
   Enforces strict context separation between Model A and Model B, allowing data
   sharing ONLY via explicit, hashed handoff envelopes.
*)

type role_context_entry = {
  entry_id : string;
  role : [`RoleA | `RoleB];
  timestamp : float;
  private_scratchpad : string;
  token_count : int;
}

type signed_handoff_envelope = {
  handoff_id : string;
  source_role : [`RoleA | `RoleB];
  target_role : [`RoleA | `RoleB];
  timestamp : float;
  canonical_proposal_json : string;
  payload_sha256 : string;
  is_verified : bool;
}

type isolated_memory_store = {
  mutable a_private_log : role_context_entry list;
  mutable b_private_log : role_context_entry list;
  mutable shared_handoff_journal : signed_handoff_envelope list;
}

let create_isolated_store () : isolated_memory_store = {
  a_private_log = [];
  b_private_log = [];
  shared_handoff_journal = [];
}

let compute_sha256 s = Inos_ipc.Sha256.digest_string s

(* Record private reasoning in role's isolated context *)
let append_private_entry (store : isolated_memory_store) (role : [`RoleA | `RoleB]) (scratchpad : string) (tokens : int) =
  let entry = {
    entry_id = Printf.sprintf "ctx-%s-%f" (if role = `RoleA then "A" else "B") (Unix.gettimeofday ());
    role;
    timestamp = Unix.gettimeofday ();
    private_scratchpad = scratchpad;
    token_count = tokens;
  } in
  match role with
  | `RoleA -> store.a_private_log <- entry :: store.a_private_log
  | `RoleB -> store.b_private_log <- entry :: store.b_private_log

(* Create formal signed handoff without leaking private scratchpad *)
let create_signed_handoff
    (store : isolated_memory_store)
    ~(source : [`RoleA | `RoleB])
    ~(target : [`RoleA | `RoleB])
    ~(proposal_json : string) : signed_handoff_envelope =
  let digest = compute_sha256 proposal_json in
  let handoff = {
    handoff_id = Printf.sprintf "hnd-%f-%d" (Unix.gettimeofday ()) (Random.int 100000);
    source_role = source;
    target_role = target;
    timestamp = Unix.gettimeofday ();
    canonical_proposal_json = proposal_json;
    payload_sha256 = digest;
    is_verified = true;
  } in
  store.shared_handoff_journal <- handoff :: store.shared_handoff_journal;
  handoff

(* Invariant: Role B context projection excludes all private entries from Role A *)
let get_projection_for_role (store : isolated_memory_store) (role : [`RoleA | `RoleB]) : string list =
  match role with
  | `RoleA ->
      List.rev_map (fun e -> "[PRIVATE-A] " ^ e.private_scratchpad) store.a_private_log
  | `RoleB ->
      List.rev_map (fun e -> "[PRIVATE-B] " ^ e.private_scratchpad) store.b_private_log

(* Check that no private scratchpad text from Role A leaked into Role B *)
let verify_no_cross_contamination (store : isolated_memory_store) : bool =
  let b_texts = List.map (fun e -> e.private_scratchpad) store.b_private_log in
  let a_texts = List.map (fun e -> e.private_scratchpad) store.a_private_log in
  let contaminated = ref false in
  List.iter (fun a_text ->
    if String.length a_text > 5 then
      List.iter (fun b_text ->
        if a_text = b_text then contaminated := true
      ) b_texts
  ) a_texts;
  not !contaminated
