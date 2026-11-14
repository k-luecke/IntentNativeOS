(* tests/test_parameter.ml - Test suite for Task I005: Experimental vs Binding Parameters *)

open Inos_ir.Mode
open Inos_ir.Contract
open Inos_ir.Parameter

let assert_true msg cond =
  if not cond then (
    Printf.eprintf "[FAIL] %s\n" msg;
    exit 1
  ) else
    Printf.printf "[PASS] %s\n" msg

let test_role_classification () =
  let p_ex = make_parameter ~name:"latency_target" ~value:"10ms" ~origin:User_selected_example ~binding:Example in
  assert_true "Example binding classifies as Experimental" (p_ex.role = Experimental);

  let p_pref = make_parameter ~name:"theme" ~value:"dark" ~origin:User_explicit ~binding:Preference in
  assert_true "Preference binding classifies as Experimental" (p_pref.role = Experimental);

  let p_con = make_parameter ~name:"memory_limit" ~value:"512MB" ~origin:User_explicit ~binding:Constraint in
  assert_true "Constraint binding classifies as Binding" (p_con.role = Binding);

  let p_act = make_parameter ~name:"target_binary" ~value:"solver.elf" ~origin:User_explicit ~binding:Authorized_action in
  assert_true "Authorized_action binding classifies as Binding" (p_act.role = Binding)

let test_latency_as_example_never_authorizes_solver_edits () =
  (* Spec 3.2: Latency selected solely as an exploration experiment *)
  let latency_param = make_parameter
    ~name:"latency"
    ~value:"MIN_LATENCY"
    ~origin:User_selected_example
    ~binding:Example
  in
  let params = [latency_param] in

  (* In Explore mode: mutating solver edits must fail *)
  let result_explore = validate_requested_effects
    ~mode:Explore
    ~parameters:params
    ~requested_effects:[ReadContext; ModifySolver]
  in
  (match result_explore with
   | Error (ConsequentialEffectInExploratoryMode _) ->
       assert_true "Explore mode rejects ModifySolver" true
   | Error _ -> assert_true "Expected ConsequentialEffectInExploratoryMode" false
   | Ok _ -> assert_true "Explore mode allowed solver edits!" false);

  (* Even in Execute mode: latency as example MUST NEVER authorize ModifySolver *)
  let result_execute = validate_requested_effects
    ~mode:Execute
    ~parameters:params
    ~requested_effects:[ReadContext; ModifySolver]
  in
  (match result_execute with
   | Error (ExperimentalParameterCannotAuthorizeMutation { parameter_name; forbidden_effect }) ->
       assert_true "Execute mode blocks ModifySolver when latency is experimental"
         (parameter_name = "latency" && forbidden_effect = "MODIFY_SOLVER")
   | Error _ -> assert_true "Expected ExperimentalParameterCannotAuthorizeMutation error" false
   | Ok _ -> assert_true "Latency-as-example authorized solver edits!" false);

  (* LaunchOptimization also blocked *)
  let result_launch = validate_requested_effects
    ~mode:Execute
    ~parameters:params
    ~requested_effects:[LaunchOptimization]
  in
  (match result_launch with
   | Error (ExperimentalParameterCannotAuthorizeMutation _) ->
       assert_true "Execute mode blocks LaunchOptimization when latency is experimental" true
   | Error _ -> assert_true "Expected ExperimentalParameterCannotAuthorizeMutation error" false
   | Ok _ -> assert_true "Latency-as-example authorized LaunchOptimization!" false)

let test_read_and_design_effects_allowed_for_experimental () =
  let latency_param = make_parameter
    ~name:"latency"
    ~value:"MIN_LATENCY"
    ~origin:User_selected_example
    ~binding:Example
  in
  let result = validate_requested_effects
    ~mode:Explore
    ~parameters:[latency_param]
    ~requested_effects:[ReadContext; GenerateDesignArtifact]
  in
  match result with
  | Ok effs ->
      assert_true "ReadContext and GenerateDesignArtifact allowed in Explore mode" (List.length effs = 2)
  | Error _ -> assert_true "Exploratory effects unexpectedly failed" false

let test_binding_parameter_authorizes_edits_in_execute_mode () =
  let binding_param = make_parameter
    ~name:"optimization_target"
    ~value:"solver_inner_loop"
    ~origin:User_explicit
    ~binding:Constraint
  in
  let result = validate_requested_effects
    ~mode:Execute
    ~parameters:[binding_param]
    ~requested_effects:[ReadContext; ModifySolver]
  in
  match result with
  | Ok effs ->
      assert_true "Binding constraint authorizes solver edits in Execute mode" (List.length effs = 2)
  | Error _ -> assert_true "Binding parameter failed to authorize in Execute mode" false

let () =
  Printf.printf "=== Running Experimental vs Binding Parameter Test Suite (I005) ===\n";
  test_role_classification ();
  test_latency_as_example_never_authorizes_solver_edits ();
  test_read_and_design_effects_allowed_for_experimental ();
  test_binding_parameter_authorizes_edits_in_execute_mode ();
  Printf.printf "=== ALL PARAMETER & EFFECT TESTS PASSED ===\n"
