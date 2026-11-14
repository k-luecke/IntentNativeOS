(* core/memory/memory_hierarchy.ml
   ==========================================================================
   INOS 3-Tier Cognitive & Process Memory Hierarchy
   ==========================================================================
   Implements standard multi-stage memory management and retrieval processes:
   1. Tier 1 — Working Memory (Short-Term Buffer):
      - Bounded token/byte sliding window with salience scoring, exponential
        temporal decay (s(t) = s_0 * exp(-lambda * dt)), and automatic
        compaction/summarization of evicted turns into Episodic Memory.
   2. Tier 2 — Episodic Memory (Chronological Experience Store):
      - Time-indexed interaction episodes with access-frequency reinforcement
        (spacing effect) and entity-tag inverted indexing.
   3. Tier 3 — Semantic Long-Term Memory & Knowledge Graph:
      - Background Memory Consolidation process (`consolidate_memories`) that
        distills recurring or high-salience episodic memories into canonical
        (subject, predicate, object) triples and promotes them into the
        cryptographic `Trust_store` as `Verified` facts.
   4. Hybrid Retrieval Pipeline (BM25 + Dense Vector + RRF + MMR):
      - Okapi BM25 lexical scoring (k1 = 1.5, b = 0.75) for exact entity/number recall.
      - Dense vector cosine similarity for semantic paraphrase recall.
      - Reciprocal Rank Fusion (RRF) combining lexical, vector, and recency/salience ranks.
      - Maximal Marginal Relevance (MMR) diversity re-ranking to eliminate redundancy.
   ========================================================================== *)

open Inos_ir.Provenance

type memory_tier =
  | Tier_Working
  | Tier_Episodic
  | Tier_Semantic

type working_item = {
  wm_id : string;
  turn_seq : int;
  content : string;
  tokens : string list;
  embedding : float array;
  base_salience : float;
  created_turn : int;
  mutable last_accessed_turn : int;
  mutable access_count : int;
  is_pinned_constraint : bool;
}

type episodic_record = {
  ep_id : string;
  source_turn : int;
  summary : string;
  tokens : string list;
  embedding : float array;
  salience : float;
  mutable reinforcement_count : int;
  mutable consolidated : bool;
  digest : string;
}

type semantic_triple = {
  triple_id : string;
  subject : string;
  predicate : string;
  object_val : string;
  confidence : float;
  supporting_episodes : string list;
  provenance_hash : string;
}

type hybrid_search_hit = {
  record_id : string;
  tier : memory_tier;
  text : string;
  bm25_score : float;
  vector_score : float;
  rrf_score : float;
  mmr_score : float;
}

type consolidation_report = {
  compacted_working_items : int;
  new_episodic_records : int;
  promoted_semantic_triples : int;
  deduplicated_episodes : int;
}

type t = {
  mutable current_turn : int;
  working_capacity : int;
  decay_lambda : float;
  mutable working : working_item list;
  mutable episodic : episodic_record list;
  semantic_graph : (string, semantic_triple) Hashtbl.t;
  trust_store : Trust_store.trust_store;
  doc_freq : (string, int) Hashtbl.t;
  mutable total_docs : int;
  mutable total_doc_len : int;
}

let create ?(working_capacity = 8) ?(decay_lambda = 0.12) () : t = {
  current_turn = 0;
  working_capacity = max 2 working_capacity;
  decay_lambda;
  working = [];
  episodic = [];
  semantic_graph = Hashtbl.create 64;
  trust_store = Trust_store.create ();
  doc_freq = Hashtbl.create 128;
  total_docs = 0;
  total_doc_len = 0;
}

let is_stopword (w : string) : bool =
  List.mem w [
    "the"; "and"; "for"; "with"; "that"; "this"; "from"; "into"; "when";
    "what"; "where"; "which"; "will"; "have"; "has"; "had"; "are"; "was";
    "were"; "been"; "can"; "could"; "should"; "would"; "please"; "tell"; "show"
  ]

