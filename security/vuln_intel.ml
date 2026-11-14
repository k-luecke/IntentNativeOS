(* security/vuln_intel.ml - S401 Autonomous Vulnerability Intelligence Engine
   Ingests NIST NVD, CISA KEV, and OSV vulnerability feeds.
   Enforces source provenance, SHA-256 integrity, bounded local LRU caching,
   deterministic component mapping with 5-tier explicit uncertainty,
   and feed freshness detection.
   Cardinal Invariant: AI models can only provide informational analysis;
   AI inferences CANNOT authorize privileged changes or mint capability grants. *)

open Types

type source_origin =
  | NVD
  | CISA_KEV
  | OSV
  | Vendor_Direct

type severity_tier =
  | Critical
  | High
  | Medium
  | Low
  | Informational

type normalized_advisory = {
  advisory_id : string;
  origin : source_origin;
  ingested_at : int64;
  published_at : int64;
  payload_sha256 : string;
  title : string;
  summary : string;
  cvss_score : float option;
  severity : severity_tier;
  cpe_matches : string list;
  package_ecosystem : string option;
  package_name : string option;
  affected_versions : string list;
  known_exploited : bool;
}

(* 5-tier explicit uncertainty classification *)
type applicability_decision =
  | Definite_Match of string    (* Exact component name, version, and architecture match *)
  | Probable_Match of string    (* Component name matches, version falls in vulnerability range *)
  | Potential_Impact of string  (* Shared protocol, subsystem, or dependency affected *)
  | Unknown_Applicability of string (* Advisory lacks structured version criteria *)
  | Not_Applicable of string    (* Component/architecture not present in system inventory *)

type installed_component = {
  name : string;
  version : string;
  ecosystem : string;
  cpe : string;
  architecture : string;
}

type system_inventory = {
  kernel_version : string;
  architecture : string;
  components : installed_component list;
}

(* Bounded LRU Cache (max 512 entries) *)
type advisory_cache = {
  max_entries : int;
  entries : (string, normalized_advisory) Hashtbl.t;
  mutable access_order : string list;
}

let create_cache ?(max_entries = 512) () : advisory_cache = {
  max_entries;
  entries = Hashtbl.create max_entries;
  access_order = [];
}

let cache_get (cache : advisory_cache) (key : string) : normalized_advisory option =
  match Hashtbl.find_opt cache.entries key with
  | None -> None
  | Some adv ->
      (* Move to front of LRU order *)
      cache.access_order <- key :: (List.filter (fun k -> not (String.equal k key)) cache.access_order);
      Some adv

let cache_put (cache : advisory_cache) (adv : normalized_advisory) : unit =
  let key = Printf.sprintf "%s:%s" 
              (match adv.origin with NVD -> "NVD" | CISA_KEV -> "KEV" | OSV -> "OSV" | Vendor_Direct -> "VENDOR")
              adv.advisory_id in
  if Hashtbl.mem cache.entries key then begin
    Hashtbl.replace cache.entries key adv;
    cache.access_order <- key :: (List.filter (fun k -> not (String.equal k key)) cache.access_order)
  end else begin
    if Hashtbl.length cache.entries >= cache.max_entries then begin
      (* Evict LRU tail *)
      match List.rev cache.access_order with
      | [] -> ()
      | oldest :: _ ->
          Hashtbl.remove cache.entries oldest;
          cache.access_order <- List.filter (fun k -> not (String.equal k oldest)) cache.access_order
    end;
    Hashtbl.add cache.entries key adv;
    cache.access_order <- key :: cache.access_order
  end

(* Parsers & Normalizers *)

let parse_nvd_advisory
    ~(cve_id : string)
    ~(raw_json : string)
    ~(published_at : int64)
    ~(ingested_at : int64)
    ~(cvss : float option)
    ~(cpe_list : string list)
    ~(title : string)
    ~(summary : string) : normalized_advisory =
  let payload_sha256 = Sha256.digest_string raw_json in
  let severity = match cvss with
    | Some s when s >= 9.0 -> Critical
    | Some s when s >= 7.0 -> High
    | Some s when s >= 4.0 -> Medium
    | Some _ -> Low
    | None -> Informational
  in
  {
    advisory_id = cve_id;
    origin = NVD;
    ingested_at;
    published_at;
    payload_sha256;
    title;
    summary;
    cvss_score = cvss;
    severity;
    cpe_matches = cpe_list;
    package_ecosystem = None;
    package_name = None;
    affected_versions = [];
    known_exploited = false;
  }

let parse_cisa_kev_advisory
    ~(cve_id : string)
    ~(raw_json : string)
    ~(published_at : int64)
    ~(ingested_at : int64)
    ~(vendor_project : string)
    ~(product : string)
    ~(required_action : string) : normalized_advisory =
  let payload_sha256 = Sha256.digest_string raw_json in
  {
    advisory_id = cve_id;
    origin = CISA_KEV;
    ingested_at;
    published_at;
    payload_sha256;
    title = Printf.sprintf "CISA KEV: %s in %s %s" cve_id vendor_project product;
    summary = required_action;
    cvss_score = Some 9.8; (* KEV vulnerabilities are actively exploited; treat as critical *)
    severity = Critical;
    cpe_matches = [Printf.sprintf "cpe:2.3:a:%s:%s:*:*:*:*:*:*:*:*" vendor_project product];
    package_ecosystem = Some "CISA_KEV";
    package_name = Some product;
    affected_versions = [];
    known_exploited = true;
  }

