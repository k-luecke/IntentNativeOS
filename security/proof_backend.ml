(* Pluggable Proof Backend Interface (A010) *)
open Types

module type PROOF_BACKEND = sig
  val backend_kind : proof_backend_kind
  val name : string
  val prove : statement:string -> witness:string -> proof_token
  val verify : proof:proof_token -> statement:string -> bool
end

(* Modular arithmetic over Fermat prime field Z_p (p = 65537, multiplicative order q = 65536, primitive root g = 3) *)
let zk_prime_p = 65537
let zk_order_q = 65536
let zk_gen_g = 3

let pow_mod (base : int) (exp : int) (m : int) : int =
  let rec loop b e acc =
    if e <= 0 then acc
    else
      let acc' = if e land 1 = 1 then (acc * b) mod m else acc in
      loop ((b * b) mod m) (e lsr 1) acc'
  in
  loop ((base mod m + m) mod m) exp 1

let hex_prefix_to_int (hex : string) : int =
  let n = min 7 (String.length hex) in
  let acc = ref 0 in
  for i = 0 to n - 1 do
    let c = hex.[i] in
    let v =
      if c >= '0' && c <= '9' then Char.code c - Char.code '0'
      else if c >= 'a' && c <= 'f' then Char.code c - Char.code 'a' + 10
      else if c >= 'A' && c <= 'F' then Char.code c - Char.code 'A' + 10
      else 0
    in
    acc := (!acc lsl 4) lor v
  done;
  !acc

module SignatureBackend : PROOF_BACKEND = struct
  let backend_kind = SignatureProofBackend
  let name = "Schnorr Signature-Backed Proof Backend"

  let prove ~statement ~witness =
    let x_hash = Sha256.digest_string ("SIG_PRIV_SCALAR|" ^ witness) in
    let x = (hex_prefix_to_int x_hash mod (zk_order_q - 1)) + 1 in
    let pub_y = pow_mod zk_gen_g x zk_prime_p in
    let k_hash = Sha256.digest_string ("SIG_NONCE_K|" ^ witness ^ "|" ^ statement) in
    let k = (hex_prefix_to_int k_hash mod (zk_order_q - 1)) + 1 in
    let r_com = pow_mod zk_gen_g k zk_prime_p in
    let e_hash = Sha256.digest_string (Printf.sprintf "SIG_CHAL_E|Y:%d|R:%d|STMT:%s" pub_y r_com statement) in
    let e = (hex_prefix_to_int e_hash mod 32767) + 1 in
    let s = (k + e * x) mod zk_order_q in
    let payload = Printf.sprintf "SIG_SCHNORR_V2|%d|%d|%d" pub_y r_com s in
    { backend = backend_kind; statement; proof_payload = payload }

  let verify ~proof ~statement =
    if not (String.equal proof.statement statement) then false
    else
      match String.split_on_char '|' proof.proof_payload with
      | [ "SIG_SCHNORR_V2"; y_str; r_str; s_str ] -> (
          try
            let pub_y = int_of_string y_str in
            let r_com = int_of_string r_str in
            let s = int_of_string s_str in
            if pub_y <= 0 || pub_y >= zk_prime_p || r_com <= 0 || r_com >= zk_prime_p || s < 0 || s >= zk_order_q then
              false
            else
              let e_hash =
                Sha256.digest_string (Printf.sprintf "SIG_CHAL_E|Y:%d|R:%d|STMT:%s" pub_y r_com statement)
              in
              let e = (hex_prefix_to_int e_hash mod 32767) + 1 in
              let lhs = pow_mod zk_gen_g s zk_prime_p in
              let rhs = (r_com * pow_mod pub_y e zk_prime_p) mod zk_prime_p in
              lhs = rhs
          with Failure _ -> false)
      | _ -> false
end

module MockZkBackend : PROOF_BACKEND = struct
  let backend_kind = MockZkBackend
  let name = "Non-Interactive Schnorr Fiat-Shamir ZK Proof Backend"

  let prove ~statement ~witness =
    (* 1. Derive secret witness scalar w in [1, q-1] and public key y = g^w mod p *)
    let w_hash = Sha256.digest_string ("ZK_WITNESS_SCALAR|" ^ witness) in
    let w = (hex_prefix_to_int w_hash mod (zk_order_q - 1)) + 1 in
    let y = pow_mod zk_gen_g w zk_prime_p in
    (* 2. Derive deterministic ephemeral nonce r in [1, q-1] and commitment t = g^r mod p *)
    let r_hash = Sha256.digest_string ("ZK_EPHEMERAL_NONCE|" ^ witness ^ "|" ^ statement) in
    let r = (hex_prefix_to_int r_hash mod (zk_order_q - 1)) + 1 in
    let t = pow_mod zk_gen_g r zk_prime_p in
    (* 3. Fiat-Shamir non-interactive challenge c = H(y, t, statement) *)
    let c_hash =
      Sha256.digest_string (Printf.sprintf "ZK_FS_CHAL|Y:%d|T:%d|STMT:%s" y t statement)
    in
    let c = (hex_prefix_to_int c_hash mod 32767) + 1 in
    (* 4. Schnorr response s = (r + c * w) mod q *)
    let s = (r + c * w) mod zk_order_q in
    let bind_tag =
      Sha256.digest_string
        (Printf.sprintf "ZK_BIND|Y:%d|T:%d|S:%d|C:%d|STMT:%s" y t s c statement)
    in
    let proof_payload = Printf.sprintf "SCHNORR_ZK_V1|%d|%d|%d|%s" y t s bind_tag in
    { backend = backend_kind; statement; proof_payload }

  let verify ~proof ~statement =
    if not (String.equal proof.statement statement) then false
    else
      match String.split_on_char '|' proof.proof_payload with
      | [ "SCHNORR_ZK_V1"; y_str; t_str; s_str; bind_tag ] -> (
          try
            let y = int_of_string y_str in
            let t = int_of_string t_str in
            let s = int_of_string s_str in
            if y <= 0 || y >= zk_prime_p || t <= 0 || t >= zk_prime_p || s < 0 || s >= zk_order_q then
              false
            else
              let c_hash =
                Sha256.digest_string (Printf.sprintf "ZK_FS_CHAL|Y:%d|T:%d|STMT:%s" y t statement)
              in
              let c = (hex_prefix_to_int c_hash mod 32767) + 1 in
              let expected_bind =
                Sha256.digest_string
                  (Printf.sprintf "ZK_BIND|Y:%d|T:%d|S:%d|C:%d|STMT:%s" y t s c statement)
              in
              if not (String.equal bind_tag expected_bind) then false
              else
                (* Schnorr verification identity: g^s == t * y^c (mod p) *)
                let lhs = pow_mod zk_gen_g s zk_prime_p in
                let rhs = (t * pow_mod y c zk_prime_p) mod zk_prime_p in
                lhs = rhs
          with Failure _ -> false)
      | _ -> false
end

let get_backend (kind : proof_backend_kind) : (module PROOF_BACKEND) =
  match kind with
  | SignatureProofBackend -> (module SignatureBackend)
  | MockZkBackend -> (module MockZkBackend)
  | SuccinctSnarkBackend -> (module MockZkBackend)

let verify_generic_proof (proof : proof_token) (statement : string) : bool =
  let (module B) = get_backend proof.backend in
  B.verify ~proof ~statement