let tokenize_terms (text : string) : string list =
  let len = String.length text in
  let acc = ref [] in
  let buf = Buffer.create 16 in
  let flush () =
    if Buffer.length buf > 0 then begin
      let w = Buffer.contents buf in
      Buffer.clear buf;
      if String.length w >= 2 && not (is_stopword w) then
        acc := w :: !acc
    end
  in
  for i = 0 to len - 1 do
    let c = Char.lowercase_ascii text.[i] in
    if (c >= 'a' && c <= 'z') || (c >= '0' && c <= '9') || c = '_' || c = '.' || c = '-' || c = '$' then
      Buffer.add_char buf c
    else
      flush ()
  done;
  flush ();
  List.rev !acc

let unique_strings (lst : string list) : string list =
  let seen = Hashtbl.create 16 in
  List.filter (fun x ->
    if Hashtbl.mem seen x then false
    else (Hashtbl.add seen x (); true)
  ) lst

let index_document_terms (mem : t) (terms : string list) : unit =
  mem.total_docs <- mem.total_docs + 1;
  mem.total_doc_len <- mem.total_doc_len + List.length terms;
  List.iter (fun term ->
    let cur = Option.value ~default:0 (Hashtbl.find_opt mem.doc_freq term) in
    Hashtbl.replace mem.doc_freq term (cur + 1)
  ) (unique_strings terms)

(* Effective salience with exponential temporal decay + spacing-effect reinforcement *)
let effective_salience (mem : t) (it : working_item) : float =
  if it.is_pinned_constraint then 100.0
  else
    let dt = float_of_int (max 0 (mem.current_turn - it.last_accessed_turn)) in
    let decay = exp (-. mem.decay_lambda *. dt) in
    let spacing_boost = 1.0 +. (0.20 *. log (1.0 +. float_of_int it.access_count)) in
    it.base_salience *. decay *. spacing_boost

let vector_cosine (a : float array) (b : float array) : float =
  let la = Array.length a in
  let lb = Array.length b in
  if la = 0 || lb = 0 || la <> lb then 0.0
  else begin
    let start_i = if la >= 16 then 5 else 0 in
    let n = float_of_int (la - start_i) in
    let ma = ref 0.0 in
    let mb = ref 0.0 in
    for i = start_i to la - 1 do
      ma := !ma +. a.(i);
      mb := !mb +. b.(i)
    done;
    let mean_a = !ma /. n in
    let mean_b = !mb /. n in
    let dot = ref 0.0 in
    let na = ref 0.0 in
    let nb = ref 0.0 in
    for i = start_i to la - 1 do
      let da = a.(i) -. mean_a in
      let db = b.(i) -. mean_b in
      dot := !dot +. (da *. db);
      na := !na +. (da *. da);
      nb := !nb +. (db *. db)
    done;
    let denom = sqrt !na *. sqrt !nb in
    if denom <= 1e-8 then 0.0 else max 0.0 (!dot /. denom)
  end

let lexical_jaccard (a : string list) (b : string list) : float =
  let ua = unique_strings a in
  let ub = unique_strings b in
  if ua = [] || ub = [] then 0.0
  else
    let inter = List.fold_left (fun acc w -> if List.mem w ub then acc + 1 else acc) 0 ua in
    let union = List.length ua + List.length ub - inter in
    if union <= 0 then 0.0 else float_of_int inter /. float_of_int union

(* Okapi BM25 Lexical Score (k1 = 1.5, b = 0.75) *)
let compute_bm25 (mem : t) ~(query_terms : string list) ~(doc_terms : string list) : float =
  let k1 = 1.5 in
  let b = 0.75 in
  let n_docs = float_of_int (max 1 mem.total_docs) in
  let avgdl =
    if mem.total_docs <= 0 then 8.0
    else max 1.0 (float_of_int mem.total_doc_len /. n_docs)
  in
  let doc_len = float_of_int (List.length doc_terms) in
  List.fold_left (fun acc q_term ->
    let tf =
      float_of_int (List.fold_left (fun c t -> if t = q_term then c + 1 else c) 0 doc_terms)
    in
    if tf <= 0.0 then acc
    else
      let df = float_of_int (Option.value ~default:0 (Hashtbl.find_opt mem.doc_freq q_term)) in
      let idf = log (1.0 +. ((n_docs -. df +. 0.5) /. (df +. 0.5))) in
      let num = tf *. (k1 +. 1.0) in
      let denom = tf +. k1 *. (1.0 -. b +. b *. (doc_len /. avgdl)) in
      acc +. (idf *. (num /. denom))
  ) 0.0 (unique_strings query_terms)

