(* Novice Test Scripts and Observational Scorecards (Q001) *)
open Types

let script_1 : task_script = {
  task_id = "NOV-SCRIPT-01";
  title = "Create, redact and sign document with persistent receipt";
  target_persona = "Nontechnical Business User";
  instructions = [
    "Open the native intent input bar";
    "Type: 'Draft summary of project status, redact confidential IP, and create audit receipt'";
    "Review the proposed differential plan in the UI";
    "Confirm action when approval prompt appears";
    "Verify receipt hash is displayed and document is saved";
  ];
  expected_steps = 5;
  max_allowed_clarifications = 2;
}

let script_2 : task_script = {
  task_id = "NOV-SCRIPT-02";
  title = "Inspect system health and rollback update";
  target_persona = "Support Engineer";
  instructions = [
    "Navigate to System Settings -> Firmware & Updates";
    "Observe current boot slot telemetry and integrity verification status";
    "Simulate update health failure check";
    "Verify automatic rollback to previous known-good slot without data loss";
  ];
  expected_steps = 4;
  max_allowed_clarifications = 1;
}

let script_3 : task_script = {
  task_id = "NOV-SCRIPT-03";
  title = "Isolate and execute legacy document in ephemeral guest capsule";
  target_persona = "General Desktop User";
  instructions = [
    "Drag legacy file sample.docx into intent window";
    "Select: 'Open in disposable guest environment'";
    "Verify guest window renders on native desktop without host filesystem access";
    "Close application and verify all guest memory and temporary storage are discarded";
  ];
  expected_steps = 4;
  max_allowed_clarifications = 1;
}

let all_novice_scripts = [script_1; script_2; script_3]

let evaluate_novice_run (scorecard : observational_scorecard) : (unit, string) result =
  if scorecard.coaching_interventions > 0 then
    Error (Printf.sprintf "Evaluation failed: %d coaching interventions required (must be 0 for independent reproducibility)" scorecard.coaching_interventions)
  else if not scorecard.task_succeeded then
    Error "Evaluation failed: Task objective was not successfully achieved"
  else if scorecard.completion_time_sec > 300.0 then
    Error (Printf.sprintf "Evaluation failed: Completion time %.1fs exceeded 300s limit" scorecard.completion_time_sec)
  else
    Ok ()
