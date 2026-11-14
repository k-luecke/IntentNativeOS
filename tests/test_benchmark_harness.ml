(* tests/test_benchmark_harness.ml - Benchmark harness acceptance tests (C008) *)

open Inos_operation.Native_backend
open Inos_operation.Benchmark_harness

let assert_true name cond =
  if cond then Printf.printf "[PASS] %s\n" name
  else (Printf.printf "[FAIL] %s\n" name; exit 1)

let test_benchmark_generation () =
  let test_func = {
    name = "bench_op";
    params = ["a"; "b"];
    body = BinOp (Mul, BinOp (Add, Var "a", Var "b"), Const 17L);
  } in

  let tmp_bin = Filename.temp_file "inos_bench_bin" "" in
  (try Sys.remove tmp_bin with _ -> ());

  match compile_native test_func ~out_bin:tmp_bin with
  | Error err ->
      Printf.printf "Compilation failed: %s\n" err;
      assert_true "Compile native binary for bench" false
  | Ok () ->
      let src = emit_c_source test_func in
      let inputs = [
        [10L; 20L];
        [100L; 200L];
        [1000L; 2000L];
        [5555L; 9999L];
      ] in

      let report = run_benchmark
        ~source_code:src
        ~binary_path:tmp_bin
        ~inputs
        ~warmup_runs:3
        ~timed_runs:10
      in

      assert_true "Source hash is hex digest" (String.length report.source_hash >= 32);
      assert_true "Binary hash is hex digest" (String.length report.binary_hash >= 32);
      assert_true "Total distinct inputs is 4" (report.total_inputs = 4);
      assert_true "Total timed iterations is 40" (report.stats.iterations = 40);
      assert_true "Min micros <= median micros" (report.stats.min_micros <= report.stats.median_micros);
      assert_true "Median micros <= p95 micros" (report.stats.median_micros <= report.stats.p95_micros);
      assert_true "P95 micros <= p99 micros" (report.stats.p95_micros <= report.stats.p99_micros);
      assert_true "Distribution buckets populated" (List.length report.stats.distribution_buckets > 0);

      (* Emit baseline JSON *)
      let json = report_to_json report in
      assert_true "Report JSON contains source_sha256" (String.contains json 's');
      assert_true "Report JSON contains binary_sha256" (String.contains json 'b');
      assert_true "Report JSON contains distributions" (String.contains json 'd');

      (* Write to evidence/benchmark_baseline.json *)
      (try Unix.mkdir "evidence" 0o755 with _ -> ());
      let oc = open_out "evidence/benchmark_baseline.json" in
      output_string oc json;
      close_out oc;
      assert_true "Evidence artifact evidence/benchmark_baseline.json written"
        (Sys.file_exists "evidence/benchmark_baseline.json");

      (* Cleanup *)
      (try Sys.remove tmp_bin with _ -> ());
      (try Sys.remove (tmp_bin ^ ".c") with _ -> ())

let () =
  Printf.printf "=== Running Benchmark Harness Test Suite (C008) ===\n";
  test_benchmark_generation ();
  Printf.printf "=== ALL BENCHMARK HARNESS TESTS PASSED ===\n"
