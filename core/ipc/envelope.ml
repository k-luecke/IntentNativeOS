(* core/ipc/envelope.ml - Versioned IPC Message Envelope *)

type role =
  | Kernel
  | Broker
  | IntentCompiler
  | ModelA
  | ModelB
  | Capsule
  | UI
  | UnknownRole of string

type msg_type =
  | Request
  | Response
  | Notification
  | MsgError
  | UnknownType of string

type envelope = {
  schema : string;
  msg_id : string;
  timestamp : float;
  sender : role;
  recipient : role;
  msg_type : msg_type;
  correlation_id : string;
  payload : string;
  digest : string;
}

let current_schema = "inos.ipc.v0.1"

let string_of_role = function
  | Kernel -> "KERNEL"
  | Broker -> "BROKER"
  | IntentCompiler -> "INTENT_COMPILER"
  | ModelA -> "MODEL_A"
  | ModelB -> "MODEL_B"
  | Capsule -> "CAPSULE"
  | UI -> "UI"
  | UnknownRole s -> s

let role_of_string = function
  | "KERNEL" -> Kernel
  | "BROKER" -> Broker
  | "INTENT_COMPILER" -> IntentCompiler
  | "MODEL_A" -> ModelA
  | "MODEL_B" -> ModelB
  | "CAPSULE" -> Capsule
  | "UI" -> UI
  | s -> UnknownRole s

let string_of_msg_type = function
  | Request -> "REQUEST"
  | Response -> "RESPONSE"
  | Notification -> "NOTIFICATION"
  | MsgError -> "ERROR"
  | UnknownType s -> s

let msg_type_of_string = function
  | "REQUEST" -> Request
  | "RESPONSE" -> Response
  | "NOTIFICATION" -> Notification
  | "ERROR" -> MsgError
  | s -> UnknownType s

(* Compute deterministic SHA-256 cryptographic digest *)
let compute_sha256 s = Sha256.digest_string s

let compute_canonical_digest schema msg_id timestamp sender recipient msg_type correlation_id payload =
  let content = Printf.sprintf "%s|%s|%.3f|%s|%s|%s|%s|%s"
    schema msg_id timestamp (string_of_role sender) (string_of_role recipient)
    (string_of_msg_type msg_type) correlation_id payload in
  compute_sha256 content

let make ?(correlation_id="00000000-0000-0000-0000-000000000000") ~msg_id ~sender ~recipient ~msg_type ~payload () =
  let timestamp = Unix.gettimeofday () in
  let digest = compute_canonical_digest current_schema msg_id timestamp sender recipient msg_type correlation_id payload in
  {
    schema = current_schema;
    msg_id;
    timestamp;
    sender;
    recipient;
    msg_type;
    correlation_id;
    payload;
    digest;
  }

let create = make

(* Minimal, deterministic escape function for JSON strings *)
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

let to_wire_json env =
  Printf.sprintf
    "{\"schema\":\"%s\",\"msg_id\":\"%s\",\"timestamp\":%.3f,\"sender\":\"%s\",\"recipient\":\"%s\",\"msg_type\":\"%s\",\"correlation_id\":\"%s\",\"payload\":\"%s\",\"digest\":\"%s\"}"
    (json_escape env.schema)
    (json_escape env.msg_id)
    env.timestamp
    (string_of_role env.sender)
    (string_of_role env.recipient)
    (string_of_msg_type env.msg_type)
    (json_escape env.correlation_id)
    (json_escape env.payload)
    env.digest

(* Simple parser for canonical envelope JSON *)
let extract_field field json =
  let search = Printf.sprintf "\"%s\":\"" field in
  let s_len = String.length search in
  let j_len = String.length json in
  let rec find_key i =
    if i + s_len > j_len then None
    else if String.sub json i s_len = search then
      let val_start = i + s_len in
      let rec find_end k =
        if k >= j_len then None
        else if json.[k] = '\\' then find_end (k + 2)
        else if json.[k] = '"' then Some k
        else find_end (k + 1)
      in
      match find_end val_start with
      | Some val_end ->
          let raw = String.sub json val_start (val_end - val_start) in
          Some (json_unescape raw)
      | None -> None
    else find_key (i + 1)
  in
  find_key 0

let extract_float_field field json =
  let search = Printf.sprintf "\"%s\":" field in
  let idx = try Some (String.index_from json 0 search.[0]) with Not_found -> None in
  match idx with
  | None -> None
  | Some _ ->
      (* scan for key and following number *)
      let len = String.length json in
      let rec scan i =
        if i + String.length search > len then None
        else if String.sub json i (String.length search) = search then
          let start_num = i + String.length search in
          let rec find_end j =
            if j >= len || json.[j] = ',' || json.[j] = '}' then j
            else find_end (j + 1)
          in
          let end_num = find_end start_num in
          let num_str = String.trim (String.sub json start_num (end_num - start_num)) in
          float_of_string_opt num_str
        else scan (i + 1)
      in
      scan 0

type parse_error =
  | MissingField of string
  | SchemaMismatch of string * string
  | DigestMismatch of string * string
  | CorruptedFormat

let of_wire_json json =
  match (
    extract_field "schema" json,
    extract_field "msg_id" json,
    extract_float_field "timestamp" json,
    extract_field "sender" json,
    extract_field "recipient" json,
    extract_field "msg_type" json,
    extract_field "correlation_id" json,
    extract_field "payload" json,
    extract_field "digest" json
  ) with
  | (Some s, Some mid, Some ts, Some snd, Some rcv, Some mt, Some cid, Some p, Some d) ->
      if s <> current_schema then
        Error (SchemaMismatch (s, current_schema))
      else
        let sender = role_of_string snd in
        let recipient = role_of_string rcv in
        let msg_type = msg_type_of_string mt in
        let expected_digest = compute_canonical_digest s mid ts sender recipient msg_type cid p in
        if expected_digest <> d then
          Error (DigestMismatch (d, expected_digest))
        else
          Ok {
            schema = s;
            msg_id = mid;
            timestamp = ts;
            sender;
            recipient;
            msg_type;
            correlation_id = cid;
            payload = p;
            digest = d;
          }
  | _ -> Error CorruptedFormat
