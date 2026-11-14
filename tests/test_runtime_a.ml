(* tests/test_runtime_a.ml - Test suite for Task R002: Connect local runtime as Model A *)

open Inos_models.Protocol
open Inos_models.Runtime_a

let assert_true msg cond =
  if not cond then (
    Printf.eprintf "[FAIL] %s\n" msg;
    exit 1
  ) else
    Printf.printf "[PASS] %s\n" msg

let test_schema_valid_test_plan_offline () =
  MockAdapter.set_failure_mode None;
  MockAdapter.set_mock_chunks ["{\"plan_id\": \"plan-01\", \"tests\": [\"test_ipc\"] }"];
  match generate_test_plan ~model_name:"qwen-local" ~intent_id:"intent-01" ~intent_objective:"compile kernel" with
  | Ok plan ->
      assert_true "Model A returns valid test plan" (String.length plan.plan_id > 0);
      assert_true "Plan includes verification commands" (List.mem "dune runtest" plan.verification_commands);
      assert_true "Plan has no cloud dependency (cloud_free = true)" (plan.cloud_free)
  | Error _ -> assert_true "Failed to generate valid test plan" false

let test_cloud_dependency_rejected () =
  let cloud_payload = "{\"endpoint\": \"https://api.openai.com/v1/chat/completions\"}" in
  match parse_test_plan cloud_payload with
  | Error (CloudDependencyDetected msg) ->
      assert_true "Cloud dependency detected and rejected" (String.length msg > 0)
  | _ -> assert_true "Cloud dependency was accepted!" false

let () =
  Printf.printf "=== Running Model A Runtime Test Suite (R002) ===\n";
  test_schema_valid_test_plan_offline ();
  test_cloud_dependency_rejected ();
  Printf.printf "=== ALL MODEL A RUNTIME TESTS PASSED ===\n"
