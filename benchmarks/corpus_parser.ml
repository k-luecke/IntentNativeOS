(* benchmarks/corpus_parser.ml - Pure OCaml Parser for Benchmark Corpus JSON *)

open Metrics

let clean_str s =
  let len = String.length s in
  if len >= 2 && s.[0] = '"' && s.[len - 1] = '"' then
    String.sub s 1 (len - 2)
  else
    s

let unescape_json s =
  let b = Buffer.create (String.length s) in
  let i = ref 0 in
  let len = String.length s in
  while !i < len do
    if s.[!i] = '\\' && !i + 1 < len then begin
      let c = s.[!i + 1] in
      (match c with
       | '"' -> Buffer.add_char b '"'
       | '\\' -> Buffer.add_char b '\\'
       | '/' -> Buffer.add_char b '/'
       | 'n' -> Buffer.add_char b '\n'
       | 't' -> Buffer.add_char b '\t'
       | 'r' -> Buffer.add_char b '\r'
       | _ -> Buffer.add_char b '\\'; Buffer.add_char b c);
      i := !i + 2
    end else begin
      Buffer.add_char b s.[!i];
      incr i
    end
  done;
  Buffer.contents b

let extract_field_string key line =
  let pat = "\"" ^ key ^ "\"[ \t]*:[ \t]*\"" in
  let r = Str.regexp pat in
  try
    let pos = Str.search_forward r line 0 in
    let start_val = pos + String.length (Str.matched_string line) in
    let end_val = String.index_from line start_val '"' in
    let raw = String.sub line start_val (end_val - start_val) in
    Some (unescape_json raw)
  with _ -> None

let extract_field_target line =
  if String.contains line ':' then
    let pat_null = "\"expected_target\"[ \t]*:[ \t]*null" in
    let r_null = Str.regexp pat_null in
    try
      ignore (Str.search_forward r_null line 0);
      None
    with _ ->
      extract_field_string "expected_target" line
  else
    None

let extract_field_bool key line =
  let pat = "\"" ^ key ^ "\"[ \t]*:[ \t]*\\(true\\|false\\)" in
  let r = Str.regexp pat in
  try
    ignore (Str.search_forward r line 0);
    let matched = Str.matched_group 1 line in
    Some (matched = "true")
  with _ -> None

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

let parse_corpus_file (path : string) : benchmark_case list =
  let resolved_path = resolve_file path in
  let ic = open_in resolved_path in
  let cases = ref [] in
  let current_id = ref "" in
  let current_split = ref "" in
  let current_cat = ref "" in
  let current_prompt = ref "" in
  let current_exp_cls = ref "" in
  let current_target = ref None in
  let current_req_model = ref false in
  let current_should_auth = ref false in
  let in_case = ref false in

  let flush_case () =
    if !in_case && !current_id <> "" then begin
      let c = {
        id = !current_id;
        split = !current_split;
        category = !current_cat;
        prompt = !current_prompt;
        expected_classification = classification_of_string !current_exp_cls;
        expected_target = !current_target;
        requires_model = !current_req_model;
        should_authorize = !current_should_auth;
      } in
      cases := c :: !cases;
      current_id := "";
      in_case := false;
    end
  in

  (try
     while true do
       let line = String.trim (input_line ic) in
       if String.length line > 0 && line.[0] = '{' && not !in_case then begin
         in_case := true;
         current_id := "";
         current_split := "";
         current_cat := "";
         current_prompt := "";
         current_exp_cls := "";
         current_target := None;
         current_req_model := false;
         current_should_auth := false;
       end;

       (match extract_field_string "id" line with
        | Some id -> current_id := id
        | None -> ());
       (match extract_field_string "split" line with
        | Some s -> current_split := s
        | None -> ());
       (match extract_field_string "category" line with
        | Some c -> current_cat := c
        | None -> ());
       (match extract_field_string "prompt" line with
        | Some p -> current_prompt := p
        | None -> ());
       (match extract_field_string "expected_classification" line with
        | Some cls -> current_exp_cls := cls
        | None -> ());
       (match extract_field_target line with
        | Some t -> current_target := Some t
        | None -> ());
       (match extract_field_bool "requires_model" line with
        | Some b -> current_req_model := b
        | None -> ());
       (match extract_field_bool "should_authorize" line with
        | Some b -> current_should_auth := b
        | None -> ());

       if String.length line > 0 && (line.[0] = '}' || String.sub line 0 (min 2 (String.length line)) = "},") then begin
         flush_case ()
       end
     done
   with End_of_file ->
     flush_case ();
     close_in ic);

  List.rev !cases
