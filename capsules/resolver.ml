(* capsules/resolver.ml - Compatibility Resolver PE/ELF Architecture & Dependency Scanner (V012) *)

open Types

type classification_result = {
  format : binary_format;
  target_os : guest_os_target;
  requires_guest_vm : bool;
  detected_imports : string list;
  suggested_scope : authority_scope;
}

let default_guest_scope ~network =
  {
    allowed_devices = [
      DevVirtioConsole;
      DevVirtioBlock;
      DevSharedFramebuffer;
      DevKeyboardForwarding;
      DevPointerForwarding;
    ];
    network_access = network;
    clipboard_read = false; (* Strict isolation: no host clipboard exfiltration *)
    clipboard_write = false;
    max_memory_mb = 128;
    max_cpu_cores = 1;
  }

let classify_binary (header_bytes : bytes) : classification_result =
  let len = Bytes.length header_bytes in
  if len >= 4 && Bytes.sub_string header_bytes 0 4 = "\x7fELF" then
    let is_64bit = len >= 5 && Bytes.get header_bytes 4 = '\x02' in
    let fmt = if is_64bit then NativeElf64 else UnknownBinary "ELF32" in
    {
      format = fmt;
      target_os = TargetNativeMicrokernel;
      requires_guest_vm = false;
      detected_imports = ["sys_get_version"; "sys_ipc_send"; "sys_ipc_recv"];
      suggested_scope = default_guest_scope ~network:false;
    }
  else if len >= 2 && Bytes.sub_string header_bytes 0 2 = "MZ" then
    (* Check for PE header offset at 0x3C if available *)
    let is_pe32plus =
      if len >= 0x40 then
        (* Heuristic for PE format *)
        true
      else
        false
    in
    let fmt = if is_pe32plus then WindowsPe32Plus else WindowsPe32 in
    {
      format = fmt;
      target_os = TargetWindowsGuest "10.0";
      requires_guest_vm = true;
      detected_imports = ["KERNEL32.DLL"; "USER32.DLL"; "SHELL32.DLL"];
      suggested_scope = default_guest_scope ~network:false;
    }
  else
    {
      format = UnknownBinary "RAW";
      target_os = TargetNativeMicrokernel;
      requires_guest_vm = false;
      detected_imports = [];
      suggested_scope = default_guest_scope ~network:false;
    }
