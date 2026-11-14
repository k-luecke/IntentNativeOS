(* models/tokenizer/bpe_tokenizer.ml - Trustworthy BPE Tokenizer & Truncation Engine (Task L102) *)

open Utf8_cleaner

type truncation_policy =
  | TruncateHead of int
  | TruncateTail of int
  | NoTruncation

type token_id = int

type vocab_table = {
  token_to_id : (string, int) Hashtbl.t;
  id_to_token : (int, string) Hashtbl.t;
  bos_token_id : int;
  eos_token_id : int;
  unk_token_id : int;
  pad_token_id : int;
}

let create_vocab ?(bos=1) ?(eos=2) ?(unk=0) ?(pad=3) () : vocab_table =
  let t2i = Hashtbl.create 1024 in
  let i2t = Hashtbl.create 1024 in
  
  (* Standard special tokens *)
  Hashtbl.add t2i "<unk>" unk; Hashtbl.add i2t unk "<unk>";
  Hashtbl.add t2i "<s>" bos; Hashtbl.add i2t bos "<s>";
  Hashtbl.add t2i "</s>" eos; Hashtbl.add i2t eos "</s>";
  Hashtbl.add t2i "<pad>" pad; Hashtbl.add i2t pad "<pad>";

  {
    token_to_id = t2i;
    id_to_token = i2t;
    bos_token_id = bos;
    eos_token_id = eos;
    unk_token_id = unk;
    pad_token_id = pad;
  }

let register_token (vocab : vocab_table) (token : string) (id : int) =
  Hashtbl.replace vocab.token_to_id token id;
  Hashtbl.replace vocab.id_to_token id token

let encode_bytes (vocab : vocab_table) (input : string) : token_id list =
  let clean = sanitize_utf8 input in
  let len = String.length clean in
  let tokens = ref [] in
  let i = ref 0 in

  while !i < len do
    (* Check for registered multi-character token or fallback to byte token *)
    let matched = ref false in
    let max_probe = min 16 (len - !i) in
    let probe_len = ref max_probe in
    while !probe_len > 1 && not !matched do
      let sub = String.sub clean !i !probe_len in
      if Hashtbl.mem vocab.token_to_id sub then begin
        tokens := Hashtbl.find vocab.token_to_id sub :: !tokens;
        i := !i + !probe_len;
        matched := true
      end else
        decr probe_len
    done;

    if not !matched then begin
      (* Single char / byte fallback *)
      let single = String.sub clean !i 1 in
      if Hashtbl.mem vocab.token_to_id single then
        tokens := Hashtbl.find vocab.token_to_id single :: !tokens
      else
        tokens := vocab.unk_token_id :: !tokens;
      incr i
    end
  done;

  List.rev !tokens

let decode_tokens (vocab : vocab_table) (tokens : token_id list) : string =
  let buf = Buffer.create (List.length tokens * 4) in
  List.iter (fun id ->
    if id <> vocab.bos_token_id && id <> vocab.eos_token_id && id <> vocab.pad_token_id then
      match Hashtbl.find_opt vocab.id_to_token id with
      | Some tok -> Buffer.add_string buf tok
      | None -> Buffer.add_string buf replacement_char
  ) tokens;
  Buffer.contents buf

let apply_truncation (policy : truncation_policy) (tokens : token_id list) : token_id list =
  match policy with
  | NoTruncation -> tokens
  | TruncateHead max_len ->
      let len = List.length tokens in
      if len <= max_len then tokens
      else
        let rec drop n lst =
          if n <= 0 then lst
          else match lst with
            | [] -> []
            | _ :: tl -> drop (n - 1) tl
        in
        drop (len - max_len) tokens
  | TruncateTail max_len ->
      let rec take n lst =
        if n <= 0 then []
        else match lst with
          | [] -> []
          | hd :: tl -> hd :: take (n - 1) tl
      in
      take max_len tokens

let round_trip_check (vocab : vocab_table) (text : string) : bool =
  let encoded = encode_bytes vocab text in
  let decoded = decode_tokens vocab encoded in
  let sanitized_expected = sanitize_utf8 text in
  decoded = sanitized_expected
