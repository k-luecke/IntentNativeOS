(* compiler/ir/serialize.ml - Versioned JSON Serialization and Deserialization for Intent IR *)

open Contract

let current_ir_schema = "inos.intent.v0.1"

let json_escape s =
  let buf = Buffer.create (String.length s + 16) in
  String.iter (function
    | '"' -> Buffer.add_string buf "\\\""
    | '\\' -> Buffer.add_string buf "\\\\"
    | '\n' -> Buffer.add_string buf "\\n"
    | '\r' -> Buffer.add_string buf "\\r"
    | '\t' -> Buffer.add_string buf "\\t"
    | c -> Buffer.add_char buf c
  ) s;
  Buffer.contents buf

let serialize_qualified_string (q : string qualified) =
  Printf.sprintf "{\"value\":\"%s\",\"origin\":\"%s\",\"binding\":\"%s\"}"
    (json_escape q.value)
    (string_of_origin q.origin)
    (string_of_binding q.binding)

let serialize_qualified_target (q : target_ref qualified) =
  Printf.sprintf "{\"value\":\"%s\",\"origin\":\"%s\",\"binding\":\"%s\"}"
    (json_escape (string_of_target q.value))
    (string_of_origin q.origin)
    (string_of_binding q.binding)

let serialize_ambiguity (a : material_ambiguity) =
  let alts = String.concat "," (List.map (fun alt -> Printf.sprintf "\"%s\"" (json_escape alt)) a.alternatives) in
  Printf.sprintf "{\"field_path\":\"%s\",\"alternatives\":[%s],\"consequence\":\"%s\"}"
    (json_escape a.field_path) alts (json_escape a.consequence)

let to_json (contract : intent_contract) : string =
  let targets_json = String.concat "," (List.map serialize_qualified_target contract.targets) in
  let constraints_json = String.concat "," (List.map serialize_qualified_string contract.constraints) in
  let ambiguities_json = String.concat "," (List.map serialize_ambiguity contract.ambiguities) in
  let criteria_json = String.concat "," (List.map (fun c -> Printf.sprintf "\"%s\"" (json_escape c)) contract.verification_criteria) in
  let scope_json = String.concat "," (List.map (fun s -> Printf.sprintf "\"%s\"" (json_escape s)) contract.authority_scope) in
  let parent_hash_json = match contract.parent_revision_hash with
    | None -> "null"
    | Some h -> Printf.sprintf "\"%s\"" (json_escape h)
  in
  Printf.sprintf
    "{\"schema\":\"%s\",\"id\":\"%s\",\"revision\":%d,\"mode\":\"%s\",\"objective\":%s,\"targets\":[%s],\"constraints\":[%s],\"ambiguities\":[%s],\"verification_criteria\":[%s],\"authority_scope\":[%s],\"parent_revision_hash\":%s,\"content_hash\":\"%s\"}"
    current_ir_schema
    (json_escape contract.id)
    contract.revision
    (Mode.to_string contract.mode)
    (serialize_qualified_string contract.objective)
    targets_json
    constraints_json
    ambiguities_json
    criteria_json
    scope_json
    parent_hash_json
    (json_escape contract.content_hash)

let find_sub ~pattern str start =
  let p_len = String.length pattern in
  let s_len = String.length str in
  let rec check idx =
    if idx + p_len > s_len then None
    else if String.sub str idx p_len = pattern then Some idx
    else check (idx + 1)
  in
  check start

let json_unescape s =
  let buf = Buffer.create (String.length s) in
  let len = String.length s in
  let rec loop idx =
    if idx >= len then Buffer.contents buf
    else if s.[idx] = '\\' && idx + 1 < len then
      match s.[idx + 1] with
      | '"' -> Buffer.add_char buf '"'; loop (idx + 2)
      | '\\' -> Buffer.add_char buf '\\'; loop (idx + 2)
      | 'n' -> Buffer.add_char buf '\n'; loop (idx + 2)
      | 'r' -> Buffer.add_char buf '\r'; loop (idx + 2)
      | 't' -> Buffer.add_char buf '\t'; loop (idx + 2)
      | c -> Buffer.add_char buf '\\'; Buffer.add_char buf c; loop (idx + 2)
    else (Buffer.add_char buf s.[idx]; loop (idx + 1))
  in
  loop 0

