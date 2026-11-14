(* services/bridge/bridge_server.ml - L402 Hosted Bridge Server
   EXPLICIT CLASSIFICATION: [HOST_INTEGRATED_BRIDGE]
*)

open Bridge_protocol

type server_status = {
  mutable requests_processed : int;
  mutable bytes_transferred : int;
  mutable active_sessions : int;
  mutable last_reclaim_timestamp : float;
}

let create_server_status () = {
  requests_processed = 0;
  bytes_transferred = 0;
  active_sessions = 0;
  last_reclaim_timestamp = Unix.gettimeofday ();
}

let handle_frame (status : server_status) (f : frame) : frame =
  status.requests_processed <- status.requests_processed + 1;
  status.bytes_transferred <- status.bytes_transferred + String.length f.payload;
  match f.msg_type with
  | MsgHealthCheck ->
      { msg_type = MsgHealthCheck;
        session_id = f.session_id;
        nonce = f.nonce + 1;
        payload = Printf.sprintf "{\"status\":\"OK\",\"classification\":\"HOST_INTEGRATED_BRIDGE\",\"requests\":%d}" status.requests_processed }
  | MsgReclaimMemory ->
      status.last_reclaim_timestamp <- Unix.gettimeofday ();
      { msg_type = MsgReclaimMemory;
        session_id = f.session_id;
        nonce = f.nonce + 1;
        payload = "{\"status\":\"RECLAIMED\",\"bytes_freed\":0,\"target\":\"HOST_INFERENCE_CACHE\"}" }
  | MsgInferenceReq ->
      (* Sanitize and bound payload *)
      let clean_payload =
        let b = Buffer.create (String.length f.payload) in
        String.iter (fun c ->
          if c >= ' ' && c <= '~' || c = '\n' || c = '\t' then Buffer.add_char b c
        ) f.payload;
        Buffer.contents b in
      { msg_type = MsgInferenceResp;
        session_id = f.session_id;
        nonce = f.nonce + 1;
        payload = Printf.sprintf "{\"model_result\":\"ack\",\"sanitized_len\":%d,\"bridge\":\"host\"}" (String.length clean_payload) }
  | _ ->
      { msg_type = MsgError;
        session_id = f.session_id;
        nonce = f.nonce + 1;
        payload = "{\"error\":\"Unsupported bridge message type\"}" }
