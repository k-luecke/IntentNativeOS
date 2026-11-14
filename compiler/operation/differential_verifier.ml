(* compiler/operation/differential_verifier.ml - Differential Verifier for Planned vs Actual Output
   Enforces invariant: Technically valid output (e.g. exit code 0, syntactically well-formed)
   that departs from planned semantic intent or contract invariants is flagged as IntentMismatch.
*)

type expectation =
  | ExactOutput of string
  | ExpectedDigest of string
  | RequiredSubstring of string
  | ForbiddenSubstring of string
  | UniqueLines
  | SortedLines of [ `Ascending | `Descending ]
  | LineCount of int
  | CustomInvariant of string * (string -> bool)

type technical_check = {
  exit_code: int;
  stderr: string;
  is_well_formed: bool;
}

type differential_result =
  | Conformant of { receipt_hash: string; checked_invariants: int }
  | IntentMismatch of { technical_validity: bool; violations: string list; diagnostic: string }
  | TechnicalFailure of { exit_code: int; reason: string }

let sha256_hex str =
  Inos_ipc.Sha256.digest_string str

let check_expectation (exp : expectation) (output : string) : (unit, string) result =
  match exp with
  | ExactOutput expected ->
      if String.equal expected output then Ok ()
      else Error (Printf.sprintf "Output mismatch: expected '%s', got '%s'" expected output)

  | ExpectedDigest expected_hash ->
      let actual_hash = sha256_hex output in
      if String.equal expected_hash actual_hash then Ok ()
      else Error (Printf.sprintf "Digest mismatch: expected sha256 %s, got %s" expected_hash actual_hash)

  | RequiredSubstring sub ->
      let len_sub = String.length sub in
      let len_out = String.length output in
      let rec search i =
        if i + len_sub > len_out then false
        else if String.sub output i len_sub = sub then true
        else search (i + 1)
      in
      if search 0 then Ok ()
      else Error (Printf.sprintf "Missing required intent substring: '%s'" sub)

  | ForbiddenSubstring sub ->
      let len_sub = String.length sub in
      let len_out = String.length output in
      let rec search i =
        if i + len_sub > len_out then false
        else if String.sub output i len_sub = sub then true
        else search (i + 1)
      in
      if not (search 0) then Ok ()
      else Error (Printf.sprintf "Output contains forbidden content: '%s'" sub)

  | UniqueLines ->
      let lines = String.split_on_char '\n' output in
      let seen = Hashtbl.create 16 in
      let rec check = function
        | [] -> Ok ()
        | l :: rest ->
            if Hashtbl.mem seen l then
              Error (Printf.sprintf "Intent violation: duplicate line detected: '%s'" l)
            else begin
              Hashtbl.add seen l ();
              check rest
            end
      in
      check lines

  | SortedLines order ->
      let lines = String.split_on_char '\n' output in
      let rec check = function
        | [] | [_] -> Ok ()
        | a :: (b :: _ as rest) ->
            let cmp = String.compare a b in
            let valid =
              match order with
              | `Ascending -> cmp <= 0
              | `Descending -> cmp >= 0
            in
            if valid then check rest
            else Error (Printf.sprintf "Intent violation: unsorted lines '%s' and '%s'" a b)
      in
      check lines

  | LineCount expected_count ->
      let lines = String.split_on_char '\n' output in
      let count = List.length lines in
      if count = expected_count then Ok ()
      else Error (Printf.sprintf "Intent violation: expected %d lines, got %d" expected_count count)

  | CustomInvariant (name, predicate) ->
      if predicate output then Ok ()
      else Error (Printf.sprintf "Custom intent invariant '%s' failed" name)

let verify
    ~(planned_expectations : expectation list)
    ~(technical_status : technical_check)
    ~(actual_output : string)
  : differential_result =
  (* 1. Check technical validity first *)
  if technical_status.exit_code <> 0 then
    TechnicalFailure {
      exit_code = technical_status.exit_code;
      reason = Printf.sprintf "Process failed with exit code %d: %s"
                 technical_status.exit_code technical_status.stderr;
    }
  else if not technical_status.is_well_formed then
    TechnicalFailure {
      exit_code = 0;
      reason = "Output is malformed or corrupted at transport layer";
    }
  else
    (* 2. Technical execution succeeded cleanly! Now check intent conformance *)
    let violations = ref [] in
    let checked_count = ref 0 in
    List.iter (fun exp ->
      incr checked_count;
      match check_expectation exp actual_output with
      | Ok () -> ()
      | Error msg -> violations := msg :: !violations
    ) planned_expectations;

    if !violations = [] then
      let receipt_hash = sha256_hex (Printf.sprintf "%s:%d" (sha256_hex actual_output) !checked_count) in
      Conformant { receipt_hash; checked_invariants = !checked_count }
    else
      let violation_list = List.rev !violations in
      let diag = String.concat "; " violation_list in
      IntentMismatch {
        technical_validity = true;
        violations = violation_list;
        diagnostic = Printf.sprintf "Execution succeeded (exit 0) but output departed from intent: %s" diag;
      }
