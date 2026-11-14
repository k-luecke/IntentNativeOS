(* tests/test_serialize.ml - Test suite for Task I010: Versioned JSON / CBOR envelope serialization *)

open Inos_ir.Mode
open Inos_ir.Contract
open Inos_ir.Serialize

let assert_true msg cond =
  if not cond then (
    Printf.eprintf "[FAIL] %s\n" msg;
    exit 1
  ) else
    Printf.printf "[PASS] %s\n" msg

let test_save_load_roundtrip () =
  let original = make
    ~id:"intent-42"
    ~revision:3
    ~mode:Design
    ~objective:{ value = "compile capability manifest"; origin = User_explicit; binding = Constraint; evidence = [] }
    ~targets:[{ value = FileTarget "/workspace/capsule.yaml"; origin = User_explicit; binding = Constraint; evidence = [] }]
    ~constraints:[]
    ~ambiguities:[]
    ~verification_criteria:["dune build passes"]
    ~authority_scope:["fs:read:/workspace"]
    ()
  in
  let json_str = to_json original in
  assert_true "Serialized JSON has content" (String.length json_str > 0);
  assert_true "Serialized JSON contains schema identifier"
    (find_sub ~pattern:"inos.intent.v0.1" json_str 0 <> None);

  match from_json json_str with
  | Ok loaded ->
      assert_true "Roundtrip preserves id" (loaded.id = original.id);
      assert_true "Roundtrip preserves revision" (loaded.revision = original.revision);
      assert_true "Roundtrip preserves mode" (loaded.mode = original.mode);
      assert_true "Roundtrip preserves objective value" (loaded.objective.value = original.objective.value);
      assert_true "Roundtrip preserves objective origin" (loaded.objective.origin = original.objective.origin);
      assert_true "Roundtrip preserves objective binding" (loaded.objective.binding = original.objective.binding)
  | Error msg ->
      assert_true (Printf.sprintf "Roundtrip deserialization failed: %s" msg) false

let test_version_mismatch_diagnosed () =
  let original = make
    ~id:"intent-99"
    ~revision:1
    ~mode:Explore
    ~objective:{ value = "explore memory map"; origin = User_explicit; binding = Preference; evidence = [] }
    ~targets:[]
    ~constraints:[]
    ~ambiguities:[]
    ~verification_criteria:[]
    ~authority_scope:[]
    ()
  in
  let json_str = to_json original in
  (* Alter schema to simulate incompatible future version *)
  let tampered_json =
    let prefix = "{\"schema\":\"inos.intent.v0.1\"" in
    let replacement = "{\"schema\":\"inos.intent.v2.0\"" in
    match find_sub ~pattern:prefix json_str 0 with
    | Some idx ->
        replacement ^ String.sub json_str (idx + String.length prefix) (String.length json_str - (idx + String.length prefix))
    | None -> json_str
  in
  match from_json tampered_json with
  | Error msg ->
      assert_true "Version mismatch diagnosed"
        (find_sub ~pattern:"Schema version mismatch: expected 'inos.intent.v0.1', found 'inos.intent.v2.0'" msg 0 <> None)
  | Ok _ ->
      assert_true "Incompatible schema version accepted!" false

let test_missing_required_fields () =
  let bad_json = "{\"schema\":\"inos.intent.v0.1\",\"revision\":1}" in
  match from_json bad_json with
  | Error msg ->
      assert_true "Missing 'id' diagnosed" (find_sub ~pattern:"missing 'id' field" msg 0 <> None)
  | Ok _ ->
      assert_true "Malformed JSON accepted!" false

let () =
  Printf.printf "=== Running Versioned IR Serialization Test Suite (I010) ===\n";
  test_save_load_roundtrip ();
  test_version_mismatch_diagnosed ();
  test_missing_required_fields ();
  Printf.printf "=== ALL SERIALIZATION TESTS PASSED ===\n"
