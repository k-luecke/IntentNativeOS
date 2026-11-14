(* tools/supervisor/verifier.ml - Independent execution and test verification *)

open Types

type verification_result = {
  passed : bool;
  exit_code : int;
  stdout_summary : string;
  duration_ms : float;
}

let string_contains s sub =
  let len_s = String.length s in
  let len_sub = String.length sub in
  if len_sub > len_s then false
  else
    let rec check i =
      if i + len_sub > len_s then false
      else if String.sub s i len_sub = sub then true
      else check (i + 1)
    in
    check 0

let run_command cmd =
  let t0 = Unix.gettimeofday () in
  let ic = Unix.open_process_in (cmd ^ " 2>&1") in
  let buf = Buffer.create 1024 in
  (try
     while true do
       let line = input_line ic in
       Buffer.add_string buf line;
       Buffer.add_char buf '\n'
     done
   with End_of_file -> ());
  let status = Unix.close_process_in ic in
  let t1 = Unix.gettimeofday () in
  let exit_code = match status with
    | Unix.WEXITED c -> c
    | Unix.WSIGNALED _ -> 128
    | Unix.WSTOPPED _ -> 129
  in
  let raw_output = Buffer.contents buf in
  let stdout_summary =
    if String.length raw_output > 500 then
      String.sub raw_output (String.length raw_output - 500) 500
    else raw_output
  in
  {
    passed = (exit_code = 0);
    exit_code;
    stdout_summary;
    duration_ms = (t1 -. t0) *. 1000.0;
  }

let verify_task task =
  (* Standard verification pipeline based on task metadata or dune test suite *)
  if string_contains task.done_when "fails" || string_contains task.title "intentional fail" then
    {
      passed = false;
      exit_code = 1;
      stdout_summary = "Simulated intentional test failure for repair loop test";
      duration_ms = 5.0;
    }
  else
    (* Real execution check: run dune build and dune runtest *)
    let result = run_command "dune runtest" in
    result
