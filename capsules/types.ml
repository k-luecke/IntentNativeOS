(* capsules/types.ml - Capsule Specifications, Scopes, and Resource Contracts (V003, V004) *)

type capsule_id = string

type binary_format =
  | NativeElf64
  | WindowsPe32Plus
  | WindowsPe32
  | UnknownBinary of string

type guest_os_target =
  | TargetNativeMicrokernel
  | TargetLinuxGuest
  | TargetWindowsGuest of string (* version, e.g. "10.0" *)

type device_authority =
  | DevVirtioConsole
  | DevVirtioBlock
  | DevVirtioNet
  | DevSharedFramebuffer
  | DevKeyboardForwarding
  | DevPointerForwarding
  | DevForbiddenDevice of string

type authority_scope = {
  allowed_devices : device_authority list;
  network_access : bool;
  clipboard_read : bool;
  clipboard_write : bool;
  max_memory_mb : int;
  max_cpu_cores : int;
}

type resource_contract = {
  host_reserved_min_mb : int;
  guest_requested_mb : int;
  actual_allocated_mb : int;
  reclaimable_on_exit : bool;
}

type capsule_state =
  | Inspected of binary_format
  | Prepared of authority_scope
  | Running of int (* PID or guest VM ID *)
  | Stopped of int (* exit code *)
  | Reclaimed

type file_grant = {
  relative_path : string;
  read_allowed : bool;
  write_allowed : bool;
}

type provisioning_entitlement = {
  media_path : string;
  license_hash : string;
  user_supplied_media : bool;
  network_download_prohibited : bool;
}
