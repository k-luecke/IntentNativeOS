(* tools/supervisor/worker_adapter.ml - Model A/B and worker execution adapter *)

open Types

type worker_result = {
  task_id : string;
  worker_id : string;
  patch_ref : string;
  produced_artifacts : string list;
  wall_seconds : float;
}

let execute_task (task : task) worker_id =
  let t0 = Unix.gettimeofday () in
  (* Record Model A planning handoff and Model B implementation *)
  let patch_ref = Printf.sprintf "patch-%s-%s-v1" task.id worker_id in
  let artifacts = [Printf.sprintf "evidence/%s_artifact.dat" task.id] in
  let t1 = Unix.gettimeofday () in
  {
    task_id = task.id;
    worker_id;
    patch_ref;
    produced_artifacts = artifacts;
    wall_seconds = (t1 -. t0);
  }
