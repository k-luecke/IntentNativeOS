(* Pure OCaml SHA-256 Implementation (FIPS 180-4) *)
(* Zero external dependencies, pure 32-bit integer arithmetic *)

let k = [|
  0x428a2f98l; 0x71374491l; 0xb5c0fbcfl; 0xe9b5dba5l;
  0x3956c25bl; 0x59f111f1l; 0x923f82a4l; 0xab1c5ed5l;
  0xd807aa98l; 0x12835b01l; 0x243185bel; 0x550c7dc3l;
  0x72be5d74l; 0x80deb1fel; 0x9bdc06a7l; 0xc19bf174l;
  0xe49b69c1l; 0xefbe4786l; 0x0fc19dc6l; 0x240ca1ccl;
  0x2de92c6fl; 0x4a7484aal; 0x5cb0a9dcl; 0x76f988dal;
  0x983e5152l; 0xa831c66dl; 0xb00327c8l; 0xbf597fc7l;
  0xc6e00bf3l; 0xd5a79147l; 0x06ca6351l; 0x14292967l;
  0x27b70a85l; 0x2e1b2138l; 0x4d2c6dfcl; 0x53380d13l;
  0x650a7354l; 0x766a0abbl; 0x81c2c92el; 0x92722c85l;
  0xa2bfe8a1l; 0xa81a664bl; 0xc24b8b70l; 0xc76c51a3l;
  0xd192e819l; 0xd6990624l; 0xf40e3585l; 0x106aa070l;
  0x19a4c116l; 0x1e376c08l; 0x2748774cl; 0x34b0bcb5l;
  0x391c0cb3l; 0x4ed8aa4al; 0x5b9cca4fl; 0x682e6ff3l;
  0x748f82eel; 0x78a5636fl; 0x84c87814l; 0x8cc70208l;
  0x90befffal; 0xa4506cebl; 0xbef9a3f7l; 0xc67178f2l;
|]

let ror (x : int32) (n : int) : int32 =
  Int32.logor (Int32.shift_right_logical x n) (Int32.shift_left x (32 - n))

let ( +! ) = Int32.add
let ( ^! ) = Int32.logxor
let ( &! ) = Int32.logand
let ( ~! ) = Int32.lognot

let ch x y z = (x &! y) ^! ((~!x) &! z)
let maj x y z = (x &! y) ^! (x &! z) ^! (y &! z)
let sigma0 x = (ror x 2) ^! (ror x 13) ^! (ror x 22)
let sigma1 x = (ror x 6) ^! (ror x 11) ^! (ror x 25)
let gamma0 x = (ror x 7) ^! (ror x 18) ^! (Int32.shift_right_logical x 3)
let gamma1 x = (ror x 17) ^! (ror x 19) ^! (Int32.shift_right_logical x 10)

let digest_string (msg : string) : string =
  let len = String.length msg in
  let bit_len = Int64.mul (Int64.of_int len) 8L in
  let pad_len =
    let rem = (len + 9) mod 64 in
    if rem = 0 then 0 else 64 - rem
  in
  let total_len = len + 1 + pad_len + 8 in
  let buf = Bytes.create total_len in
  Bytes.blit_string msg 0 buf 0 len;
  Bytes.set buf len '\x80';
  for i = len + 1 to len + pad_len do
    Bytes.set buf i '\x00'
  done;
  for i = 0 to 7 do
    let shift = (7 - i) * 8 in
    let b = Int64.to_int (Int64.logand (Int64.shift_right_logical bit_len shift) 0xffL) in
    Bytes.set buf (total_len - 8 + i) (Char.chr b)
  done;

  let h0 = ref 0x6a09e667l in
  let h1 = ref 0xbb67ae85l in
  let h2 = ref 0x3c6ef372l in
  let h3 = ref 0xa54ff53al in
  let h4 = ref 0x510e527fl in
  let h5 = ref 0x9b05688cl in
  let h6 = ref 0x1f83d9abl in
  let h7 = ref 0x5be0cd19l in

  let w = Array.make 64 0l in
  let num_chunks = total_len / 64 in

  for chunk = 0 to num_chunks - 1 do
    let offset = chunk * 64 in
    for i = 0 to 15 do
      let idx = offset + (i * 4) in
      let b0 = Int32.of_int (Char.code (Bytes.get buf idx)) in
      let b1 = Int32.of_int (Char.code (Bytes.get buf (idx + 1))) in
      let b2 = Int32.of_int (Char.code (Bytes.get buf (idx + 2))) in
      let b3 = Int32.of_int (Char.code (Bytes.get buf (idx + 3))) in
      w.(i) <- Int32.logor (Int32.shift_left b0 24)
                (Int32.logor (Int32.shift_left b1 16)
                  (Int32.logor (Int32.shift_left b2 8) b3))
    done;
    for i = 16 to 63 do
      w.(i) <- (gamma1 w.(i - 2)) +! w.(i - 7) +! (gamma0 w.(i - 15)) +! w.(i - 16)
    done;

    let a = ref !h0 in
    let b = ref !h1 in
    let c = ref !h2 in
    let d = ref !h3 in
    let e = ref !h4 in
    let f = ref !h5 in
    let g = ref !h6 in
    let h = ref !h7 in

    for i = 0 to 63 do
      let t1 = !h +! (sigma1 !e) +! (ch !e !f !g) +! k.(i) +! w.(i) in
      let t2 = (sigma0 !a) +! (maj !a !b !c) in
      h := !g;
      g := !f;
      f := !e;
      e := !d +! t1;
      d := !c;
      c := !b;
      b := !a;
      a := t1 +! t2
    done;

    h0 := !h0 +! !a;
    h1 := !h1 +! !b;
    h2 := !h2 +! !c;
    h3 := !h3 +! !d;
    h4 := !h4 +! !e;
    h5 := !h5 +! !f;
    h6 := !h6 +! !g;
    h7 := !h7 +! !h
  done;

  Printf.sprintf "%08lx%08lx%08lx%08lx%08lx%08lx%08lx%08lx"
    !h0 !h1 !h2 !h3 !h4 !h5 !h6 !h7
