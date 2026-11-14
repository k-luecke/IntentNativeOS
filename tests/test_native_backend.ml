(* tests/test_native_backend.ml - Native-code backend prototype acceptance tests (C007) *)

open Inos_operation.Native_backend

let assert_true name cond =
  if cond then Printf.printf "[PASS] %s\n" name
  else (Printf.printf "[FAIL] %s\n" name; exit 1)

let test_pure_function_compilation_and_vectors () =
  (* Define a pure function:
     compute_metric(x, y, mask) =
       if mask == 0 then (x + y) * 31
       else (x ^ y) << 2 - 7
  *)
  let test_func = {
    name = "compute_metric";
    params = ["x"; "y"; "mask"];
    body =
      IfZero (
        Var "mask",
        BinOp (Mul, BinOp (Add, Var "x", Var "y"), Const 31L),
        BinOp (Sub, BinOp (Shl, BinOp (Xor, Var "x", Var "y"), Const 2L), Const 7L)
      );
  } in

  let tmp_bin = Filename.temp_file "inos_pure_bin" "" in
  (try Sys.remove tmp_bin with _ -> ());

  (* 1. Compile native binary *)
  match compile_native test_func ~out_bin:tmp_bin with
  | Error err ->
      Printf.printf "Compilation failed: %s\n" err;
      assert_true "Compile native binary" false
  | Ok () ->
      assert_true "Native binary compiled cleanly by gcc" true;

      (* 2. Test vectors covering zero, positives, bitwise XOR, shifts, and sign overflow *)
      let test_vectors = [
        ([0L; 0L; 0L], "mask=0 branch zeros");
        ([10L; 20L; 0L], "mask=0 branch (10+20)*31 = 930");
        ([5L; 12L; 1L], "mask=1 branch ((5^12)<<2) - 7");
        ([1024L; 2048L; 0L], "mask=0 branch large values");
        ([255L; 15L; 99L], "mask!=0 branch mixed values");
        ([123456789L; 987654321L; 42L], "mask!=0 branch large integers");
      ] in

      List.iter (fun (args, desc) ->
        let (matches, ref_res, nat_res) = verify_vector test_func tmp_bin args in
        assert_true
          (Printf.sprintf "Vector '%s': ref=%Ld native=%Ld" desc ref_res nat_res)
          (matches && ref_res = nat_res)
      ) test_vectors;

      (* Cleanup temporary files *)
      (try Sys.remove tmp_bin with _ -> ());
      (try Sys.remove (tmp_bin ^ ".c") with _ -> ())

let () =
  Printf.printf "=== Running Native Backend Prototype Test Suite (C007) ===\n";
  test_pure_function_compilation_and_vectors ();
  Printf.printf "=== ALL NATIVE BACKEND TESTS PASSED ===\n"