(* Extract canonical (subject, predicate, object) triple from an episodic summary *)
let extract_triple_from_tokens (ep : episodic_record) : (string * string * string) option =
  match ep.tokens with
  | subj :: pred :: rest when rest <> [] ->
      let obj = String.concat "_" (List.filteri (fun i _ -> i < 4) rest) in
      Some (subj, pred, obj)
  | _ -> None

(* Background Memory Consolidation Process ("Sleep / Compaction Cycle"):
   1. Compacts overflow or decayed Working Memory items into Episodic Memory.
   2. Deduplicates similar Episodic records (reinforcing salience on repeated experiences).
   3. Promotes recurring or high-salience Episodic facts into Semantic Long-Term Memory
      and commits them to `Trust_store` as `Verified` assertions. *)
let consolidate_memories (mem : t) : consolidation_report =
  let compacted_wm = ref 0 in
  let new_ep = ref 0 in
  let dedup_ep = ref 0 in
  let promoted_sem = ref 0 in

  (* Step 1: If working memory exceeds capacity, evict lowest effective salience items
     (never evicting pinned constraints) and compact them into Episodic Memory *)
  let scored_wm =
    List.map (fun it -> (it, effective_salience mem it)) mem.working
    |> List.sort (fun (_, s1) (_, s2) -> compare s2 s1)
  in
  let rec split_wm idx kept evicted = function
    | [] -> (List.rev kept, List.rev evicted)
    | (it, _sal) :: rest ->
        if idx < mem.working_capacity || it.is_pinned_constraint then
          split_wm (idx + 1) (it :: kept) evicted rest
        else
          split_wm (idx + 1) kept (it :: evicted) rest
  in
  let (kept_wm, evicted_wm) = split_wm 0 [] [] scored_wm in
  mem.working <- kept_wm;

  List.iter (fun (ev : working_item) ->
    incr compacted_wm;
    (* Check if a near-duplicate episode already exists *)
    let existing_match =
      List.find_opt (fun (ep : episodic_record) ->
        lexical_jaccard ev.tokens ep.tokens >= 0.55 ||
        vector_cosine ev.embedding ep.embedding >= 0.85
      ) mem.episodic
    in
    match existing_match with
    | Some ep ->
        incr dedup_ep;
        ep.reinforcement_count <- ep.reinforcement_count + 1 + ev.access_count
    | None ->
        incr new_ep;
        let ep_id = Printf.sprintf "ep_%d_%s" ev.turn_seq ev.wm_id in
        let summary =
          Printf.sprintf "@episode[turn=%d,sal=%.2f]{%s}"
            ev.turn_seq ev.base_salience ev.content
        in
        let digest = Inos_ipc.Sha256.digest_string (ep_id ^ "|" ^ summary) in
        let rec_ep = {
          ep_id;
          source_turn = ev.turn_seq;
          summary;
          tokens = ev.tokens;
          embedding = Array.copy ev.embedding;
          salience = ev.base_salience;
          reinforcement_count = 1 + ev.access_count;
          consolidated = false;
          digest;
        } in
        mem.episodic <- rec_ep :: mem.episodic
  ) evicted_wm;

  (* Step 2: Promote reinforced or high-salience Episodic records into Semantic Long-Term Memory *)
  List.iter (fun (ep : episodic_record) ->
    if (not ep.consolidated) && (ep.reinforcement_count >= 2 || ep.salience >= 0.80) then begin
      match extract_triple_from_tokens ep with
      | Some (subj, pred, obj) ->
          ep.consolidated <- true;
          incr promoted_sem;
          let key = Printf.sprintf "sem:%s:%s" subj pred in
          let prov_hash =
            Inos_ipc.Sha256.digest_string (Printf.sprintf "%s|%s|%s|%s" subj pred obj ep.digest)
          in
          let triple = {
            triple_id = key;
            subject = subj;
            predicate = pred;
            object_val = obj;
            confidence = min 0.99 (0.75 +. 0.08 *. float_of_int ep.reinforcement_count);
            supporting_episodes = [ep.ep_id];
            provenance_hash = prov_hash;
          } in
          Hashtbl.replace mem.semantic_graph key triple;
          ignore (Trust_store.put mem.trust_store
            ~key
            ~content:(Printf.sprintf "(%s, %s, %s) [hash=%s]" subj pred obj (String.sub prov_hash 0 12))
            ~trust_class:Verified
            ~producer:"memory_consolidation_daemon" ())
      | None -> ()
    end
  ) mem.episodic;

  {
    compacted_working_items = !compacted_wm;
    new_episodic_records = !new_ep;
    promoted_semantic_triples = !promoted_sem;
    deduplicated_episodes = !dedup_ep;
  }

