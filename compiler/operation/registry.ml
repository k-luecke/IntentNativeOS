(* compiler/operation/registry.ml - Pure Native Operation Registry and Deterministic Resolver *)

open Operation_ir

type execution_metrics = {
  duration_ms : float;
  llm_calls : int;
  bytes_processed : int;
}

type native_handler =
  (string * string) list -> ((string * string) list, string) result

type registered_op = {
  name : string;
  description : string;
  input_ports : (string * data_type) list;
  output_ports : (string * data_type) list;
  handler : native_handler;
}

type registry = {
  table : (string, registered_op) Hashtbl.t;
}

let create () = {
  table = Hashtbl.create 32;
}

let register (reg : registry) (op : registered_op) =
  Hashtbl.replace reg.table op.name op

let find (reg : registry) (name : string) : registered_op option =
  try Some (Hashtbl.find reg.table name) with Not_found -> None

(* Built-in pure native transformations *)
let default_registry () =
  let reg = create () in

  (* Operation 1: Compute SHA256 digest *)
  register reg {
    name = "compute_sha256";
    description = "Computes deterministic SHA256 hex digest of string content";
    input_ports = [("content", TypeString)];
    output_ports = [("digest", TypeHashDigest)];
    handler = (fun inputs ->
      match List.assoc_opt "content" inputs with
      | Some c -> Ok [("digest", Inos_ipc.Sha256.digest_string c)]
      | None -> Error "Missing required input 'content'");
  };

  (* Operation 2: Filter lines by prefix *)
  register reg {
    name = "filter_lines";
    description = "Filters lines matching a prefix from input text";
    input_ports = [("text", TypeString); ("prefix", TypeString)];
    output_ports = [("filtered", TypeString)];
    handler = (fun inputs ->
      match (List.assoc_opt "text" inputs, List.assoc_opt "prefix" inputs) with
      | (Some text, Some prefix) ->
          let lines = String.split_on_char '\n' text in
          let matched = List.filter (fun l -> String.starts_with ~prefix l) lines in
          Ok [("filtered", String.concat "\n" matched)]
      | _ -> Error "Missing required inputs 'text' or 'prefix'");
  };

  (* Operation 3: Synthesize Staged Diff *)
  register reg {
    name = "synthesize_diff";
    description = "Generates unified diff between original and updated strings";
    input_ports = [("original", TypeString); ("updated", TypeString)];
    output_ports = [("diff", TypeString)];
    handler = (fun inputs ->
      match (List.assoc_opt "original" inputs, List.assoc_opt "updated" inputs) with
      | (Some orig, Some upd) ->
          let diff = Printf.sprintf "--- a/staged\n+++ b/staged\n@@ -1 +1 @@\n-%s\n+%s" orig upd in
          Ok [("diff", diff)]
      | _ -> Error "Missing inputs 'original' or 'updated'");
  };

  reg

(* Core Invariant / Task C002:
   Known transformation resolved without LLM call *)
let execute_pure_transformation
    (reg : registry)
    (op_name : string)
    (inputs : (string * string) list) : ((string * string) list * execution_metrics, string) result =
  let t0 = Unix.gettimeofday () in
  match find reg op_name with
  | None ->
      Error (Printf.sprintf "Unknown operation '%s': not found in native registry" op_name)
  | Some op ->
      match op.handler inputs with
      | Error err -> Error err
      | Ok outputs ->
          let t1 = Unix.gettimeofday () in
          let duration_ms = (t1 -. t0) *. 1000.0 in
          let bytes_processed = List.fold_left (fun acc (_, v) -> acc + String.length v) 0 inputs in
          let metrics = {
            duration_ms;
            llm_calls = 0; (* STRICT: Exactly 0 LLM calls for native registered operations *)
            bytes_processed;
          } in
          Ok (outputs, metrics)
