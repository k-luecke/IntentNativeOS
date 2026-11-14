(* core/memory/model_budget.ml - Model Load/Unload, Cache, and Budget Policy (Task L106)
   Monitors resident memory, governs model load/unload cycles, enforces reclamation,
   and implements low-memory deterministic fallbacks.
*)

type model_tier =
  | TierCompact       (* ~450 MB ceiling *)
  | TierMedium        (* ~1.0 GB ceiling *)
  | TierLarger        (* ~2.5 GB ceiling *)
  | TierDeterministic (* 0 MB model budget *)

let ceiling_kb_of_tier = function
  | TierCompact -> 524_288       (* 512 MiB *)
  | TierMedium -> 1_048_576      (* 1024 MiB *)
  | TierLarger -> 2_621_440      (* 2560 MiB *)
  | TierDeterministic -> 0

type lifecycle_state =
  | Unloaded
  | Loading of { model_id : string; started_at : float }
  | Active of {
      model_id : string;
      tier : model_tier;
      resident_kb : int;
      loaded_at : float;
      mutable last_accessed : float;
      mutable query_count : int;
    }
  | FallbackOnly of { reason : string }

type memory_snapshot = {
  timestamp : float;
  process_rss_kb : int;
  system_available_kb : int;
  active_model : string option;
  lifecycle : string;
}

let read_proc_field (path : string) (prefix : string) : int option =
  try
    let ic = open_in path in
    let result = ref None in
    (try
       while !result = None do
         let line = String.trim (input_line ic) in
         let p_len = String.length prefix in
         if String.length line >= p_len && String.sub line 0 p_len = prefix then begin
           let rest = String.trim (String.sub line p_len (String.length line - p_len)) in
           let num_str =
             match String.index_opt rest ' ' with
             | Some idx -> String.sub rest 0 idx
             | None -> rest
           in
           result := int_of_string_opt (String.trim num_str)
         end
       done
     with End_of_file -> close_in ic);
    !result
  with _ -> None

let get_live_rss_kb () : int =
  match read_proc_field "/proc/self/status" "VmRSS:" with
  | Some k -> k
  | None -> 1024

let get_system_available_kb () : int =
  match read_proc_field "/proc/meminfo" "MemAvailable:" with
  | Some k -> k
  | None -> 8_388_608 (* Fallback to 8 GiB in virtual environments *)

let take_snapshot (state : lifecycle_state) : memory_snapshot =
  let rss = get_live_rss_kb () in
  let avail = get_system_available_kb () in
  let (model_opt, life_str) =
    match state with
    | Unloaded -> (None, "UNLOADED")
    | Loading { model_id; _ } -> (Some model_id, "LOADING")
    | Active { model_id; _ } -> (Some model_id, "ACTIVE")
    | FallbackOnly { reason } -> (None, "FALLBACK: " ^ reason)
  in
  {
    timestamp = Unix.gettimeofday ();
    process_rss_kb = rss;
    system_available_kb = avail;
    active_model = model_opt;
    lifecycle = life_str;
  }

let load_model
    ~(model_id : string)
    ~(tier : model_tier)
    ~(estimated_kb : int)
    (current_state : lifecycle_state) : (lifecycle_state, string) result =
  let avail_kb = get_system_available_kb () in
  let reserve_kb = 1_048_576 (* Keep 1 GiB minimum OS headroom *) in
  if avail_kb - estimated_kb < reserve_kb then
    Error (Printf.sprintf "Insufficient system memory to load model '%s' (needed: %d KB, available: %d KB, reserve: %d KB)"
             model_id estimated_kb avail_kb reserve_kb)
  else
    match current_state with
    | Active { model_id = curr_id; _ } when curr_id = model_id ->
        Ok current_state (* Already loaded *)
    | Active { model_id = other_id; _ } ->
        Error (Printf.sprintf "Another model '%s' is already active. Must unload before loading '%s'." other_id model_id)
    | _ ->
        let now = Unix.gettimeofday () in
        Ok (Active {
          model_id;
          tier;
          resident_kb = estimated_kb;
          loaded_at = now;
          last_accessed = now;
          query_count = 0;
        })

let touch_model (state : lifecycle_state) : unit =
  match state with
  | Active a ->
      a.last_accessed <- Unix.gettimeofday ();
      a.query_count <- a.query_count + 1
  | _ -> ()

let unload_model (current_state : lifecycle_state) : (lifecycle_state * int, string) result =
  match current_state with
  | Active { model_id = _model_id; resident_kb; _ } ->
      (* In production, this drops model tensors and invokes garbage collection *)
      Gc.full_major ();
      let reclaimed_kb = resident_kb in
      Ok (Unloaded, reclaimed_kb)
  | Loading _ ->
      Ok (Unloaded, 0)
  | Unloaded ->
      Ok (Unloaded, 0)
  | FallbackOnly _ ->
      Ok (Unloaded, 0)

let select_operating_tier ~(available_kb : int) : model_tier =
  if available_kb >= 4_194_304 then TierLarger
  else if available_kb >= 2_097_152 then TierMedium
  else if available_kb >= 1_048_576 then TierCompact
  else TierDeterministic
