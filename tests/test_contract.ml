(* tests/test_contract.ml - Tests for Task I002: Typed objective, target, and contract *)

open Inos_ir.Mode
open Inos_ir.Contract

let assert_true msg cond =
  if not cond then (
    Printf.eprintf "[FAIL] %s\n" msg;
    exit 1
  ) else
    Printf.printf "[PASS] %s\n" msg

let test_origins_and_bindings () =
  let origins = [
    ("USER_EXPLICIT", User_explicit);
    ("USER_SELECTED_EXAMPLE", User_selected_example);
    ("WORKSPACE_OBSERVED", Workspace_observed);
    ("MODEL_INFERRED", Model_inferred);
    ("POLICY_DEFAULT", Policy_default);
  ] in
  List.iter (fun (str, expected) ->
    match origin_of_string str with
    | Ok o ->
        assert_true (Printf.sprintf "Origin %s parses correctly" str) (o = expected);
        assert_true (Printf.sprintf "Origin %s formats back" str) (string_of_origin o = str)
    | Error _ -> assert_true "Failed origin parse" false
  ) origins;

  let bindings = [
    ("EXAMPLE", Example);
    ("PREFERENCE", Preference);
    ("CONSTRAINT", Constraint);
    ("AUTHORIZED_ACTION", Authorized_action);
  ] in
  List.iter (fun (str, expected) ->
    match binding_of_string str with
    | Ok b ->
        assert_true (Printf.sprintf "Binding %s parses correctly" str) (b = expected);
        assert_true (Printf.sprintf "Binding %s formats back" str) (string_of_binding b = str)
    | Error _ -> assert_true "Failed binding parse" false
  ) bindings

let test_valid_contract () =
  let contract = make
    ~id:"intent-001"
    ~revision:1
    ~mode:Explore
    ~objective:{ value = "organize files in directory"; origin = User_explicit; binding = Constraint; evidence = [] }
    ~targets:[{ value = DirectoryTarget "/home/user/downloads"; origin = User_explicit; binding = Constraint; evidence = [] }]
    ~constraints:[]
    ~ambiguities:[]
    ~verification_criteria:["file_count_preserved"; "no_unauthorized_deletion"]
    ~authority_scope:["scope:downloads"]
    () in
  match validate contract with
  | Ok () -> assert_true "Valid exploration contract passes validation" true
  | Error _ -> assert_true "Valid contract failed validation" false

let test_material_ambiguity_blocks_execution () =
  let contract = make
    ~id:"intent-002"
    ~revision:1
    ~mode:Execute (* Consequential mode *)
    ~objective:{ value = "delete unreferenced files"; origin = User_explicit; binding = Authorized_action; evidence = [] }
    ~targets:[{ value = DirectoryTarget "/tmp"; origin = User_explicit; binding = Constraint; evidence = [] }]
    ~constraints:[]
    ~ambiguities:[{ field_path = "targets.retention"; alternatives = ["archive"; "permanent_delete"]; consequence = "Data loss risk" }]
    ~verification_criteria:[]
    ~authority_scope:[]
    () in
  match validate contract with
  | Error (MaterialAmbiguityUnresolved _) ->
      assert_true "Material ambiguity blocks consequential execution via I-01" true
  | _ -> assert_true "Material ambiguity was erroneously allowed in Execute mode" false

let test_consequential_requires_targets () =
  let contract = make
    ~id:"intent-003"
    ~revision:1
    ~mode:Execute
    ~objective:{ value = "format disk"; origin = User_explicit; binding = Authorized_action; evidence = [] }
    ~targets:[] (* Missing targets *)
    ~constraints:[]
    ~ambiguities:[]
    ~verification_criteria:[]
    ~authority_scope:[]
    () in
  match validate contract with
  | Error ConsequentialModeWithNoTargets ->
      assert_true "Consequential mode with empty targets correctly rejected" true
  | _ -> assert_true "Consequential mode with empty targets was erroneously allowed" false

let test_tampered_contract_hash () =
  let contract = make
    ~id:"intent-004"
    ~revision:1
    ~mode:Explore
    ~objective:{ value = "clean cache"; origin = User_explicit; binding = Constraint; evidence = [] }
    ~targets:[]
    ~constraints:[]
    ~ambiguities:[]
    ~verification_criteria:[]
    ~authority_scope:[]
    () in
  let tampered = { contract with content_hash = "00000000000000000000000000000000" } in
  match validate tampered with
  | Error (InvalidHash _) ->
      assert_true "Tampered contract hash correctly detected and rejected" true
  | _ -> assert_true "Tampered contract was erroneously accepted" false

let () =
  Printf.printf "=== Running Typed Intent Contract Test Suite (I002) ===\n";
  test_origins_and_bindings ();
  test_valid_contract ();
  test_material_ambiguity_blocks_execution ();
  test_consequential_requires_targets ();
  test_tampered_contract_hash ();
  Printf.printf "=== ALL TYPED INTENT CONTRACT TESTS PASSED ===\n"