let rec skip_ws json idx =
  if idx >= String.length json then idx
  else match json.[idx] with
  | ' ' | '\t' | '\n' | '\r' -> skip_ws json (idx + 1)
  | _ -> idx

let find_string_end json idx =
  let len = String.length json in
  let rec loop i =
    if i >= len then i
    else if json.[i] = '\\' then loop (i + 2)
    else if json.[i] = '"' then i
    else loop (i + 1)
  in
  loop idx

(* Lightweight JSON token extractor *)
let extract_string_field key json =
  let target = "\"" ^ key ^ "\"" in
  match find_sub ~pattern:target json 0 with
  | None -> None
  | Some pos ->
      try
        let colon_pos = String.index_from json (pos + String.length target) ':' in
        let val_pos = skip_ws json (colon_pos + 1) in
        if val_pos >= String.length json || json.[val_pos] <> '"' then
          None
        else
          let end_quote = find_string_end json (val_pos + 1) in
          let raw = String.sub json (val_pos + 1) (end_quote - val_pos - 1) in
          Some (json_unescape raw)
      with _ -> None

let extract_int_field key json =
  let target = "\"" ^ key ^ "\"" in
  match find_sub ~pattern:target json 0 with
  | None -> None
  | Some pos ->
      try
        let colon_pos = String.index_from json (pos + String.length target) ':' in
        let rec find_digit idx =
          if idx >= String.length json then idx
          else match json.[idx] with
          | '0'..'9' | '-' -> idx
          | _ -> find_digit (idx + 1)
        in
        let start_digit = find_digit (colon_pos + 1) in
        let rec find_end idx =
          if idx >= String.length json then idx
          else match json.[idx] with
          | '0'..'9' -> find_end (idx + 1)
          | _ -> idx
        in
        let end_digit = find_end start_digit in
        Some (int_of_string (String.sub json start_digit (end_digit - start_digit)))
      with _ -> None

let extract_delimited_field open_ch close_ch key json =
  let target = "\"" ^ key ^ "\"" in
  match find_sub ~pattern:target json 0 with
  | None -> None
  | Some pos ->
      try
        let colon_pos = String.index_from json (pos + String.length target) ':' in
        let start_pos = skip_ws json (colon_pos + 1) in
        let len = String.length json in
        if start_pos >= len || json.[start_pos] <> open_ch then None
        else
          let rec scan i depth =
            if i >= len then None
            else if json.[i] = '"' then
              let q_end = find_string_end json (i + 1) in
              scan (q_end + 1) depth
            else if json.[i] = open_ch then
              scan (i + 1) (depth + 1)
            else if json.[i] = close_ch then
              if depth = 1 then Some i
              else scan (i + 1) (depth - 1)
            else
              scan (i + 1) depth
          in
          match scan start_pos 0 with
          | Some end_pos ->
              Some (String.sub json (start_pos + 1) (end_pos - start_pos - 1))
          | None -> None
      with _ -> None

let split_json_objects inner =
  let len = String.length inner in
  let rec loop i acc =
    if i >= len then List.rev acc
    else if inner.[i] = '{' then
      let rec scan j depth =
        if j >= len then j
        else if inner.[j] = '"' then
          let q_end = find_string_end inner (j + 1) in
          scan (q_end + 1) depth
        else if inner.[j] = '{' then scan (j + 1) (depth + 1)
        else if inner.[j] = '}' then
          if depth = 1 then j else scan (j + 1) (depth - 1)
        else scan (j + 1) depth
      in
      let end_j = scan i 0 in
      let obj_str = String.sub inner i (min (len - i) (end_j - i + 1)) in
      loop (end_j + 1) (obj_str :: acc)
    else
      loop (i + 1) acc
  in
  loop 0 []

