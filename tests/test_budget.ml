(* tests/test_budget.ml - Test suite for Task M005: Maximum context budgets and retrieval receipts *)

open Inos_memory.Budget

let assert_true msg cond =
  if not cond then (
    Printf.eprintf "[FAIL] %s\n" msg;
    exit 1
  ) else
    Printf.printf "[PASS] %s\n" msg

let test_truncation_omission_and_binding_preservation () =
  let items = [
    { id = "binding-1"; content = "Invariant I-01"; byte_size = 100; is_binding_constraint = true; priority = 10 };
    { id = "binding-2"; content = "Invariant I-02"; byte_size = 150; is_binding_constraint = true; priority = 10 };
    { id = "opt-high"; content = "Core interface AST"; byte_size = 100; is_binding_constraint = false; priority = 5 };
    { id = "opt-low"; content = "Historic logs"; byte_size = 200; is_binding_constraint = false; priority = 1 };
  ] in

  (* Total items = 100 + 150 + 100 + 200 = 550 bytes. Budget = 400 bytes. *)
  (* Binding = 250 bytes. Remaining for optional = 150 bytes.
     opt-high (100) fits -> remaining = 50.
     opt-low (200) cannot fit -> omitted! *)
  match enforce_budget ~receipt_id:"rcpt-001" ~max_budget:400 items with
  | Error _ -> assert_true "Budget enforcement unexpectedly failed" false
  | Ok receipt ->
      assert_true "Receipt used bytes within budget" (receipt.used_bytes <= 400);
      assert_true "Both binding constraints preserved" (receipt.binding_constraints_preserved = 2);
      assert_true "Binding 1 retained" (List.exists (fun i -> i.id = "binding-1") receipt.retained_items);
      assert_true "Binding 2 retained" (List.exists (fun i -> i.id = "binding-2") receipt.retained_items);
      assert_true "Higher priority optional retained" (List.exists (fun i -> i.id = "opt-high") receipt.retained_items);
      assert_true "Lower priority optional omitted" (not (List.exists (fun i -> i.id = "opt-low") receipt.retained_items));
      assert_true "Explicit omission record generated" (List.length receipt.omissions = 1);
      let omission = List.hd receipt.omissions in
      assert_true "Omission record identifies item" (omission.item_id = "opt-low");
      assert_true "Omission records byte size" (omission.omitted_bytes = 200);
      assert_true "Omission records descriptive reason" (String.length omission.reason > 0);
      assert_true "Receipt hash generated" (String.length receipt.receipt_hash > 0)

let test_binding_constraint_exceeds_budget_fails_closed () =
  let items = [
    { id = "binding-large"; content = "Large constraint"; byte_size = 500; is_binding_constraint = true; priority = 10 };
  ] in
  (* Budget is 300, but binding constraint is 500. MUST FAIL CLOSED, never drop constraint *)
  match enforce_budget ~receipt_id:"rcpt-002" ~max_budget:300 items with
  | Error (BindingConstraintExceedsBudget { constraint_id; required_size; max_budget }) ->
      assert_true "Binding constraint exceeding budget fails closed"
        (constraint_id = "binding-large" && required_size = 500 && max_budget = 300)
  | Ok _ ->
      assert_true "Binding constraint was dropped or budget violated!" false

let () =
  Printf.printf "=== Running Context Budget & Retrieval Receipts Suite (M005) ===\n";
  test_truncation_omission_and_binding_preservation ();
  test_binding_constraint_exceeds_budget_fails_closed ();
  Printf.printf "=== ALL BUDGET TESTS PASSED ===\n"
