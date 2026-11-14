(* compiler/ir/provenance.ml - Epistemic Status, Trust Classes, and Provenance without Permission Conflation *)

type trust_class =
  | Authoritative
  | Verified
  | Provisional
  | Superseded

type epistemic_status =
  | Fact
  | Hypothesis
  | Inference
  | Observation

type confidence = {
  score : float;
  sample_count : int option;
  calibration_method : string option;
}

type provenance_record = {
  id : string;
  schema_version : string;
  producer : string;
  trust_class : trust_class;
  epistemic_status : epistemic_status;
  confidence : confidence option;
  source_refs : string list;
  created_at : float;
  validity : string;
  content_hash : string;
  parent_refs : string list;
}

type approved_fact = {
  id : string;
  statement : string;
  trust_class : trust_class;
  provenance : provenance_record;
  verification_receipt : string;
}

let string_of_trust_class = function
  | Authoritative -> "AUTHORITATIVE"
  | Verified -> "VERIFIED"
  | Provisional -> "PROVISIONAL"
  | Superseded -> "SUPERSEDED"

let trust_class_of_string = function
  | "AUTHORITATIVE" -> Ok Authoritative
  | "VERIFIED" -> Ok Verified
  | "PROVISIONAL" -> Ok Provisional
  | "SUPERSEDED" -> Ok Superseded
  | s -> Error (Printf.sprintf "Invalid trust class: '%s'" s)

let string_of_epistemic_status = function
  | Fact -> "FACT"
  | Hypothesis -> "HYPOTHESIS"
  | Inference -> "INFERENCE"
  | Observation -> "OBSERVATION"

let epistemic_status_of_string = function
  | "FACT" -> Ok Fact
  | "HYPOTHESIS" -> Ok Hypothesis
  | "INFERENCE" -> Ok Inference
  | "OBSERVATION" -> Ok Observation
  | s -> Error (Printf.sprintf "Invalid epistemic status: '%s'" s)

let make_confidence ?sample_count ?calibration_method score =
  if score < 0.0 || score > 1.0 then
    Error (Printf.sprintf "Confidence score %f must be in range [0.0, 1.0]" score)
  else
    Ok { score; sample_count; calibration_method }

let compute_provenance_hash ~id ~producer ~trust_class ~epistemic_status ~created_at =
  let content = Printf.sprintf "%s|%s|%s|%s|%.4f"
    id producer (string_of_trust_class trust_class) (string_of_epistemic_status epistemic_status) created_at in
  Inos_ipc.Sha256.digest_string content

let make_provenance
    ~id
    ?(schema_version = "inos.provenance.v0.1")
    ~producer
    ~trust_class
    ~epistemic_status
    ?confidence
    ?(source_refs = [])
    ?(created_at = Unix.gettimeofday ())
    ?(validity = "ACTIVE")
    ?(parent_refs = [])
    () =
  let content_hash = compute_provenance_hash ~id ~producer ~trust_class ~epistemic_status ~created_at in
  {
    id;
    schema_version;
    producer;
    trust_class;
    epistemic_status;
    confidence;
    source_refs;
    created_at;
    validity;
    content_hash;
    parent_refs;
  }

(* Core Invariant I-04: Hypothesis cannot deserialize as approved fact.
   Epistemic confidence (model belief) does NOT grant permission or factual certainty.
   Even a hypothesis with 1.0 confidence cannot be deserialized as an approved fact. *)
type deserialization_error =
  | CannotDeserializeHypothesisAsFact of string
  | ProvisionalStatusRejected of string
  | MissingVerificationReceipt
  | InvalidTrustClassForFact of trust_class
  | MalformedRecord of string

let deserialize_approved_fact
    ~id
    ~statement
    ~trust_class_str
    ~epistemic_status_str
    ~confidence_score
    ~verification_receipt
    ~producer
    ~source_refs
    ~created_at
    ~parent_refs =
  match epistemic_status_of_string epistemic_status_str with
  | Error msg -> Error (MalformedRecord msg)
  | Ok Hypothesis ->
      Error (CannotDeserializeHypothesisAsFact
        (Printf.sprintf "Hypothesis cannot deserialize as approved fact (producer=%s, confidence=%.2f)"
           producer (match confidence_score with Some s -> s | None -> 0.0)))
  | Ok Inference when verification_receipt = "" ->
      Error (CannotDeserializeHypothesisAsFact "Inference without verification receipt cannot deserialize as approved fact")
  | Ok (Fact | Observation | Inference) ->
      match trust_class_of_string trust_class_str with
      | Error msg -> Error (MalformedRecord msg)
      | Ok Provisional ->
          Error (ProvisionalStatusRejected
            (Printf.sprintf "Provisional record '%s' cannot be deserialized as approved fact without formal verification" id))
      | Ok Superseded ->
          Error (MalformedRecord (Printf.sprintf "Superseded record '%s' cannot be deserialized as an active approved fact" id))
      | Ok (Authoritative | Verified as valid_class) ->
          if verification_receipt = "" && valid_class = Verified then
            Error MissingVerificationReceipt
          else
            let conf = match confidence_score with
              | Some s -> Some { score = s; sample_count = None; calibration_method = None }
              | None -> None
            in
            let prov = make_provenance
              ~id
              ~producer
              ~trust_class:valid_class
              ~epistemic_status:Fact
              ?confidence:conf
              ~source_refs
              ~created_at
              ~parent_refs
              ()
            in
            Ok {
              id;
              statement;
              trust_class = valid_class;
              provenance = prov;
              verification_receipt;
            }

(* Explicit verification pipeline: promote provisional hypothesis to verified fact
   ONLY upon presenting a valid, non-empty verification receipt and verifier identity. *)
let promote_to_verified
    ~(receipt : string)
    ~(verifier : string)
    (provisional : provenance_record)
    (statement : string) : (approved_fact, string) result =
  if receipt = "" then
    Error "Promotion rejected: verification receipt cannot be empty"
  else if verifier = "" then
    Error "Promotion rejected: verifier identity cannot be empty"
  else
    let verified_prov = make_provenance
      ~id:(provisional.id ^ ".verified")
      ~producer:verifier
      ~trust_class:Verified
      ~epistemic_status:Fact
      ~confidence:{ score = 1.0; sample_count = Some 1; calibration_method = Some "DETERMINISTIC_VERIFICATION" }
      ~source_refs:[provisional.id]
      ~parent_refs:[provisional.content_hash]
      ()
    in
    Ok {
      id = provisional.id ^ ".fact";
      statement;
      trust_class = Verified;
      provenance = verified_prov;
      verification_receipt = receipt;
    }
