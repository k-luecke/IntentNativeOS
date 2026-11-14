(* compiler/operation/operation_ir.ml - Typed Operation IR with Explicit Inputs, Outputs, and Effects *)

type data_type =
  | TypeString
  | TypeBytes
  | TypeFileUri
  | TypeDirectoryUri
  | TypeHashDigest
  | TypeExitCode
  | TypeCustom of string

type effect =
  | Pure
  | ReadFs of string
  | WriteFs of string
  | MutateFs of string
  | ExecuteBinary of string
  | AllocateMemory of int

type port = {
  name : string;
  data_type : data_type;
  description : string;
}

type precondition =
  | PathExists of string
  | TargetWritable of string
  | HashMatches of { path : string; expected_hash : string }
  | MemoryAvailable of int

type operation_node = {
  node_id : string;
  op_name : string;
  inputs : port list;
  outputs : port list;
  effects : effect list;
  preconditions : precondition list;
  required_capabilities : string list;
}

type edge = {
  from_node : string;
  from_port : string;
  to_node : string;
  to_port : string;
}

type operation_graph = {
  graph_id : string;
  nodes : operation_node list;
  edges : edge list;
}

type validation_error =
  | SourceNodeNotFound of string
  | TargetNodeNotFound of string
  | SourcePortNotFound of { node_id : string; port : string }
  | TargetPortNotFound of { node_id : string; port : string }
  | TypeMismatchOnEdge of {
      from_node : string;
      from_port : string;
      out_type : string;
      to_node : string;
      to_port : string;
      in_type : string;
    }

let string_of_data_type = function
  | TypeString -> "STRING"
  | TypeBytes -> "BYTES"
  | TypeFileUri -> "FILE_URI"
  | TypeDirectoryUri -> "DIRECTORY_URI"
  | TypeHashDigest -> "HASH_DIGEST"
  | TypeExitCode -> "EXIT_CODE"
  | TypeCustom s -> "CUSTOM:" ^ s

let are_types_compatible (t1 : data_type) (t2 : data_type) : bool =
  match (t1, t2) with
  | (TypeString, TypeString) -> true
  | (TypeBytes, TypeBytes) -> true
  | (TypeFileUri, TypeFileUri) -> true
  | (TypeDirectoryUri, TypeDirectoryUri) -> true
  | (TypeHashDigest, TypeHashDigest) -> true
  | (TypeExitCode, TypeExitCode) -> true
  | (TypeCustom a, TypeCustom b) -> a = b
  | (TypeFileUri, TypeString) -> true (* FileUri can be read as a path string *)
  | _ -> false

let find_node (nodes : operation_node list) (id : string) =
  List.find_opt (fun n -> n.node_id = id) nodes

let find_port (ports : port list) (name : string) =
  List.find_opt (fun p -> p.name = name) ports

(* Core Invariant / Task C001:
   Illegal input/output edges rejected. Type mismatches, dangling edges,
   or undefined ports fail closed. *)
let validate_edge (nodes : operation_node list) (edge : edge) : (unit, validation_error) result =
  match find_node nodes edge.from_node with
  | None -> Error (SourceNodeNotFound edge.from_node)
  | Some src_node ->
      match find_node nodes edge.to_node with
      | None -> Error (TargetNodeNotFound edge.to_node)
      | Some tgt_node ->
          match find_port src_node.outputs edge.from_port with
          | None -> Error (SourcePortNotFound { node_id = edge.from_node; port = edge.from_port })
          | Some out_p ->
              match find_port tgt_node.inputs edge.to_port with
              | None -> Error (TargetPortNotFound { node_id = edge.to_node; port = edge.to_port })
              | Some in_p ->
                  if not (are_types_compatible out_p.data_type in_p.data_type) then
                    Error (TypeMismatchOnEdge {
                      from_node = edge.from_node;
                      from_port = edge.from_port;
                      out_type = string_of_data_type out_p.data_type;
                      to_node = edge.to_node;
                      to_port = edge.to_port;
                      in_type = string_of_data_type in_p.data_type;
                    })
                  else
                    Ok ()

let validate_graph (graph : operation_graph) : (unit, validation_error) result =
  let rec check_edges = function
    | [] -> Ok ()
    | e :: rest ->
        match validate_edge graph.nodes e with
        | Ok () -> check_edges rest
        | Error err -> Error err
  in
  check_edges graph.edges
