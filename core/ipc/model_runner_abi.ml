(* core/ipc/model_runner_abi.ml - Versioned Model Runner Process & IPC ABI (Task L105)
   Provides typed request/response messaging, structured limits, cancellation,
   and memory accounting without command-line string interpolation of user input.
*)

open Envelope

let abi_version = "inos.model_runner_abi.v1.0"

type model_precision =
  | FP32
  | FP16
  | Q8_0
  | Q4_K_M
  | Q4_0

let string_of_precision = function
  | FP32 -> "FP32"
  | FP16 -> "FP16"
  | Q8_0 -> "Q8_0"
  | Q4_K_M -> "Q4_K_M"
  | Q4_0 -> "Q4_0"

let precision_of_string = function
  | "FP32" -> FP32
  | "FP16" -> FP16
  | "Q8_0" -> Q8_0
  | "Q4_K_M" -> Q4_K_M
  | "Q4_0" -> Q4_0
  | s -> failwith ("Unknown model precision: " ^ s)

type runner_request =
  | LoadModel of {
      model_path : string;
      tokenizer_path : string;
      precision : model_precision;
      max_context : int;
    }
  | Generate of {
      request_id : string;
      prompt : string;
      max_tokens : int;
      temperature : float;
      seed : int option;
      stop_sequences : string list;
    }
  | Cancel of { request_id : string }
  | UnloadModel
  | HealthCheck

type runner_telemetry = {
  tok_per_sec : float;
  latency_ms : float;
  time_to_first_token_ms : float;
  peak_rss_kb : int;
}

type runner_response =
  | ModelLoaded of {
      model_id : string;
      parameter_count : int;
      resident_bytes : int;
    }
  | GenerationSuccess of {
      request_id : string;
      output_text : string;
      tokens_generated : int;
      finish_reason : string; (* "length" | "stop_sequence" | "eos" *)
      telemetry : runner_telemetry;
    }
  | GenerationCancelled of { request_id : string }
  | ModelUnloaded
  | HealthReport of {
      status : string;
      active_model : string option;
      current_rss_kb : int;
    }
  | RunnerError of {
      request_id : string option;
      code : string;
      message : string;
      fatal : bool;
    }

(* Protocol wire serialization helpers using length-prefixed or JSON structures *)
let serialize_request (req : runner_request) : string =
  match req with
  | LoadModel { model_path; tokenizer_path; precision; max_context } ->
      Printf.sprintf {|{"cmd":"load","model_path":"%s","tokenizer_path":"%s","precision":"%s","max_context":%d}|}
        (String.escaped model_path) (String.escaped tokenizer_path) (string_of_precision precision) max_context
  | Generate { request_id; prompt; max_tokens; temperature; seed; stop_sequences } ->
      let seed_str = match seed with Some s -> string_of_int s | None -> "null" in
      let stops = String.concat "," (List.map (fun s -> "\"" ^ String.escaped s ^ "\"") stop_sequences) in
      Printf.sprintf {|{"cmd":"generate","request_id":"%s","prompt":"%s","max_tokens":%d,"temperature":%.2f,"seed":%s,"stop_sequences":[%s]}|}
        (String.escaped request_id) (String.escaped prompt) max_tokens temperature seed_str stops
  | Cancel { request_id } ->
      Printf.sprintf {|{"cmd":"cancel","request_id":"%s"}|} (String.escaped request_id)
  | UnloadModel ->
      {|{"cmd":"unload"}|}
  | HealthCheck ->
      {|{"cmd":"health"}|}

let compute_sha256 s = Sha256.digest_string s

let to_ipc_envelope (sender : role) (recipient : role) (req : runner_request) : envelope =
  let payload = serialize_request req in
  let msg_id = Printf.sprintf "msg-%f-%d" (Unix.gettimeofday ()) (Random.int 1000000) in
  Envelope.make ~correlation_id:msg_id ~msg_id ~sender ~recipient ~msg_type:Request ~payload ()

