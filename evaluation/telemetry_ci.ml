(* Boot, GUI Responsiveness and Crash-Free Repetition Telemetry (Q007) *)
open Types

let standard_ci_thresholds : telemetry_thresholds = {
  max_boot_time_sec = 2.5;
  target_framerate_fps = 60;
  max_input_latency_ms = 16.0;
  target_crash_free_cycles = 100;
}

let evaluate_telemetry_against_targets (run : run_telemetry) (targets : telemetry_thresholds) : (unit, string) result =
  if run.observed_boot_time_sec > targets.max_boot_time_sec then
    Error (Printf.sprintf "Boot time regression: observed %.2fs > target %.2fs" run.observed_boot_time_sec targets.max_boot_time_sec)
  else if run.observed_framerate_fps < targets.target_framerate_fps then
    Error (Printf.sprintf "GUI framerate regression: observed %d fps < target %d fps" run.observed_framerate_fps targets.target_framerate_fps)
  else if run.observed_input_latency_ms > targets.max_input_latency_ms then
    Error (Printf.sprintf "Input latency regression: observed %.1fms > target %.1fms" run.observed_input_latency_ms targets.max_input_latency_ms)
  else if run.completed_cycles_without_crash < targets.target_crash_free_cycles then
    Error (Printf.sprintf "Stability regression: completed %d cycles < target %d cycles" run.completed_cycles_without_crash targets.target_crash_free_cycles)
  else
    Ok ()
