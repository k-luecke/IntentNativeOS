(* tests/test_numerical_parity.ml - Independent Validation of Numerical Inference (Task L104) *)

let assert_true name cond =
  if cond then Printf.printf "  [PASS] %s\n" name
  else (Printf.eprintf "  [FAIL] %s\n" name; exit 1)

let resolve_file rel =
  let candidates = [
    rel;
    "../" ^ rel;
    "../../" ^ rel;
    "../../../" ^ rel;
  ] in
  match List.find_opt Sys.file_exists candidates with
  | Some p -> p
  | None -> rel

let test_numerical_parity_and_kv_cache () =
  Printf.printf "=== [TEST 1] Testing Numerical Inference Parity and Quantization Invariants ===\n";
  let engine_src = resolve_file "models/inference/quant_engine.c" in
  let engine_bin = Filename.concat (Filename.dirname engine_src) "quant_engine" in
  if not (Sys.file_exists engine_bin) then
    ignore (Sys.command (Printf.sprintf "cc -O3 -o %s %s -lm" (Filename.quote engine_bin) (Filename.quote engine_src)));
  assert_true "Quantized engine binary built from source" (Sys.file_exists engine_bin);
  let cleanup_bin = true in

  let cmd = Printf.sprintf "%s --test-parity 2>&1" engine_bin in
  let ic = Unix.open_process_in cmd in
  let output_lines = ref [] in
  (try
     while true do
       output_lines := input_line ic :: !output_lines
     done
   with End_of_file -> ());
  let status = Unix.close_process_in ic in
  let exit_code = match status with Unix.WEXITED c -> c | _ -> -1 in

  assert_true "Engine --test-parity exited cleanly (exit code 0)" (exit_code = 0);
  let all_out = String.concat "\n" (List.rev !output_lines) in
  assert_true "Output confirms Q8_0 parity" (let r = Str.regexp_string "Q8_0 Dot" in
    try ignore (Str.search_forward r all_out 0); true with _ -> false);
  assert_true "Output confirms Q4_0 parity" (let r = Str.regexp_string "Q4_0 Dot" in
    try ignore (Str.search_forward r all_out 0); true with _ -> false);
  assert_true "Output confirms RoPE norm preservation" (let r = Str.regexp_string "RoPE pos=0 preserves norm" in
    try ignore (Str.search_forward r all_out 0); true with _ -> false);
  assert_true "Output confirms KV cache allocation" (let r = Str.regexp_string "KV cache allocated" in
    try ignore (Str.search_forward r all_out 0); true with _ -> false);
  if cleanup_bin then (try Sys.remove engine_bin with _ -> ())

let () =
  Printf.printf "\n=== Running Test Suite: Numerical Inference Validation (L104) ===\n";
  test_numerical_parity_and_kv_cache ();
  Printf.printf "=== [ALL PASS] Numerical Inference Validated ===\n\n"
