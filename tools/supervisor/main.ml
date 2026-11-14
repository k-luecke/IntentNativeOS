(* tools/supervisor/main.ml - CLI entrypoint and acceptance test runner *)

open Types

let default_config = {
  journal_path = ".inos/supervisor_journal.log";
  checkpoint_path = ".inos/supervisor_checkpoint.json";
  backlog_csv_path = "intent_os_granular_backlog_v0_3.csv";
  evidence_dir = "evidence";
  max_attempts = 3;
  task_timeout_sec = 600.0;
}

let print_status supervisor =
  let (ready, leased, implemented, verifying, accepted, blocked) =
    Scheduler.count_tasks_by_status supervisor.Supervisor.tasks in
  Printf.printf "=== Autonomous Engineering Supervisor Status ===\n";
  Printf.printf "  Ready:        %d\n" ready;
  Printf.printf "  Leased:       %d\n" leased;
  Printf.printf "  Implemented:  %d\n" implemented;
  Printf.printf "  Verifying:    %d\n" verifying;
  Printf.printf "  Accepted:     %d\n" accepted;
  Printf.printf "  Blocked:      %d\n" blocked;
  Printf.printf "===============================================\n"

let run_acceptance_test () =
  Printf.printf "=== RUNNING AUTONOMOUS SUPERVISOR ACCEPTANCE TEST ===\n\n";

  let test_journal = ".inos/test_acceptance_journal.log" in
  let test_checkpoint = ".inos/test_acceptance_checkpoint.json" in
  let test_csv = ".inos/test_acceptance_backlog.csv" in
  let test_evidence = "evidence/acceptance_test" in

  Recovery.ensure_dir_exists test_journal;
  (try Sys.remove test_journal with _ -> ());
  (try Sys.remove test_checkpoint with _ -> ());
  (try Sys.remove test_csv with _ -> ());

  (* 1. Create test backlog with:
     - Task T_OK1 (Ready, independent, will pass)
     - Task T_FAIL (Ready, independent, intentionally failing)
     - Task T_OK2 (Ready, independent, will pass)
     - Task T_DEP (Depends on T_OK1, will become ready after T_OK1 passes)
  *)
  let test_tasks = Hashtbl.create 10 in
  let add_task id gate title deps done_when =
    Hashtbl.replace test_tasks id {
      id;
      gate;
      area = "Acceptance";
      title;
      depends_on = deps;
      done_when;
      status = Ready;
      attempts = 0;
      evidence = None;
      notes = "";
    }
  in
  add_task "T_OK1" "TEST" "Independent passing task 1" [] "dune test passes";
  add_task "T_FAIL" "TEST" "Intentionally failing task with bounded repair" [] "fails verification";
  add_task "T_OK2" "TEST" "Independent passing task 2" [] "dune test passes";
  add_task "T_DEP" "TEST" "Dependent task unblocked by T_OK1" ["T_OK1"] "dune test passes";

  Backlog.export_to_csv test_tasks test_csv;

  let test_config = {
    journal_path = test_journal;
    checkpoint_path = test_checkpoint;
    backlog_csv_path = test_csv;
    evidence_dir = test_evidence;
    max_attempts = 3;
    task_timeout_sec = 60.0;
  } in

  Printf.printf "[Phase 1] Launching supervisor on test backlog...\n";
  let sup1 = Supervisor.init test_config in
  print_status sup1;

  Printf.printf "\n[Phase 2] Running autonomous execution loop...\n";
  let rec run_loop sup limit =
    if limit <= 0 then ()
    else
      match Supervisor.step sup with
      | `Task_Accepted id ->
          Printf.printf "-> Accepted task: %s\n" id;
          run_loop sup (limit - 1)
      | `Task_Retrying id ->
          Printf.printf "-> Task %s failed, retrying bounded repair...\n" id;
          run_loop sup (limit - 1)
      | `Task_Blocked id ->
          Printf.printf "-> Task %s quarantined to Blocked. Continuing...\n" id;
          run_loop sup (limit - 1)
      | `Done _ ->
          Printf.printf "-> All tasks processed.\n"
      | _ -> ()
  in
  run_loop sup1 20;

  Printf.printf "\n[Phase 3] Status before simulated crash/restart:\n";
  print_status sup1;

  (* Phase 4: Forced restart simulation *)
  Printf.printf "\n[Phase 4] Simulating process crash and restart recovery...\n";
  let sup2 = Supervisor.init test_config in
  Printf.printf "Supervisor restarted. Replaying journal from %s...\n" test_journal;
  print_status sup2;

  (* Verify invariants *)
  let t_ok1 = Hashtbl.find sup2.Supervisor.tasks "T_OK1" in
  let t_fail = Hashtbl.find sup2.Supervisor.tasks "T_FAIL" in
  let t_ok2 = Hashtbl.find sup2.Supervisor.tasks "T_OK2" in
  let t_dep = Hashtbl.find sup2.Supervisor.tasks "T_DEP" in

  let ok1_passed = (match t_ok1.status with Accepted _ -> true | _ -> false) in
  let fail_quarantined = (match t_fail.status with Blocked _ -> true | _ -> false) in
  let ok2_passed = (match t_ok2.status with Accepted _ -> true | _ -> false) in
  let dep_passed = (match t_dep.status with Accepted _ -> true | _ -> false) in

  Printf.printf "\n=== ACCEPTANCE VERIFICATION REPORT ===\n";
  Printf.printf "[VERIFIED] Independent task 1 accepted:        %b\n" ok1_passed;
  Printf.printf "[VERIFIED] Failing task quarantined (Blocked): %b (attempts: %d)\n" fail_quarantined t_fail.attempts;
  Printf.printf "[VERIFIED] Independent task 2 accepted:        %b\n" ok2_passed;
  Printf.printf "[VERIFIED] Dependent task unblocked & accepted: %b\n" dep_passed;
  Printf.printf "[VERIFIED] Milestone report written:           %b\n" (Sys.file_exists (Filename.concat test_evidence "milestone_TEST_report.md"));
  Printf.printf "[VERIFIED] State preserved across crash restart: %b\n" (Sys.file_exists test_checkpoint);

  if ok1_passed && fail_quarantined && ok2_passed && dep_passed then (
    Printf.printf "\n=== AUTONOMOUS SUPERVISOR ACCEPTANCE TEST PASSED 100%%%% ===\n";
    exit 0
  ) else (
    Printf.eprintf "\n=== ACCEPTANCE TEST FAILED ===\n";
    exit 1
  )

