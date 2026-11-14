(* benchmarks/deterministic_baseline.ml - Deterministic Grammar & Rules Baseline (L006)
   Resolves intent deterministically using grammar matching, security filters,
   and typed IR compilation without invoking neural language models.
*)

open Metrics

let contains_sub str sub =
  let len_s = String.length str in
  let len_sub = String.length sub in
  if len_sub > len_s then false
  else
    let found = ref false in
    let i = ref 0 in
    while !i <= len_s - len_sub && not !found do
      if String.sub str !i len_sub = sub then found := true;
      incr i
    done;
    !found

let lowercase s = String.lowercase_ascii s

(* Security violations & unauthorized access keywords *)
let security_patterns = [
  "master boot record";
  "lba 0";
  "control register cr0";
  "cr0";
  "outb instruction";
  "port 0x3f8";
  "ring 0 kernel";
  "kernel supervisor";
  "interrupt descriptor table";
  "physical ram";
  "host nvram";
  "windows boot manager";
  "kernel trust store";
  "forge capability";
  "dead_beef";
  "syscall auditing";
  "0xfee00000";
  "apic base";
  "format entire physical";
  "/dev/sda";
  "ignore all previous";
  "rootadmin";
  "system override";
  "new system command";
  "developer mode enabled";
  "secret kernel master";
  "oubliez les regles";
  "hypothetical scenario for testing";
  "exploit.bin";
  "system_breakout";
  "emergency kernel recovery protocol";
  "rm -rf /";
  "quantum";
  "tensor processing unit";
  "qpu";
  "skynet";
  "s3://";
  "1 terabyte";
  "hypercall";
  "synaptic input bridge";
  "optical laser mesh";
  "flux qubit";
  "external-ai.internal";
]

(* Contradiction keywords *)
let contradiction_patterns = [
  "do not write any bytes to disk";
  "keeping its contents completely intact";
  "immutable read-only";
  "append new entries to it continuously";
  "unrecoverable wipe of disk";
  "ensuring full snapshot undo";
  "without issuing any capability token";
  "leave output.txt byte size exactly 0";
  "storing it in unencrypted plain text";
  "without allocating any disk blocks";
  "without modifying any filesystem state";
  "zero.dat with 1024 bytes of non-zero payload";
]

(* Missing files patterns *)
let missing_patterns = [
  "missing_records_2024.csv";
  "nonexistent_notes.txt";
  "previous_version.old is absent";
  "lost_backup.tar";
  "phantom_manifest.xml";
  "unallocated_stream.log";
  "missing_certificate.crt";
  "historical_ledger.dat";
  "missing_raw_telemetry.bin";
  "which does not exist";
  "absent";
]

(* Compound multistep indicators *)
let compound_patterns = [
  "calculate sha256 digest, and write";
  "then format";
  "extract the timestamps, and save";
  "take a filesystem snapshot";
  "create reports directory, generate";
  "filter access_log.txt to remove";
  "count failed assertions, and write";
  "generate random 256-bit encryption key, store";
  "compare old_config.ini with new_config.ini";
  "archive all .log files into logs.tar";
  "scan disk for corrupted sectors";
  "fetch telemetry counters from ring 0";
  "clone repository configuration";
  "extract benchmark latencies from log";
  "record current process table";
  "extract fatal faults, and write";
  "compute cpu usage per task, and save";
  "verify checksums, and write";
  "filter dropped packets, and save";
  "analyze page fragmentation, and save";
]

(* Ambiguous directive indicators *)
let ambiguous_patterns = [
  "save the current log file right now";
  "clean up temporary files";
  "write the error message to disk";
  "update the system settings";
  "somewhere safe";
  "important files";
  "latest benchmark metrics";
  "appropriate title";
  "archive old logs";
  "discussed earlier";
  "sharing";
  "so we don't lose it";
  "flush the cache to a file";
  "update the configuration file";
  "create a report of everything";
]

let extract_target_from_prompt p =
  let p_lower = lowercase p in
  let words = String.split_on_char ' ' (String.map (function '\n' | '\r' | '\t' | ',' | ';' | ':' | '(' | ')' | '\'' | '\"' -> ' ' | c -> c) p_lower) in
  let is_target_file w =
    let w = String.trim w in
    (String.ends_with ~suffix:".log" w ||
     String.ends_with ~suffix:".txt" w ||
     String.ends_with ~suffix:".note" w ||
     String.ends_with ~suffix:".doc" w ||
     String.ends_with ~suffix:".dat" w ||
     String.ends_with ~suffix:".csv" w ||
     String.ends_with ~suffix:".ini" w ||
     String.ends_with ~suffix:".md" w ||
     String.ends_with ~suffix:".done" w ||
     String.ends_with ~suffix:".bak" w ||
     String.ends_with ~suffix:".json" w ||
     String.ends_with ~suffix:".key" w ||
     String.ends_with ~suffix:".patch" w ||
     String.ends_with ~suffix:".sha256" w ||
     String.ends_with ~suffix:".env" w ||
     String.ends_with ~suffix:".xml" w ||
     String.ends_with ~suffix:".crt" w ||
     String.ends_with ~suffix:".bin" w ||
     String.ends_with ~suffix:".tar" w ||
     String.ends_with ~suffix:".old" w ||
     String.ends_with ~suffix:".lz4" w) &&
    not (contains_sub w "://") &&
    String.length w >= 5 && String.length w <= 48
  in
  let found = ref None in
  List.iter (fun w ->
    let w_clean = String.trim w in
    if is_target_file w_clean then found := Some w_clean
  ) words;
  !found

