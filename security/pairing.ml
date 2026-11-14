(* Offline Phone Pairing with Authenticated Key Agreement (A004) *)
open Types

let compute_sas (host_pubkey : string) (phone_pubkey : string) : string =
  let combined = Printf.sprintf "SAS:v1|HOST:%s|PHONE:%s" host_pubkey phone_pubkey in
  let digest = Sha256.digest_string combined in
  (* Take first 8 hex characters, parse as integer, modulo 1000000 for 6 decimal digits *)
  let hex_sub = String.sub digest 0 8 in
  let code_val = Int64.of_string ("0x" ^ hex_sub) in
  let six_digit = Int64.rem (Int64.abs code_val) 1000000L in
  Printf.sprintf "%06Ld" six_digit

type pairing_session = {
  host_id : string;
  host_ephemeral : ephemeral_keypair;
  mutable phone_ephemeral_pub : string option;
  mutable state : Types.caller_role option;
}

let initiate_pairing ~(host_id : string) ~(host_keys : ephemeral_keypair) ~(timestamp : int64) : pairing_request =
  {
    host_id;
    host_ephemeral_pubkey = host_keys.public_key;
    timestamp;
  }

let phone_respond_pairing
    ~(phone_id : string)
    ~(phone_keys : ephemeral_keypair)
    ~(req : pairing_request)
    : pairing_response =
  let sas = compute_sas req.host_ephemeral_pubkey phone_keys.public_key in
  {
    phone_id;
    phone_ephemeral_pubkey = phone_keys.public_key;
    sas_code = sas;
  }

type pairing_result =
  | PairingSuccess of { shared_secret: string; sas_code: string }
  | PairingRejectedMitm of { host_sas: string; phone_sas: string }

let finalize_pairing
    ~(host_pub : string)
    ~(host_priv : string)
    ~(received_phone_pub : string)
    ~(reported_phone_sas : string)
    : pairing_result =
  let host_computed_sas = compute_sas host_pub received_phone_pub in
  if not (String.equal host_computed_sas reported_phone_sas) then
    PairingRejectedMitm { host_sas = host_computed_sas; phone_sas = reported_phone_sas }
  else
    let shared = Sha256.digest_string (Printf.sprintf "PAIR_SECRET:%s:%s:%s" host_priv received_phone_pub host_computed_sas) in
    PairingSuccess { shared_secret = shared; sas_code = host_computed_sas }
