(* State-Verified Neural Network Cache (NNC) - Research & Inference Acceleration Module
   Resolves the "stale speculative drift" failure mode of unkeyed speculative caches
   by cryptographically binding cached neural activations, logit distributions, and
   verified intent classifications to:
     1. Normalized prompt / token prefix
     2. Workspace state SHA-256 digest (file tree + metadata generation)
     3. Capability security epoch
     4. Deterministic verifier pass attestation
*)

type cache_mode =
  | Disabled
  | UnkeyedSpeculative (* Legacy mode: keys by prompt only; vulnerable to stale state drift *)
  | StateVerified      (* Production-safe mode: keys by SHA-256(prompt | state_hash | epoch) *)

type config = {
  mode : cache_mode;
  max_entries : int;
}

let default_config = {
  mode = (match Sys.getenv_opt "INOS_ENABLE_NNC" with
          | Some "1" | Some "true" | Some "TRUE" | Some "verified" -> StateVerified
          | Some "speculative" -> UnkeyedSpeculative
          | _ -> Disabled);
  max_entries = 256;
}

type cached_entry = {
  cache_key : string;
  prompt_norm : string;
  workspace_state_hash : string;
  capability_epoch : int;
  logits : float array;
  classification_id : int;
  target_resource : string option;
  verified : bool;
  hidden_embedding : float array;
  entity_tags : string list;
  distilled_fact : string option;
  merkle_resource_hash : string option;
}

type lookup_result =
  | CacheHit of cached_entry
  | CacheMiss_NotFound
  | CacheMiss_StaleState of { cached_state : string; current_state : string }
  | CacheMiss_Disabled

type associative_recall_result =
  | AssociativeHit of {
      entry : cached_entry;
      cosine_similarity : float;
      entity_overlap : float;
      combined_score : float;
      recalled_fact : string;
      sparse_merkle_survived : bool;
    }
  | AssociativeMiss

type crdt_tombstone_tag = {
  resource_path : string;
  new_resource_hash : string;
  causal_epoch : int;
}

type stats = {
  mutable hits : int;
  mutable associative_hits : int;
  mutable sparse_merkle_survivors : int;
  mutable misses : int;
  mutable stale_evictions : int;
  mutable rejected_unverified : int;
  mutable crdt_retraction_batches : int;
  mutable causal_horizon_violations : int;
}

type t = {
  cfg : config;
  table : (string, cached_entry) Hashtbl.t;
  mutable tombstone_set : crdt_tombstone_tag list;
  stats : stats;
}

let create ?(cfg = default_config) () : t = {
  cfg;
  table = Hashtbl.create 64;
  tombstone_set = [];
  stats = {
    hits = 0;
    associative_hits = 0;
    sparse_merkle_survivors = 0;
    misses = 0;
    stale_evictions = 0;
    rejected_unverified = 0;
    crdt_retraction_batches = 0;
    causal_horizon_violations = 0;
  };
}

let normalize_prompt (s : string) : string =
  String.trim (String.lowercase_ascii s)

let is_recall_stopword (w : string) : bool =
  List.mem w [
    "the"; "and"; "for"; "with"; "that"; "this"; "from"; "into"; "when";
    "what"; "where"; "which"; "will"; "have"; "has"; "had"; "are"; "was";
    "were"; "can"; "could"; "should"; "would"; "please"; "about"; "tell";
    "show"; "give"; "recall"; "remember"; "find"; "check"; "run"; "too"
  ]

let extract_entity_tags (text : string) : string list =
  let len = String.length text in
  let acc = ref [] in
  let buf = Buffer.create 16 in
  let flush () =
    if Buffer.length buf > 0 then begin
      let w = Buffer.contents buf in
      Buffer.clear buf;
      if String.length w >= 3 && not (is_recall_stopword w) && not (List.mem w !acc) then
        acc := w :: !acc
    end
  in
  for i = 0 to len - 1 do
    let c = Char.lowercase_ascii text.[i] in
    if (c >= 'a' && c <= 'z') || (c >= '0' && c <= '9') || c = '_' || c = '.' || c = '-' then
      Buffer.add_char buf c
    else
      flush ()
  done;
  flush ();
  List.rev !acc

