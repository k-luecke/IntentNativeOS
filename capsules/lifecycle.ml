(* capsules/lifecycle.ml - Capsule Lifecycle & Resource Reclaim (V003, V004, V011, V014) *)

open Types

type capsule_instance = {
  id : capsule_id;
  scope : authority_scope;
  contract : resource_contract;
  mutable state : capsule_state;
}

let create_instance cid scope contract =
  {
    id = cid;
    scope;
    contract;
    state = Prepared scope;
  }

let validate_device_request instance (dev : device_authority) =
  match dev with
  | DevForbiddenDevice _ -> false
  | d -> List.mem d instance.scope.allowed_devices

let launch instance vm_pid =
  match instance.state with
  | Prepared _ ->
      instance.state <- Running vm_pid;
      Ok ()
  | _ -> Error "Invalid state transition for launch"

let stop instance exit_code =
  match instance.state with
  | Running _ ->
      instance.state <- Stopped exit_code;
      Ok ()
  | _ -> Error "Capsule not running"

let reclaim instance =
  match instance.state with
  | Stopped _ ->
      instance.state <- Reclaimed;
      Ok instance.contract.actual_allocated_mb
  | _ -> Error "Cannot reclaim running or unprepared capsule"

let verify_lawful_provisioning (entitlement : provisioning_entitlement) =
  if not entitlement.user_supplied_media then
    Error "Unauthorized: guest provisioning requires user-supplied lawful media"
  else if not entitlement.network_download_prohibited then
    Error "Security policy: external automated media downloads prohibited"
  else if String.length entitlement.license_hash < 8 then
    Error "Invalid license entitlement binding"
  else
    Ok true
