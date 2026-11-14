(* tests/test_journal.ml - Test suite for Task M001: Append-only event journal and corruption detection *)

open Inos_memory.Journal

let assert_true msg cond =
  if not cond then (
    Printf.eprintf "[FAIL] %s\n" msg;
    exit 1
  ) else
    Printf.printf "[PASS] %s\n" msg

let test_journal_roundtrip_and_replay () =
  let path = Filename.temp_file "inos_journal_test_" ".ndjson" in
  let j = match open_journal path with
    | Ok j -> j
    | Error _ -> failwith "Failed to open clean journal"
  in
  let _ = append_event j ~event_id:"ev-1" ~event_type:"INTENT_CREATED" ~producer:"user" ~payload:"build kernel" () in
  let _ = append_event j ~event_id:"ev-2" ~event_type:"CAPABILITY_GRANTED" ~producer:"broker" ~payload:"cap:net" () in
  let _ = append_event j ~event_id:"ev-3" ~event_type:"STATUS_UPDATED" ~producer:"supervisor" ~payload:"DONE" () in

  (* Re-open from disk (restart reconstruction) *)
  match open_journal path with
  | Error _ -> assert_true "Restart reconstruction failed" false
  | Ok reloaded ->
      assert_true "Restart reconstructed all 3 entries" (List.length reloaded.entries = 3);
      assert_true "Monotonic sequence preserved" (reloaded.last_seq = 3);
      assert_true "Hash chain continuous" (reloaded.last_digest = j.last_digest);
      Sys.remove path

let test_tampered_entry_detected () =
  let path = Filename.temp_file "inos_journal_tamper_" ".ndjson" in
  let j = match open_journal path with
    | Ok j -> j
    | Error _ -> failwith "Failed to open clean journal"
  in
  let _ = append_event j ~event_id:"ev-1" ~event_type:"INTENT_CREATED" ~producer:"user" ~payload:"legitimate payload" () in
  let _ = append_event j ~event_id:"ev-2" ~event_type:"INTENT_CREATED" ~producer:"user" ~payload:"second payload" () in

  (* Tamper with the file contents: modify payload of ev-1 without recalculating digest *)
  let content =
    let ic = open_in path in
    let lines = ref [] in
    (try while true do lines := input_line ic :: !lines done with End_of_file -> ());
    close_in ic;
    List.rev !lines
  in
  let tampered_lines = List.mapi (fun idx line ->
    if idx = 0 then
      (* Replace "legitimate payload" with "malicious tampered payload" *)
      let p_target = "\"payload\":\"legitimate payload\"" in
      let p_sub = "\"payload\":\"malicious tampered payload\"" in
      match find_sub ~pattern:p_target line 0 with
      | Some pos ->
          String.sub line 0 pos ^ p_sub ^ String.sub line (pos + String.length p_target) (String.length line - (pos + String.length p_target))
      | None -> line
    else line
  ) content in
  let oc = open_out path in
  List.iter (fun l -> output_string oc (l ^ "\n")) tampered_lines;
  close_out oc;

  (* Re-opening tampered journal MUST FAIL with ChecksumMismatch *)
  match open_journal path with
  | Error (ChecksumMismatch { seq; _ }) ->
      assert_true "Tampered entry detected via checksum verification" (seq = 1);
      Sys.remove path
  | Error (HashChainBroken _) ->
      assert_true "Tampered entry detected via hash chain break" true;
      Sys.remove path
  | Error _ ->
      assert_true "Tampered entry detected with error" true;
      Sys.remove path
  | Ok _ ->
      Sys.remove path;
      assert_true "Tampered journal was accepted without error!" false

let () =
  Printf.printf "=== Running Event Journal & Tamper Detection Suite (M001) ===\n";
  test_journal_roundtrip_and_replay ();
  test_tampered_entry_detected ();
  Printf.printf "=== ALL JOURNAL TESTS PASSED ===\n"
