(* tests/test_operation_ir.ml - Test suite for Task C001: Typed Operation IR and Edge Validation *)

open Inos_operation.Operation_ir

let assert_true msg cond =
  if not cond then (
    Printf.eprintf "[FAIL] %s\n" msg;
    exit 1
  ) else
    Printf.printf "[PASS] %s\n" msg

let test_valid_operation_graph () =
  let node_a = {
    node_id = "node-read";
    op_name = "fs_read";
    inputs = [{ name = "path"; data_type = TypeFileUri; description = "input path" }];
    outputs = [{ name = "content"; data_type = TypeString; description = "file content" }];
    effects = [ReadFs "/workspace/input.txt"];
    preconditions = [PathExists "/workspace/input.txt"];
    required_capabilities = ["fs:read:/workspace"];
  } in
  let node_b = {
    node_id = "node-hash";
    op_name = "compute_sha256";
    inputs = [{ name = "content"; data_type = TypeString; description = "data to hash" }];
    outputs = [{ name = "digest"; data_type = TypeHashDigest; description = "sha256 hex" }];
    effects = [Pure];
    preconditions = [];
    required_capabilities = [];
  } in
  let edge = {
    from_node = "node-read";
    from_port = "content";
    to_node = "node-hash";
    to_port = "content";
  } in
  let graph = {
    graph_id = "graph-01";
    nodes = [node_a; node_b];
    edges = [edge];
  } in
  match validate_graph graph with
  | Ok () -> assert_true "Valid typed edge passes validation" true
  | Error _ -> assert_true "Valid graph failed validation" false

let test_type_mismatch_edge_rejected () =
  let node_a = {
    node_id = "node-hash";
    op_name = "compute_sha256";
    inputs = [{ name = "content"; data_type = TypeString; description = "data" }];
    outputs = [{ name = "digest"; data_type = TypeHashDigest; description = "digest" }];
    effects = [Pure];
    preconditions = [];
    required_capabilities = [];
  } in
  let node_b = {
    node_id = "node-dir";
    op_name = "list_dir";
    inputs = [{ name = "dir_path"; data_type = TypeDirectoryUri; description = "directory" }];
    outputs = [{ name = "entries"; data_type = TypeString; description = "entries" }];
    effects = [ReadFs "/tmp"];
    preconditions = [];
    required_capabilities = [];
  } in
  (* Illegal edge: HashDigest connected to DirectoryUri *)
  let edge = {
    from_node = "node-hash";
    from_port = "digest";
    to_node = "node-dir";
    to_port = "dir_path";
  } in
  let graph = {
    graph_id = "graph-bad-type";
    nodes = [node_a; node_b];
    edges = [edge];
  } in
  match validate_graph graph with
  | Error (TypeMismatchOnEdge { from_node; out_type; to_node; in_type; _ }) ->
      assert_true "Illegal type edge rejected"
        (from_node = "node-hash" && out_type = "HASH_DIGEST" && to_node = "node-dir" && in_type = "DIRECTORY_URI")
  | _ -> assert_true "Illegal type mismatch edge was accepted!" false

let test_missing_node_or_port_rejected () =
  let node_a = {
    node_id = "node-1";
    op_name = "op";
    inputs = [];
    outputs = [{ name = "out"; data_type = TypeString; description = "out" }];
    effects = [Pure];
    preconditions = [];
    required_capabilities = [];
  } in
  let edge_missing_node = {
    from_node = "node-1";
    from_port = "out";
    to_node = "non-existent-node";
    to_port = "in";
  } in
  let graph = {
    graph_id = "graph-bad-node";
    nodes = [node_a];
    edges = [edge_missing_node];
  } in
  match validate_graph graph with
  | Error (TargetNodeNotFound id) ->
      assert_true "Missing target node detected" (id = "non-existent-node")
  | _ -> assert_true "Missing node edge was accepted!" false

let () =
  Printf.printf "=== Running Operation IR & Edge Validation Suite (C001) ===\n";
  test_valid_operation_graph ();
  test_type_mismatch_edge_rejected ();
  test_missing_node_or_port_rejected ();
  Printf.printf "=== ALL OPERATION IR TESTS PASSED ===\n"
