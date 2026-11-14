(* tests/test_resolver.ml - Test suite for Task I006: Contextual reference resolver *)

open Inos_ir.Contract
open Inos_ir.Resolver

let assert_true msg cond =
  if not cond then (
    Printf.eprintf "[FAIL] %s\n" msg;
    exit 1
  ) else
    Printf.printf "[PASS] %s\n" msg

let test_unambiguous_file_selection () =
  let ctx = {
    empty_context with
    selected_files = ["/workspace/src/main.ml"];
  } in
  match resolve_reference ctx "this file" with
  | Resolved (FileTarget path) ->
      assert_true "'this file' resolves to unique selected file" (path = "/workspace/src/main.ml")
  | _ -> assert_true "Expected Resolved (FileTarget ...)" false

let test_zero_file_selection_unresolved () =
  let ctx = {
    empty_context with
    selected_files = [];
  } in
  match resolve_reference ctx "this file" with
  | Unresolved { phrase; _ } ->
      assert_true "'this file' with no selection yields Unresolved" (phrase = "this file")
  | _ -> assert_true "Expected Unresolved when no file selected" false

let test_multiple_file_selection_ambiguous () =
  let ctx = {
    empty_context with
    selected_files = ["/workspace/src/a.ml"; "/workspace/src/b.ml"];
  } in
  match resolve_reference ctx "this file" with
  | Ambiguous { candidates; _ } ->
      assert_true "'this file' with multiple selections yields Ambiguous" (List.length candidates = 2)
  | _ -> assert_true "Expected Ambiguous when multiple files selected" false

let test_active_project_resolution () =
  let ctx_none = empty_context in
  (match resolve_reference ctx_none "active project" with
   | Unresolved _ -> assert_true "Missing active project yields Unresolved" true
   | _ -> assert_true "Expected Unresolved for missing active project" false);

  let ctx_some = { empty_context with active_project = Some "/workspace/my_project" } in
  match resolve_reference ctx_some "active project" with
  | Resolved (ProjectTarget p) ->
      assert_true "Active project resolves to open project" (p = "/workspace/my_project")
  | _ -> assert_true "Expected Resolved (ProjectTarget ...)" false

let test_explicit_uri_resolution () =
  let ctx = empty_context in
  match resolve_reference ctx "file:/var/log/syslog" with
  | Resolved (FileTarget p) ->
      assert_true "Explicit file URI resolves directly" (p = "/var/log/syslog")
  | _ -> assert_true "Expected Resolved for explicit URI" false

let () =
  Printf.printf "=== Running Contextual Reference Resolver Test Suite (I006) ===\n";
  test_unambiguous_file_selection ();
  test_zero_file_selection_unresolved ();
  test_multiple_file_selection_ambiguous ();
  test_active_project_resolution ();
  test_explicit_uri_resolution ();
  Printf.printf "=== ALL RESOLVER TESTS PASSED ===\n"
