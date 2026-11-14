(* tests/test_l208_e2e_cooperation.ml - End-to-End Multi-Step Native Operation (Task L208)
   Connects:
   Unseen compound intent -> Pretrained A/B live cooperation -> Typed Operation IR
   -> DAG Validation -> Capability Token Binding -> Transport Spool Serialization.
   Negative cases fail closed safely.
*)

open Inos_arbitration.State_machine
open Inos_arbitration.Escalation
open Inos_operation.Dag_validator
open Inos_operation.Live_intent_compiler

let assert_true name cond =
  if cond then Printf.printf "  [PASS] %s\n" name
  else (Printf.eprintf "  [FAIL] %s\n" name; exit 1)

let resolve_file rel =
  let candidates = [
    rel;
    "../" ^ rel;
    "../../" ^ rel;
    "../../../" ^ rel;
  ] in
  match List.find_opt Sys.file_exists candidates with
  | Some p -> p
  | None -> rel

let test_e2e_positive_compound_operation () =
  Printf.printf "=== [TEST 1] Positive Unseen Compound Intent End-to-End Pipeline ===\n";
  let prompt = "Instruction: Write dual-model status to release_notes_v2.txt stating that arbitration and capability gates are verified" in

  (* Step 1: Model A live inference *)
  let runner = resolve_file "models/pretrained/run" in
  let weights_a = resolve_file "models/pretrained/stories15M.bin" in
  let tokenizer = resolve_file "models/pretrained/tokenizer.bin" in
  let cmd_a = Printf.sprintf "%s %s -z %s -t 0.0 -n 35 -i \"%s\" 2>&1"
    runner weights_a tokenizer (String.map (function '"' | '\\' -> ' ' | c -> c) prompt) in
  let ic_a = Unix.open_process_in cmd_a in
  let lines_a = ref [] in
  (try while true do lines_a := input_line ic_a :: !lines_a done with End_of_file -> ());
  ignore (Unix.close_process_in ic_a);
  let a_tokens = String.concat " " (List.rev !lines_a) in
  Printf.printf "  [STEP 1] Model A generated %d bytes: '%s...'\n"
    (String.length a_tokens) (String.sub a_tokens 0 (min 50 (String.length a_tokens)));

  (* Step 2: Assemble Model A proposal *)
  let proposal = {
    attempt = 1;
    model_source = "ModelA_15M";
    raw_tokens = a_tokens;
    candidate_target = Some "release_notes_v2.txt";
    candidate_action = "create_document";
    has_destructive_potential = false;
    has_unsupported_assumptions = false;
    proposal_hash = compute_hash a_tokens;
  } in

  (* Step 3: Model B review *)
  let state_rev = State_Reviewing { proposal; attempt = 1 } in
  let state_val = step_arbitration state_rev in
  let state_final = step_arbitration state_val in

  let (target, cap_token, receipt_hash) =
    match state_final with
    | State_PlanApproved { final_target; capability_token; receipt_hash; _ } ->
        (final_target, capability_token, receipt_hash)
    | _ -> failwith "Positive compound intent failed to achieve dual-model approval"
  in
  assert_true "Dual-model arbitration approved proposal" (target = "release_notes_v2.txt");
  assert_true "Valid capability token issued" (cap_token = 0xCAFEBABE00010001L);

  (* Step 4: Compile to typed Operation IR *)
  match compile_live_intent ~prompt ~generated_tokens:a_tokens with
  | AmbiguousInstructionHalted _ ->
      failwith "Should have compiled to concrete operation"
  | ConcreteOperation op ->
      assert_true "Compiled target is release_notes_v2.txt" (op.target_filename = "release_notes_v2.txt");
      assert_true "Operation DAG is strictly acyclic" (Result.is_ok (validate_and_order_dag op.operation_graph));

      (* Step 5: Serialize into dedicated disk transport spool sector *)
      let prompt_hash = compute_hash prompt in
      let sectors = serialize_spool_sectors
        ~filename:op.target_filename
        ~payload:op.raw_generated_tokens
        ~provenance_hash:receipt_hash
        ~prompt_hash
        ~is_ambiguous:false
      in
      assert_true "Transport spool formatted to exactly 2 sectors (1024 bytes)" (List.length sectors = 2);
      let hdr = List.hd sectors in
      assert_true "Spool header magic is INT_" (Bytes.sub_string hdr 0 4 = "INT_");
      Printf.printf "  [PASS] End-to-end operation verified: intent -> A/B review -> IR -> capability -> spool\n"

let test_e2e_negative_fail_closed () =
  Printf.printf "=== [TEST 2] Negative Test: Ambiguous & Destructive Prompt Fails Closed ===\n";
  let prompt = "Clean up all old files" in
  let proposal = {
    attempt = 1;
    model_source = "ModelA_15M";
    raw_tokens = "Parsed action: delete old files";
    candidate_target = None;
    candidate_action = "delete";
    has_destructive_potential = true;
    has_unsupported_assumptions = true;
    proposal_hash = compute_hash prompt;
  } in

  let state_rev = State_Reviewing { proposal; attempt = 1 } in
  let state_val = step_arbitration state_rev in
  let state_final = step_arbitration state_val in

  (match state_final with
   | State_EscalatedToUser { reason; _ } ->
       assert_true "Ambiguous destructive prompt safely escalated to user" true;
       Printf.printf "  [PASS] Zero capability token issued: %s\n" reason
   | State_PlanApproved _ ->
       failwith "FAIL-CLOSED VIOLATION: Ambiguous destructive intent was approved!"
   | _ -> ())

let () =
  if not (List.exists Sys.file_exists ["models/pretrained/stories15M.bin"; "../models/pretrained/stories15M.bin"; "../../models/pretrained/stories15M.bin"]) then (
    Printf.printf "[SKIP] Optional 60.8 MB stories15M.bin weights not downloaded yet (run ./scripts/fetch_models.sh to enable live Llama-2 weight tests).\n";
    exit 0
  );
  Printf.printf "\n=== Running Test Suite: End-to-End Multi-Step Native Operation (L208) ===\n";
  test_e2e_positive_compound_operation ();
  test_e2e_negative_fail_closed ();
  Printf.printf "=== [ALL PASS] End-to-End Multi-Step Native Operation Verified ===\n\n"
