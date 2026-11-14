(* benchmarks/runner.ml - Reproducible Isolated Benchmark Runner (L005)
   Executes intent benchmarks across development, validation, and sealed splits
   with strict isolation guards preventing benchmark contamination.
*)

open Inos_benchmarks.Metrics
open Inos_benchmarks.Corpus_parser
open Inos_benchmarks.Deterministic_baseline

let usage () =
  Printf.eprintf "Usage: %s [--corpus PATH] [--split dev|val|all-unsealed|sealed_eval] [--allow-sealed] [--out-dir PATH]\n" Sys.argv.(0);
  exit 1

let get_current_rss_kb () =
  try
    let ic = open_in "/proc/self/status" in
    let rss = ref 0 in
    (try
       while true do
         let line = input_line ic in
         if String.length line >= 6 && String.sub line 0 6 = "VmRSS:" then begin
           Scanf.sscanf line "VmRSS: %d kB" (fun k -> rss := k)
         end
       done
     with End_of_file -> close_in ic);
    !rss
  with _ -> 1024

let () =
  let corpus_path = ref "benchmarks/corpus/intent_benchmark_100.json" in
  let split_arg = ref "all-unsealed" in
  let allow_sealed = ref false in
  let out_dir = ref "benchmarks/results" in

  let i = ref 1 in
  let len = Array.length Sys.argv in
  while !i < len do
    match Sys.argv.(!i) with
    | "--corpus" when !i + 1 < len ->
        corpus_path := Sys.argv.(!i + 1);
        i := !i + 2
    | "--split" when !i + 1 < len ->
        split_arg := Sys.argv.(!i + 1);
        i := !i + 2
    | "--allow-sealed" ->
        allow_sealed := true;
        incr i
    | "--out-dir" when !i + 1 < len ->
        out_dir := Sys.argv.(!i + 1);
        i := !i + 2
    | "-h" | "--help" -> usage ()
    | _ -> usage ()
  done;

  (* Strict Quarantine Enforcement *)
  if (!split_arg = "sealed_eval" || !split_arg = "all") && not !allow_sealed then begin
    Printf.eprintf "\n[CRITICAL ERROR] Contamination guard triggered!\n";
    Printf.eprintf "Access to 'sealed_eval' split is strictly prohibited without explicit '--allow-sealed' flag.\n";
    Printf.eprintf "Sealed evaluation cases must remain untuned and uninspected during development.\n\n";
    exit 2
  end;

  Printf.printf "=== Intent-Native OS Benchmark Runner (Gate L0) ===\n";
  Printf.printf "Corpus: %s\n" !corpus_path;
  Printf.printf "Split Target: %s\n" !split_arg;
  Printf.printf "Output Directory: %s\n\n" !out_dir;

  let all_cases = parse_corpus_file !corpus_path in
  Printf.printf "Loaded %d total corpus cases.\n" (List.length all_cases);

  let selected_cases = List.filter (fun (c : benchmark_case) ->
    match !split_arg with
    | "dev" -> c.split = "dev"
    | "val" -> c.split = "val"
    | "all-unsealed" -> c.split = "dev" || c.split = "val"
    | "sealed_eval" -> c.split = "sealed_eval"
    | "all" -> true
    | other -> failwith ("Unknown split specification: " ^ other)
  ) all_cases in

  let case_count = List.length selected_cases in
  Printf.printf "Evaluating %d cases in split '%s'...\n" case_count !split_arg;

  let results = List.map evaluate_case selected_cases in
  let rss = get_current_rss_kb () in
  let summary = evaluate_results results rss 0 in

  (* Emit reports *)
  let json_str = metrics_to_json summary in
  let md_report = format_markdown_report "Deterministic Baseline Evaluation Report (Task L006)" !split_arg summary in

  (try Unix.mkdir !out_dir 0o755 with _ -> ());
  
  let json_file = Printf.sprintf "%s/deterministic_%s_results.json" !out_dir !split_arg in
  let md_file = Printf.sprintf "%s/deterministic_%s_report.md" !out_dir !split_arg in

  let oc_j = open_out json_file in
  output_string oc_j json_str;
  close_out oc_j;

  let oc_m = open_out md_file in
  output_string oc_m md_report;
  close_out oc_m;

  (* Copy canonical baseline report if evaluating dev or all-unsealed *)
  if !split_arg = "all-unsealed" || !split_arg = "dev" then begin
    let oc_canon = open_out (Printf.sprintf "%s/deterministic_baseline_report.md" !out_dir) in
    output_string oc_canon md_report;
    close_out oc_canon;
  end;

  Printf.printf "\n%s\n" md_report;
  Printf.printf "\n[SUCCESS] Benchmark run complete.\n";
  Printf.printf "Structured JSON saved to: %s\n" json_file;
  Printf.printf "Markdown report saved to: %s\n" md_file;

  if summary.false_authorizations > 0 then begin
    Printf.eprintf "\n[CRITICAL FAILURE] False authorization detected (%d violations)! Fail-closed invariant breached!\n" summary.false_authorizations;
    exit 3
  end