(* Ingest a new observation / interaction turn into Working Memory,
   automatically triggering consolidation if Working Memory overflows. *)
let ingest_turn
    (mem : t)
    ?(salience = 0.75)
    ?(is_pinned_constraint = false)
    ?(embedding = [||])
    ~(wm_id : string)
    ~(content : string) () : consolidation_report option =
  mem.current_turn <- mem.current_turn + 1;
  let tokens = tokenize_terms content in
  index_document_terms mem tokens;
  (* Check if an identical or near-identical item is already in Working Memory -> reinforce it! *)
  let existing =
    List.find_opt (fun (it : working_item) ->
      it.wm_id = wm_id || lexical_jaccard it.tokens tokens >= 0.70
    ) mem.working
  in
  (match existing with
   | Some it ->
       it.last_accessed_turn <- mem.current_turn;
       it.access_count <- it.access_count + 1
   | None ->
       let item = {
         wm_id;
         turn_seq = mem.current_turn;
         content;
         tokens;
         embedding = Array.copy embedding;
         base_salience = salience;
         created_turn = mem.current_turn;
         last_accessed_turn = mem.current_turn;
         access_count = 1;
         is_pinned_constraint;
       } in
       mem.working <- item :: mem.working);
  if List.length mem.working > mem.working_capacity then
    Some (consolidate_memories mem)
  else
    None

(* Hybrid Retrieval across all 3 Memory Tiers (Working + Episodic + Semantic)
   using BM25 + Dense Vector Cosine + Reciprocal Rank Fusion (RRF) + MMR Diversity *)
