(* compiler/operation/dag_validator.ml - Dependency DAG Validator, Cycle Detection, and Topological Sorting *)

open Operation_ir

type color = Unvisited | Visiting | Visited

type dag_error =
  | CyclicDependencyDetected of {
      cycle_path : string list;
      diagnostic : string;
    }
  | DanglingReference of string

(* Core Invariant / Task C003:
   Cyclic plan rejected with actionable diagnostic. Topological sort computes valid execution order. *)
let validate_and_order_dag (graph : operation_graph) : (string list, dag_error) result =
  (* Build adjacency list: node_id -> set of successor node_ids *)
  let node_map = Hashtbl.create 32 in
  let adj = Hashtbl.create 32 in
  List.iter (fun n ->
    Hashtbl.replace node_map n.node_id n;
    Hashtbl.replace adj n.node_id []
  ) graph.nodes;

  let dangling_err = ref None in
  List.iter (fun (e : edge) ->
    if !dangling_err <> None then ()
    else if not (Hashtbl.mem node_map e.from_node) then
      dangling_err := Some (DanglingReference (Printf.sprintf "Edge references non-existent from_node '%s'" e.from_node))
    else if not (Hashtbl.mem node_map e.to_node) then
      dangling_err := Some (DanglingReference (Printf.sprintf "Edge references non-existent to_node '%s'" e.to_node))
    else
      let existing = Hashtbl.find adj e.from_node in
      if not (List.mem e.to_node existing) then
        Hashtbl.replace adj e.from_node (e.to_node :: existing)
  ) graph.edges;

  match !dangling_err with
  | Some err -> Error err
  | None ->

  let marks = Hashtbl.create 32 in
  List.iter (fun n -> Hashtbl.replace marks n.node_id Unvisited) graph.nodes;

  let sorted_nodes = ref [] in
  let path_stack = ref [] in
  let cycle_found = ref None in

  let rec visit node_id =
    match Hashtbl.find marks node_id with
    | Visited -> Ok ()
    | Visiting ->
        (* Cycle detected! Reconstruct path from stack *)
        let full_stack = List.rev (node_id :: !path_stack) in
        let cycle_path = full_stack in
        let diagnostic = Printf.sprintf
          "Cyclic dependency detected: %s. Operations cannot execute in circular order."
          (String.concat " -> " cycle_path)
        in
        cycle_found := Some (CyclicDependencyDetected { cycle_path; diagnostic });
        Error ()
    | Unvisited ->
        Hashtbl.replace marks node_id Visiting;
        path_stack := node_id :: !path_stack;
        let neighbors = try Hashtbl.find adj node_id with Not_found -> [] in
        let rec loop_neighbors = function
          | [] -> Ok ()
          | neighbor :: rest ->
              match visit neighbor with
              | Ok () -> loop_neighbors rest
              | Error () -> Error ()
        in
        let res = loop_neighbors neighbors in
        path_stack := List.tl !path_stack;
        Hashtbl.replace marks node_id Visited;
        if res = Ok () then sorted_nodes := node_id :: !sorted_nodes;
        res
  in

  let rec visit_all = function
    | [] -> Ok ()
    | n :: rest ->
        if Hashtbl.find marks n.node_id = Unvisited then
          match visit n.node_id with
          | Ok () -> visit_all rest
          | Error () -> Error ()
        else
          visit_all rest
  in

  match visit_all graph.nodes with
  | Ok () ->
      (* Topological sort: post-order reversed *)
      Ok !sorted_nodes
  | Error () ->
      match !cycle_found with
      | Some err -> Error err
      | None -> Error (CyclicDependencyDetected { cycle_path = []; diagnostic = "Cycle detected in graph" })
