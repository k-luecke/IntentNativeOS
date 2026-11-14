(* tests/test_capability_binding.ml - Test suite for Task C004: Capability-to-operation binding contracts *)

open Inos_operation.Operation_ir
open Inos_operation.Capability_binding

let assert_true msg cond =
  if not cond then (
    Printf.eprintf "[FAIL] %s\n" msg;
    exit 1
  ) else
    Printf.printf "[PASS] %s\n" msg

let test_valid_capability_binding () =
  let grant = {
    grant_id = "grant-101";
    target_operation_id = "op-read";
    target_resource = "/workspace/file.txt";
    granted_capabilities = ["fs:read:/workspace"];
    permitted_effects = ["READ_FS:/workspace/file.txt"];
    expires_at = 9999999999.0;
  } in
  let node = {
    node_id = "op-read";
    op_name = "fs_read";
    inputs = [];
    outputs = [];
    effects = [ReadFs "/workspace/file.txt"];
    preconditions = [];
    required_capabilities = ["fs:read:/workspace"];
  } in
  match verify_operation_binding grant node with
  | Ok () -> assert_true "Valid capability grant binds cleanly" true
  | Error _ -> assert_true "Valid binding failed" false

let test_undeclared_capability_blocked () =
  let grant = {
    grant_id = "grant-102";
    target_operation_id = "op-proc";
    target_resource = "/bin/ls";
    granted_capabilities = ["fs:read:/bin"]; (* Missing proc:spawn *)
    permitted_effects = ["ALL_EFFECTS"];
    expires_at = 9999999999.0;
  } in
  let node = {
    node_id = "op-proc";
    op_name = "proc_spawn";
    inputs = [];
    outputs = [];
    effects = [Pure];
    preconditions = [];
    required_capabilities = ["fs:read:/bin"; "proc:spawn"];
  } in
  match verify_operation_binding grant node with
  | Error (UndeclaredCapabilityBlocked { operation_id; undeclared_cap; _ }) ->
      assert_true "Operation requesting undeclared capability blocked"
        (operation_id = "op-proc" && undeclared_cap = "proc:spawn")
  | _ -> assert_true "Undeclared capability was permitted!" false

let test_effect_exceeding_grant_blocked () =
  let grant = {
    grant_id = "grant-103";
    target_operation_id = "op-write";
    target_resource = "/tmp/scratch";
    granted_capabilities = ["fs:write:/tmp"];
    permitted_effects = ["WRITE_FS:/tmp/scratch"]; (* Does NOT allow write to /etc *)
    expires_at = 9999999999.0;
  } in
  let node = {
    node_id = "op-write";
    op_name = "fs_write";
    inputs = [];
    outputs = [];
    effects = [WriteFs "/etc/shadow"];
    preconditions = [];
    required_capabilities = ["fs:write:/tmp"];
  } in
  match verify_operation_binding grant node with
  | Error (EffectExceedsCapabilityGrant { forbidden_effect; _ }) ->
      assert_true "Effect exceeding capability grant blocked" (forbidden_effect = "WRITE_FS:/etc/shadow")
  | _ -> assert_true "Effect exceeding grant was permitted!" false

let test_sandbox_execution_and_escape_prevention () =
  let sandbox_root = "/var/anos/sandbox" in
  (* Valid read within sandbox *)
  match execute_sandbox_operation ~sandbox_root ~op_name:"read" ~target_path:"/var/anos/sandbox/fixtures/data.txt" () with
  | Ok res -> assert_true "Fixture read within sandbox succeeds" (String.length res > 0)
  | Error _ -> assert_true "Valid sandbox read failed" false;

  (* Valid copy within sandbox *)
  match execute_sandbox_operation ~sandbox_root ~op_name:"copy" ~target_path:"/var/anos/sandbox/dest.txt" ~content:"payload" () with
  | Ok res -> assert_true "Copy within sandbox succeeds" (String.contains res 'p')
  | Error _ -> assert_true "Valid sandbox copy failed" false;

  (* Attempt sandbox escape via directory traversal / outside path *)
  match execute_sandbox_operation ~sandbox_root ~op_name:"read" ~target_path:"/var/anos/sandbox/../etc/passwd" () with
  | Error (SandboxEscapeAttempted _) ->
      assert_true "Sandbox escape via .. blocked" true
  | _ -> assert_true "Sandbox escape via .. was permitted!" false;

  match execute_sandbox_operation ~sandbox_root ~op_name:"read" ~target_path:"/etc/hosts" () with
  | Error (SandboxEscapeAttempted _) ->
      assert_true "Direct external path escape blocked" true
  | _ -> assert_true "Direct external read was permitted!" false

let () =
  Printf.printf "=== Running Capability-to-Operation Binding Suite (C004) ===\n";
  test_valid_capability_binding ();
  test_undeclared_capability_blocked ();
  test_effect_exceeding_grant_blocked ();
  test_sandbox_execution_and_escape_prevention ();
  Printf.printf "=== ALL CAPABILITY BINDING TESTS PASSED ===\n"
