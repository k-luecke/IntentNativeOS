(* tests/test_dag_validator.ml - Test suite for Task C003: Dependency DAG validator and cycle rejection *)

open Inos_operation.Operation_ir
open Inos_operation.Dag_validator

let assert_true msg cond =
  if not cond then (
    Printf.eprintf "[FAIL] %s\n" msg;
    exit 1
  ) else
    Printf.printf "[PASS] %s\n" msg

let make_dummy_node id = {
  node_id = id;
  op_name = "dummy";
  inputs = [];
  outputs = [];
  effects = [Pure];
  preconditions = [];
  required_capabilities = [];
}

let test_acyclic_plan_topological_sort () =
  let nodes = [make_dummy_node "A"; make_dummy_node "B"; make_dummy_node "C"] in
  let edges = [
    { from_node = "A"; from_port = "out"; to_node = "B"; to_port = "in" };
    { from_node = "B"; from_port = "out"; to_node = "C"; to_port = "in" };
  ] in
  let graph = { graph_id = "g-linear"; nodes; edges } in
  match validate_and_order_dag graph with
  | Ok order ->
      assert_true "Acyclic graph produces execution order" (List.length order = 3);
      assert_true "A runs before B" (List.nth order 0 = "A" && List.nth order 1 = "B");
      assert_true "B runs before C" (List.nth order 2 = "C")
  | Error _ -> assert_true "Acyclic plan failed ordering" false

let test_cyclic_plan_rejected_with_diagnostic () =
  let nodes = [make_dummy_node "A"; make_dummy_node "B"; make_dummy_node "C"] in
  (* Create a cycle: A -> B -> C -> A *)
  let edges = [
    { from_node = "A"; from_port = "out"; to_node = "B"; to_port = "in" };
    { from_node = "B"; from_port = "out"; to_node = "C"; to_port = "in" };
    { from_node = "C"; from_port = "out"; to_node = "A"; to_port = "in" };
  ] in
  let graph = { graph_id = "g-cycle"; nodes; edges } in
  match validate_and_order_dag graph with
  | Error (CyclicDependencyDetected { cycle_path; diagnostic }) ->
      assert_true "Cyclic plan rejected with error" (List.length cycle_path > 0);
      assert_true "Diagnostic describes the cycle"
        (String.contains diagnostic 'A' && String.contains diagnostic 'B' && String.contains diagnostic 'C');
      assert_true "Actionable explanation present" (String.length diagnostic > 20)
  | _ -> assert_true "Cyclic graph was accepted!" false

let () =
  Printf.printf "=== Running DAG Validator & Cycle Rejection Suite (C003) ===\n";
  test_acyclic_plan_topological_sort ();
  test_cyclic_plan_rejected_with_diagnostic ();
  Printf.printf "=== ALL DAG VALIDATOR TESTS PASSED ===\n"
