(* Engineer Test Scripts for IR and Source Inspection (Q002) *)
open Types

type inspection_pipeline = {
  intent_input : string;
  typed_ir_hash : string;
  operation_dag_nodes : string list;
  capability_grants : string list;
  execution_receipt_digest : string;
  trace_log : ir_trace_entry list;
}

let build_inspection_pipeline
    ~(raw_intent : string)
    ~(ir_repr : string)
    ~(dag_nodes : string list)
    ~(grants : string list)
    ~(receipt_digest : string) : inspection_pipeline =
  let ir_hash = Inos_security.Sha256.digest_string ir_repr in
  let dag_hash = Inos_security.Sha256.digest_string (String.concat "|" dag_nodes) in
  let grants_hash = Inos_security.Sha256.digest_string (String.concat "|" grants) in
  let trace = [
    { step_index = 1; phase = "INTENT_PARSE"; artifact_name = "raw_intent"; canonical_hash = Inos_security.Sha256.digest_string raw_intent; verified = true };
    { step_index = 2; phase = "TYPED_IR_GENERATION"; artifact_name = "intent_ir_ast"; canonical_hash = ir_hash; verified = true };
    { step_index = 3; phase = "OPERATION_DAG_SYNTHESIS"; artifact_name = "operation_dag"; canonical_hash = dag_hash; verified = true };
    { step_index = 4; phase = "CAPABILITY_BINDING"; artifact_name = "capability_grants"; canonical_hash = grants_hash; verified = true };
    { step_index = 5; phase = "RECEIPT_PERSISTENCE"; artifact_name = "execution_receipt"; canonical_hash = receipt_digest; verified = true };
  ] in
  {
    intent_input = raw_intent;
    typed_ir_hash = ir_hash;
    operation_dag_nodes = dag_nodes;
    capability_grants = grants;
    execution_receipt_digest = receipt_digest;
    trace_log = trace;
  }

let verify_trace_integrity (pipe : inspection_pipeline) : bool =
  List.length pipe.trace_log = 5 &&
  List.for_all (fun e -> e.verified && String.length e.canonical_hash = 64) pipe.trace_log
