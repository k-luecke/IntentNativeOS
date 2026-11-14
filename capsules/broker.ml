(* capsules/broker.ml - File Broker with Document Grants and Clipboard Isolation (V009, V010) *)

open Types

type broker_session = {
  capsule_id : capsule_id;
  active_grants : file_grant list;
  clipboard_buffer : string option;
}

let create_session cid grants =
  {
    capsule_id = cid;
    active_grants = grants;
    clipboard_buffer = None;
  }

let check_file_access session req_path is_write =
  let matching = List.find_opt (fun g -> g.relative_path = req_path) session.active_grants in
  match matching with
  | Some grant ->
      if is_write then grant.write_allowed else grant.read_allowed
  | None ->
      (* Path traversal or ungranted document access denied fail-closed *)
      false

let attempt_clipboard_read session (scope : authority_scope) =
  if scope.clipboard_read then
    Ok session.clipboard_buffer
  else
    Error "Forbidden: Guest capsule denied clipboard read permission"

let attempt_clipboard_write session (scope : authority_scope) data =
  if scope.clipboard_write then
    Ok { session with clipboard_buffer = Some data }
  else
    Error "Forbidden: Guest capsule denied clipboard write permission"
