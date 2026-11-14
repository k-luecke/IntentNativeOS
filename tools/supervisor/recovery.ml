(* tools/supervisor/recovery.ml - Durable event journaling, checkpointing, and replay *)

open Types

let string_of_event = function
  | EventTaskLeased { task_id; worker_id; timestamp } ->
      Printf.sprintf "TASK_LEASED|%s|%s|%.3f" task_id worker_id timestamp
  | EventTaskImplemented { task_id; patch_ref; timestamp } ->
      Printf.sprintf "TASK_IMPLEMENTED|%s|%s|%.3f" task_id patch_ref timestamp
  | EventTaskVerified { task_id; passed; details; timestamp } ->
      Printf.sprintf "TASK_VERIFIED|%s|%b|%s|%.3f" task_id passed details timestamp
  | EventTaskAccepted { task_id; evidence_ref; timestamp } ->
      Printf.sprintf "TASK_ACCEPTED|%s|%s|%.3f" task_id evidence_ref timestamp
  | EventTaskFailed { task_id; reason; attempt; timestamp } ->
      Printf.sprintf "TASK_FAILED|%s|%s|%d|%.3f" task_id reason attempt timestamp
  | EventTaskBlocked { task_id; reason; timestamp } ->
      Printf.sprintf "TASK_BLOCKED|%s|%s|%.3f" task_id reason timestamp
  | EventCheckpoint { timestamp; accepted_count; blocked_count } ->
      Printf.sprintf "CHECKPOINT|%d|%d|%.3f" accepted_count blocked_count timestamp

let event_of_string line =
  match String.split_on_char '|' (String.trim line) with
  | ["TASK_LEASED"; task_id; worker_id; ts] ->
      Some (EventTaskLeased { task_id; worker_id; timestamp = float_of_string_opt ts |> Option.value ~default:0.0 })
  | ["TASK_IMPLEMENTED"; task_id; patch_ref; ts] ->
      Some (EventTaskImplemented { task_id; patch_ref; timestamp = float_of_string_opt ts |> Option.value ~default:0.0 })
  | ["TASK_VERIFIED"; task_id; passed; details; ts] ->
      Some (EventTaskVerified { task_id; passed = bool_of_string_opt passed |> Option.value ~default:false; details; timestamp = float_of_string_opt ts |> Option.value ~default:0.0 })
  | ["TASK_ACCEPTED"; task_id; evidence_ref; ts] ->
      Some (EventTaskAccepted { task_id; evidence_ref; timestamp = float_of_string_opt ts |> Option.value ~default:0.0 })
  | ["TASK_FAILED"; task_id; reason; attempt; ts] ->
      Some (EventTaskFailed { task_id; reason; attempt = int_of_string_opt attempt |> Option.value ~default:1; timestamp = float_of_string_opt ts |> Option.value ~default:0.0 })
  | ["TASK_BLOCKED"; task_id; reason; ts] ->
      Some (EventTaskBlocked { task_id; reason; timestamp = float_of_string_opt ts |> Option.value ~default:0.0 })
  | ["CHECKPOINT"; acc; blk; ts] ->
      Some (EventCheckpoint { accepted_count = int_of_string_opt acc |> Option.value ~default:0; blocked_count = int_of_string_opt blk |> Option.value ~default:0; timestamp = float_of_string_opt ts |> Option.value ~default:0.0 })
  | _ -> None

let ensure_dir_exists path =
  let dir = Filename.dirname path in
  if not (Sys.file_exists dir) then
    (try Unix.mkdir dir 0o755 with _ -> ())

let append_event journal_path event =
  ensure_dir_exists journal_path;
  let oc = open_out_gen [Open_creat; Open_append; Open_wronly] 0o644 journal_path in
  output_string oc (string_of_event event ^ "\n");
  flush oc;
  close_out oc

let replay_journal tasks journal_path =
  if Sys.file_exists journal_path then begin
    let ic = open_in journal_path in
    let rec loop () =
      match input_line ic with
      | line ->
          (match event_of_string line with
           | Some (EventTaskLeased { task_id; worker_id; timestamp }) ->
               (match Hashtbl.find_opt tasks task_id with
                | Some t -> t.status <- Leased { worker_id; leased_at = timestamp }
                | None -> ())
           | Some (EventTaskImplemented { task_id; patch_ref; _ }) ->
               (match Hashtbl.find_opt tasks task_id with
                | Some t -> t.status <- Implemented { patch_ref }
                | None -> ())
           | Some (EventTaskAccepted { task_id; evidence_ref; timestamp }) ->
               (match Hashtbl.find_opt tasks task_id with
                | Some t ->
                    t.status <- Accepted { evidence_ref; accepted_at = timestamp };
                    t.evidence <- Some evidence_ref
                | None -> ())
           | Some (EventTaskFailed { task_id; attempt; _ }) ->
               (match Hashtbl.find_opt tasks task_id with
                | Some t -> t.attempts <- attempt
                | None -> ())
           | Some (EventTaskBlocked { task_id; reason; _ }) ->
               (match Hashtbl.find_opt tasks task_id with
                | Some t -> t.status <- Blocked { reason; attempts = t.attempts }
                | None -> ())
           | _ -> ());
          loop ()
      | exception End_of_file -> ()
    in
    loop ();
    close_in ic;

    (* Crash recovery rule: If a task was left in Leased or Verifying when supervisor died,
       reset it to Ready so another worker can take it, but preserve attempt count! *)
    Hashtbl.iter (fun _ t ->
      match t.status with
      | Leased _ | Verifying | Implemented _ ->
          t.status <- Ready;
          t.attempts <- t.attempts + 1
      | _ -> ()
    ) tasks
  end

let save_checkpoint tasks checkpoint_path =
  ensure_dir_exists checkpoint_path;
  let (ready, leased, implemented, verifying, accepted, blocked) = Scheduler.count_tasks_by_status tasks in
  let oc = open_out checkpoint_path in
  Printf.fprintf oc "{\n";
  Printf.fprintf oc "  \"timestamp\": %.3f,\n" (Unix.gettimeofday ());
  Printf.fprintf oc "  \"stats\": {\n";
  Printf.fprintf oc "    \"ready\": %d,\n" ready;
  Printf.fprintf oc "    \"leased\": %d,\n" leased;
  Printf.fprintf oc "    \"implemented\": %d,\n" implemented;
  Printf.fprintf oc "    \"verifying\": %d,\n" verifying;
  Printf.fprintf oc "    \"accepted\": %d,\n" accepted;
  Printf.fprintf oc "    \"blocked\": %d\n" blocked;
  Printf.fprintf oc "  }\n";
  Printf.fprintf oc "}\n";
  flush oc;
  close_out oc
