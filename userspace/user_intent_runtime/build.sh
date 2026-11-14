#!/usr/bin/env bash
set -e

cd "$(dirname "$0")"

echo "[BUILD] Compiling pure_intent.ml to native object..."
ocamlopt -c src/pure_intent.ml
ocamlopt -output-obj -o caml_pure.o src/pure_intent.cmx

OCAML_ENTRY_SYM=$(nm caml_pure.o | awk '/camlPure_intent__ocaml_entry_[0-9]+/ {print $3; exit}')
if [ -z "$OCAML_ENTRY_SYM" ]; then
    OCAML_ENTRY_SYM="camlPure_intent__ocaml_entry_394"
fi
echo "[BUILD] Resolved OCaml entry symbol: ${OCAML_ENTRY_SYM}"

echo "[BUILD] Compiling freestanding C runtime and transformer..."
gcc -c -nostdlib -fno-stack-protector -fno-builtin -m64 -DOCAML_ENTRY_SYM="${OCAML_ENTRY_SYM}" -I"$(ocamlopt -where)" src/runtime_nostdlib.c -o runtime_nostdlib.o
gcc -c -nostdlib -fno-stack-protector -fno-builtin -m64 -O2 src/freestanding_transformer.c -o freestanding_transformer.o

echo "[BUILD] Linking static Ring 3 ELF..."
ld -T link.ld -static -o user_intent_runtime runtime_nostdlib.o freestanding_transformer.o caml_pure.o

echo "[BUILD] Stripping debug symbols while keeping entry point..."
strip --strip-debug user_intent_runtime

echo "[BUILD] Auditing binary symbols:"
nm user_intent_runtime | grep -E 'caml|ocaml|_start' || true

echo "[BUILD] Auditing program headers:"
readelf -l user_intent_runtime

echo "[BUILD] Success: user_intent_runtime generated ($(wc -c < user_intent_runtime) bytes)."
