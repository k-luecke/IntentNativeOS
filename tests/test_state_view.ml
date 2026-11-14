(* tests/test_state_view.ml - Test suite for Task M002: Materialized current-state view and deterministic replay *)

open Inos_memory.Journal
open Inos_memory.State_view

let assert_true msg cond =
  if not cond then (
    Printf.eprintf "[FAIL] %s\n" msg;
    exit 1
  ) else
    Printf.printf "[PASS] %s\n" msg

let test_replay_produces_identical_state_hash_twice () =
  let path = Filename.temp_file "inos_state_replay_" ".ndjson" in
  let j = match open_journal path with
    | Ok j -> j
    | Error _ -> failwith "Failed to open journal"
  in
  let _ = append_event j ~event_id:"intent-101" ~event_type:"INTENT_REGISTERED" ~producer:"user" ~payload:"DRAFT" () in
  let _ = append_event j ~event_id:"entity-agent" ~event_type:"CAPABILITY_GRANTED" ~producer:"broker" ~payload:"fs:read:/tmp" () in
  let _ = append_event j ~event_id:"entity-agent" ~event_type:"CAPABILITY_GRANTED" ~producer:"broker" ~payload:"fs:write:/tmp" () in
  let _ = append_event j ~event_id:"fact-alpha" ~event_type:"FACT_ASSERTED" ~producer:"model-a" ~payload:"verified ABI compatibility" () in
  let _ = append_event j ~event_id:"intent-101" ~event_type:"INTENT_TRANSITION" ~producer:"supervisor" ~payload:"READY" () in
  let _ = append_event j ~event_id:"entity-agent" ~event_type:"CAPABILITY_REVOKED" ~producer:"broker" ~payload:"fs:write:/tmp" () in

  (* Replay #1 *)
  let state1 = replay_journal j in
  let hash1 = state1.state_hash in
  assert_true "Replay 1 computed state hash" (String.length hash1 > 0);
  assert_true "Replay 1 tracked 6 events" (state1.event_count = 6);

  (* Replay #2 on a fresh instance *)
  let j2 = match open_journal path with
    | Ok j -> j
    | Error _ -> failwith "Failed to reload journal"
  in
  let state2 = replay_journal j2 in
  let hash2 = state2.state_hash in

  assert_true "Replay produces identical state hash twice" (hash1 = hash2);
  assert_true "Materialized intents match"
    (Hashtbl.find state1.intents "intent-101" = Hashtbl.find state2.intents "intent-101");
  assert_true "Capability revocation accurately reflected"
    (Hashtbl.find state1.capabilities "entity-agent" = ["fs:read:/tmp"]);

  Sys.remove path

let () =
  Printf.printf "=== Running Materialized State View Replay Suite (M002) ===\n";
  test_replay_produces_identical_state_hash_twice ();
  Printf.printf "=== ALL STATE VIEW TESTS PASSED ===\n"
