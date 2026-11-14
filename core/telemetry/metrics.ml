(* core/telemetry/metrics.ml - Telemetry measurement and reporting *)

type telemetry_record = {
  task_id : string;
  timestamp : float;
  clarifications_asked : int;
  manual_corrections : int;
  selection_clicks : int;
  executed_successfully : bool;
  peak_rss_bytes : int;
  start_time : float;
  end_time : float;
  uncontained_crashes : int;
}

let calculate_hic record =
  (float_of_int record.clarifications_asked) +.
  (2.0 *. float_of_int record.manual_corrections) +.
  (0.5 *. float_of_int record.selection_clicks)

let calculate_latency_ms record =
  (record.end_time -. record.start_time) *. 1000.0

let to_json_string record =
  let hic = calculate_hic record in
  let latency_ms = calculate_latency_ms record in
  let fidelity = if record.executed_successfully then 1.0 else 0.0 in
  Printf.sprintf
    "{\"schema\":\"inos.telemetry.v0.1\",\"task_id\":\"%s\",\"timestamp\":%.3f,\"metrics\":{\"hic_score\":%.2f,\"intent_fidelity\":%.2f,\"peak_rss_bytes\":%d,\"latency_ms\":%.2f,\"uncontained_crashes\":%d}}"
    record.task_id
    record.timestamp
    hic
    fidelity
    record.peak_rss_bytes
    latency_ms
    record.uncontained_crashes
