(* Offline Recovery, Replacement Phone and Revocation (A007) *)
open Types

type recovery_system = {
  verifier : Assertion.verifier_state;
  threshold : int;
  valid_share_hashes : (int, string) Hashtbl.t;
}

let init_recovery (verifier : Assertion.verifier_state) (shares : (int * string) list) (threshold : int) : recovery_system =
  let tbl = Hashtbl.create 8 in
  List.iter (fun (idx, secret) ->
    let h = Sha256.digest_string (Printf.sprintf "SHARE:%d:%s" idx secret) in
    Hashtbl.replace tbl idx h
  ) shares;
  { verifier; threshold; valid_share_hashes = tbl }

let revoke_device (verifier : Assertion.verifier_state) (key_id : string) : bool =
  match Hashtbl.find_opt verifier.enrolled_devices key_id with
  | None -> false
  | Some dev ->
      dev.revoked <- true;
      true

type recovery_error =
  | InsufficientShares of int * int
  | InvalidShare of int
  | EnrollmentFailed of string

let recover_and_replace_device
    (rec_sys : recovery_system)
    ~(lost_key_id : string)
    ~(new_key_id : string)
    ~(new_pubkey : string)
    ~(presented_shares : (int * string) list)
    ~(timestamp : int64)
    : (enrolled_device, recovery_error) result =
  if List.length presented_shares < rec_sys.threshold then
    Error (InsufficientShares (List.length presented_shares, rec_sys.threshold))
  else
    (* Validate presented recovery shares *)
    let shares_valid = List.for_all (fun (idx, secret) ->
      match Hashtbl.find_opt rec_sys.valid_share_hashes idx with
      | None -> false
      | Some expected_h ->
          let actual_h = Sha256.digest_string (Printf.sprintf "SHARE:%d:%s" idx secret) in
          String.equal actual_h expected_h
    ) presented_shares in
    if not shares_valid then
      Error (InvalidShare 0)
    else begin
      (* Revoke the lost device *)
      ignore (revoke_device rec_sys.verifier lost_key_id);
      (* Enroll the new replacement phone *)
      Assertion.enroll_device rec_sys.verifier ~key_id:new_key_id ~pubkey:new_pubkey ~enrolled_at:timestamp;
      match Hashtbl.find_opt rec_sys.verifier.enrolled_devices new_key_id with
      | Some dev -> Ok dev
      | None -> Error (EnrollmentFailed "Could not retrieve newly enrolled device")
    end
