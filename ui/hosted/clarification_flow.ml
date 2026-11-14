(* ui/hosted/clarification_flow.ml - Clarification & Low-Confidence Behavior (Task L502)
   Prevents consequential execution on ambiguous targets and conducts structured dialogue.
*)

type clarification_request = {
  intent_text : string;
  detected_ambiguity : string;
  proposed_options : string list;
  created_at : float;
}

type resolution =
  | UserSelected of int * string
  | UserCancelled
  | TimeoutExpired

type flow_metrics = {
  mutable total_ambiguities_flagged : int;
  mutable unnecessary_interruptions : int;
  mutable resolutions_completed : int;
  mutable aborts : int;
}

let create_flow_metrics () = {
  total_ambiguities_flagged = 0;
  unnecessary_interruptions = 0;
  resolutions_completed = 0;
  aborts = 0;
}

let generate_clarification (intent : string) : clarification_request option =
  let lower = String.lowercase_ascii intent in
  if (try ignore (Str.search_forward (Str.regexp "clean[ ]*up") lower 0); true with _ -> false) ||
     (try ignore (Str.search_forward (Str.regexp "delete[ ]*old") lower 0); true with _ -> false) then
    Some {
      intent_text = intent;
      detected_ambiguity = "Unbounded deletion target with unspecified file age criteria.";
      proposed_options = [
        "Delete only temporary build artifacts in /build";
        "Archive files untouched for more than 30 days into /archive.tar";
        "Show full file list and let me select specific files";
        "Cancel operation";
      ];
      created_at = Unix.gettimeofday ();
    }
  else
    None

let resolve_clarification (metrics : flow_metrics) (req : clarification_request) (res : resolution) : (string, string) result =
  metrics.total_ambiguities_flagged <- metrics.total_ambiguities_flagged + 1;
  match res with
  | UserSelected (idx, choice) ->
      if idx = 4 || choice = "Cancel operation" then begin
        metrics.aborts <- metrics.aborts + 1;
        Error "Operation cancelled by user."
      end else begin
        metrics.resolutions_completed <- metrics.resolutions_completed + 1;
        Ok (Printf.sprintf "Resolved: Executing confirmed strategy '%s' for '%s'" choice req.intent_text)
      end
  | UserCancelled ->
      metrics.aborts <- metrics.aborts + 1;
      Error "Operation explicitly cancelled."
  | TimeoutExpired ->
      metrics.aborts <- metrics.aborts + 1;
      Error "Clarification timed out; failing safe without execution."
