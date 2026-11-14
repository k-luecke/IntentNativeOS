(* compiler/ir/resolver.ml - Contextual Reference Resolver for Files, Directories, and Projects *)

open Contract

type workspace_context = {
  active_project : string option;
  selected_files : string list;
  selected_directory : string option;
  focused_symbol : string option;
}

type resolution_result =
  | Resolved of target_ref
  | Ambiguous of { phrase : string; candidates : string list; explanation : string }
  | Unresolved of { phrase : string; explanation : string }

let empty_context = {
  active_project = None;
  selected_files = [];
  selected_directory = None;
  focused_symbol = None;
}

let normalize_phrase s =
  String.lowercase_ascii (String.trim s)

(* Core Invariant / Task I006:
   'this file' resolves ONLY with unambiguous single selection.
   Empty selection -> Unresolved.
   Multiple selection -> Ambiguous. *)
let resolve_reference (ctx : workspace_context) (phrase : string) : resolution_result =
  let norm = normalize_phrase phrase in
  match norm with
  | "this file" | "current file" | "selected file" | "the file" ->
      (match ctx.selected_files with
       | [] ->
           Unresolved {
             phrase;
             explanation = "No file is currently selected; 'this file' cannot be resolved";
           }
       | [single_file] ->
           Resolved (FileTarget single_file)
       | candidates ->
           Ambiguous {
             phrase;
             candidates;
             explanation = Printf.sprintf "Multiple files selected (%s); 'this file' is ambiguous" (String.concat ", " candidates);
           })
  | "active project" | "this project" | "current project" | "the project" ->
      (match ctx.active_project with
       | None ->
           Unresolved {
             phrase;
             explanation = "No active project is currently open";
           }
       | Some proj ->
           Resolved (ProjectTarget proj))
  | "this directory" | "current directory" | "selected directory" | "the folder" ->
      (match ctx.selected_directory with
       | None ->
           Unresolved {
             phrase;
             explanation = "No directory is currently selected";
           }
       | Some dir ->
           Resolved (DirectoryTarget dir))
  | s when String.starts_with ~prefix:"file:" s ->
      Resolved (FileTarget (String.sub phrase 5 (String.length phrase - 5)))
  | s when String.starts_with ~prefix:"dir:" s ->
      Resolved (DirectoryTarget (String.sub phrase 4 (String.length phrase - 4)))
  | s when String.starts_with ~prefix:"project:" s ->
      Resolved (ProjectTarget (String.sub phrase 8 (String.length phrase - 8)))
  | raw ->
      if String.starts_with ~prefix:"/" raw || String.contains raw '/' || String.contains raw '\\' then
        Resolved (FileTarget raw)
      else
        Unresolved {
          phrase = raw;
          explanation = Printf.sprintf "Unrecognized reference phrase '%s'" raw;
        }
