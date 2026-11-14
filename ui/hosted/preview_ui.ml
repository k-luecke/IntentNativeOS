(* ui/hosted/preview_ui.ml - Plan, Execution & Reversible Result Preview (U003)
   Displays staged plan and diff before final commit, enabling the user to inspect
   the changes and either commit them or safely trigger a reversible undo.
*)

open Inos_operation.Staged_execution

type preview_view = {
  plan_title: string;
  target_file: string;
  original_content: string;
  proposed_content: string;
  commit_hash: string;
  is_committed: bool;
  is_rolled_back: bool;
}

let render_preview_html (view : preview_view) : string =
  let status_html =
    if view.is_rolled_back then
      "<div class=\"status-rolledback\">[REVERTED] Changes undone. Original file state preserved.</div>"
    else if view.is_committed then
      "<div class=\"status-committed\">[COMMITTED] Changes finalized and logged to journal.</div>"
    else
      "<div class=\"status-staged\">[STAGED FOR REVIEW] Inspect changes before committing.</div>"
  in
  let action_buttons =
    if not view.is_committed && not view.is_rolled_back then
      {|<div class="button-bar">
          <form method="POST" action="/api/execution/commit" style="display:inline;">
            <button type="submit" class="btn-commit">Commit Changes</button>
          </form>
          <form method="POST" action="/api/execution/undo" style="display:inline;">
            <button type="submit" class="btn-undo">Undo / Rollback Staged Copy</button>
          </form>
        </div>|}
    else
      {|<div class="button-bar">
          <a href="/" class="btn-return">Return to Dashboard</a>
        </div>|}
  in
  Printf.sprintf {|<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <title>Execution Result Preview & Undo</title>
  <style>
    body { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; margin: 2rem; background: #0f172a; color: #f8fafc; }
    .container { max-width: 900px; margin: auto; background: #1e293b; padding: 2rem; border-radius: 8px; border: 1px solid #334155; }
    .diff-container { display: flex; gap: 1rem; margin-top: 1rem; }
    .diff-box { flex: 1; background: #0f172a; padding: 1rem; border-radius: 6px; border: 1px solid #334155; }
    pre { white-space: pre-wrap; font-family: monospace; font-size: 0.85rem; color: #e2e8f0; }
    .btn-commit { background: #2563eb; color: white; padding: 0.75rem 1.5rem; border: none; border-radius: 6px; cursor: pointer; font-weight: bold; }
    .btn-undo { background: #dc2626; color: white; padding: 0.75rem 1.5rem; border: none; border-radius: 6px; cursor: pointer; font-weight: bold; }
    .btn-return { display: inline-block; background: #475569; color: white; padding: 0.75rem 1.5rem; border-radius: 6px; text-decoration: none; }
    .status-staged { color: #38bdf8; font-weight: bold; margin-bottom: 1rem; }
    .status-committed { color: #4ade80; font-weight: bold; margin-bottom: 1rem; }
    .status-rolledback { color: #f87171; font-weight: bold; margin-bottom: 1rem; }
    .receipt-info { background: #0f172a; padding: 0.75rem; border-radius: 4px; font-size: 0.8rem; color: #94a3b8; margin-top: 1rem; }
  </style>
</head>
<body>
  <div class="container">
    <h2>Staged Plan Preview: %s</h2>
    <p>Target: <code>%s</code></p>
    %s

    <div class="diff-container">
      <div class="diff-box">
        <h3>Original State</h3>
        <pre>%s</pre>
      </div>
      <div class="diff-box">
        <h3>Proposed State</h3>
        <pre>%s</pre>
      </div>
    </div>

    <div class="receipt-info">
      Staged Hash: <code>%s</code>
    </div>

    %s
  </div>
</body>
</html>|} view.plan_title view.target_file status_html view.original_content
   view.proposed_content view.commit_hash
   action_buttons

let perform_undo (executed_stack : staged_step list) (reason : string) : rollback_receipt =
  let restored = ref [] in
  let compensated = ref [] in
  let irreversible = ref [] in
  List.iter (fun (step : staged_step) ->
    if not step.is_reversible then
      irreversible := step.step_id :: !irreversible
    else
      match step.rollback_action () with
      | Ok PerfectRestoration ->
          restored := step.step_id :: !restored
      | Ok (Compensated { details }) ->
          compensated := (step.step_id, details) :: !compensated
      | Ok (Irreversible { reason = r }) ->
          irreversible := (Printf.sprintf "%s (%s)" step.step_id r) :: !irreversible
      | Error err ->
          compensated := (step.step_id, "Rollback error: " ^ err) :: !compensated
  ) executed_stack;
  let is_clean = (List.length !irreversible = 0) && (List.length !compensated = 0) in
  let receipt_hash = compute_receipt_hash "USER_UNDO" reason !restored !compensated !irreversible in
  {
    failed_step_id = "USER_UNDO";
    failure_reason = reason;
    restored_steps = !restored;
    compensated_steps = !compensated;
    irreversible_steps = !irreversible;
    is_clean_rollback = is_clean;
    receipt_hash;
  }
