(* models/tokenizer/chat_template.ml - Chat Template Formatter and Boundary Guard (Task L102) *)

open Utf8_cleaner

type role =
  | System
  | User
  | Assistant

type message = {
  role : role;
  content : string;
}

type template_format =
  | ChatML
  | Llama3
  | RawPrompt

let string_of_role = function
  | System -> "system"
  | User -> "user"
  | Assistant -> "assistant"

(* Strip prompt injection boundary markers to prevent format hijacking *)
let escape_delimiters fmt content =
  match fmt with
  | ChatML ->
      let s = Str.global_replace (Str.regexp "<|im_start|>") "&lt;|im_start|&gt;" content in
      let s = Str.global_replace (Str.regexp "<|im_end|>") "&lt;|im_end|&gt;" s in
      Str.global_replace (Str.regexp "<|endoftext|>") "&lt;|endoftext|&gt;" s
  | Llama3 ->
      let s = Str.global_replace (Str.regexp "<|start_header_id|>") "&lt;|start_header_id|&gt;" content in
      let s = Str.global_replace (Str.regexp "<|end_header_id|>") "&lt;|end_header_id|&gt;" s in
      Str.global_replace (Str.regexp "<|eot_id|>") "&lt;|eot_id|&gt;" s
  | RawPrompt -> content

let format_chatml (messages : message list) : string =
  let buf = Buffer.create 256 in
  List.iter (fun msg ->
    let clean_body = escape_delimiters ChatML (sanitize_utf8 msg.content) in
    Buffer.add_string buf "<|im_start|>";
    Buffer.add_string buf (string_of_role msg.role);
    Buffer.add_string buf "\n";
    Buffer.add_string buf clean_body;
    Buffer.add_string buf "<|im_end|>\n"
  ) messages;
  (* Prime generation with assistant turn *)
  Buffer.add_string buf "<|im_start|>assistant\n";
  Buffer.contents buf

let format_llama3 (messages : message list) : string =
  let buf = Buffer.create 256 in
  Buffer.add_string buf "<|begin_of_text|>";
  List.iter (fun msg ->
    let clean_body = escape_delimiters Llama3 (sanitize_utf8 msg.content) in
    Buffer.add_string buf "<|start_header_id|>";
    Buffer.add_string buf (string_of_role msg.role);
    Buffer.add_string buf "<|end_header_id|>\n\n";
    Buffer.add_string buf clean_body;
    Buffer.add_string buf "<|eot_id|>\n"
  ) messages;
  Buffer.add_string buf "<|start_header_id|>assistant<|end_header_id|>\n\n";
  Buffer.contents buf

let apply_template (fmt : template_format) (messages : message list) : string =
  match fmt with
  | ChatML -> format_chatml messages
  | Llama3 -> format_llama3 messages
  | RawPrompt ->
      let last = List.rev messages in
      (match last with
       | [] -> ""
       | m :: _ -> sanitize_utf8 m.content)
