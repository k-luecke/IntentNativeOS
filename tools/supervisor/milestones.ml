(* tools/supervisor/milestones.ml - Autonomous non-blocking milestone progress reporting *)

open Types

let generate_gate_report tasks gate out_dir =
  Recovery.ensure_dir_exists (Filename.concat out_dir "placeholder");
  let gate_tasks = ref [] in
  Hashtbl.iter (fun _ t ->
    if t.gate = gate then
      gate_tasks := t :: !gate_tasks
  ) tasks;
  let total = List.length !gate_tasks in
  if total = 0 then ()
  else
    let accepted = ref 0 in
    let blocked = ref 0 in
    let ready = ref 0 in
    let in_flight = ref 0 in
    List.iter (fun t ->
      match t.status with
      | Accepted _ -> incr accepted
      | Blocked _ -> incr blocked
      | Ready -> incr ready
      | _ -> incr in_flight
    ) !gate_tasks;
    let pct = if total > 0 then (float_of_int !accepted /. float_of_int total) *. 100.0 else 0.0 in
    let report_filename = Printf.sprintf "milestone_%s_report.md" gate in
    let report_path = Filename.concat out_dir report_filename in
    let oc = open_out report_path in
    Printf.fprintf oc "# Milestone Progress Report: Gate %s\n\n" gate;
    Printf.fprintf oc "**Generated at:** %.3f (Autonomous Engineering Supervisor)\n" (Unix.gettimeofday ());
    Printf.fprintf oc "**Completion Status:** %d / %d tasks (%.1f%%)\n\n" !accepted total pct;
    Printf.fprintf oc "| Metric | Count |\n";
    Printf.fprintf oc "| :--- | :--- |\n";
    Printf.fprintf oc "| **Total Tasks** | %d |\n" total;
    Printf.fprintf oc "| **Accepted (Done)** | %d |\n" !accepted;
    Printf.fprintf oc "| **In-Flight / Leased** | %d |\n" !in_flight;
    Printf.fprintf oc "| **Ready to Schedule** | %d |\n" !ready;
    Printf.fprintf oc "| **Blocked / Quarantined** | %d |\n\n" !blocked;
    Printf.fprintf oc "## Task Breakdown\n\n";
    Printf.fprintf oc "| ID | Task | Status | Attempts | Evidence |\n";
    Printf.fprintf oc "| :--- | :--- | :--- | :--- | :--- |\n";
    List.iter (fun t ->
      let ev = match t.evidence with Some e -> e | None -> "-" in
      Printf.fprintf oc "| %s | %s | %s | %d | %s |\n"
        t.id t.title (Backlog.string_of_status t.status) t.attempts ev
    ) (List.sort (fun a b -> String.compare a.id b.id) !gate_tasks);
    Printf.fprintf oc "\n*Note: This report was automatically emitted by the autonomous supervisor. Execution continues without interruption.*\n";
    flush oc;
    close_out oc;
    Printf.printf "[Milestone] Updated report for Gate %s: %d/%d (%.1f%% complete)\n" gate !accepted total pct
