(* Canonical Serialization and Digest Binding (A001) *)
open Types

let string_of_action = function
  | File_Write -> "FILE_WRITE"
  | File_Delete -> "FILE_DELETE"
  | Kernel_Module_Load -> "KERNEL_MODULE_LOAD"
  | VM_Capsule_Launch -> "VM_CAPSULE_LAUNCH"
  | System_Reboot -> "SYSTEM_REBOOT"
  | Key_Enroll -> "KEY_ENROLL"
  | Device_Control -> "DEVICE_CONTROL"

let canonical_serialize (op : privileged_op) : string =
  let sorted_params = List.sort (fun (k1, _) (k2, _) -> String.compare k1 k2) op.parameters in
  let params_str =
    sorted_params
    |> List.map (fun (k, v) -> Printf.sprintf "%s=%s" k v)
    |> String.concat ";"
  in
  let sorted_constraints = List.sort String.compare op.constraints in
  let constraints_str = String.concat ";" sorted_constraints in
  Printf.sprintf
    "CANONICAL_OP:v1|ACTION:%s|TARGET:%s|CALLER:%s|TIME:%Ld|PARAMS:[%s]|CONSTRAINTS:[%s]"
    (string_of_action op.action)
    op.target
    op.caller_id
    op.timestamp
    params_str
    constraints_str

let compute_digest (op : privileged_op) : canonical_digest =
  let bytes = canonical_serialize op in
  Sha256.digest_string bytes

let verify_digest (op : privileged_op) (expected : canonical_digest) : bool =
  let actual = compute_digest op in
  String.equal actual expected
