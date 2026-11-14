(* models/tokenizer/utf8_cleaner.ml - Robust UTF-8 Stream Sanitizer and Validator (Task L102)
   Prevents tokenizer crashes and injection payloads from malformed byte sequences.
*)

let replacement_char = "\xEF\xBF\xBD" (* U+FFFD *)

let sanitize_utf8 (input : string) : string =
  let len = String.length input in
  let buf = Buffer.create (len + 8) in
  let i = ref 0 in

  while !i < len do
    let b0 = Char.code input.[!i] in
    if b0 < 0x80 then begin
      (* 1-byte ASCII: 00..7F *)
      Buffer.add_char buf input.[!i];
      incr i
    end else if b0 >= 0xC2 && b0 <= 0xDF then begin
      (* 2-byte sequence: C2..DF 80..BF *)
      if !i + 1 < len then
        let b1 = Char.code input.[!i + 1] in
        if b1 >= 0x80 && b1 <= 0xBF then begin
          Buffer.add_char buf input.[!i];
          Buffer.add_char buf input.[!i + 1];
          i := !i + 2
        end else begin
          Buffer.add_string buf replacement_char;
          incr i
        end
      else begin
        Buffer.add_string buf replacement_char;
        incr i
      end
    end else if b0 >= 0xE0 && b0 <= 0xEF then begin
      (* 3-byte sequence: E0..EF 80..BF 80..BF *)
      if !i + 2 < len then
        let b1 = Char.code input.[!i + 1] in
        let b2 = Char.code input.[!i + 2] in
        let valid =
          (b1 >= 0x80 && b1 <= 0xBF) && (b2 >= 0x80 && b2 <= 0xBF) &&
          not (b0 = 0xE0 && b1 < 0xA0) && (* overlong *)
          not (b0 = 0xED && b1 >= 0xA0)    (* surrogate *)
        in
        if valid then begin
          Buffer.add_char buf input.[!i];
          Buffer.add_char buf input.[!i + 1];
          Buffer.add_char buf input.[!i + 2];
          i := !i + 3
        end else begin
          Buffer.add_string buf replacement_char;
          incr i
        end
      else begin
        Buffer.add_string buf replacement_char;
        incr i
      end
    end else if b0 >= 0xF0 && b0 <= 0xF4 then begin
      (* 4-byte sequence: F0..F4 80..BF 80..BF 80..BF *)
      if !i + 3 < len then
        let b1 = Char.code input.[!i + 1] in
        let b2 = Char.code input.[!i + 2] in
        let b3 = Char.code input.[!i + 3] in
        let valid =
          (b1 >= 0x80 && b1 <= 0xBF) && (b2 >= 0x80 && b2 <= 0xBF) && (b3 >= 0x80 && b3 <= 0xBF) &&
          not (b0 = 0xF0 && b1 < 0x90) && (* overlong *)
          not (b0 = 0xF4 && b1 > 0x8F)    (* codepoint > 10FFFF *)
        in
        if valid then begin
          Buffer.add_char buf input.[!i];
          Buffer.add_char buf input.[!i + 1];
          Buffer.add_char buf input.[!i + 2];
          Buffer.add_char buf input.[!i + 3];
          i := !i + 4
        end else begin
          Buffer.add_string buf replacement_char;
          incr i
        end
      else begin
        Buffer.add_string buf replacement_char;
        incr i
      end
    end else begin
      (* Invalid lead byte (80..C1, F5..FF) *)
      Buffer.add_string buf replacement_char;
      incr i
    end
  done;

  Buffer.contents buf

let is_valid_utf8 (input : string) : bool =
  let sanitized = sanitize_utf8 input in
  sanitized = input
