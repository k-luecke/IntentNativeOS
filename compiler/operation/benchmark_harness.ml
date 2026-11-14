(* compiler/operation/benchmark_harness.ml - Reproducible Benchmark Harness & Baseline (C008)
   Records source digest, binary hash, inputs, and latency distributions (min, p50, p95, p99, mean)
   into a reproducible audit report.
*)

type timing_stats = {
  iterations: int;
  min_micros: float;
  median_micros: float;
  p95_micros: float;
  p99_micros: float;
  max_micros: float;
  mean_micros: float;
  stddev_micros: float;
  distribution_buckets: (string * int) list;
}

type benchmark_report = {
  source_hash: string;
  binary_hash: string;
  input_summary: string;
  total_inputs: int;
  stats: timing_stats;
  timestamp: string;
}

let compute_sha256 s =
  Inos_ipc.Sha256.digest_string s

let file_sha256 path =
  let ic = open_in_bin path in
  let len = in_channel_length ic in
  let buf = Bytes.create len in
  really_input ic buf 0 len;
  close_in ic;
  Inos_ipc.Sha256.digest_bytes buf

let string_sha256 s =
  compute_sha256 s

let compute_percentile sorted_arr pct =
  let len = Array.length sorted_arr in
  if len = 0 then 0.0
  else
    let idx = int_of_float (float_of_int (len - 1) *. (pct /. 100.0)) in
    sorted_arr.(idx)

let compute_stats (samples : float list) : timing_stats =
  let arr = Array.of_list samples in
  Array.sort Float.compare arr;
  let len = Array.length arr in
  if len = 0 then {
    iterations = 0;
    min_micros = 0.0;
    median_micros = 0.0;
    p95_micros = 0.0;
    p99_micros = 0.0;
    max_micros = 0.0;
    mean_micros = 0.0;
    stddev_micros = 0.0;
    distribution_buckets = [];
  } else
    let min_val = arr.(0) in
    let max_val = arr.(len - 1) in
    let median_val = compute_percentile arr 50.0 in
    let p95_val = compute_percentile arr 95.0 in
    let p99_val = compute_percentile arr 99.0 in
    let sum = Array.fold_left ( +. ) 0.0 arr in
    let mean_val = sum /. float_of_int len in
    let variance_sum =
      Array.fold_left (fun acc x -> acc +. ((x -. mean_val) ** 2.0)) 0.0 arr
    in
    let stddev_val = sqrt (variance_sum /. float_of_int len) in

    (* Bucketing for distribution *)
    let bucket_step = (max_val -. min_val) /. 5.0 in
    let buckets =
      if bucket_step <= 0.0001 then [("all", len)]
      else
        let b = Array.make 5 0 in
        Array.iter (fun x ->
          let idx = min 4 (int_of_float ((x -. min_val) /. bucket_step)) in
          b.(idx) <- b.(idx) + 1
        ) arr;
        [
          (Printf.sprintf "[%.1f - %.1f µs]" min_val (min_val +. bucket_step), b.(0));
          (Printf.sprintf "[%.1f - %.1f µs]" (min_val +. bucket_step) (min_val +. 2. *. bucket_step), b.(1));
          (Printf.sprintf "[%.1f - %.1f µs]" (min_val +. 2. *. bucket_step) (min_val +. 3. *. bucket_step), b.(2));
          (Printf.sprintf "[%.1f - %.1f µs]" (min_val +. 3. *. bucket_step) (min_val +. 4. *. bucket_step), b.(3));
          (Printf.sprintf "[%.1f - %.1f µs]" (min_val +. 4. *. bucket_step) max_val, b.(4));
        ]
    in
    {
      iterations = len;
      min_micros = min_val;
      median_micros = median_val;
      p95_micros = p95_val;
      p99_micros = p99_val;
      max_micros = max_val;
      mean_micros = mean_val;
      stddev_micros = stddev_val;
      distribution_buckets = buckets;
    }

let run_benchmark
    ~(source_code : string)
    ~(binary_path : string)
    ~(inputs : int64 list list)
    ~(warmup_runs : int)
    ~(timed_runs : int)
  : benchmark_report =
  let src_hash = string_sha256 source_code in
  let bin_hash =
    if Sys.file_exists binary_path then file_sha256 binary_path
    else string_sha256 "in-memory-backend"
  in

  (* Warmup *)
  for _ = 1 to warmup_runs do
    List.iter (fun inps ->
      ignore (Native_backend.run_native binary_path inps)
    ) inputs
  done;

  (* Timed executions *)
  let samples = ref [] in
  for _ = 1 to timed_runs do
    List.iter (fun inps ->
      let t0 = Unix.gettimeofday () in
      let _ = Native_backend.run_native binary_path inps in
      let t1 = Unix.gettimeofday () in
      let micros = (t1 -. t0) *. 1_000_000.0 in
      samples := micros :: !samples
    ) inputs
  done;

  let stats = compute_stats !samples in
  let now =
    let tm = Unix.gmtime (Unix.gettimeofday ()) in
    Printf.sprintf "%04d-%02d-%02dT%02d:%02d:%02dZ"
      (tm.tm_year + 1900) (tm.tm_mon + 1) tm.tm_mday tm.tm_hour tm.tm_min tm.tm_sec
  in
  let input_summary =
    Printf.sprintf "%d distinct vectors; total vector runs = %d"
      (List.length inputs) (List.length inputs * timed_runs)
  in
  {
    source_hash = src_hash;
    binary_hash = bin_hash;
    input_summary;
    total_inputs = List.length inputs;
    stats;
    timestamp = now;
  }

let report_to_json (r : benchmark_report) : string =
  let bucket_items =
    List.map (fun (range, count) ->
      Printf.sprintf "      {\"range\": \"%s\", \"count\": %d}" range count
    ) r.stats.distribution_buckets |> String.concat ",\n"
  in
  Printf.sprintf {|{
  "benchmark_report_version": "1.0",
  "timestamp": "%s",
  "source_sha256": "%s",
  "binary_sha256": "%s",
  "inputs": {
    "summary": "%s",
    "total_distinct_vectors": %d
  },
  "distributions": {
    "iterations": %d,
    "min_micros": %.3f,
    "median_micros": %.3f,
    "p95_micros": %.3f,
    "p99_micros": %.3f,
    "max_micros": %.3f,
    "mean_micros": %.3f,
    "stddev_micros": %.3f,
    "buckets": [
%s
    ]
  }
}
|} r.timestamp r.source_hash r.binary_hash r.input_summary r.total_inputs
   r.stats.iterations r.stats.min_micros r.stats.median_micros r.stats.p95_micros
   r.stats.p99_micros r.stats.max_micros r.stats.mean_micros r.stats.stddev_micros
   bucket_items
