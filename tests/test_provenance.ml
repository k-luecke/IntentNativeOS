(* tests/test_provenance.ml - Test suite for I003: Confidence, Provenance, and Epistemic Invariants *)

open Inos_ir.Provenance

let assert_true msg cond =
  if not cond then (
    Printf.eprintf "[FAIL] %s\n" msg;
    exit 1
  ) else
    Printf.printf "[PASS] %s\n" msg

let test_confidence_bounds () =
  match make_confidence 0.85 with
  | Ok c ->
      assert_true "Confidence 0.85 is within bounds" (c.score = 0.85);
      assert_true "Sample count defaults to None" (c.sample_count = None)
  | Error e ->
      assert_true (Printf.sprintf "Failed to create valid confidence: %s" e) false

let test_confidence_invalid_bounds () =
  let err_high = match make_confidence 1.2 with Error _ -> true | Ok _ -> false in
  assert_true "Confidence > 1.0 is rejected" err_high;
  let err_low = match make_confidence (-0.1) with Error _ -> true | Ok _ -> false in
  assert_true "Confidence < 0.0 is rejected" err_low

let test_hypothesis_cannot_deserialize_as_fact () =
  (* Invariant I-04: Even with 100% confidence, a hypothesis MUST NOT deserialize as approved fact *)
  let result = deserialize_approved_fact
    ~id:"hyp-001"
    ~statement:"Cache hit rate is 99%"
    ~trust_class_str:"PROVISIONAL"
    ~epistemic_status_str:"HYPOTHESIS"
    ~confidence_score:(Some 1.0)
    ~verification_receipt:"receipt-dummy"
    ~producer:"model-a"
    ~source_refs:["run-123"]
    ~created_at:1000.0
    ~parent_refs:[]
  in
  match result with
  | Error (CannotDeserializeHypothesisAsFact msg) ->
      assert_true "Hypothesis cannot deserialize as approved fact (fails closed)" (String.length msg > 0)
  | Error _ -> assert_true "Expected CannotDeserializeHypothesisAsFact error" false
  | Ok _ -> assert_true "Invariant I-04 violation: Hypothesis deserialized as approved fact!" false

let test_provisional_cannot_deserialize_as_fact () =
  (* Fact epistemic status, but PROVISIONAL trust class must still be rejected *)
  let result = deserialize_approved_fact
    ~id:"prov-002"
    ~statement:"Memory footprint is 256MB"
    ~trust_class_str:"PROVISIONAL"
    ~epistemic_status_str:"FACT"
    ~confidence_score:(Some 0.95)
    ~verification_receipt:"receipt-dummy"
    ~producer:"model-b"
    ~source_refs:[]
    ~created_at:1000.0
    ~parent_refs:[]
  in
  match result with
  | Error (ProvisionalStatusRejected msg) ->
      assert_true "Provisional record cannot deserialize as approved fact" (String.length msg > 0)
  | Error _ -> assert_true "Expected ProvisionalStatusRejected error" false
  | Ok _ -> assert_true "Provisional record deserialized as approved fact without verification!" false

let test_missing_verification_receipt_rejected () =
  let result = deserialize_approved_fact
    ~id:"fact-003"
    ~statement:"Kernel booted in 85ms"
    ~trust_class_str:"VERIFIED"
    ~epistemic_status_str:"FACT"
    ~confidence_score:(Some 1.0)
    ~verification_receipt:"" (* empty receipt *)
    ~producer:"qemu-runner"
    ~source_refs:["serial.log"]
    ~created_at:1000.0
    ~parent_refs:[]
  in
  match result with
  | Error MissingVerificationReceipt ->
      assert_true "Verified fact without receipt is rejected" true
  | Error _ -> assert_true "Expected MissingVerificationReceipt error" false
  | Ok _ -> assert_true "Verified fact accepted without verification receipt!" false

let test_authoritative_fact_accepted () =
  let result = deserialize_approved_fact
    ~id:"auth-004"
    ~statement:"Memory safety invariant T0 enforced"
    ~trust_class_str:"AUTHORITATIVE"
    ~epistemic_status_str:"FACT"
    ~confidence_score:None
    ~verification_receipt:""
    ~producer:"t0-kernel-policy"
    ~source_refs:[]
    ~created_at:1000.0
    ~parent_refs:[]
  in
  match result with
  | Ok fact ->
      assert_true "Authoritative fact accepted" (fact.trust_class = Authoritative && fact.id = "auth-004")
  | Error _ -> assert_true "Authoritative fact failed to deserialize" false

let test_promotion_pipeline () =
  let prov = make_provenance
    ~id:"hyp-005"
    ~producer:"model-b"
    ~trust_class:Provisional
    ~epistemic_status:Hypothesis
    ?confidence:(match make_confidence 0.9 with Ok c -> Some c | Error _ -> None)
    ()
  in
  (* Empty receipt must fail *)
  let rejected_no_receipt = match promote_to_verified ~receipt:"" ~verifier:"dune-test-runner" prov "Tests pass" with
    | Error _ -> true
    | Ok _ -> false
  in
  assert_true "Promotion without receipt fails closed" rejected_no_receipt;

  (* Valid promotion *)
  match promote_to_verified ~receipt:"sha256:abc123receipt" ~verifier:"dune-test-runner" prov "Tests pass" with
  | Ok fact ->
      assert_true "Promoted fact has Verified trust class" (fact.trust_class = Verified);
      assert_true "Promoted fact provenance has Verified class" (fact.provenance.trust_class = Verified);
      assert_true "Promoted fact epistemic status is Fact" (fact.provenance.epistemic_status = Fact);
      assert_true "Promoted fact retains receipt" (fact.verification_receipt = "sha256:abc123receipt");
      assert_true "Promoted fact links parent hash" (List.mem prov.content_hash fact.provenance.parent_refs)
  | Error e -> assert_true (Printf.sprintf "Valid promotion failed: %s" e) false

let () =
  Printf.printf "=== Running Confidence & Provenance Test Suite (I003) ===\n";
  test_confidence_bounds ();
  test_confidence_invalid_bounds ();
  test_hypothesis_cannot_deserialize_as_fact ();
  test_provisional_cannot_deserialize_as_fact ();
  test_missing_verification_receipt_rejected ();
  test_authoritative_fact_accepted ();
  test_promotion_pipeline ();
  Printf.printf "=== ALL CONFIDENCE & PROVENANCE TESTS PASSED ===\n"
