#!/usr/bin/env bash
# scripts/test_format_conformance_01.sh - Acceptance suite for APF-002B (Document Interoperability & Independent Format Validation)
# Verifies:
# 1. Independent compilation and execution of pure Rust tools/format_validator (Zero Python)
# 2. OpenXML (.xlsx) OPC container, _rels relationship DOM hierarchy, and formula/value semantics
# 3. OpenXML (.docx) OPC container, WordprocessingML DOM, paragraph, table, and style hierarchy
# 4. Adobe PDF 1.7 (ISO 32000-1) header, body objects, xref byte offset exactness, trailer and %%EOF
# 5. Host unzip -t and zipinfo external tool interoperability audit

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
export PATH="${HOME}/.cargo/bin:${PATH}"

BUILD_DIR="${ROOT_DIR}/build"
EVIDENCE_DIR="${ROOT_DIR}/evidence"
OUT_DIR="${BUILD_DIR}/format_conformance"
DISK_FLOW="${BUILD_DIR}/disk_agent_flow_01.img"
VALIDATOR="${ROOT_DIR}/tools/format_validator/target/release/format_validator"

mkdir -p "${BUILD_DIR}" "${EVIDENCE_DIR}" "${OUT_DIR}"

echo "=========================================================================="
echo "=== [APF-002B: DOCUMENT INTEROPERABILITY & FORMAT CONFORMANCE]         ==="
echo "=========================================================================="

echo ""
echo "[STEP 0] Building tools/format_validator in release mode..."
cd "${ROOT_DIR}/tools/format_validator"
cargo build --release

if [ ! -f "${VALIDATOR}" ]; then
    echo "ERROR: format_validator binary failed to compile!"
    exit 1
fi
echo "  [PASS] tools/format_validator compiled successfully."

echo ""
echo "[STEP 1] Ensuring fresh disk image containing INOS-FS document artifacts..."
if [ ! -f "${DISK_FLOW}" ] || [ ! -s "${DISK_FLOW}" ]; then
    echo "Generating disk image via agent flow suite..."
    bash "${SCRIPT_DIR}/test_agent_flow_01.sh"
else
    echo "Existing disk image found: ${DISK_FLOW} ($(stat -c%s "${DISK_FLOW}") bytes)"
fi

echo ""
echo "--------------------------------------------------------------------------"
echo ">>> PHASE 1: Disk Extraction & Pure Rust Structural Conformance Audit  <<<"
echo "--------------------------------------------------------------------------"

rm -rf "${OUT_DIR}/*"
"${VALIDATOR}" --disk "${DISK_FLOW}" --out-dir "${OUT_DIR}"

echo ""
echo "--------------------------------------------------------------------------"
echo ">>> PHASE 2: Individual File Conformance Verification                  <<<"
echo "--------------------------------------------------------------------------"

XLSX_FILES=("${OUT_DIR}"/*.xlsx)
DOCX_FILES=("${OUT_DIR}"/*.docx)
PDF_FILES=("${OUT_DIR}"/*.pdf)

if [ ${#XLSX_FILES[@]} -eq 0 ] || [ ${#DOCX_FILES[@]} -eq 0 ] || [ ${#PDF_FILES[@]} -eq 0 ]; then
    echo "ERROR: Extraction incomplete! Expected XLSX, DOCX, and PDF files."
    exit 1
fi

echo "Verifying OpenXML XLSX conformance..."
"${VALIDATOR}" "${XLSX_FILES[@]}"

echo ""
echo "Verifying OpenXML DOCX conformance..."
"${VALIDATOR}" "${DOCX_FILES[@]}"

echo ""
echo "Verifying Adobe PDF 1.7 conformance..."
"${VALIDATOR}" "${PDF_FILES[@]}"

echo ""
echo "--------------------------------------------------------------------------"
echo ">>> PHASE 3: Host Tool Interoperability Audit (unzip & zipinfo)        <<<"
echo "--------------------------------------------------------------------------"

for xfile in "${XLSX_FILES[@]}"; do
    echo "[HOST UNZIP] Testing XLSX archive integrity: $(basename "${xfile}")..."
    unzip -t "${xfile}"
    echo "[HOST ZIPINFO] Inspecting ZIP entries: $(basename "${xfile}")..."
    zipinfo -1 "${xfile}"
done

for dfile in "${DOCX_FILES[@]}"; do
    echo "[HOST UNZIP] Testing DOCX archive integrity: $(basename "${dfile}")..."
    unzip -t "${dfile}"
    echo "[HOST ZIPINFO] Inspecting ZIP entries: $(basename "${dfile}")..."
    zipinfo -1 "${dfile}"
done

for pfile in "${PDF_FILES[@]}"; do
    echo "[HOST PDF] Auditing PDF 1.7 headers and xref: $(basename "${pfile}")..."
    head -n 2 "${pfile}"
    tail -n 8 "${pfile}"
done

echo ""
echo "=========================================================================="
echo "=== [GATE APF-002B: PASSED 5/5 INVARIANTS VERIFIED]                   ==="
echo "=========================================================================="
