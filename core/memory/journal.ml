(* core/memory/journal.ml - Append-only Event Journal with Checksums, Hash Chaining, and Corruption Detection *)

type entry = {
  seq : int;
  timestamp : float;
  event_id : string;
  event_type : string;
  producer : string;
  payload : string;
  prev_digest : string;
  digest : string;
}

type journal = {
  file_path : string;
  mutable last_seq : int;
  mutable last_digest : string;
  mutable entries : entry list;
}

type verification_error =
  | ChecksumMismatch of { seq : int; expected : string; found : string }
  | HashChainBroken of { seq : int; expected_prev : string; found_prev : string }
  | SequenceGap of { expected_seq : int; found_seq : int }
  | CorruptedFormat of { line_number : int; details : string }

let genesis_digest = "0000000000000000000000000000000000000000000000000000000000000000"

let compute_sha256 s = Inos_ipc.Sha256.digest_string s

let compute_digest ~seq ~timestamp ~event_id ~event_type ~producer ~prev_digest ~payload =
  let content = Printf.sprintf "%d|%.4f|%s|%s|%s|%s|%s"
    seq timestamp event_id event_type producer prev_digest payload in
  compute_sha256 content

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

let serialize_entry e =
  Printf.sprintf
    "{\"seq\":%d,\"timestamp\":%.4f,\"event_id\":\"%s\",\"event_type\":\"%s\",\"producer\":\"%s\",\"prev_digest\":\"%s\",\"digest\":\"%s\",\"payload\":\"%s\"}\n"
    e.seq e.timestamp (json_escape e.event_id) (json_escape e.event_type)
    (json_escape e.producer) e.prev_digest e.digest (json_escape e.payload)

(* Pure substring helper for line parsing *)
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

let extract_str key json =
  let target = "\"" ^ key ^ "\"" in
  match find_sub ~pattern:target json 0 with
  | None -> None
  | Some pos ->
      try
        let colon_pos = String.index_from json pos ':' in
        let first_quote = String.index_from json colon_pos '"' in
        let rec find_end idx =
          if idx >= String.length json then idx
          else if json.[idx] = '"' && json.[idx - 1] <> '\\' then idx
          else find_end (idx + 1)
        in
        let end_quote = find_end (first_quote + 1) in
        let raw = String.sub json (first_quote + 1) (end_quote - first_quote - 1) in
        Some (json_unescape raw)
      with _ -> None

let extract_int key json =
  let target = "\"" ^ key ^ "\"" in
  match find_sub ~pattern:target json 0 with
  | None -> None
  | Some pos ->
      try
        let colon_pos = String.index_from json pos ':' in
        let rec find_digit idx =
          if idx >= String.length json then idx
          else match json.[idx] with '0'..'9' | '-' -> idx | _ -> find_digit (idx + 1)
        in
        let s_idx = find_digit (colon_pos + 1) in
        let rec find_end idx =
          if idx >= String.length json then idx
          else match json.[idx] with '0'..'9' -> find_end (idx + 1) | _ -> idx
        in
        let e_idx = find_end s_idx in
        Some (int_of_string (String.sub json s_idx (e_idx - s_idx)))
      with _ -> None

let extract_float key json =
  let target = "\"" ^ key ^ "\"" in
  match find_sub ~pattern:target json 0 with
  | None -> None
  | Some pos ->
      try
        let colon_pos = String.index_from json pos ':' in
        let rec find_digit idx =
          if idx >= String.length json then idx
          else match json.[idx] with '0'..'9' | '-' | '.' -> idx | _ -> find_digit (idx + 1)
        in
        let s_idx = find_digit (colon_pos + 1) in
        let rec find_end idx =
          if idx >= String.length json then idx
          else match json.[idx] with '0'..'9' | '.' -> find_end (idx + 1) | _ -> idx
        in
        let e_idx = find_end s_idx in
        Some (float_of_string (String.sub json s_idx (e_idx - s_idx)))
      with _ -> None

let parse_entry line line_no : (entry, verification_error) result =
  match (extract_int "seq" line,
         extract_float "timestamp" line,
         extract_str "event_id" line,
         extract_str "event_type" line,
         extract_str "producer" line,
         extract_str "prev_digest" line,
         extract_str "digest" line,
         extract_str "payload" line) with
  | (Some seq, Some timestamp, Some event_id, Some event_type, Some producer, Some prev_digest, Some digest, Some payload) ->
      Ok { seq; timestamp; event_id; event_type; producer; prev_digest; digest; payload }
  | _ ->
      Error (CorruptedFormat { line_number = line_no; details = "Malformed NDJSON record" })

let open_journal (path : string) : (journal, verification_error) result =
  let j = {
    file_path = path;
    last_seq = 0;
    last_digest = genesis_digest;
    entries = [];
  } in
  if Sys.file_exists path then begin
    let ic = open_in path in
    let rec read_loop line_no expected_seq expected_prev acc =
      match input_line ic with
      | line ->
          let trimmed = String.trim line in
          if trimmed = "" then
            read_loop (line_no + 1) expected_seq expected_prev acc
          else
            (match parse_entry trimmed line_no with
             | Error e ->
                 close_in ic;
                 Error e
             | Ok entry ->
                 (* Invariant: Sequence number must increment strictly monotonically *)
                 if entry.seq <> expected_seq then (
                   close_in ic;
                   Error (SequenceGap { expected_seq; found_seq = entry.seq })
                 ) else if entry.prev_digest <> expected_prev then (
                   close_in ic;
                   Error (HashChainBroken { seq = entry.seq; expected_prev; found_prev = entry.prev_digest })
                 ) else
                   let expected_digest = compute_digest
                     ~seq:entry.seq
                     ~timestamp:entry.timestamp
                     ~event_id:entry.event_id
                     ~event_type:entry.event_type
                     ~producer:entry.producer
                     ~prev_digest:entry.prev_digest
                     ~payload:entry.payload
                   in
                   if entry.digest <> expected_digest then (
                     close_in ic;
                     Error (ChecksumMismatch { seq = entry.seq; expected = expected_digest; found = entry.digest })
                   ) else
                     read_loop (line_no + 1) (expected_seq + 1) entry.digest (entry :: acc))
      | exception End_of_file ->
          close_in ic;
          Ok (List.rev acc)
    in
    match read_loop 1 1 genesis_digest [] with
    | Error e -> Error e
    | Ok validated_entries ->
        (match List.rev validated_entries with
         | [] -> Ok j
         | last :: _ ->
             j.last_seq <- last.seq;
             j.last_digest <- last.digest;
             j.entries <- validated_entries;
             Ok j)
  end else
    Ok j

let append_event
    (j : journal)
    ?(timestamp = Unix.gettimeofday ())
    ~event_id
    ~event_type
    ~producer
    ~payload () : entry =
  let seq = j.last_seq + 1 in
  let prev_digest = j.last_digest in
  let digest = compute_digest ~seq ~timestamp ~event_id ~event_type ~producer ~prev_digest ~payload in
  let entry = {
    seq;
    timestamp;
    event_id;
    event_type;
    producer;
    payload;
    prev_digest;
    digest;
  } in
  (* Write to append-only file *)
  let oc = open_out_gen [Open_wronly; Open_append; Open_creat] 0o644 j.file_path in
  output_string oc (serialize_entry entry);
  flush oc;
  close_out oc;
  j.last_seq <- seq;
  j.last_digest <- digest;
  j.entries <- j.entries @ [entry];
  entry
