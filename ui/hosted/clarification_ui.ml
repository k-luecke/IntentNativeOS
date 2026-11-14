(* ui/hosted/clarification_ui.ml - Intent Clarification as Contextual Choices (U002)
   Displays ambiguity questions with selectable options and lets the user either
   approve a specific contextual choice or abort and return to exploration.
*)

type clarification_choice = {
  choice_id: string;
  label: string;
  target: string;
  consequence: string;
}

type clarification_dialog = {
  question_id: string;
  prompt: string;
  severity: string;
  choices: clarification_choice list;
}

type user_decision =
  | ChoiceApproved of { choice_id: string; target: string }
  | ReturnToExploration of { reason: string }

let render_clarification_html (d : clarification_dialog) : string =
  let choices_html =
    List.map (fun c ->
      Printf.sprintf {|
        <div class="choice-item">
          <label>
            <input type="radio" name="choice_id" value="%s" required>
            <strong>%s</strong> &mdash; <code>%s</code>
          </label>
          <div class="consequence-note">%s</div>
        </div>
      |} c.choice_id c.label c.target c.consequence
    ) d.choices |> String.concat "\n"
  in
  let sev_class = match String.lowercase_ascii d.severity with
    | "critical" -> "badge-critical"
    | "medium" -> "badge-medium"
    | _ -> "badge-low"
  in
  Printf.sprintf {|<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <title>Intent Clarification Required</title>
  <style>
    body { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; margin: 2rem; background: #0f172a; color: #f8fafc; }
    .dialog { background: #1e293b; border-radius: 8px; padding: 2rem; max-width: 600px; margin: auto; border: 1px solid #475569; }
    .badge { display: inline-block; padding: 0.25rem 0.6rem; border-radius: 4px; font-size: 0.8rem; font-weight: bold; text-transform: uppercase; margin-bottom: 1rem; }
    .badge-critical { background: #dc2626; color: white; }
    .badge-medium { background: #d97706; color: white; }
    .badge-low { background: #2563eb; color: white; }
    .choice-item { background: #0f172a; padding: 1rem; border-radius: 6px; margin: 0.75rem 0; border: 1px solid #334155; }
    .consequence-note { font-size: 0.85rem; color: #94a3b8; margin-top: 0.5rem; margin-left: 1.5rem; }
    .button-bar { display: flex; gap: 1rem; margin-top: 1.5rem; }
    .btn-approve { background: #16a34a; color: white; padding: 0.75rem 1.5rem; border: none; border-radius: 6px; cursor: pointer; font-weight: 600; flex: 1; }
    .btn-explore { background: #475569; color: white; padding: 0.75rem 1.5rem; border: none; border-radius: 6px; cursor: pointer; font-weight: 600; flex: 1; }
    .btn-approve:hover { background: #15803d; }
    .btn-explore:hover { background: #334155; }
  </style>
</head>
<body>
  <div class="dialog">
    <span class="badge %s">%s Ambiguity</span>
    <h2>Clarification Required</h2>
    <p>%s</p>

    <form method="POST" action="/api/clarify/decide">
      <input type="hidden" name="question_id" value="%s">
      %s
      <div class="button-bar">
        <button type="submit" name="action" value="approve" class="btn-approve">Approve Choice</button>
        <button type="submit" name="action" value="explore" class="btn-explore">Return to Exploration</button>
      </div>
    </form>
  </div>
</body>
</html>|} sev_class d.severity d.prompt d.question_id choices_html

let handle_decision (params : (string * string) list) (dialog : clarification_dialog) : user_decision =
  let action = List.assoc_opt "action" params |> Option.value ~default:"explore" in
  if action = "explore" then
    ReturnToExploration { reason = "User aborted clarification and returned to exploration" }
  else
    let choice_id = List.assoc_opt "choice_id" params |> Option.value ~default:"" in
    match List.find_opt (fun c -> c.choice_id = choice_id) dialog.choices with
    | Some choice -> ChoiceApproved { choice_id = choice.choice_id; target = choice.target }
    | None -> ReturnToExploration { reason = "Invalid or unselected choice; returned to exploration" }
