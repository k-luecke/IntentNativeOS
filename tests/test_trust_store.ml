(* tests/test_trust_store.ml - Test suite for Task M003: Trust classifications and overwrite protection *)

open Inos_ir.Provenance
open Inos_memory.Trust_store

let assert_true msg cond =
  if not cond then (
    Printf.eprintf "[FAIL] %s\n" msg;
    exit 1
  ) else
    Printf.printf "[PASS] %s\n" msg

let test_provisional_cannot_overwrite_authoritative () =
  let st = create () in
  (* Record authoritative system constraint *)
  let auth_res = put st
    ~key:"security.t0.isolation"
    ~content:"T0 kernel memory is isolated from T2 model plane"
    ~trust_class:Authoritative
    ~producer:"kernel-policy"
    ()
  in
  (match auth_res with
   | Ok a -> assert_true "Authoritative constraint successfully stored" (a.trust_class = Authoritative)
   | Error _ -> assert_true "Failed to store authoritative constraint" false);

  (* Model B attempts to put provisional assertion over the authoritative constraint *)
  let prov_res = put st
    ~key:"security.t0.isolation"
    ~content:"Model B claims shared buffers can bypass T0 isolation"
    ~trust_class:Provisional
    ~producer:"model-b"
    ()
  in
  match prov_res with
  | Error (CannotOverwriteAuthoritativeConstraint { key; existing_class; attempted_class; _ }) ->
      assert_true "Provisional model assertion cannot overwrite authoritative constraint"
        (key = "security.t0.isolation" && existing_class = Authoritative && attempted_class = Provisional);
      let current = get st "security.t0.isolation" in
      (match current with
       | Some c -> assert_true "Authoritative constraint remains unchanged" (c.trust_class = Authoritative)
       | None -> assert_true "Constraint was deleted!" false)
  | Error _ -> assert_true "Expected CannotOverwriteAuthoritativeConstraint error" false
  | Ok _ -> assert_true "Provisional assertion illegally overwrote authoritative constraint!" false

let test_provisional_cannot_overwrite_verified () =
  let st = create () in
  let _ = put st
    ~key:"benchmark.cold_start"
    ~content:"Boot time measured at 42ms"
    ~trust_class:Verified
    ~producer:"ci-runner"
    ()
  in
  let prov_res = put st
    ~key:"benchmark.cold_start"
    ~content:"Model A estimates boot time is 10ms"
    ~trust_class:Provisional
    ~producer:"model-a"
    ()
  in
  match prov_res with
  | Error (CannotOverwriteVerifiedFactWithProvisional _) ->
      assert_true "Provisional assertion cannot overwrite verified fact" true
  | _ -> assert_true "Provisional assertion overwrote verified fact!" false

let test_authoritative_can_supersede () =
  let st = create () in
  let _ = put st
    ~key:"sys.policy"
    ~content:"v1 policy"
    ~trust_class:Authoritative
    ~producer:"admin"
    ()
  in
  let res = put st
    ~key:"sys.policy"
    ~content:"v2 policy updated"
    ~trust_class:Authoritative
    ~producer:"admin"
    ()
  in
  match res with
  | Ok a ->
      assert_true "Authoritative policy can be updated by authoritative producer"
        (a.revision = 2 && a.content = "v2 policy updated")
  | Error _ -> assert_true "Authoritative update failed" false

let () =
  Printf.printf "=== Running Trust Classifications & Overwrite Invariant Suite (M003) ===\n";
  test_provisional_cannot_overwrite_authoritative ();
  test_provisional_cannot_overwrite_verified ();
  test_authoritative_can_supersede ();
  Printf.printf "=== ALL TRUST STORE TESTS PASSED ===\n"
