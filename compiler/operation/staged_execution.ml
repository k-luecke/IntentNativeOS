(* compiler/operation/staged_execution.ml - Staged Execution, Verification, and Rollback Receipts *)

type rollback_outcome =
  | PerfectRestoration
  | Compensated of { details : string }
  | Irreversible of { reason : string }

type staged_step = {
  step_id : string;
  operation_id : string;
  is_reversible : bool;
  forward_action : unit -> (string, string) result;
  rollback_action : unit -> (rollback_outcome, string) result;
}

type rollback_receipt = {
  failed_step_id : string;
  failure_reason : string;
  restored_steps : string list;
  compensated_steps : (string * string) list;
  irreversible_steps : string list;
  is_clean_rollback : bool;
  receipt_hash : string;
}

type execution_summary =
  | Committed of {
      completed_steps : string list;
      outputs : (string * string) list;
      commit_hash : string;
    }
  | RolledBack of rollback_receipt

let compute_receipt_hash failed_id reason restored compensated irreversible =
  let buf = Buffer.create 128 in
  Buffer.add_string buf (Printf.sprintf "FAIL:%s:%s|" failed_id reason);
  List.iter (fun s -> Buffer.add_string buf (Printf.sprintf "R:%s;" s)) restored;
  List.iter (fun (s, d) -> Buffer.add_string buf (Printf.sprintf "C:%s=%s;" s d)) compensated;
  List.iter (fun s -> Buffer.add_string buf (Printf.sprintf "I:%s;" s)) irreversible;
  Inos_ipc.Sha256.digest_string (Buffer.contents buf)

(* Core Invariant / Task C005:
   Reversible staged plan with rollback receipts.
   Rollback distinguishes perfect restoration from best-effort compensation on partial failures. *)
let execute_staged_plan (steps : staged_step list) : execution_summary =
  let executed_stack = ref [] in
  let outputs = ref [] in
  let failure = ref None in

  let rec run_forward = function
    | [] -> ()
    | step :: rest ->
        match step.forward_action () with
        | Ok output ->
            executed_stack := step :: !executed_stack;
            outputs := (step.step_id, output) :: !outputs;
            run_forward rest
        | Error err ->
            failure := Some (step.step_id, err)
  in

  run_forward steps;

  match !failure with
  | None ->
      let completed = List.map (fun s -> s.step_id) steps in
      let commit_hash = Inos_ipc.Sha256.digest_string (String.concat "|" completed) in
      Committed {
        completed_steps = completed;
        outputs = List.rev !outputs;
        commit_hash;
      }
  | Some (failed_step_id, failure_reason) ->
      (* Partial failure encountered: rollback in reverse order *)
      let restored = ref [] in
      let compensated = ref [] in
      let irreversible = ref [] in

      List.iter (fun (step : staged_step) ->
        if not step.is_reversible then
          irreversible := step.step_id :: !irreversible
        else
          match step.rollback_action () with
          | Ok PerfectRestoration ->
              restored := step.step_id :: !restored
          | Ok (Compensated { details }) ->
              compensated := (step.step_id, details) :: !compensated
          | Ok (Irreversible { reason }) ->
              irreversible := (Printf.sprintf "%s (%s)" step.step_id reason) :: !irreversible
          | Error err ->
              compensated := (step.step_id, "Rollback error: " ^ err) :: !compensated
      ) !executed_stack;

      let is_clean = (List.length !irreversible = 0) && (List.length !compensated = 0) in
      let receipt_hash = compute_receipt_hash failed_step_id failure_reason !restored !compensated !irreversible in
      RolledBack {
        failed_step_id;
        failure_reason;
        restored_steps = !restored;
        compensated_steps = !compensated;
        irreversible_steps = !irreversible;
        is_clean_rollback = is_clean;
        receipt_hash;
      }
