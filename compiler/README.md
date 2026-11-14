# compiler/ (Trust Boundary T2)

Intent-Native Compiler pipeline: input parsing, typed intent intermediate representation (IR), material ambiguity resolution, clarification gate state machine, and capability selection.

- **Language:** OCaml 5.x
- **Subdirectories:**
  - `ir/`: Typed Intent IR, Plan IR, Operation IR.
  - `gate/`: Clarification gate state machine and consequence evaluator.
  - `codegen/`: Lowering to native capability invocations and reversible operations.
