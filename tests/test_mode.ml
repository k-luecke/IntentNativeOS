(* tests/test_mode.ml - Tests for Task I001: Mode enum & transition invariants *)

open Inos_ir.Mode

let assert_true msg cond =
  if not cond then (
    Printf.eprintf "[FAIL] %s\n" msg;
    exit 1
  ) else
    Printf.printf "[PASS] %s\n" msg

let test_valid_modes () =
  let modes = [
    ("EXPLORE", Explore);
    ("DESIGN", Design);
    ("SIMULATE", Simulate);
    ("EXECUTE", Execute);
    ("MAINTAIN", Maintain);
  ] in
  List.iter (fun (str, expected) ->
    match of_string str with
    | Ok m ->
        assert_true (Printf.sprintf "Mode %s parses correctly" str) (m = expected);
        assert_true (Printf.sprintf "Mode %s formats back correctly" str) (to_string m = str)
    | Error err ->
        assert_true (Printf.sprintf "Failed to parse %s: %s" str err) false
  ) modes

let test_case_insensitivity () =
  match of_string "explore" with
  | Ok Explore -> assert_true "Lowercase 'explore' parses to Explore" true
  | _ -> assert_true "Lowercase 'explore' failed to parse" false

let test_invalid_modes () =
  let invalid_inputs = [""; "RUN"; "HACK"; "DELETE"; "UNKNOWN"; "execute_now"; "123"] in
  List.iter (fun input ->
    match of_string input with
    | Ok _ ->
        assert_true (Printf.sprintf "Invalid input '%s' should have failed" input) false
    | Error _ ->
        assert_true (Printf.sprintf "Invalid input '%s' correctly rejected" input) true
  ) invalid_inputs

let test_consequential_classification () =
  assert_true "Explore is non-consequential" (not (is_consequential Explore));
  assert_true "Design is non-consequential" (not (is_consequential Design));
  assert_true "Simulate is non-consequential" (not (is_consequential Simulate));
  assert_true "Execute is consequential" (is_consequential Execute);
  assert_true "Maintain is consequential" (is_consequential Maintain)

let test_transition_invariant_i02 () =
  (* Unauthorized escalation must fail closed *)
  (match validate_transition ~from_mode:Explore ~to_mode:Execute ~authorized:false with
   | Error (SilentEscalationForbidden _) ->
       assert_true "Unauthorized Explore -> Execute blocked via I-02" true
   | _ -> assert_true "Unauthorized Explore -> Execute was erroneously allowed" false);

  (match validate_transition ~from_mode:Design ~to_mode:Maintain ~authorized:false with
   | Error (SilentEscalationForbidden _) ->
       assert_true "Unauthorized Design -> Maintain blocked via I-02" true
   | _ -> assert_true "Unauthorized Design -> Maintain was erroneously allowed" false);

  (* Authorized escalation succeeds *)
  (match validate_transition ~from_mode:Explore ~to_mode:Execute ~authorized:true with
   | Ok () ->
       assert_true "Authorized Explore -> Execute allowed" true
   | Error _ -> assert_true "Authorized Explore -> Execute failed" false);

  (* De-escalation always safe *)
  (match validate_transition ~from_mode:Execute ~to_mode:Explore ~authorized:false with
   | Ok () ->
       assert_true "De-escalation Execute -> Explore allowed without grant" true
   | Error _ -> assert_true "De-escalation failed" false)

let () =
  Printf.printf "=== Running Mode Enum & Invariant I-02 Test Suite (I001) ===\n";
  test_valid_modes ();
  test_case_insensitivity ();
  test_invalid_modes ();
  test_consequential_classification ();
  test_transition_invariant_i02 ();
  Printf.printf "=== ALL MODE & INVARIANT I-02 TESTS PASSED ===\n"