let extract_explicit_filename (tags : string list) : string option =
  List.find_opt (fun w ->
    String.contains w '.' && String.length w >= 4 && w.[0] <> '.'
  ) tags

let cosine_similarity (a : float array) (b : float array) : float =
  let len_a = Array.length a in
  let len_b = Array.length b in
  if len_a = 0 || len_b = 0 || len_a <> len_b then 0.0
  else begin
    (* Indices 0..4 in R^64 encode the 5 verb-intent axes (Create/Verify/Restore/Audit/Query);
       indices 5..63 encode the 59-dim domain & entity representation subspace. *)
    let start_idx = if len_a >= 16 then 5 else 0 in
    let count = float_of_int (len_a - start_idx) in
    let mean_a = ref 0.0 in
    let mean_b = ref 0.0 in
    for i = start_idx to len_a - 1 do
      mean_a := !mean_a +. a.(i);
      mean_b := !mean_b +. b.(i)
    done;
    let ma = !mean_a /. count in
    let mb = !mean_b /. count in
    let dot = ref 0.0 in
    let na = ref 0.0 in
    let nb = ref 0.0 in
    for i = start_idx to len_a - 1 do
      let da = a.(i) -. ma in
      let db = b.(i) -. mb in
      dot := !dot +. (da *. db);
      na := !na +. (da *. da);
      nb := !nb +. (db *. db)
    done;
    let denom = sqrt !na *. sqrt !nb in
    if denom <= 1e-8 then 0.0 else max 0.0 (!dot /. denom)
  end

let compute_key ~(mode : cache_mode) ~prompt_norm ~workspace_state_hash ~capability_epoch : string =
  match mode with
  | Disabled -> ""
  | UnkeyedSpeculative ->
      Inos_ipc.Sha256.digest_string prompt_norm
  | StateVerified ->
      Inos_ipc.Sha256.digest_string
        (Printf.sprintf "%s|%s|%d" prompt_norm workspace_state_hash capability_epoch)

(* FNWD V0.4 Enabler-Optimizer Composition (E_crdt (x) O_crdt):
   - O_crdt (`stage_tombstone_invalidation`): Lock-free O(1) staging of resource mutation
     tombstones without scanning the NNC table on every individual file edit.
   - E_crdt (`retract_tombstones_crdt`): Idempotent CRDT OR-Set tombstone compaction retraction
     executed at batch/read boundaries, guarded by the Causal Horizon Check
     (tombstones with causal_epoch < entry.capability_epoch are rejected to prevent
     out-of-order causal inversions). *)
let stage_tombstone_invalidation
    (cache : t)
    ~(resource_path : string)
    ~(new_resource_hash : string)
    ?(causal_epoch = 0)
    () : unit =
  cache.tombstone_set <-
    { resource_path; new_resource_hash; causal_epoch } :: cache.tombstone_set

