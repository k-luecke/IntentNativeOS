(* models/adapter/code_sandbox.ml - Separation of Proposed Code from Executable Operations *)

open Inos_ir.Authority

type proposed_code = {
  proposal_id : string;
  language : string;
  raw_code : string;
  claimed_intent : string;
}

type typed_operation =
  | PureTransform of { input_hash : string; output_hash : string }
  | StagedFileWrite of { target_path : string; content : string }
  | TestRunnerInvocation of { test_suite : string }

type executable_operation = {
  operation_id : string;
  op_type : typed_operation;
  required_capability : string;
  grant_token : authority_token;
}

type execution_error =
  | ExecutionBlockedAbsentValidatedOperation of string
  | UnverifiedCapabilityGrant of string
  | DirectShellExecutionForbidden of string

(* Core Invariant / Task R007:
   Generated shell string cannot execute absent validated operation.
   Raw code or shell commands are untrusted data and cannot be executed directly. *)
let attempt_direct_shell_execution (raw_shell : string) : (unit, execution_error) result =
  Error (ExecutionBlockedAbsentValidatedOperation
    (Printf.sprintf "Security violation: Generated shell string '%s' cannot execute absent validated operation"
       (String.sub raw_shell 0 (min (String.length raw_shell) 40))))

let validate_and_lower_operation
    ~(proposal : proposed_code)
    ~(token : authority_token)
    ~(required_cap : string) : (executable_operation, execution_error) result =
  if not (List.mem required_cap token.scope) then
    Error (UnverifiedCapabilityGrant
      (Printf.sprintf "Token scope does not grant required capability '%s'" required_cap))
  else
    (* Convert untrusted proposal into structured, typed operation *)
    let op_type = match proposal.language with
      | "ocaml" | "rust" -> StagedFileWrite { target_path = "/staging/code.ml"; content = proposal.raw_code }
      | "test" -> TestRunnerInvocation { test_suite = proposal.claimed_intent }
      | other ->
          PureTransform {
            input_hash = Inos_ipc.Sha256.digest_string other;
            output_hash = Inos_ipc.Sha256.digest_string proposal.raw_code;
          }
    in
    Ok {
      operation_id = "op-" ^ proposal.proposal_id;
      op_type;
      required_capability = required_cap;
      grant_token = token;
    }
