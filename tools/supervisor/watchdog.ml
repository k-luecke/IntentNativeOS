(* tools/supervisor/watchdog.ml - Watchdog, timeouts, and bounded retry loops *)

open Types

let max_allowed_attempts = 3

let check_task_retry task reason =
  task.attempts <- task.attempts + 1;
  if task.attempts >= max_allowed_attempts then (
    task.status <- Blocked {
      reason = Printf.sprintf "Exceeded %d repair attempts: %s" max_allowed_attempts reason;
      attempts = task.attempts;
    };
    `Quarantined
  ) else (
    task.status <- Ready;
    `Retry_Scheduled
  )

let is_timed_out (task : task) timeout_sec current_time =
  match task.status with
  | Leased { leased_at; _ } ->
      (current_time -. leased_at) > timeout_sec
  | _ -> false
