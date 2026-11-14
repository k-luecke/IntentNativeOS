(* tests/test_context.ml - Test suite for Task M004: Task-scoped model context projection and secret filtering *)

open Inos_memory.Context

let assert_true msg cond =
  if not cond then (
    Printf.eprintf "[FAIL] %s\n" msg;
    exit 1
  ) else
    Printf.printf "[PASS] %s\n" msg

let test_projections_include_references_and_exclude_secrets () =
  let items = [
    { item_id = "req-1"; scope = "kernel:ipc"; content = "IPC wire spec"; is_secret = false; tags = [] };
    { item_id = "req-2"; scope = "kernel:ipc"; content = "Envelope definition"; is_secret = false; tags = [] };
    { item_id = "glob-1"; scope = "general"; content = "Architecture invariants"; is_secret = false; tags = ["GLOBAL"] };
    { item_id = "secret-key"; scope = "kernel:ipc"; content = "TOP_SECRET_SIGNING_KEY"; is_secret = true; tags = ["SECRET"] };
    { item_id = "test-secret"; scope = "test"; content = "TEST_RUNNER_AUTH_TOKEN"; is_secret = false; tags = ["TEST_SECRET"] };
    { item_id = "unrelated"; scope = "ui:display"; content = "Framebuffer config"; is_secret = false; tags = [] };
  ] in

  (* Model A projection *)
  let proj_a = project_context
    ~task_id:"task-ipc-arch"
    ~required_scopes:["kernel:ipc"]
    ~target_role:Architect
    items
  in

  (* Model B projection *)
  let proj_b = project_context
    ~task_id:"task-ipc-impl"
    ~required_scopes:["kernel:ipc"]
    ~target_role:Implementer
    items
  in

  (* Checks for Model A *)
  assert_true "Model A includes req-1" (List.exists (fun i -> i.item_id = "req-1") proj_a.included_items);
  assert_true "Model A includes req-2" (List.exists (fun i -> i.item_id = "req-2") proj_a.included_items);
  assert_true "Model A includes global invariants" (List.exists (fun i -> i.item_id = "glob-1") proj_a.included_items);
  assert_true "Model A excludes secret key" (not (List.exists (fun i -> i.item_id = "secret-key") proj_a.included_items));
  assert_true "Model A excludes test secret" (not (List.exists (fun i -> i.item_id = "test-secret") proj_a.included_items));
  assert_true "Model A excludes unrelated scope" (not (List.exists (fun i -> i.item_id = "unrelated") proj_a.included_items));

  (* Checks for Model B *)
  assert_true "Model B includes req-1" (List.exists (fun i -> i.item_id = "req-1") proj_b.included_items);
  assert_true "Model B includes req-2" (List.exists (fun i -> i.item_id = "req-2") proj_b.included_items);
  assert_true "Model B excludes secret key" (not (List.exists (fun i -> i.item_id = "secret-key") proj_b.included_items));
  assert_true "Model B excludes test secret" (not (List.exists (fun i -> i.item_id = "test-secret") proj_b.included_items));

  assert_true "A and B projections contain required references and exclude unrelated test secrets"
    (List.length proj_a.included_items = 3 && List.length proj_b.included_items = 3)

let () =
  Printf.printf "=== Running Task-Scoped Context Projection Suite (M004) ===\n";
  test_projections_include_references_and_exclude_secrets ();
  Printf.printf "=== ALL CONTEXT PROJECTION TESTS PASSED ===\n"
