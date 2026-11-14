(* tools/supervisor/scheduler.ml - Select ready tasks from dependency graph *)

open Types

let is_accepted tasks task_id =
  match Hashtbl.find_opt tasks task_id with
  | Some t ->
      (match t.status with
       | Accepted _ -> true
       | _ -> false)
  | None -> false

let dependencies_satisfied tasks task =
  List.for_all (fun dep_id -> is_accepted tasks dep_id) task.depends_on

let get_ready_tasks tasks =
  let ready = ref [] in
  Hashtbl.iter (fun _ t ->
    match t.status with
    | Ready ->
        if dependencies_satisfied tasks t then
          ready := t :: !ready
    | _ -> ()
  ) tasks;
  (* Sort ready tasks stably by ID *)
  List.sort (fun a b -> String.compare a.id b.id) !ready

let count_tasks_by_status tasks =
  let ready = ref 0 in
  let leased = ref 0 in
  let implemented = ref 0 in
  let verifying = ref 0 in
  let accepted = ref 0 in
  let blocked = ref 0 in
  Hashtbl.iter (fun _ t ->
    match t.status with
    | Ready -> incr ready
    | Leased _ -> incr leased
    | Implemented _ -> incr implemented
    | Verifying -> incr verifying
    | Accepted _ -> incr accepted
    | Blocked _ -> incr blocked
  ) tasks;
  (!ready, !leased, !implemented, !verifying, !accepted, !blocked)
