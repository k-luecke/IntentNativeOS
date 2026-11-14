(* tests/test_neural_engine.ml - Unit Test Suite for Genuine Neural Tensor Inference Engine (GAP-G7-03a) *)

open Inos_models.Neural_engine
open Inos_models.Protocol

let test_gelu_numerical () =
  (* Known values for GELU: gelu(0) == 0, gelu(1) ~ 0.8413, gelu(-1) ~ -0.1587 *)
  let g0 = gelu 0.0 in
  assert (abs_float g0 < 1e-6);
  let g1 = gelu 1.0 in
  assert (abs_float (g1 -. 0.8413) < 0.01);
  let gm1 = gelu (-1.0) in
  assert (abs_float (gm1 -. (-0.1587)) < 0.01);
  Printf.printf "  [PASS] GELU activation numerical values verified.\n"

let test_softmax_sum () =
  let logits = [| 2.0; 1.0; 0.1; -1.0; 3.0 |] in
  let probs = softmax logits in
  let sum = Array.fold_left (+.) 0.0 probs in
  assert (abs_float (sum -. 1.0) < 1e-6);
  assert (probs.(4) > probs.(0)); (* highest logit has highest prob *)
  Printf.printf "  [PASS] Softmax probability distribution sums to 1.0.\n"

let test_matvec_flops () =
  let lw = {
    dim_in = 4;
    dim_out = 3;
    weights = [| [| 1.0; 0.0; 0.0; 0.0 |];
                 [| 0.0; 1.0; 0.0; 0.0 |];
                 [| 0.0; 0.0; 1.0; 0.0 |] |];
    biases = [| 0.5; 0.5; 0.5 |];
    lora = None;
  } in
  let x = [| 2.0; 3.0; 4.0; 5.0 |] in
  let (y, flops) = matvec_mul lw x in
  assert (abs_float (y.(0) -. 2.5) < 1e-6);
  assert (abs_float (y.(1) -. 3.5) < 1e-6);
  assert (abs_float (y.(2) -. 4.5) < 1e-6);
  assert (flops = 27L); (* 3 * (2*4 + 1) = 27 *)
  Printf.printf "  [PASS] Matrix-vector multiplication and FLOP counting verified.\n"

let test_end_to_end_inference () =
  let prompt = "Create a document summarizing the changes we have made to this operating system" in
  let req = {
    request_id = "req-test-001";
    model_name = "inos-neural-tensor-v1";
    prompt;
    max_tokens = 64;
    temperature = 0.0;
    timeout_seconds = 5.0;
  } in
  match NeuralTensorAdapter.invoke_buffered req with
  | Error _ -> failwith "Inference failed unexpectedly"
  | Ok resp ->
      Printf.printf "  [PASS] Forward-pass completed in %.3f ms (%d tokens, model: %s)\n"
        resp.duration_ms resp.total_tokens resp.model_name;
      Printf.printf "  [PASS] Output: %s\n" resp.full_text;
      assert (resp.total_tokens > 5);
      assert (String.length resp.full_text > 10)

let () =
  Printf.printf "=== Running Test Suite: Numerical Neural Tensor Inference Engine ===\n";
  test_gelu_numerical ();
  test_softmax_sum ();
  test_matvec_flops ();
  test_end_to_end_inference ();
  Printf.printf "=== [ALL PASS] GAP-G7-03a Neural Execution Verified ===\n"
