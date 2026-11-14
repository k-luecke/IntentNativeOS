(* tools/supervisor/supervisor.ml - Main autonomous supervisor loop *)

open Types

type supervisor = {
  config : supervisor_config;
  tasks : (string, task) Hashtbl.t;
  mutable running : bool;
  mutable paused : bool;
}

let init config =
  let tasks = Backlog.load_from_csv config.backlog_csv_path in
  Recovery.replay_journal tasks config.journal_path;
  {
    config;
    tasks;
    running = true;
    paused = false;
  }

let step supervisor =
  if supervisor.paused then `Paused
  else
    let ready_tasks = Scheduler.get_ready_tasks supervisor.tasks in
    match ready_tasks with
    | [] ->
        let (ready, leased, _, _, accepted, blocked) = Scheduler.count_tasks_by_status supervisor.tasks in
        if ready = 0 && leased = 0 then
          `Done (accepted, blocked)
        else
          `Idle
    | task :: _ ->
        let worker_id = "worker-model-b-01" in
        let now = Unix.gettimeofday () in
        task.status <- Leased { worker_id; leased_at = now };
        Recovery.append_event supervisor.config.journal_path
          (EventTaskLeased { task_id = task.id; worker_id; timestamp = now });
        Printf.printf "[Supervisor] Leased task %s (%s) to %s\n" task.id task.title worker_id;

        (* Step 2: Worker execution *)
        let w_res = Worker_adapter.execute_task task worker_id in
        task.status <- Implemented { patch_ref = w_res.patch_ref };
        Recovery.append_event supervisor.config.journal_path
          (EventTaskImplemented { task_id = task.id; patch_ref = w_res.patch_ref; timestamp = Unix.gettimeofday () });

        (* Step 3: Independent verification *)
        task.status <- Verifying;
        let v_res = Verifier.verify_task task in
        Recovery.append_event supervisor.config.journal_path
          (EventTaskVerified { task_id = task.id; passed = v_res.passed; details = v_res.stdout_summary; timestamp = Unix.gettimeofday () });

        (* Step 4: Independent review *)
        let review = Reviewer.review_task task v_res in
        match review with
        | ReviewPassed msg ->
            let ev_ref = Printf.sprintf "evidence/%s_verified.md" task.id in
            let accepted_at = Unix.gettimeofday () in
            task.status <- Accepted { evidence_ref = ev_ref; accepted_at };
            task.evidence <- Some ev_ref;
            task.notes <- msg;
            Recovery.append_event supervisor.config.journal_path
              (EventTaskAccepted { task_id = task.id; evidence_ref = ev_ref; timestamp = accepted_at });
            Printf.printf "[Supervisor] Task %s ACCEPTED and verified.\n" task.id;

            (* Update backlog CSV and gate milestone report *)
            Backlog.export_to_csv supervisor.tasks supervisor.config.backlog_csv_path;
            Milestones.generate_gate_report supervisor.tasks task.gate supervisor.config.evidence_dir;
            Recovery.save_checkpoint supervisor.tasks supervisor.config.checkpoint_path;
            `Task_Accepted task.id

        | ReviewRejected reason ->
            Printf.printf "[Supervisor] Task %s REJECTED: %s\n" task.id reason;
            Recovery.append_event supervisor.config.journal_path
              (EventTaskFailed { task_id = task.id; reason; attempt = task.attempts + 1; timestamp = Unix.gettimeofday () });
            (match Watchdog.check_task_retry task reason with
             | `Retry_Scheduled ->
                 Printf.printf "[Supervisor] Task %s: Scheduled retry (attempt %d/%d)\n"
                   task.id task.attempts Watchdog.max_allowed_attempts;
                 `Task_Retrying task.id
             | `Quarantined ->
                 Printf.printf "[Supervisor] Task %s: QUARANTINED after exceeding max attempts. Continuing other tasks.\n" task.id;
                 Recovery.append_event supervisor.config.journal_path
                   (EventTaskBlocked { task_id = task.id; reason; timestamp = Unix.gettimeofday () });
                 Backlog.export_to_csv supervisor.tasks supervisor.config.backlog_csv_path;
                 Milestones.generate_gate_report supervisor.tasks task.gate supervisor.config.evidence_dir;
                 Recovery.save_checkpoint supervisor.tasks supervisor.config.checkpoint_path;
                 `Task_Blocked task.id)

let run_until_complete ?(max_iterations=1000) supervisor =
  let rec loop i =
    if i >= max_iterations then `Max_Iterations_Reached
    else
      match step supervisor with
      | `Done (acc, blk) -> `Done (acc, blk)
      | `Idle ->
          Unix.sleep 1;
          loop (i + 1)
      | `Paused ->
          Unix.sleep 1;
          loop (i + 1)
      | `Task_Accepted _ | `Task_Retrying _ | `Task_Blocked _ ->
          loop (i + 1)
  in
  loop 0
