(* tools/supervisor/reviewer.ml - Independent implementation review *)

open Types

type review_verdict =
  | ReviewPassed of string
  | ReviewRejected of string

let review_task (task : task) (v_result : Verifier.verification_result) =
  if not v_result.passed then
    ReviewRejected (Printf.sprintf "Verification failed with exit code %d" v_result.exit_code)
  else
    (* Ensure no python files introduced *)
    let py_check = Verifier.run_command "find . -name '*.py' ! -path './.git/*' ! -path './scratch/*' | wc -l" in
    let py_count = String.trim py_check.stdout_summary in
    if py_count <> "0" && py_count <> "" then
      ReviewRejected "Policy violation: Python files detected in codebase"
    else
      ReviewPassed (Printf.sprintf "Independent review passed for %s (exit 0, zero python files)" task.id)