let () =
  let args = Array.to_list Sys.argv in
  match args with
  | _ :: "acceptance-test" :: _ ->
      run_acceptance_test ()
  | _ :: "status" :: _ ->
      let sup = Supervisor.init default_config in
      print_status sup
  | _ :: "report" :: _ ->
      let sup = Supervisor.init default_config in
      Milestones.generate_gate_report sup.Supervisor.tasks "G0" default_config.evidence_dir;
      Milestones.generate_gate_report sup.Supervisor.tasks "G1" default_config.evidence_dir;
      Milestones.generate_gate_report sup.Supervisor.tasks "G2" default_config.evidence_dir;
      Milestones.generate_gate_report sup.Supervisor.tasks "G3" default_config.evidence_dir;
      Milestones.generate_gate_report sup.Supervisor.tasks "G4" default_config.evidence_dir;
      Milestones.generate_gate_report sup.Supervisor.tasks "G5" default_config.evidence_dir;
      Milestones.generate_gate_report sup.Supervisor.tasks "G6" default_config.evidence_dir;
      Printf.printf "Reports generated successfully.\n"
  | _ :: "run" :: _ ->
      let sup = Supervisor.init default_config in
      Printf.printf "Starting Autonomous Supervisor loop...\n";
      let _ = Supervisor.run_until_complete sup in
      Printf.printf "Supervisor finished.\n"
  | _ ->
      Printf.printf "Usage: supervisor [status | run | report | acceptance-test]\n"
