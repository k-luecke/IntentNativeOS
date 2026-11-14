(* tools/supervisor/backlog.ml - Backlog CSV loading, dependency graphing, and export *)

open Types

let parse_csv_line line =
  let len = String.length line in
  let rec parse_fields i in_quotes current acc =
    if i >= len then
      List.rev (current :: acc)
    else
      match line.[i] with
      | '"' -> parse_fields (i + 1) (not in_quotes) current acc
      | ',' when not in_quotes -> parse_fields (i + 1) false "" (current :: acc)
      | c -> parse_fields (i + 1) in_quotes (current ^ String.make 1 c) acc
  in
  parse_fields 0 false "" []

let parse_depends_on str =
  let clean = String.trim str in
  if clean = "" then []
  else
    String.split_on_char ',' clean
    |> List.map String.trim
    |> List.filter (fun s -> s <> "")

let status_of_string s evidence =
  match String.lowercase_ascii (String.trim s) with
  | "done" -> Accepted { evidence_ref = (match evidence with Some e -> e | None -> "recorded"); accepted_at = 0.0 }
  | "blocked" -> Blocked { reason = "Loaded as blocked"; attempts = 3 }
  | "leased" -> Ready
  | _ -> Ready

let string_of_status = function
  | Ready -> "Ready"
  | Leased _ -> "Leased"
  | Implemented _ -> "Implemented"
  | Verifying -> "Verifying"
  | Accepted _ -> "Done"
  | Blocked _ -> "Blocked"

let load_from_csv filepath =
  let tasks = Hashtbl.create 150 in
  if not (Sys.file_exists filepath) then tasks
  else
    let ic = open_in filepath in
    try
      let _header = input_line ic in (* skip header *)
      let rec loop () =
        match input_line ic with
        | line ->
            (match parse_csv_line line with
             | id :: gate :: area :: task_title :: depends_raw :: done_when :: status_raw :: _owner :: evidence_raw :: notes :: _ ->
                 let id = String.trim id in
                 let ev_opt = if String.trim evidence_raw = "" then None else Some (String.trim evidence_raw) in
                 let status = status_of_string status_raw ev_opt in
                 let t = {
                   id;
                   gate = String.trim gate;
                   area = String.trim area;
                   title = String.trim task_title;
                   depends_on = parse_depends_on depends_raw;
                   done_when = String.trim done_when;
                   status;
                   attempts = 0;
                   evidence = ev_opt;
                   notes = String.trim notes;
                 } in
                 Hashtbl.replace tasks id t
             | _ -> ());
            loop ()
        | exception End_of_file -> ()
      in
      loop ();
      close_in ic;
      tasks
    with e ->
      close_in_noerr ic;
      raise e

let export_to_csv tasks filepath =
  let oc = open_out filepath in
  output_string oc "ID,Gate,Area,Task,Depends_on,Done_when,Status,Owner,Evidence,Notes\n";
  (* Sort tasks in deterministic order *)
  let task_list = Hashtbl.fold (fun _ t acc -> t :: acc) tasks [] in
  let sorted = List.sort (fun a b -> String.compare a.id b.id) task_list in
  List.iter (fun t ->
    let deps = String.concat "," t.depends_on in
    let deps_str = if String.contains deps ',' then Printf.sprintf "\"%s\"" deps else deps in
    let title_str = if String.contains t.title ',' then Printf.sprintf "\"%s\"" t.title else t.title in
    let done_str = if String.contains t.done_when ',' then Printf.sprintf "\"%s\"" t.done_when else t.done_when in
    let ev_str = match t.evidence with Some e -> e | None -> "" in
    let line = Printf.sprintf "%s,%s,%s,%s,%s,%s,%s,Supervisor,%s,%s\n"
      t.id t.gate t.area title_str deps_str done_str (string_of_status t.status) ev_str t.notes in
    output_string oc line
  ) sorted;
  close_out oc
