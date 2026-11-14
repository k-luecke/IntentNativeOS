(* Algorithm-Agile Crypto Registry and Security Profiles (A008) *)
open Types

type suite_metadata = {
  suite_id : crypto_suite_id;
  name : string;
  is_post_quantum : bool;
  security_level_bits : int;
}

let suites_table = [
  (Suite_Ed25519_Sha256, {
    suite_id = Suite_Ed25519_Sha256;
    name = "Ed25519-SHA256";
    is_post_quantum = false;
    security_level_bits = 128;
  });
  (Suite_ML_DSA_65_Sha3, {
    suite_id = Suite_ML_DSA_65_Sha3;
    name = "ML-DSA-65-SHA3 (Dilithium-3)";
    is_post_quantum = true;
    security_level_bits = 192;
  });
  (Suite_ML_KEM_768, {
    suite_id = Suite_ML_KEM_768;
    name = "ML-KEM-768 (Kyber-768)";
    is_post_quantum = true;
    security_level_bits = 192;
  });
]

let get_suite_metadata (suite : crypto_suite_id) : suite_metadata option =
  List.assoc_opt suite suites_table

let is_suite_permitted (profile : security_profile) (suite : crypto_suite_id) : (suite_metadata, string) result =
  match get_suite_metadata suite with
  | None -> Error "Unknown or unregistered cryptographic suite"
  | Some meta ->
      match profile with
      | Legacy_Compatibility -> Ok meta
      | Standard_Modern -> Ok meta
      | Post_Quantum_Strict ->
          if meta.is_post_quantum then Ok meta
          else Error (Printf.sprintf "Security Policy Violation: Suite %s is non-post-quantum and rejected under Post_Quantum_Strict policy (Downgrade protection)" meta.name)
