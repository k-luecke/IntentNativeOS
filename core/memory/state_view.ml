(* core/memory/state_view.ml - Materialized Current-State View and Deterministic Replay *)

open Journal

type materialized_state = {
  intents : (string, string) Hashtbl.t;
  capabilities : (string, string list) Hashtbl.t;
  facts : (string, string) Hashtbl.t;
  metadata : (string, string) Hashtbl.t;
  mutable event_count : int;
  mutable state_hash : string;
}

let create_empty () = {
  intents = Hashtbl.create 32;
  capabilities = Hashtbl.create 32;
  facts = Hashtbl.create 32;
  metadata = Hashtbl.create 32;
  event_count = 0;
  state_hash = Journal.genesis_digest;
}

let sorted_bindings tbl =
  let pairs = Hashtbl.fold (fun k v acc -> (k, v) :: acc) tbl [] in
  List.sort (fun (k1, _) (k2, _) -> String.compare k1 k2) pairs

let sorted_list_bindings tbl =
  let pairs = Hashtbl.fold (fun k vs acc ->
    let sorted_vs = List.sort String.compare vs in
    (k, String.concat "," sorted_vs) :: acc
  ) tbl [] in
  List.sort (fun (k1, _) (k2, _) -> String.compare k1 k2) pairs

let compute_sha256 s = Inos_ipc.Sha256.digest_string s

let compute_state_hash (st : materialized_state) : string =
  let buf = Buffer.create 256 in
  Buffer.add_string buf (Printf.sprintf "EVENTS:%d|" st.event_count);

  Buffer.add_string buf "INTENTS:";
  List.iter (fun (k, v) ->
    Buffer.add_string buf (Printf.sprintf "%s=%s;" k v)
  ) (sorted_bindings st.intents);

  Buffer.add_string buf "|CAPS:";
  List.iter (fun (k, v) ->
    Buffer.add_string buf (Printf.sprintf "%s=[%s];" k v)
  ) (sorted_list_bindings st.capabilities);

  Buffer.add_string buf "|FACTS:";
  List.iter (fun (k, v) ->
    Buffer.add_string buf (Printf.sprintf "%s=%s;" k v)
  ) (sorted_bindings st.facts);

  Buffer.add_string buf "|META:";
  List.iter (fun (k, v) ->
    Buffer.add_string buf (Printf.sprintf "%s=%s;" k v)
  ) (sorted_bindings st.metadata);

  compute_sha256 (Buffer.contents buf)

let apply_event (st : materialized_state) (e : entry) : unit =
  st.event_count <- st.event_count + 1;
  match e.event_type with
  | "INTENT_REGISTERED" | "INTENT_TRANSITION" ->
      Hashtbl.replace st.intents e.event_id e.payload
  | "CAPABILITY_GRANTED" ->
      let current = try Hashtbl.find st.capabilities e.event_id with Not_found -> [] in
      if not (List.mem e.payload current) then
        Hashtbl.replace st.capabilities e.event_id (e.payload :: current)
  | "CAPABILITY_REVOKED" ->
      let current = try Hashtbl.find st.capabilities e.event_id with Not_found -> [] in
      let updated = List.filter (fun c -> c <> e.payload) current in
      Hashtbl.replace st.capabilities e.event_id updated
  | "FACT_ASSERTED" ->
      Hashtbl.replace st.facts e.event_id e.payload
  | "FACT_SUPERSEDED" ->
      Hashtbl.remove st.facts e.event_id
  | _ ->
      Hashtbl.replace st.metadata (Printf.sprintf "%s:%s" e.event_type e.event_id) e.payload

(* Core Invariant / Task M002:
   Replay produces identical state hash twice *)
let replay_journal (j : journal) : materialized_state =
  let st = create_empty () in
  List.iter (apply_event st) j.entries;
  st.state_hash <- compute_state_hash st;
  st
