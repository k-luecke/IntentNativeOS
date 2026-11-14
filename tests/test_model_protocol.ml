(* tests/test_model_protocol.ml - Test suite for Task R001: Local model adapter protocol *)

open Inos_models.Protocol

let assert_true msg cond =
  if not cond then (
    Printf.eprintf "[FAIL] %s\n" msg;
    exit 1
  ) else
    Printf.printf "[PASS] %s\n" msg

let test_health_check () =
  MockAdapter.set_failure_mode None;
  match MockAdapter.check_health () with
  | Healthy { latency_ms; _ } ->
      assert_true "Mock adapter health check healthy" (latency_ms > 0.0)
  | _ -> assert_true "Expected Healthy status" false;

  MockAdapter.set_failure_mode (Some (ConnectionFailed "Socket unreachable"));
  match MockAdapter.check_health () with
  | Unhealthy { error } ->
      assert_true "Health check reports Unhealthy on connection failure" (error = "Socket unreachable")
  | _ -> assert_true "Expected Unhealthy status" false;
  MockAdapter.set_failure_mode None

let test_streaming_and_buffered () =
  MockAdapter.set_mock_chunks ["chunk_1 "; "chunk_2 "; "final_chunk"];
  let chunks_received = ref [] in
  let req = {
    request_id = "test-req-1";
    model_name = "qwen-local";
    prompt = "generate plan";
    max_tokens = 100;
    temperature = 0.1;
    timeout_seconds = 5.0;
  } in
  let stream_res = MockAdapter.invoke_streaming req ~on_chunk:(fun c ->
    chunks_received := c :: !chunks_received
  ) in
  match stream_res with
  | Ok resp ->
      assert_true "Streaming completed successfully" (resp.request_id = "test-req-1");
      assert_true "Streamed full text combined accurately"
        (resp.full_text = "chunk_1 chunk_2 final_chunk");
      assert_true "All 3 chunks streamed" (List.length !chunks_received = 3);
      let last_chunk = List.hd !chunks_received in
      assert_true "Final chunk marked is_final" (last_chunk.is_final)
  | Error _ -> assert_true "Streaming unexpectedly failed" false

let test_timeout_and_error_handling () =
  let req_timeout = {
    request_id = "test-req-timeout";
    model_name = "qwen-local";
    prompt = "slow query";
    max_tokens = 100;
    temperature = 0.1;
    timeout_seconds = 0.01; (* triggers timeout *)
  } in
  match MockAdapter.invoke_buffered req_timeout with
  | Error (TimeoutExceeded { elapsed; limit }) ->
      assert_true "Timeout error caught and reported" (elapsed > limit)
  | _ -> assert_true "Expected TimeoutExceeded error" false

let () =
  Printf.printf "=== Running Local Model Adapter Protocol Suite (R001) ===\n";
  test_health_check ();
  test_streaming_and_buffered ();
  test_timeout_and_error_handling ();
  Printf.printf "=== ALL MODEL PROTOCOL TESTS PASSED ===\n"