let retract_tombstones_crdt (cache : t) : int =
  (* Manifold Stability Fast-Path: sigma in M ==> R(sigma) = sigma in O(1) *)
  match cache.tombstone_set with
  | [] -> 0
  | tombs ->
      cache.tombstone_set <- [];
      cache.stats.crdt_retraction_batches <- cache.stats.crdt_retraction_batches + 1;
      let to_remove = ref [] in
      Hashtbl.iter (fun k v ->
        match v.target_resource, v.merkle_resource_hash with
        | Some res, Some old_hash ->
            List.iter (fun tb ->
              if tb.resource_path = res && tb.new_resource_hash <> old_hash then begin
                (* Causal Horizon Guard (FNWD Failure Boundary #3):
                   Only retract if tombstone causal_epoch >= entry.capability_epoch *)
                if tb.causal_epoch > 0 && tb.causal_epoch < v.capability_epoch then
                  cache.stats.causal_horizon_violations <- cache.stats.causal_horizon_violations + 1
                else if not (List.mem k !to_remove) then
                  to_remove := k :: !to_remove
              end
            ) tombs
        | _ -> ()
      ) cache.table;
      List.iter (fun k ->
        Hashtbl.remove cache.table k;
        cache.stats.stale_evictions <- cache.stats.stale_evictions + 1
      ) !to_remove;
      List.length !to_remove

let lookup (cache : t) ~(prompt : string) ~(workspace_state_hash : string) ~(capability_epoch : int) : lookup_result =
  let _ = retract_tombstones_crdt cache in
  match cache.cfg.mode with
  | Disabled ->
      cache.stats.misses <- cache.stats.misses + 1;
      CacheMiss_Disabled
  | UnkeyedSpeculative ->
      let prompt_norm = normalize_prompt prompt in
      let key = compute_key ~mode:UnkeyedSpeculative ~prompt_norm ~workspace_state_hash ~capability_epoch in
      (match Hashtbl.find_opt cache.table key with
       | Some entry ->
           cache.stats.hits <- cache.stats.hits + 1;
           CacheHit { entry with logits = Array.copy entry.logits }
       | None ->
           cache.stats.misses <- cache.stats.misses + 1;
           CacheMiss_NotFound)
  | StateVerified ->
      let prompt_norm = normalize_prompt prompt in
      let key = compute_key ~mode:StateVerified ~prompt_norm ~workspace_state_hash ~capability_epoch in
      (* O(1) exact-key fast path first *)
      (match Hashtbl.find_opt cache.table key with
       | Some entry when entry.verified ->
           cache.stats.hits <- cache.stats.hits + 1;
           CacheHit { entry with logits = Array.copy entry.logits }
       | _ ->
           (* On exact-key miss, check if a stale-state entry exists for this prompt *)
           let stale_found = ref None in
           Hashtbl.iter (fun k v ->
             if v.prompt_norm = prompt_norm &&
                (v.workspace_state_hash <> workspace_state_hash || v.capability_epoch <> capability_epoch)
             then stale_found := Some (k, v.workspace_state_hash)
           ) cache.table;
           match !stale_found with
           | Some (old_key, old_state) ->
               Hashtbl.remove cache.table old_key;
               cache.stats.stale_evictions <- cache.stats.stale_evictions + 1;
               cache.stats.misses <- cache.stats.misses + 1;
               CacheMiss_StaleState { cached_state = old_state; current_state = workspace_state_hash }
           | None ->
               cache.stats.misses <- cache.stats.misses + 1;
               CacheMiss_NotFound)

let insert
    ?(hidden_embedding = [||])
    ?(entity_tags : string list option)
    ?(distilled_fact : string option)
    ?(merkle_resource_hash : string option)
    (cache : t)
    ~(prompt : string)
    ~(workspace_state_hash : string)
    ~(capability_epoch : int)
    ~(logits : float array)
    ~(classification_id : int)
    ~(target_resource : string option)
    ~(verified : bool) : bool =
  match cache.cfg.mode with
  | Disabled -> false
  | mode ->
      (* StateVerified mode strictly refuses to cache unverified or ambiguous proposals *)
      if mode = StateVerified && not verified then begin
        cache.stats.rejected_unverified <- cache.stats.rejected_unverified + 1;
        false
      end else begin
        let _ = retract_tombstones_crdt cache in
        if Hashtbl.length cache.table >= cache.cfg.max_entries then
          Hashtbl.clear cache.table;
        let prompt_norm = normalize_prompt prompt in
        let tags =
          match entity_tags with
          | Some t when t <> [] -> t
          | _ -> extract_entity_tags prompt_norm
        in
        let fact =
          match distilled_fact with
          | Some f -> Some f
          | None ->
              Some (Printf.sprintf "@nnc_mem[cls=%d,res=%s]{tags:[%s],state:%s}"
                classification_id
                (Option.value ~default:"global" target_resource)
                (String.concat "," tags)
                workspace_state_hash)
        in
        let cache_key = compute_key ~mode ~prompt_norm ~workspace_state_hash ~capability_epoch in
        let entry = {
          cache_key;
          prompt_norm;
          workspace_state_hash;
          capability_epoch;
          logits = Array.copy logits;
          classification_id;
          target_resource;
          verified;
          hidden_embedding = Array.copy hidden_embedding;
          entity_tags = tags;
          distilled_fact = fact;
          merkle_resource_hash;
        } in
        Hashtbl.replace cache.table cache_key entry;
        true
      end

(* Sparse Merkle Invalidation: Evict ONLY NNC nodes whose specific target_resource
   mutated to a different Merkle digest, preserving all other recalled facts & nodes. *)
let invalidate_sparse_merkle
    (cache : t)
    ~(resource_path : string)
    ~(new_resource_hash : string) : int =
  stage_tombstone_invalidation cache ~resource_path ~new_resource_hash ();
  retract_tombstones_crdt cache

(* Network-Native Computing (NNC) Associative Memory Recall:
   Recalls verified neural activations, logit priors, and distilled facts across
   paraphrased queries using hybrid hidden-state cosine similarity + entity overlap,
   with Sparse Merkle survival across unrelated workspace mutations. *)
let recall_associative
    (cache : t)
    ~(prompt : string)
    ~(query_embedding : float array)
    ~(workspace_state_hash : string)
    ~(capability_epoch : int)
    ?(min_score = 0.58)
    ?(resource_merkle_hashes : (string * string) list = [])
    () : associative_recall_result =
  let _ = retract_tombstones_crdt cache in
  if cache.cfg.mode = Disabled then AssociativeMiss
  else begin
    let q_norm = normalize_prompt prompt in
    let q_tags = extract_entity_tags q_norm in
    let q_file = extract_explicit_filename q_tags in
    let best_cand : (cached_entry * float * float * float * bool) option ref = ref None in
    Hashtbl.iter (fun _k (v : cached_entry) ->
      if v.verified && v.capability_epoch = capability_epoch then begin
        (* Check Merkle validity: either exact global workspace_state_hash match,
           OR sparse Merkle resource survival (target_resource hash unchanged or global fact) *)
        let (merkle_ok, survived_sparse) =
          if v.workspace_state_hash = workspace_state_hash then
            (true, false)
          else
            match v.target_resource, v.merkle_resource_hash with
            | None, _ ->
                (* Pure domain knowledge node not bound to a mutable file survives workspace edits *)
                (true, true)
            | Some res, Some r_hash ->
                (match List.assoc_opt res resource_merkle_hashes with
                 | Some cur_r_hash when cur_r_hash = r_hash -> (true, true)
                 | _ -> (false, false))
            | _ -> (false, false)
        in
        (* Prevent cross-file false recall when both query and entry name explicit files *)
        let e_file = extract_explicit_filename v.entity_tags in
        let file_conflict =
          match q_file, e_file with
          | Some qf, Some ef when qf <> ef -> true
          | _ -> false
        in
        if merkle_ok && not file_conflict then begin
          let overlap_cnt =
            List.fold_left (fun acc qt ->
              if List.mem qt v.entity_tags then acc + 1 else acc
            ) 0 q_tags
          in
          let denom_tags = max 1 (min (List.length q_tags) (List.length v.entity_tags)) in
          let entity_overlap =
            if overlap_cnt = 0 then 0.0
            else float_of_int overlap_cnt /. float_of_int denom_tags
          in
          let cos_sim = cosine_similarity query_embedding v.hidden_embedding in
          let combined =
            if Array.length query_embedding > 0 && Array.length v.hidden_embedding > 0 then
              (0.45 *. cos_sim) +. (0.55 *. entity_overlap)
            else
              entity_overlap
          in
          (* Require at least 1 shared entity tag so unrelated prompts never collide *)
          if overlap_cnt >= 1 && combined >= min_score then begin
            match !best_cand with
            | None -> best_cand := Some (v, cos_sim, entity_overlap, combined, survived_sparse)
            | Some (_, _, _, prev_score, _) when combined > prev_score ->
                best_cand := Some (v, cos_sim, entity_overlap, combined, survived_sparse)
            | _ -> ()
          end
        end
      end
    ) cache.table;
    match !best_cand with
    | None -> AssociativeMiss
    | Some (entry, cos_sim, ent_ov, comb, survived_sparse) ->
        cache.stats.associative_hits <- cache.stats.associative_hits + 1;
        if survived_sparse then
          cache.stats.sparse_merkle_survivors <- cache.stats.sparse_merkle_survivors + 1;
        let fact_str =
          Option.value
            ~default:(Printf.sprintf "@nnc_mem[cls=%d]{prompt:\"%s\"}" entry.classification_id entry.prompt_norm)
            entry.distilled_fact
        in
        AssociativeHit {
          entry = { entry with logits = Array.copy entry.logits };
          cosine_similarity = cos_sim;
          entity_overlap = ent_ov;
          combined_score = comb;
          recalled_fact = fact_str;
          sparse_merkle_survived = survived_sparse;
        }
  end

