(* tests/test_runtime_b.ml - Test suite for Task R003: Connect second local runtime as Model B *)

open Inos_models.Protocol
open Inos_models.Runtime_b

let assert_true msg cond =
  if not cond then (
    Printf.eprintf "[FAIL] %s\n" msg;
    exit 1
  ) else
    Printf.printf "[PASS] %s\n" msg

let test_schema_valid_proposal_offline () =
  MockAdapter.set_failure_mode None;
  MockAdapter.set_mock_chunks ["implementation proposal for zero copy shm"];
  match generate_implementation_proposal ~model_name:"llama-local" ~plan_ref:"plan-01" ~plan_description:"zero copy shm" with
  | Ok prop ->
      assert_true "Model B returns valid proposal" (String.length prop.proposal_id > 0);
      assert_true "Plan reference preserved" (prop.plan_ref = "plan-01");
      assert_true "Proposed modules identified" (List.length prop.proposed_modules > 0);
      assert_true "Proposed diff present" (String.contains prop.proposed_diff '+');
      assert_true "Offline verified flag is true" (prop.offline_verified)
  | Error _ -> assert_true "Failed to generate proposal" false

let test_cloud_reference_in_proposal_rejected () =
  let cloud_payload = "Download artifact from https://cloud-storage.com/blob" in
  match parse_proposal ~plan_ref:"plan-01" cloud_payload with
  | Error (CloudDependencyInProposal msg) ->
      assert_true "Cloud reference in proposal rejected" (String.length msg > 0)
  | _ -> assert_true "Cloud reference in proposal accepted!" false

let () =
  Printf.printf "=== Running Model B Runtime Test Suite (R003) ===\n";
  test_schema_valid_proposal_offline ();
  test_cloud_reference_in_proposal_rejected ();
  Printf.printf "=== ALL MODEL B RUNTIME TESTS PASSED ===\n"