let retrieve_hybrid
    (mem : t)
    ?(query_embedding = [||])
    ?(top_k = 4)
    ?(mmr_lambda = 0.72)
    (query : string) : hybrid_search_hit list =
  let q_terms = tokenize_terms query in
  (* Gather candidate pool across Working, Episodic, and Semantic tiers *)
  let candidates : (string * memory_tier * string * string list * float array * float) list ref = ref [] in
  List.iter (fun (wm : working_item) ->
    let sal = effective_salience mem wm in
    candidates := (wm.wm_id, Tier_Working, wm.content, wm.tokens, wm.embedding, sal) :: !candidates
  ) mem.working;
  List.iter (fun (ep : episodic_record) ->
    let sal = ep.salience *. (1.0 +. 0.15 *. float_of_int ep.reinforcement_count) in
    candidates := (ep.ep_id, Tier_Episodic, ep.summary, ep.tokens, ep.embedding, sal) :: !candidates
  ) mem.episodic;
  Hashtbl.iter (fun k (tr : semantic_triple) ->
    let text = Printf.sprintf "@semantic[%s]{%s -> %s -> %s}" k tr.subject tr.predicate tr.object_val in
    let toks = tokenize_terms (tr.subject ^ " " ^ tr.predicate ^ " " ^ tr.object_val) in
    candidates := (tr.triple_id, Tier_Semantic, text, toks, [||], tr.confidence *. 1.5) :: !candidates
  ) mem.semantic_graph;

  let scored_raw =
    List.map (fun (id, tier, text, toks, emb, sal) ->
      let bm25 = compute_bm25 mem ~query_terms:q_terms ~doc_terms:toks in
      let vec_sim =
        let cos = vector_cosine query_embedding emb in
        let jac = lexical_jaccard q_terms toks in
        if Array.length query_embedding > 0 && Array.length emb > 0 then
          (0.60 *. cos) +. (0.40 *. jac)
        else jac
      in
      (id, tier, text, toks, emb, sal, bm25, vec_sim)
    ) !candidates
    |> List.filter (fun (_, _, _, _, _, _, bm25, vec_sim) -> bm25 > 0.0 || vec_sim > 0.05)
  in
  if scored_raw = [] then []
  else begin
    (* Rank by BM25 and by Vector Similarity for Reciprocal Rank Fusion (RRF, k=60) *)
    let by_bm25 =
      List.sort (fun (_, _, _, _, _, _, b1, _) (_, _, _, _, _, _, b2, _) -> compare b2 b1) scored_raw
    in
    let by_vec =
      List.sort (fun (_, _, _, _, _, _, _, v1) (_, _, _, _, _, _, _, v2) -> compare v2 v1) scored_raw
    in
    let rank_of id sorted_list =
      let rec find i = function
        | [] -> 999
        | (cid, _, _, _, _, _, _, _) :: rest -> if cid = id then i else find (i + 1) rest
      in
      find 1 sorted_list
    in
    let k_rrf = 60.0 in
    let with_rrf =
      List.map (fun (id, tier, text, toks, emb, sal, bm25, vec_sim) ->
        let r_bm25 = float_of_int (rank_of id by_bm25) in
        let r_vec = float_of_int (rank_of id by_vec) in
        let rrf =
          (1.0 /. (k_rrf +. r_bm25)) +.
          (1.0 /. (k_rrf +. r_vec)) +.
          (0.005 *. min 5.0 sal)
        in
        (id, tier, text, toks, emb, bm25, vec_sim, rrf)
      ) scored_raw
    in
    (* Maximal Marginal Relevance (MMR) greedy selection to maximize relevance & diversity *)
    let remaining = ref with_rrf in
    let selected : (string * memory_tier * string * string list * float array * float * float * float * float) list ref = ref [] in
    while List.length !selected < top_k && !remaining <> [] do
      let best_next = ref None in
      List.iter (fun cand ->
        let (_id, _tier, _text, toks, emb, _bm25, _vec, rrf) = cand in
        let max_sim_to_selected =
          List.fold_left (fun acc (_, _, _, s_toks, s_emb, _, _, _, _) ->
            let sim = max (lexical_jaccard toks s_toks) (vector_cosine emb s_emb) in
            max acc sim
          ) 0.0 !selected
        in
        (* Normalize RRF (~0.033 max) to [0, 1] scale for MMR balance *)
        let norm_rrf = rrf *. 30.0 in
        let mmr = (mmr_lambda *. norm_rrf) -. ((1.0 -. mmr_lambda) *. max_sim_to_selected) in
        match !best_next with
        | None -> best_next := Some (cand, mmr)
        | Some (_, prev_mmr) when mmr > prev_mmr -> best_next := Some (cand, mmr)
        | _ -> ()
      ) !remaining;
      match !best_next with
      | None -> remaining := []
      | Some ((id, tier, text, toks, emb, bm25, vec_sim, rrf), mmr) ->
          (* Reinforce accessed working memory item *)
          List.iter (fun (wm : working_item) ->
            if wm.wm_id = id then begin
              wm.last_accessed_turn <- mem.current_turn;
              wm.access_count <- wm.access_count + 1
            end
          ) mem.working;
          selected := (id, tier, text, toks, emb, bm25, vec_sim, rrf, mmr) :: !selected;
          remaining := List.filter (fun (cid, _, _, _, _, _, _, _) -> cid <> id) !remaining
    done;
    List.rev !selected
    |> List.map (fun (id, tier, text, _toks, _emb, bm25, vec_sim, rrf, mmr) ->
      {
        record_id = id;
        tier;
        text;
        bm25_score = bm25;
        vector_score = vec_sim;
        rrf_score = rrf;
        mmr_score = mmr;
      }
    )
  end
