(* core/memory/context.ml - Task-Scoped Model Context Projections and Secret Filtering *)

type role = Architect | Implementer

type context_item = {
  item_id : string;
  scope : string;
  content : string;
  is_secret : bool;
  tags : string list;
}

type projection = {
  task_id : string;
  target_role : role;
  included_items : context_item list;
  omitted_count : int;
  projection_hash : string;
}

let compute_sha256 s = Inos_ipc.Sha256.digest_string s

let compute_projection_hash task_id role items =
  let role_str = match role with Architect -> "ARCHITECT" | Implementer -> "IMPLEMENTER" in
  let buf = Buffer.create 128 in
  Buffer.add_string buf (Printf.sprintf "%s|%s|" task_id role_str);
  List.iter (fun item ->
    Buffer.add_string buf (Printf.sprintf "%s:%s;" item.item_id item.content)
  ) items;
  compute_sha256 (Buffer.contents buf)

(* Core Invariant / Task M004:
   A and B projections contain required references and exclude unrelated test secrets. *)
let project_context
    ~(task_id : string)
    ~(required_scopes : string list)
    ~(target_role : role)
    (all_items : context_item list) : projection =
  let is_relevant item =
    List.mem item.scope required_scopes || List.mem "GLOBAL" item.tags
  in
  let is_prohibited item =
    item.is_secret || List.mem "SECRET" item.tags || List.mem "TEST_SECRET" item.tags
  in
  let included, omitted = List.partition (fun item ->
    is_relevant item && not (is_prohibited item)
  ) all_items in
  let projection_hash = compute_projection_hash task_id target_role included in
  {
    task_id;
    target_role;
    included_items = included;
    omitted_count = List.length omitted;
    projection_hash;
  }
