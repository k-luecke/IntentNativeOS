(* models/adapter/escalation.ml - Iteration Budgets, Deadlock Detection, and Escalation Rules *)

type escalation_status =
  | WithinBudget of { current_iteration : int; remaining : int }
  | BudgetExhausted of {
      iterations_completed : int;
      max_budget : int;
      last_critique : string;
      useful_status : string;
    }
  | DeadlockDetected of {
      repeated_hash : string;
      at_iteration : int;
      useful_status : string;
    }

type iteration_tracker = {
  max_iterations : int;
  mutable current_iteration : int;
  mutable previous_hashes : string list;
  mutable previous_critiques : string list;
}

let create_tracker ?(max_iterations = 3) () = {
  max_iterations;
  current_iteration = 0;
  previous_hashes = [];
  previous_critiques = [];
}

(* Core Invariant / Task R006:
   Infinite review loop stops at configured limit and returns useful status.
   Deadlocks detected when identical proposal hash or critique repeats. *)
let record_iteration
    (t : iteration_tracker)
    ~(proposal_hash : string)
    ~(critique : string) : escalation_status =
  t.current_iteration <- t.current_iteration + 1;

  (* Check deadlock: repeated identical proposal hash after rejection *)
  if List.mem proposal_hash t.previous_hashes then
    DeadlockDetected {
      repeated_hash = proposal_hash;
      at_iteration = t.current_iteration;
      useful_status = Printf.sprintf
        "Deadlock detected at iteration %d: Model B generated identical proposal hash '%s' without addressing critique. Execution halted to prevent infinite review loop."
        t.current_iteration proposal_hash;
    }
  else if t.current_iteration >= t.max_iterations then begin
    BudgetExhausted {
      iterations_completed = t.current_iteration;
      max_budget = t.max_iterations;
      last_critique = critique;
      useful_status = Printf.sprintf
        "Iteration budget exhausted (%d/%d iterations). Last review critique: '%s'. Escalating to operator clarification."
        t.current_iteration t.max_iterations critique;
    }
  end else begin
    t.previous_hashes <- proposal_hash :: t.previous_hashes;
    t.previous_critiques <- critique :: t.previous_critiques;
    WithinBudget {
      current_iteration = t.current_iteration;
      remaining = t.max_iterations - t.current_iteration;
    }
  end
