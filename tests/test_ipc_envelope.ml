(* tests/test_ipc_envelope.ml - OCaml unit tests for IPC Envelope *)

open Inos_ipc.Envelope

let assert_true msg cond =
  if not cond then (
    Printf.eprintf "[FAIL] %s\n" msg;
    exit 1
  ) else
    Printf.printf "[PASS] %s\n" msg

let test_envelope_creation () =
  let env = make ~msg_id:"msg-001" ~sender:IntentCompiler ~recipient:ModelA ~msg_type:Request ~payload:"test-payload" () in
  assert_true "Schema is inos.ipc.v0.1" (env.schema = "inos.ipc.v0.1");
  assert_true "Sender is IntentCompiler" (env.sender = IntentCompiler);
  assert_true "Recipient is ModelA" (env.recipient = ModelA);
  assert_true "Msg type is Request" (env.msg_type = Request);
  assert_true "Digest is non-empty" (String.length env.digest > 0)

let test_roundtrip_serialization () =
  let env = make ~msg_id:"msg-002" ~sender:Broker ~recipient:Capsule ~msg_type:Response ~payload:"auth-grant" () in
  let wire = to_wire_json env in
  match of_wire_json wire with
  | Ok decoded ->
      assert_true "Decoded msg_id matches" (decoded.msg_id = env.msg_id);
      assert_true "Decoded sender matches" (decoded.sender = env.sender);
      assert_true "Decoded recipient matches" (decoded.recipient = env.recipient);
      assert_true "Decoded payload matches" (decoded.payload = env.payload);
      assert_true "Decoded digest matches" (decoded.digest = env.digest)
  | Error _ ->
      assert_true "Failed to decode valid wire JSON" false

let test_tamper_detection () =
  let env = make ~msg_id:"msg-003" ~sender:ModelB ~recipient:IntentCompiler ~msg_type:Notification ~payload:"clean-proposal" () in
  let wire = to_wire_json env in
  (* Intentionally tamper with the payload without updating digest *)
  let tampered = String.concat "malicious-proposal" (String.split_on_char 'c' wire) in
  match of_wire_json tampered with
  | Ok _ ->
      assert_true "Tampered envelope was erroneously accepted" false
  | Error (DigestMismatch _) ->
      assert_true "Tampered envelope was correctly rejected via digest mismatch" true
  | Error _ ->
      assert_true "Tampered envelope was rejected" true

let test_schema_mismatch () =
  let env = make ~msg_id:"msg-004" ~sender:UI ~recipient:IntentCompiler ~msg_type:Request ~payload:"query" () in
  let wire = to_wire_json env in
  (* Modify schema to invalid version *)
  let parts = String.split_on_char 'v' wire in
  let modified_schema = String.concat "v9" parts in
  match of_wire_json modified_schema with
  | Ok _ ->
      assert_true "Invalid schema was erroneously accepted" false
  | Error (SchemaMismatch _) ->
      assert_true "Schema mismatch was correctly rejected" true
  | Error _ ->
      assert_true "Schema mismatch rejected" true

let () =
  Printf.printf "=== Running IPC Envelope Test Suite ===\n";
  test_envelope_creation ();
  test_roundtrip_serialization ();
  test_tamper_detection ();
  test_schema_mismatch ();
  Printf.printf "=== ALL IPC ENVELOPE TESTS PASSED ===\n"