let parse_json_string_list inner =
  let len = String.length inner in
  let rec loop i acc =
    if i >= len then List.rev acc
    else if inner.[i] = '"' then
      let q_end = find_string_end inner (i + 1) in
      let raw = String.sub inner (i + 1) (q_end - i - 1) in
      loop (q_end + 1) (json_unescape raw :: acc)
    else
      loop (i + 1) acc
  in
  loop 0 []

let parse_qualified_string obj_str =
  let v = match extract_string_field "value" obj_str with Some s -> s | None -> "unspecified" in
  let orig_str = match extract_string_field "origin" obj_str with Some o -> o | None -> "USER_EXPLICIT" in
  let bind_str = match extract_string_field "binding" obj_str with Some b -> b | None -> "CONSTRAINT" in
  let origin = match origin_of_string orig_str with Ok o -> o | Error _ -> User_explicit in
  let binding = match binding_of_string bind_str with Ok b -> b | Error _ -> Constraint in
  { value = v; origin; binding; evidence = [] }

let parse_qualified_target obj_str =
  let q_str = parse_qualified_string obj_str in
  { q_str with value = target_of_string q_str.value }

let parse_ambiguity obj_str =
  let field_path = match extract_string_field "field_path" obj_str with Some f -> f | None -> "" in
  let alternatives =
    match extract_delimited_field '[' ']' "alternatives" obj_str with
    | Some arr -> parse_json_string_list arr
    | None -> []
  in
  let consequence = match extract_string_field "consequence" obj_str with Some c -> c | None -> "" in
  { field_path; alternatives; consequence }

(* Core Invariant / Task I010:
   Roundtrip serialization with version mismatch diagnosis *)
let from_json (json_str : string) : (intent_contract, string) result =
  match extract_string_field "schema" json_str with
  | None -> Error "Invalid IR JSON: missing 'schema' field"
  | Some schema when schema <> current_ir_schema ->
      Error (Printf.sprintf "Schema version mismatch: expected '%s', found '%s'" current_ir_schema schema)
  | Some _ ->
      match extract_string_field "id" json_str with
      | None -> Error "Invalid IR JSON: missing 'id' field"
      | Some id ->
          match extract_int_field "revision" json_str with
          | None -> Error "Invalid IR JSON: missing 'revision' field"
          | Some revision ->
              match extract_string_field "mode" json_str with
              | None -> Error "Invalid IR JSON: missing 'mode' field"
              | Some mode_str ->
                  match Mode.of_string mode_str with
                  | Error msg -> Error ("Invalid IR mode: " ^ msg)
                  | Ok mode ->
                      let objective =
                        match extract_delimited_field '{' '}' "objective" json_str with
                        | Some obj_json -> parse_qualified_string ("{" ^ obj_json ^ "}")
                        | None -> parse_qualified_string json_str
                      in
                      let targets =
                        match extract_delimited_field '[' ']' "targets" json_str with
                        | Some arr -> List.map parse_qualified_target (split_json_objects arr)
                        | None -> []
                      in
                      let constraints =
                        match extract_delimited_field '[' ']' "constraints" json_str with
                        | Some arr -> List.map parse_qualified_string (split_json_objects arr)
                        | None -> []
                      in
                      let ambiguities =
                        match extract_delimited_field '[' ']' "ambiguities" json_str with
                        | Some arr -> List.map parse_ambiguity (split_json_objects arr)
                        | None -> []
                      in
                      let verification_criteria =
                        match extract_delimited_field '[' ']' "verification_criteria" json_str with
                        | Some arr -> parse_json_string_list arr
                        | None -> []
                      in
                      let authority_scope =
                        match extract_delimited_field '[' ']' "authority_scope" json_str with
                        | Some arr -> parse_json_string_list arr
                        | None -> []
                      in
                      let parent_revision_hash = extract_string_field "parent_revision_hash" json_str in
                      let contract = make
                        ?parent_revision_hash
                        ~id
                        ~revision
                        ~mode
                        ~objective
                        ~targets
                        ~constraints
                        ~ambiguities
                        ~verification_criteria
                        ~authority_scope
                        ()
                      in
                      Ok contract
