(* core/memory/trust_store.ml - Trust-Classified Store and Invariant M003 *)

open Inos_ir.Provenance

type stored_assertion = {
  key : string;
  content : string;
  trust_class : trust_class;
  producer : string;
  updated_at : float;
  revision : int;
}

type trust_store = {
  table : (string, stored_assertion) Hashtbl.t;
}

type store_error =
  | CannotOverwriteAuthoritativeConstraint of {
      key : string;
      existing_class : trust_class;
      attempted_class : trust_class;
      producer : string;
    }
  | CannotOverwriteVerifiedFactWithProvisional of {
      key : string;
      producer : string;
    }
  | StaleRevision of { key : string; current_rev : int; attempted_rev : int }

let create () = {
  table = Hashtbl.create 64;
}

let trust_rank = function
  | Authoritative -> 4
  | Verified -> 3
  | Provisional -> 2
  | Superseded -> 1

let get (st : trust_store) (key : string) : stored_assertion option =
  try Some (Hashtbl.find st.table key) with Not_found -> None

(* Core Invariant / Task M003:
   Provisional model assertion cannot overwrite authoritative constraint *)
let put
    (st : trust_store)
    ?(updated_at = Unix.gettimeofday ())
    ~key
    ~content
    ~trust_class
    ~producer
    () : (stored_assertion, store_error) result =
  match get st key with
  | None ->
      let assertion = {
        key;
        content;
        trust_class;
        producer;
        updated_at;
        revision = 1;
      } in
      Hashtbl.replace st.table key assertion;
      Ok assertion
  | Some existing ->
      let prod_lower = String.lowercase_ascii producer in
      let is_untrusted_producer =
        String.starts_with ~prefix:"model" prod_lower ||
        String.starts_with ~prefix:"untrusted" prod_lower ||
        String.starts_with ~prefix:"guest" prod_lower
      in
      if existing.trust_class = Authoritative && trust_class = Provisional then
        Error (CannotOverwriteAuthoritativeConstraint {
          key;
          existing_class = Authoritative;
          attempted_class = Provisional;
          producer;
        })
      else if existing.trust_class = Verified && trust_class = Provisional then
        Error (CannotOverwriteVerifiedFactWithProvisional {
          key;
          producer;
        })
      else if trust_rank trust_class < trust_rank existing.trust_class ||
              ((existing.trust_class = Authoritative || existing.trust_class = Verified) && is_untrusted_producer) then
        Error (CannotOverwriteAuthoritativeConstraint {
          key;
          existing_class = existing.trust_class;
          attempted_class = trust_class;
          producer;
        })
      else
        let updated = {
          key;
          content;
          trust_class;
          producer;
          updated_at;
          revision = existing.revision + 1;
        } in
        Hashtbl.replace st.table key updated;
        Ok updated