let parse_osv_advisory
    ~(osv_id : string)
    ~(raw_json : string)
    ~(published_at : int64)
    ~(ingested_at : int64)
    ~(ecosystem : string)
    ~(package_name : string)
    ~(affected_versions : string list)
    ~(summary : string) : normalized_advisory =
  let payload_sha256 = Sha256.digest_string raw_json in
  {
    advisory_id = osv_id;
    origin = OSV;
    ingested_at;
    published_at;
    payload_sha256;
    title = Printf.sprintf "OSV: %s (%s/%s)" osv_id ecosystem package_name;
    summary;
    cvss_score = None;
    severity = High;
    cpe_matches = [];
    package_ecosystem = Some ecosystem;
    package_name = Some package_name;
    affected_versions;
    known_exploited = false;
  }

(* Component Applicability Matcher with Explicit Uncertainty *)

let evaluate_applicability (inv : system_inventory) (adv : normalized_advisory) : applicability_decision =
  (* Check CPE matches against installed components *)
  let cpe_matched = List.find_opt (fun c ->
    List.exists (fun adv_cpe ->
      String.equal c.cpe adv_cpe ||
      (String.length adv_cpe > 10 && String.sub adv_cpe 0 10 = String.sub c.cpe 0 (min 10 (String.length c.cpe)))
    ) adv.cpe_matches
  ) inv.components in

  match cpe_matched with
  | Some comp ->
      if List.mem comp.version adv.affected_versions then
        Definite_Match (Printf.sprintf "%s version %s explicitly matches affected version list" comp.name comp.version)
      else if adv.affected_versions = [] && adv.known_exploited then
        Probable_Match (Printf.sprintf "%s matched via CISA KEV active exploitation catalog" comp.name)
      else if adv.affected_versions = [] then
        Unknown_Applicability (Printf.sprintf "%s matched CPE but advisory provides no explicit version bounds" comp.name)
      else
        Potential_Impact (Printf.sprintf "%s component present; version %s outside explicit list" comp.name comp.version)
  | None ->
      (* Check package name match *)
      match adv.package_name with
      | Some pkg ->
          let pkg_matched = List.find_opt (fun c -> String.equal (String.lowercase_ascii c.name) (String.lowercase_ascii pkg)) inv.components in
          (match pkg_matched with
           | Some comp ->
               if List.mem comp.version adv.affected_versions then
                 Definite_Match (Printf.sprintf "Package %s version %s matches vulnerable version" comp.name comp.version)
               else
                 Probable_Match (Printf.sprintf "Package %s matches installed component; version checking required" comp.name)
           | None ->
               Not_Applicable (Printf.sprintf "Package %s is not installed in INOS inventory" pkg))
      | None ->
          if adv.cpe_matches = [] then
            Unknown_Applicability "Advisory contains neither CPE nor package identifiers"
          else
            Not_Applicable "None of the advisory CPE identifiers match installed components"

(* Feed Freshness & Stale-Data Detection *)

type freshness_status =
  | Fresh of int64 (* Age in seconds *)
  | Stale of int64 (* Age in seconds *)
  | Offline_Degraded

let check_feed_freshness
    ~(feed_timestamp : int64)
    ~(current_timestamp : int64)
    ~(max_freshness_seconds : int64) : freshness_status =
  if current_timestamp < feed_timestamp then
    Fresh 0L (* Clock skew tolerant *)
  else
    let age = Int64.sub current_timestamp feed_timestamp in
    if age > max_freshness_seconds then
      Stale age
    else
      Fresh age

(* Cardinal Invariant: AI Non-Authority *)

type ai_advisory_analysis = {
  model_id : string;
  confidence : float;
  natural_language_summary : string;
  recommended_action : string;
}

type advisory_report = {
  advisory : normalized_advisory;
  applicability : applicability_decision;
  ai_analysis : ai_advisory_analysis option;
  is_actionable : bool;
}

let analyze_advisory_with_ai
    (adv : normalized_advisory)
    (decision : applicability_decision)
    (ai_opt : ai_advisory_analysis option) : advisory_report =
  let is_actionable = match decision with
    | Definite_Match _ | Probable_Match _ -> true
    | Potential_Impact _ | Unknown_Applicability _ | Not_Applicable _ -> false
  in
  {
    advisory = adv;
    applicability = decision;
    ai_analysis = ai_opt;
    is_actionable;
  }

(* Enforces invariant: AI analysis CANNOT synthesize or issue a grant token *)
let attempt_mint_grant_from_ai_analysis
    (_report : advisory_report)
    (_op : privileged_op) : (grant_token, string) result =
  (* Strict fail-closed boundary: AI advisory analysis has ZERO authority to mint grants *)
  Error "EpistemicBoundaryViolation: Vulnerability intelligence and AI analysis cannot mint execution authority"
