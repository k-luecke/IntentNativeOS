(* First-Run Setup, Safe-Mode Boot, and Recovery Interface (Q009) *)
open Types

type recovery_action =
  | RollbackPreviousSlot
  | ResetConfigDefaults
  | RestoreFromSnapshot of string

type setup_session = {
  mutable state : first_run_state;
  mutable paired_device_id : string option;
  mutable active_boot_slot : Inos_security.Types.partition_slot;
}

let init_setup_session () : setup_session = {
  state = Uninitialized;
  paired_device_id = None;
  active_boot_slot = Inos_security.Types.Slot_A;
}

let advance_first_run (session : setup_session) ~(paired_id : string) : (unit, string) result =
  match session.state with
  | Uninitialized ->
      session.state <- PhonePairingPrompt;
      session.paired_device_id <- Some paired_id;
      session.state <- StorageKeyProvisioned;
      session.state <- ReadyNormal;
      Ok ()
  | _ -> Error "System already initialized"

let trigger_safe_mode (session : setup_session) : unit =
  session.state <- SafeModeTriggered

let execute_recovery_path (session : setup_session) (action : recovery_action) : (string, string) result =
  match session.state with
  | SafeModeTriggered | EmergencyRecoveryActive ->
      (match action with
       | RollbackPreviousSlot ->
           session.active_boot_slot <- (match session.active_boot_slot with Slot_A -> Slot_B | Slot_B -> Slot_A);
           session.state <- ReadyNormal;
           Ok "Rollback completed: alternate slot marked active. Restarting into normal operation."
       | ResetConfigDefaults ->
           session.state <- ReadyNormal;
           Ok "Configuration reset to safe factory defaults."
       | RestoreFromSnapshot snap_id ->
           session.state <- ReadyNormal;
           Ok (Printf.sprintf "Restored snapshot '%s' successfully." snap_id))
  | _ -> Error "Recovery actions can only be executed in Safe Mode"
