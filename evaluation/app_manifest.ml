(* Application Compatibility Manifest and Failure Diagnostics (Q008) *)
open Types

let compatibility_manifest : app_compat_record list = [
  {
    app_name = "INOS Native Document Workspace";
    binary_format = "ELF64 Native";
    compatibility_level = `Native;
    tested_version = "v0.1-preview";
    known_limitations = [];
    fallback_strategy = "Native execution in user-space Ring 3";
  };
  {
    app_name = "Notepad / Text Viewer";
    binary_format = "PE32+ (Windows x86-64)";
    compatibility_level = `Ephemeral_Capsule;
    tested_version = "Build 19041";
    known_limitations = [
      "No COM/OLE automation across host boundary";
      "File I/O restricted to explicit broker document grants";
    ];
    fallback_strategy = "Ephemeral guest container with disposable COW disk";
  };
  {
    app_name = "7-Zip CLI Compression Engine";
    binary_format = "PE32+ (Windows x86-64)";
    compatibility_level = `Ephemeral_Capsule;
    tested_version = "23.01";
    known_limitations = [
      "Cannot access unmapped physical disk blocks";
      "Maximum memory budget capped at 1024 MB";
    ];
    fallback_strategy = "Ephemeral guest container with disposable COW disk";
  };
  {
    app_name = "Proprietary Kernel-Mode Anticheat Game";
    binary_format = "PE32+ Driver / Subsystem";
    compatibility_level = `Unsupported;
    tested_version = "v4.1.0";
    known_limitations = [
      "Demands Ring 0 kernel module loading";
      "Requires unrestricted raw physical PCI MMIO and GPU passthrough";
      "Incompatible with microkernel capability security architecture";
    ];
    fallback_strategy = "Explicit diagnostic rejection: ERR_COMPAT_RING0_DRIVER_PROHIBITED";
  };
]

let query_compatibility (app_name : string) : app_compat_record option =
  List.find_opt (fun r -> String.equal r.app_name app_name) compatibility_manifest
