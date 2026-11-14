(* core/memory/budget.ml - Maximum Context Budgets, Omission Records, and Binding Preservation *)

type budgeted_item = {
  id : string;
  content : string;
  byte_size : int;
  is_binding_constraint : bool;
  priority : int; (* higher = retain first *)
}

type omission_record = {
  item_id : string;
  omitted_bytes : int;
  reason : string;
}

type retrieval_receipt = {
  receipt_id : string;
  total_budget : int;
  used_bytes : int;
  retained_items : budgeted_item list;
  omissions : omission_record list;
  binding_constraints_preserved : int;
  receipt_hash : string;
}

type budget_error =
  | BindingConstraintExceedsBudget of {
      constraint_id : string;
      required_size : int;
      max_budget : int;
    }

let compute_sha256 s = Inos_ipc.Sha256.digest_string s

let compute_receipt_hash receipt_id budget used retained omissions =
  let buf = Buffer.create 128 in
  Buffer.add_string buf (Printf.sprintf "%s|%d|%d|" receipt_id budget used);
  List.iter (fun item -> Buffer.add_string buf (Printf.sprintf "R:%s;" item.id)) retained;
  List.iter (fun o -> Buffer.add_string buf (Printf.sprintf "O:%s;" o.item_id)) omissions;
  compute_sha256 (Buffer.contents buf)

(* Core Invariant / Task M005:
   Truncation has explicit omission record and never drops binding constraints. *)
let enforce_budget
    ~(receipt_id : string)
    ~(max_budget : int)
    (items : budgeted_item list) : (retrieval_receipt, budget_error) result =
  (* Step 1: Separate binding constraints from non-binding items *)
  let binding_constraints = List.filter (fun it -> it.is_binding_constraint) items in
  let non_binding_items = List.filter (fun it -> not it.is_binding_constraint) items in

  (* Step 2: Sum binding constraints *)
  let binding_bytes = List.fold_left (fun acc it -> acc + it.byte_size) 0 binding_constraints in
  if binding_bytes > max_budget then
    let first_violator = List.hd binding_constraints in
    Error (BindingConstraintExceedsBudget {
      constraint_id = first_violator.id;
      required_size = binding_bytes;
      max_budget;
    })
  else
    (* Step 3: Sort non-binding items by priority descending *)
    let sorted_optional = List.sort (fun a b -> compare b.priority a.priority) non_binding_items in
    let remaining_budget = ref (max_budget - binding_bytes) in
    let retained_optional = ref [] in
    let omissions = ref [] in

    List.iter (fun it ->
      if it.byte_size <= !remaining_budget then begin
        remaining_budget := !remaining_budget - it.byte_size;
        retained_optional := it :: !retained_optional
      end else begin
        omissions := {
          item_id = it.id;
          omitted_bytes = it.byte_size;
          reason = Printf.sprintf "Budget limit reached (%d bytes remaining, item needs %d bytes)" !remaining_budget it.byte_size;
        } :: !omissions
      end
    ) sorted_optional;

    let all_retained = binding_constraints @ (List.rev !retained_optional) in
    let used_bytes = max_budget - !remaining_budget in
    let receipt_hash = compute_receipt_hash receipt_id max_budget used_bytes all_retained !omissions in
    Ok {
      receipt_id;
      total_budget = max_budget;
      used_bytes;
      retained_items = all_retained;
      omissions = List.rev !omissions;
      binding_constraints_preserved = List.length binding_constraints;
      receipt_hash;
    }
