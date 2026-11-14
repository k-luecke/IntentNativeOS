(* compiler/ir/authority.ml - Authority Tokens, Scope Verification, and Approval State Transitions *)

open Mode

type authority_token = {
  token_id : string;
  issuer : string; (* T1 capability broker or human operator *)
  granted_to : string; (* Intent ID *)
  scope : string list;
  parent_contract_hash : string;
  issued_at : float;
  valid_until : float;
  signature : string;
}

type approval_state =
  | Unapproved
  | AwaitingApproval of {
      requested_scope : string list;
      requested_at : float;
      justification : string;
    }
  | Authorized of authority_token
  | Rejected of {
      reason : string;
      rejected_at : float;
    }
  | Expired of {
      expired_at : float;
    }

type transition_error =
  | MissingAuthorityGrant of string
  | ScopeMismatch of { requested : string list; granted : string list }
  | ContractHashMismatch of { expected : string; found : string }
  | TokenExpired of { valid_until : float; now : float }
  | InvalidTokenSignature of string
  | IllegalStateTransition of string

let string_of_approval_state = function
  | Unapproved -> "UNAPPROVED"
  | AwaitingApproval _ -> "AWAITING_APPROVAL"
  | Authorized _ -> "AUTHORIZED"
  | Rejected _ -> "REJECTED"
  | Expired _ -> "EXPIRED"

let compute_token_signature ~secret ~token_id ~issuer ~granted_to ~scope ~contract_hash ~valid_until =
  let scope_str = String.concat "," (List.sort String.compare scope) in
  let payload = Printf.sprintf "%s|%s|%s|%s|%s|%s|%.4f"
    secret token_id issuer granted_to scope_str contract_hash valid_until in
  Inos_ipc.Sha256.digest_string payload

let make_token
    ~token_id
    ~issuer
    ~granted_to
    ~scope
    ~parent_contract_hash
    ?(issued_at = Unix.gettimeofday ())
    ~valid_until
    ~secret
    () =
  let signature = compute_token_signature ~secret ~token_id ~issuer ~granted_to ~scope ~contract_hash:parent_contract_hash ~valid_until in
  {
    token_id;
    issuer;
    granted_to;
    scope;
    parent_contract_hash;
    issued_at;
    valid_until;
    signature;
  }

let verify_token ~(secret : string) (token : authority_token) (contract_hash : string) (now : float) =
  if now > token.valid_until then
    Error (TokenExpired { valid_until = token.valid_until; now })
  else if token.parent_contract_hash <> contract_hash then
    Error (ContractHashMismatch { expected = contract_hash; found = token.parent_contract_hash })
  else
    let expected_sig = compute_token_signature
      ~secret
      ~token_id:token.token_id
      ~issuer:token.issuer
      ~granted_to:token.granted_to
      ~scope:token.scope
      ~contract_hash:token.parent_contract_hash
      ~valid_until:token.valid_until
    in
    if expected_sig <> token.signature then
      Error (InvalidTokenSignature "Cryptographic signature does not match token payload")
    else
      Ok ()

let is_scope_covered ~(required : string list) ~(granted : string list) =
  List.for_all (fun req -> List.mem req granted) required

(* Core Invariant: Simulated request cannot transition to authorized without grant *)
let transition_to_authorized
    ~(secret : string)
    ~(token_opt : authority_token option)
    ~(contract_id : string)
    ~(contract_hash : string)
    ~(required_scope : string list)
    ~(current_mode : mode)
    ~(current_approval : approval_state)
    ?(now = Unix.gettimeofday ())
    () : (approval_state * mode, transition_error) result =
  match current_approval with
  | Rejected { reason; _ } ->
      Error (IllegalStateTransition (Printf.sprintf "Cannot authorize rejected contract: %s" reason))
  | Expired _ ->
      Error (IllegalStateTransition "Cannot authorize expired contract")
  | Unapproved | AwaitingApproval _ | Authorized _ ->
        match token_opt with
        | None ->
            Error (MissingAuthorityGrant
              (Printf.sprintf "Simulated or pending request for '%s' cannot transition to authorized without grant" contract_id))
        | Some token ->
      match verify_token ~secret token contract_hash now with
      | Error err -> Error err
      | Ok () ->
          if not (is_scope_covered ~required:required_scope ~granted:token.scope) then
            Error (ScopeMismatch { requested = required_scope; granted = token.scope })
          else if token.granted_to <> contract_id then
            Error (ContractHashMismatch { expected = contract_id; found = token.granted_to })
          else
            (* Transition authorized: mode can now be promoted to Execute if desired *)
            let new_mode = if current_mode = Simulate then Execute else current_mode in
            Ok (Authorized token, new_mode)
