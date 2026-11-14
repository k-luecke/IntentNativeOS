(* tools/supervisor/types.ml - Core types for the Autonomous Supervisor *)

type task_status =
  | Ready
  | Leased of { worker_id : string; leased_at : float }
  | Implemented of { patch_ref : string }
  | Verifying
  | Accepted of { evidence_ref : string; accepted_at : float }
  | Blocked of { reason : string; attempts : int }

type task = {
  id : string;
  gate : string;
  area : string;
  title : string;
  depends_on : string list;
  done_when : string;
  mutable status : task_status;
  mutable attempts : int;
  mutable evidence : string option;
  mutable notes : string;
}

type event =
  | EventTaskLeased of { task_id : string; worker_id : string; timestamp : float }
  | EventTaskImplemented of { task_id : string; patch_ref : string; timestamp : float }
  | EventTaskVerified of { task_id : string; passed : bool; details : string; timestamp : float }
  | EventTaskAccepted of { task_id : string; evidence_ref : string; timestamp : float }
  | EventTaskFailed of { task_id : string; reason : string; attempt : int; timestamp : float }
  | EventTaskBlocked of { task_id : string; reason : string; timestamp : float }
  | EventCheckpoint of { timestamp : float; accepted_count : int; blocked_count : int }

type supervisor_config = {
  journal_path : string;
  checkpoint_path : string;
  backlog_csv_path : string;
  evidence_dir : string;
  max_attempts : int;
  task_timeout_sec : float;
}