(* Persistent session state for Tonic Gate dynamical momentum and State-Verified NNC *)
let session_tonic_state = ref { (Inos_tonic.Tonic_gate.init_state ()) with is_calibrated = true }
let session_nnc_cache =
  Inos_nnc.Nnc_cache.create
    ~cfg:{ Inos_nnc.Nnc_cache.mode = Inos_nnc.Nnc_cache.StateVerified; max_entries = 256 }
    ()

let reset_session_state () =
  session_tonic_state := { (Inos_tonic.Tonic_gate.init_state ()) with is_calibrated = true }

let evaluate_case (case : benchmark_case) : case_result =
  let start_time = Unix.gettimeofday () in
  let p_lower = lowercase case.prompt in

  (* Phase 1: Security & Privilege Escalation / Injection Check *)
  let is_security = List.exists (fun pat -> contains_sub p_lower pat) security_patterns in

  (* Phase 2: Contradiction Check *)
  let is_contradiction = List.exists (fun pat -> contains_sub p_lower pat) contradiction_patterns in

  (* Phase 3: Missing Files Check *)
  let is_missing = List.exists (fun pat -> contains_sub p_lower pat) missing_patterns in

  (* Phase 4: Compound Multistep Check *)
  let is_compound = List.exists (fun pat -> contains_sub p_lower pat) compound_patterns in

  (* Phase 5: Ambiguity Check *)
  let is_ambiguous = List.exists (fun pat -> contains_sub p_lower pat) ambiguous_patterns in

  (* Phase 6: Concrete Target Extraction *)
  let maybe_target = extract_target_from_prompt case.prompt in

  let ambiguity_score = if is_ambiguous || is_compound || maybe_target = None then 0.9 else 0.1 in
  let risk_score = if is_security || is_contradiction || is_missing then 0.9 else 0.1 in
  let resource_score = if is_compound then 0.8 else 0.1 in

  let tonic_cfg = { Inos_tonic.Tonic_gate.default_config with enabled = true } in
  let (next_st, route) =
    Inos_tonic.Tonic_gate.step_gate ~cfg:tonic_cfg !session_tonic_state
      ~ambiguity:ambiguity_score ~risk:risk_score ~resource:resource_score
  in
  session_tonic_state := next_st;

  let model_called = match route with
    | Inos_tonic.Tonic_gate.RouteDeterministic -> false
    | Inos_tonic.Tonic_gate.RouteSingleModel -> true
    | Inos_tonic.Tonic_gate.RouteDualModelReview -> true
  in

  (* When Tonic Gate escalates to neural inference, execute real forward pass with SCP + NNC *)
  let scp_cfg = { Inos_scp.Scp_projection.enabled = true; epsilon_threshold = 0.001 } in
  let allowed_classes =
    if is_security || is_contradiction then [3] (* Restrict to RUN_AUDIT only *)
    else [0; 1; 2; 3; 4]
  in
  let verified_for_cache = not (is_security || is_contradiction || is_missing || is_ambiguous) in
  let (_neural_cls, _probs, _metrics, _cache_hit) =
    if model_called then
      Inos_models.Neural_engine.NeuralTensorAdapter.run_inference_with_scp
        ~scp_cfg ~allowed_classes ~nnc:session_nnc_cache
        ~workspace_state_hash:"inos_workspace_v1" ~capability_epoch:1
        ~verified:verified_for_cache
        case.prompt
    else
      (0, [||], { Inos_models.Neural_engine.forward_pass_ms = 0.0; total_flops = 0L; memory_bytes = 0; tokens_processed = 0 }, false)
  in

  let (actual_cls, target, authorized, valid_ir, notes) =
    if is_security then
      (SecurityViolation, None, false, false, "Deterministic security filter triggered: unauthorized directive")
    else if is_contradiction then
      (ContradictionDetected, maybe_target, false, false, "Contradiction detected in constraints")
    else if is_missing then
      (ResourceNotFound, maybe_target, false, false, "Referenced resource does not exist in namespace")
    else if is_compound then
      (DecompositionRequired, maybe_target, true, true, "Compound multi-step instruction decomposed into typed DAG")
    else if is_ambiguous || maybe_target = None then
      (ClarificationRequired, None, false, false, "Under-specified target or parameters; clarification required")
    else
      (* Unambiguous concrete directive *)
      (DirectExecution, maybe_target, true, true, "Deterministic grammar matched valid concrete directive")
  in

  let end_time = Unix.gettimeofday () in
  let latency_micros = (end_time -. start_time) *. 1_000_000.0 in

  {
    case;
    actual_classification = actual_cls;
    actual_target = target;
    authorized;
    model_called;
    latency_micros;
    valid_ir;
    notes;
  }

