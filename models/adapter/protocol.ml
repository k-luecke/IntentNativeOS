(* models/adapter/protocol.ml - Local Model Adapter Protocol with Health, Timeout, and Streaming *)

type health_status =
  | Healthy of { latency_ms : float; memory_rss_mb : float }
  | Degraded of { reason : string }
  | Unhealthy of { error : string }

type model_request = {
  request_id : string;
  model_name : string;
  prompt : string;
  max_tokens : int;
  temperature : float;
  timeout_seconds : float;
}

type model_chunk = {
  chunk_index : int;
  delta_text : string;
  is_final : bool;
}

type model_response = {
  request_id : string;
  model_name : string;
  full_text : string;
  total_tokens : int;
  duration_ms : float;
}

type adapter_error =
  | TimeoutExceeded of { elapsed : float; limit : float }
  | ProcessCrashed of { exit_code : int; stderr : string }
  | SchemaError of string
  | ConnectionFailed of string
  | ModelCancelled

module type MODEL_ADAPTER = sig
  val check_health : unit -> health_status
  val invoke_buffered : model_request -> (model_response, adapter_error) result
  val invoke_streaming : model_request -> on_chunk:(model_chunk -> unit) -> (model_response, adapter_error) result
end

(* Mock adapter for deterministic verification of timeouts, streaming, and error handling *)
module MockAdapter : sig
  include MODEL_ADAPTER
  val set_failure_mode : adapter_error option -> unit
  val set_mock_chunks : string list -> unit
  val set_latency : float -> unit
end = struct
  let failure_mode = ref None
  let mock_chunks = ref ["{\"plan\": \"verify ABI\"}"]
  let mock_latency = ref 5.0

  let set_failure_mode fm = failure_mode := fm
  let set_mock_chunks chunks = mock_chunks := chunks
  let set_latency lat = mock_latency := lat

  let check_health () =
    match !failure_mode with
    | Some (ConnectionFailed err) -> Unhealthy { error = err }
    | Some _ -> Degraded { reason = "Mock failure condition active" }
    | None -> Healthy { latency_ms = !mock_latency; memory_rss_mb = 128.0 }

  let invoke_streaming (req : model_request) ~on_chunk : (model_response, adapter_error) result =
    match !failure_mode with
    | Some err -> Error err
    | None ->
        if req.timeout_seconds <= 0.05 then
          Error (TimeoutExceeded { elapsed = 0.06; limit = req.timeout_seconds })
        else
          let total = List.length !mock_chunks in
          let buf = Buffer.create 256 in
          List.iteri (fun idx text ->
            Buffer.add_string buf text;
            on_chunk {
              chunk_index = idx;
              delta_text = text;
              is_final = (idx = total - 1);
            }
          ) !mock_chunks;
          Ok {
            request_id = req.request_id;
            model_name = req.model_name;
            full_text = Buffer.contents buf;
            total_tokens = 42;
            duration_ms = !mock_latency;
          }

  let invoke_buffered (req : model_request) : (model_response, adapter_error) result =
    invoke_streaming req ~on_chunk:(fun _ -> ())
end
