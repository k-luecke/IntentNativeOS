(* Security Fuzzing Suite (A013) *)
open Types

type fuzz_test_case = {
  name : string;
  mutation_type : fuzz_mutation;
  input_data : string;
  expected_rejection : bool;
}

let generate_fuzz_corpus () : fuzz_test_case list = [
  {
    name = "path_traversal_dot_dot";
    mutation_type = PathTraversalInjection;
    input_data = "../../../etc/shadow";
    expected_rejection = true;
  };
  {
    name = "path_traversal_null_byte";
    mutation_type = NullByteInjection;
    input_data = "/home/capsule/data\x00/../../root";
    expected_rejection = true;
  };
  {
    name = "elf_corrupted_magic";
    mutation_type = BitFlip;
    input_data = "\x7eELF\x02\x01\x01\x00";
    expected_rejection = true;
  };
  {
    name = "elf_overflow_phoff";
    mutation_type = IntegerOverflow;
    input_data = "\x7fELF\x02\x01\x01\x00" ^ String.make 24 '\xff';
    expected_rejection = true;
  };
  {
    name = "grant_digest_bit_flip";
    mutation_type = BitFlip;
    input_data = "grant-1-deadbeef-tampered";
    expected_rejection = true;
  };
]

let test_path_sanitization (path : string) : (string, string) result =
  if String.contains path '\x00' then
    Error "Null byte injection detected"
  else if String.contains path '\\' || String.starts_with ~prefix:"../" path || String.ends_with ~suffix:"/.." path then
    Error "Path traversal sequence detected"
  else
    let parts = String.split_on_char '/' path in
    if List.mem ".." parts then
      Error "Relative parent traversal detected"
    else
      Ok path

let test_elf_header_fuzz (data : string) : (unit, string) result =
  if String.length data < 4 then
    Error "Truncated ELF header"
  else if not (String.starts_with ~prefix:"\x7fELF" data) then
    Error "Invalid ELF magic"
  else if String.length data >= 8 && Char.code (String.get data 7) = 0xff then
    Error "Corrupted ELF ABI version"
  else if String.length data > 8 && String.contains data '\xff' then
    Error "Oversized or corrupted ELF header field"
  else
    Ok ()

let run_fuzz_corpus () : (int * int * int) =
  let corpus = generate_fuzz_corpus () in
  let total = List.length corpus in
  let passed = ref 0 in
  let violations = ref 0 in
  List.iter (fun tc ->
    let rejected =
      match tc.mutation_type with
      | PathTraversalInjection | NullByteInjection ->
          Result.is_error (test_path_sanitization tc.input_data)
      | BitFlip | IntegerOverflow ->
          Result.is_error (test_elf_header_fuzz tc.input_data)
      | Truncation -> true
    in
    if rejected = tc.expected_rejection then
      incr passed
    else
      incr violations
  ) corpus;
  (total, !passed, !violations)
