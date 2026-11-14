(* services/bridge/bridge_protocol.ml - L402 Hosted Bridge Protocol
   EXPLICIT CLASSIFICATION: [HOST_INTEGRATED_BRIDGE]
   This service runs in the host environment to facilitate model inference bridging
   and is explicitly NOT kernel-native.
*)

let magic = "BRG1"
let max_payload_size = 65536

type message_type =
  | MsgInferenceReq
  | MsgInferenceResp
  | MsgHealthCheck
  | MsgReclaimMemory
  | MsgError

let byte_of_msg_type = function
  | MsgInferenceReq -> 0x01
  | MsgInferenceResp -> 0x02
  | MsgHealthCheck -> 0x03
  | MsgReclaimMemory -> 0x04
  | MsgError -> 0xFF

let msg_type_of_byte = function
  | 0x01 -> Some MsgInferenceReq
  | 0x02 -> Some MsgInferenceResp
  | 0x03 -> Some MsgHealthCheck
  | 0x04 -> Some MsgReclaimMemory
  | 0xFF -> Some MsgError
  | _ -> None

type frame = {
  msg_type : message_type;
  session_id : int;
  nonce : int;
  payload : string;
}

let serialize_frame (f : frame) : string =
  let p_len = String.length f.payload in
  if p_len > max_payload_size then
    failwith "Bridge frame payload exceeds max_payload_size (64KB)";
  let buf = Buffer.create (16 + p_len) in
  Buffer.add_string buf magic;
  Buffer.add_char buf (char_of_int (byte_of_msg_type f.msg_type));
  Buffer.add_char buf (char_of_int ((f.session_id lsr 8) land 0xFF));
  Buffer.add_char buf (char_of_int (f.session_id land 0xFF));
  Buffer.add_char buf (char_of_int ((f.nonce lsr 8) land 0xFF));
  Buffer.add_char buf (char_of_int (f.nonce land 0xFF));
  Buffer.add_char buf (char_of_int ((p_len lsr 24) land 0xFF));
  Buffer.add_char buf (char_of_int ((p_len lsr 16) land 0xFF));
  Buffer.add_char buf (char_of_int ((p_len lsr 8) land 0xFF));
  Buffer.add_char buf (char_of_int (p_len land 0xFF));
  Buffer.add_string buf f.payload;
  Buffer.contents buf

let deserialize_frame (bytes : string) : frame option =
  let len = String.length bytes in
  if len < 12 then None
  else if String.sub bytes 0 4 <> magic then None
  else
    match msg_type_of_byte (int_of_char bytes.[4]) with
    | None -> None
    | Some mt ->
        let s_hi = int_of_char bytes.[5] in
        let s_lo = int_of_char bytes.[6] in
        let session_id = (s_hi lsl 8) lor s_lo in
        let n_hi = int_of_char bytes.[7] in
        let n_lo = int_of_char bytes.[8] in
        let nonce = (n_hi lsl 8) lor n_lo in
        let p_len =
          (int_of_char bytes.[9] lsl 24) lor
          (int_of_char bytes.[10] lsl 16) lor
          (int_of_char bytes.[11] lsl 8) lor
          int_of_char bytes.[12] in
        if p_len < 0 || p_len > max_payload_size || len < 13 + p_len then None
        else
          let payload = String.sub bytes 13 p_len in
          Some { msg_type = mt; session_id; nonce; payload }
