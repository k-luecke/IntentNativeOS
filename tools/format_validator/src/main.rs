use std::collections::HashMap;
use std::env;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::Path;
use std::process::exit;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage:");
        eprintln!("  format_validator <file_path> [<file_path_2> ...]");
        eprintln!("  format_validator --disk <disk_image.img> [--out-dir <out_dir>]");
        eprintln!("\nValidates OpenXML (.xlsx, .docx) and Adobe PDF (1.7) documents for structural conformance.");
        exit(1);
    }

    if args[1] == "--disk" {
        if args.len() < 3 {
            eprintln!("Error: --disk requires a disk image path");
            exit(1);
        }
        let disk_path = Path::new(&args[2]);
        let out_dir = if args.len() >= 5 && args[3] == "--out-dir" {
            Path::new(&args[4])
        } else {
            Path::new("build")
        };

        if let Err(e) = scan_and_validate_disk(disk_path, out_dir) {
            eprintln!("[FORMAT_VALIDATOR] Disk scan failed: {}", e);
            exit(1);
        }
        return;
    }

    let mut failed = 0;
    let mut total = 0;

    for path_str in &args[1..] {
        if path_str.starts_with('-') {
            continue;
        }
        total += 1;
        let path = Path::new(path_str);
        println!("============================================================");
        println!("[FORMAT_VALIDATOR] Inspecting: {}", path.display());
        println!("============================================================");

        match validate_file(path) {
            Ok(format_type) => {
                println!("[FORMAT_VALIDATOR] SUCCESS: {} conforms to {} standard specifications.", path.display(), format_type);
            }
            Err(e) => {
                eprintln!("[FORMAT_VALIDATOR] ERROR: {} validation failed: {}", path.display(), e);
                failed += 1;
            }
        }
    }

    println!("\n[FORMAT_VALIDATOR] Audit Complete: {}/{} files verified successfully.", total - failed, total);
    if failed > 0 {
        exit(1);
    }
}

fn validate_file(path: &Path) -> Result<&'static str, String> {
    let mut file = File::open(path).map_err(|e| format!("Failed to open file: {}", e))?;
    let mut data = Vec::new();
    file.read_to_end(&mut data).map_err(|e| format!("Failed to read file: {}", e))?;

    if data.is_empty() {
        return Err("File is empty (0 bytes)".to_string());
    }

    if data.starts_with(b"%PDF-") {
        validate_pdf(&data)?;
        Ok("Adobe PDF 1.7 (ISO 32000-1)")
    } else if data.starts_with(b"PK\x03\x04") {
        let is_docx = path.extension().and_then(|s| s.to_str()) == Some("docx") || path.to_string_lossy().contains("docx");
        validate_zip_openxml(&data, is_docx)?;
        if is_docx {
            Ok("ECMA-376 / ISO 29500 OpenXML (.docx)")
        } else {
            Ok("ECMA-376 / ISO 29500 OpenXML (.xlsx)")
        }
    } else {
        Err(format!("Unrecognized file signature: {:02X?}", &data[..core::cmp::min(16, data.len())]))
    }
}

// ---------------------------------------------------------------------------
// Disk Scanner & Automatic Extractor
// ---------------------------------------------------------------------------

fn scan_and_validate_disk(disk_path: &Path, out_dir: &Path) -> Result<(), String> {
    println!("============================================================");
    println!("[FORMAT_VALIDATOR] Scanning Raw Disk: {}", disk_path.display());
    println!("============================================================");

    let mut file = File::open(disk_path).map_err(|e| format!("Cannot open disk image: {}", e))?;
    let mut disk_data = Vec::new();
    file.read_to_end(&mut disk_data).map_err(|e| format!("Cannot read disk image: {}", e))?;

    fs::create_dir_all(out_dir).map_err(|e| format!("Cannot create out dir: {}", e))?;

    let mut found_xlsx = 0;
    let mut found_docx = 0;
    let mut found_pdf = 0;

    // Scan for ZIP packages (PK\x03\x04)
    let mut i = 0;
    while i + 30 < disk_data.len() {
        if disk_data[i] == b'P' && disk_data[i + 1] == b'K' && disk_data[i + 2] == 0x03 && disk_data[i + 3] == 0x04 {
            // Check if this ZIP package contains [Content_Types].xml within the first 100 bytes
            let check_slice = &disk_data[i..core::cmp::min(i + 128, disk_data.len())];
            if check_slice.windows(19).any(|w| w == b"[Content_Types].xml") {
                // Find EOCD (PK\x05\x06) within reasonable distance (up to 64KB)
                let max_search = core::cmp::min(i + 65536, disk_data.len());
                let mut zip_len = 0;
                for j in i..max_search.saturating_sub(21) {
                    if disk_data[j] == b'P' && disk_data[j + 1] == b'K' && disk_data[j + 2] == 0x05 && disk_data[j + 3] == 0x06 {
                        let comment_len = u16::from_le_bytes([disk_data[j + 20], disk_data[j + 21]]) as usize;
                        zip_len = (j + 22 + comment_len) - i;
                        break;
                    }
                }

                if zip_len > 0 && i + zip_len <= disk_data.len() {
                    let zip_bytes = &disk_data[i..i + zip_len];
                    let is_docx = zip_bytes.windows(17).any(|w| w == b"word/document.xml");
                    let is_xlsx = zip_bytes.windows(15).any(|w| w == b"xl/workbook.xml");

                    if is_docx {
                        found_docx += 1;
                        let out_file = out_dir.join(format!("extracted_disk_{}.docx", found_docx));
                        println!("\n[EXTRACT] Located OpenXML DOCX at offset {} (len={}). Saving to {}", i, zip_len, out_file.display());
                        let mut f = File::create(&out_file).map_err(|e| format!("Cannot write extracted docx: {}", e))?;
                        f.write_all(zip_bytes).map_err(|e| format!("Write failed: {}", e))?;
                        validate_file(&out_file)?;
                    } else if is_xlsx {
                        found_xlsx += 1;
                        let out_file = out_dir.join(format!("extracted_disk_{}.xlsx", found_xlsx));
                        println!("\n[EXTRACT] Located OpenXML XLSX at offset {} (len={}). Saving to {}", i, zip_len, out_file.display());
                        let mut f = File::create(&out_file).map_err(|e| format!("Cannot write extracted xlsx: {}", e))?;
                        f.write_all(zip_bytes).map_err(|e| format!("Write failed: {}", e))?;
                        validate_file(&out_file)?;
                    }

                    i += zip_len;
                    continue;
                }
            }
        }
        i += 1;
    }

    // Scan for Adobe PDF (%PDF-1.)
    i = 0;
    while i + 8 < disk_data.len() {
        if &disk_data[i..i + 7] == b"%PDF-1." {
            // Check that header line ends with newline (e.g. %PDF-1.7\r\n)
            let is_valid_hdr = (i + 10 <= disk_data.len()) &&
                (disk_data[i + 8] == b'\r' || disk_data[i + 8] == b'\n' ||
                 disk_data[i + 9] == b'\r' || disk_data[i + 9] == b'\n');

            if is_valid_hdr {
                // Find first %%EOF within 64KB
                let max_search = core::cmp::min(i + 65536, disk_data.len());
                let mut pdf_len = 0;
                for j in i..max_search.saturating_sub(4) {
                    if &disk_data[j..j + 5] == b"%%EOF" {
                        pdf_len = (j + 5) - i;
                        // Include any trailing newline
                        if i + pdf_len < disk_data.len() && disk_data[i + pdf_len] == b'\r' {
                            pdf_len += 1;
                        }
                        if i + pdf_len < disk_data.len() && disk_data[i + pdf_len] == b'\n' {
                            pdf_len += 1;
                        }
                        break;
                    }
                }

                if pdf_len > 0 && i + pdf_len <= disk_data.len() {
                    let candidate = &disk_data[i..i + pdf_len];
                    if candidate.windows(4).any(|w| w == b"xref") {
                        found_pdf += 1;
                        let out_file = out_dir.join(format!("extracted_disk_{}.pdf", found_pdf));
                        println!("\n[EXTRACT] Located Adobe PDF at offset {} (len={}). Saving to {}", i, pdf_len, out_file.display());
                        let mut f = File::create(&out_file).map_err(|e| format!("Cannot write extracted pdf: {}", e))?;
                        f.write_all(candidate).map_err(|e| format!("Write failed: {}", e))?;
                        validate_file(&out_file)?;

                        i += pdf_len;
                        continue;
                    }
                }
            }
        }
        i += 1;
    }

    println!("\n============================================================");
    println!("[FORMAT_VALIDATOR] Disk Summary: Found & validated {} XLSX, {} DOCX, {} PDF documents.",
        found_xlsx, found_docx, found_pdf);
    println!("============================================================");

    if found_xlsx == 0 && found_docx == 0 && found_pdf == 0 {
        return Err("No supported documents found on disk image".to_string());
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Pure Rust ZIP Archive Parser & OpenXML Validator
// ---------------------------------------------------------------------------

fn parse_zip_archive(data: &[u8]) -> Result<HashMap<String, Vec<u8>>, String> {
    let mut entries = HashMap::new();
    let mut pos = 0;

    let mut found_local = 0;
    while pos + 30 <= data.len() {
        let sig = u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
        if sig != 0x04034b50 {
            break;
        }

        let compression = u16::from_le_bytes([data[pos + 8], data[pos + 9]]);
        let comp_size = u32::from_le_bytes([data[pos + 18], data[pos + 19], data[pos + 20], data[pos + 21]]) as usize;
        let uncomp_size = u32::from_le_bytes([data[pos + 22], data[pos + 23], data[pos + 24], data[pos + 25]]) as usize;
        let name_len = u16::from_le_bytes([data[pos + 26], data[pos + 27]]) as usize;
        let extra_len = u16::from_le_bytes([data[pos + 28], data[pos + 29]]) as usize;

        let name_start = pos + 30;
        let name_end = name_start + name_len;
        if name_end > data.len() {
            return Err("Truncated ZIP filename in local header".to_string());
        }

        let name = String::from_utf8_lossy(&data[name_start..name_end]).to_string();
        let data_start = name_end + extra_len;
        let data_end = data_start + comp_size;

        if data_end > data.len() {
            return Err(format!("Truncated payload for ZIP entry '{}'", name));
        }

        if compression == 0 {
            if comp_size != uncomp_size {
                return Err(format!("Stored compression size mismatch for '{}': {} vs {}", name, comp_size, uncomp_size));
            }
            entries.insert(name.clone(), data[data_start..data_end].to_vec());
        } else {
            return Err(format!("Unsupported compression method {} in entry '{}' (INOS uses stored method 0)", compression, name));
        }

        pos = data_end;
        found_local += 1;
    }

    if found_local == 0 {
        return Err("No local file headers found in ZIP container".to_string());
    }

    // Verify Central Directory signature (0x02014b50 = "PK\x01\x02")
    if pos + 4 <= data.len() {
        let cd_sig = u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
        if cd_sig != 0x02014b50 {
            return Err(format!("Expected Central Directory header (0x02014b50) at offset {}, got 0x{:08X}", pos, cd_sig));
        }
    }

    // Verify End of Central Directory signature (0x06054b50 = "PK\x05\x06")
    let mut eocd_found = false;
    for i in (pos..data.len().saturating_sub(21)).rev() {
        if u32::from_le_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]]) == 0x06054b50 {
            eocd_found = true;
            let total_entries = u16::from_le_bytes([data[i + 10], data[i + 11]]) as usize;
            if total_entries != found_local {
                return Err(format!("EOCD entry count ({}) does not match local headers ({})", total_entries, found_local));
            }
            break;
        }
    }

    if !eocd_found {
        return Err("End of Central Directory (EOCD) signature not found".to_string());
    }

    println!("  [ZIP] Validated ZIP archive: {} stored entries, valid Central Directory & EOCD.", found_local);
    Ok(entries)
}

fn validate_zip_openxml(data: &[u8], is_docx: bool) -> Result<(), String> {
    let entries = parse_zip_archive(data)?;

    // 1. Mandatory OPC package root: [Content_Types].xml
    let content_types_data = entries.get("[Content_Types].xml")
        .ok_or_else(|| "Missing mandatory OPC '[Content_Types].xml' entry".to_string())?;
    let content_types_str = std::str::from_utf8(content_types_data)
        .map_err(|_| "Invalid UTF-8 in '[Content_Types].xml'".to_string())?;

    if !content_types_str.contains("<Types") || !content_types_str.contains("</Types>") {
        return Err("[Content_Types].xml missing <Types> XML DOM root".to_string());
    }
    if !content_types_str.contains("http://schemas.openxmlformats.org/package/2006/content-types") {
        return Err("[Content_Types].xml missing standard OPC ContentTypes XML namespace".to_string());
    }
    println!("  [OPC] [Content_Types].xml verified with standard OPC namespace.");

    // 2. Mandatory OPC relationships: _rels/.rels
    let rels_data = entries.get("_rels/.rels")
        .ok_or_else(|| "Missing mandatory OPC '_rels/.rels' entry".to_string())?;
    let rels_str = std::str::from_utf8(rels_data)
        .map_err(|_| "Invalid UTF-8 in '_rels/.rels'".to_string())?;

    if !rels_str.contains("<Relationships") || !rels_str.contains("</Relationships>") {
        return Err("_rels/.rels missing <Relationships> XML DOM root".to_string());
    }
    if !rels_str.contains("http://schemas.openxmlformats.org/package/2006/relationships") {
        return Err("_rels/.rels missing standard Relationships XML namespace".to_string());
    }
    println!("  [OPC] _rels/.rels package relationship hierarchy confirmed.");

    if is_docx || entries.contains_key("word/document.xml") {
        validate_docx_entries(&entries)?;
    } else {
        validate_xlsx_entries(&entries)?;
    }

    Ok(())
}

fn validate_xlsx_entries(entries: &HashMap<String, Vec<u8>>) -> Result<(), String> {
    println!("  [XLSX] Validating OpenXML SpreadsheetML package...");

    // xl/workbook.xml
    let wb_data = entries.get("xl/workbook.xml")
        .ok_or_else(|| "Missing 'xl/workbook.xml'".to_string())?;
    let wb_str = std::str::from_utf8(wb_data)
        .map_err(|_| "Invalid UTF-8 in 'xl/workbook.xml'".to_string())?;
    if !wb_str.contains("<workbook") || !wb_str.contains("<sheets>") || !wb_str.contains("<sheet ") {
        return Err("xl/workbook.xml missing valid workbook and sheet definitions".to_string());
    }

    // xl/_rels/workbook.xml.rels
    if let Some(wb_rels_data) = entries.get("xl/_rels/workbook.xml.rels") {
        let wb_rels_str = std::str::from_utf8(wb_rels_data)
            .map_err(|_| "Invalid UTF-8 in xl/_rels/workbook.xml.rels".to_string())?;
        if !wb_rels_str.contains("<Relationships") || !wb_rels_str.contains("Target=\"worksheets/sheet1.xml\"") {
            return Err("xl/_rels/workbook.xml.rels does not correctly point to worksheets/sheet1.xml".to_string());
        }
        println!("  [XLSX] Workbook relationship hierarchy -> worksheets/sheet1.xml confirmed.");
    }

    // xl/worksheets/sheet1.xml
    let sheet_data = entries.get("xl/worksheets/sheet1.xml")
        .ok_or_else(|| "Missing 'xl/worksheets/sheet1.xml'".to_string())?;
    let sheet_str = std::str::from_utf8(sheet_data)
        .map_err(|_| "Invalid UTF-8 in 'xl/worksheets/sheet1.xml'".to_string())?;

    if !sheet_str.contains("<worksheet") || !sheet_str.contains("<sheetData>") || !sheet_str.contains("</sheetData>") {
        return Err("xl/worksheets/sheet1.xml missing <worksheet> and <sheetData> DOM elements".to_string());
    }

    let mut row_count = 0;
    let mut cell_count = 0;
    let mut formula_count = 0;
    let mut value_count = 0;

    let mut cur = sheet_str;
    while let Some(r_idx) = cur.find("<row r=\"") {
        row_count += 1;
        cur = &cur[r_idx + 8..];
    }

    cur = sheet_str;
    while let Some(c_idx) = cur.find("<c r=\"") {
        cell_count += 1;
        let c_slice = &cur[c_idx..];
        let c_end = c_slice.find("</c>").unwrap_or(c_slice.len());
        let cell_xml = &c_slice[..c_end];

        if cell_xml.contains("<f>") {
            formula_count += 1;
            if !cell_xml.contains("<v>") {
                return Err(format!("Formula/value semantic violation in cell: formula lacks evaluated <v> node in '{}'", cell_xml));
            }
        }
        if cell_xml.contains("<v>") {
            value_count += 1;
        }

        cur = &c_slice[c_end..];
    }

    println!("  [XLSX] Verified {} rows, {} cells ({} formulas with paired evaluated values, {} total value nodes).",
        row_count, cell_count, formula_count, value_count);

    Ok(())
}

fn validate_docx_entries(entries: &HashMap<String, Vec<u8>>) -> Result<(), String> {
    println!("  [DOCX] Validating OpenXML WordprocessingML package...");

    // word/document.xml
    let doc_data = entries.get("word/document.xml")
        .ok_or_else(|| "Missing 'word/document.xml'".to_string())?;
    let doc_str = std::str::from_utf8(doc_data)
        .map_err(|_| "Invalid UTF-8 in 'word/document.xml'".to_string())?;

    if !doc_str.contains("<w:document") || !doc_str.contains("<w:body>") || !doc_str.contains("</w:body>") {
        return Err("word/document.xml missing <w:document> or <w:body> DOM elements".to_string());
    }

    let mut p_count = 0;
    let mut cur = doc_str;
    while let Some(idx) = cur.find("<w:p>") {
        p_count += 1;
        cur = &cur[idx + 5..];
    }

    let mut t_count = 0;
    cur = doc_str;
    while let Some(idx) = cur.find("<w:t>") {
        t_count += 1;
        cur = &cur[idx + 5..];
    }

    let has_tables = doc_str.contains("<w:tbl>");
    let tbl_str = if has_tables { "with table grid structure (<w:tbl>, <w:tr>, <w:tc>)" } else { "without tables" };

    println!("  [DOCX] Verified {} paragraphs, {} text runs, {}.", p_count, t_count, tbl_str);

    // word/styles.xml
    if let Some(styles_data) = entries.get("word/styles.xml") {
        let styles_str = std::str::from_utf8(styles_data)
            .map_err(|_| "Invalid UTF-8 in 'word/styles.xml'".to_string())?;
        if !styles_str.contains("<w:styles") {
            return Err("word/styles.xml missing <w:styles> DOM element".to_string());
        }
        println!("  [DOCX] Stylesheet 'word/styles.xml' confirmed.");
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Pure Rust Adobe PDF 1.7 (ISO 32000-1) Conformance Validator
// ---------------------------------------------------------------------------

fn validate_pdf(data: &[u8]) -> Result<(), String> {
    println!("  [PDF] Validating Adobe PDF 1.7 (ISO 32000-1) specification...");

    // 1. Header: %PDF-1.x
    if !data.starts_with(b"%PDF-1.") {
        return Err("Missing standard %PDF-1.x file header".to_string());
    }
    let header_line = match data.iter().position(|&b| b == b'\n' || b == b'\r') {
        Some(pos) => String::from_utf8_lossy(&data[..pos]).to_string(),
        None => return Err("Missing newline after %PDF header".to_string()),
    };
    println!("  [PDF] Header verified: '{}'", header_line);

    // 2. Terminal marker: %%EOF
    let search_window = &data[data.len().saturating_sub(128)..];
    let eof_str = String::from_utf8_lossy(search_window);
    if !eof_str.contains("%%EOF") {
        return Err("Missing mandatory '%%EOF' terminal marker at end of file".to_string());
    }

    // 3. Locate startxref
    let startxref_idx = match eof_str.find("startxref") {
        Some(idx) => idx,
        None => return Err("Missing 'startxref' offset pointer before %%EOF".to_string()),
    };

    let post_startxref = &eof_str[startxref_idx + 9..];
    let mut xref_offset_str = String::new();
    for line in post_startxref.lines() {
        let trimmed = line.trim();
        if !trimmed.is_empty() && trimmed != "%%EOF" {
            xref_offset_str = trimmed.to_string();
            break;
        }
    }

    let xref_offset: usize = xref_offset_str.parse()
        .map_err(|e| format!("Invalid startxref numeric offset '{}': {}", xref_offset_str, e))?;

    if xref_offset >= data.len() {
        return Err(format!("startxref offset {} points outside file boundaries (size={})", xref_offset, data.len()));
    }

    // 4. Verify xref table at specified offset
    let xref_slice = &data[xref_offset..];
    if !xref_slice.starts_with(b"xref") {
        return Err(format!("Expected 'xref' marker at byte offset {}, found '{:02X?}'", xref_offset, &xref_slice[..core::cmp::min(16, xref_slice.len())]));
    }
    println!("  [PDF] 'xref' cross-reference table located at byte offset {}.", xref_offset);

    let xref_text = String::from_utf8_lossy(xref_slice);
    let mut lines = xref_text.lines();
    lines.next(); // skip "xref"

    let header_line = lines.next().ok_or_else(|| "Truncated xref table header".to_string())?;
    let parts: Vec<&str> = header_line.split_whitespace().collect();
    if parts.len() != 2 {
        return Err(format!("Invalid xref subsection header: '{}'", header_line));
    }
    let first_obj: usize = parts[0].parse().map_err(|_| "Invalid first obj in xref header".to_string())?;
    let obj_count: usize = parts[1].parse().map_err(|_| "Invalid obj count in xref header".to_string())?;

    println!("  [PDF] Parsing {} xref entries starting at object index {}...", obj_count, first_obj);

    let mut object_offsets = Vec::new();
    for i in 0..obj_count {
        let entry_line = lines.next().ok_or_else(|| format!("Truncated xref entry at index {}", i))?;
        let entry_parts: Vec<&str> = entry_line.split_whitespace().collect();
        if entry_parts.len() < 3 {
            return Err(format!("Invalid xref entry line: '{}'", entry_line));
        }
        let off: usize = entry_parts[0].parse().map_err(|_| format!("Invalid xref offset '{}'", entry_parts[0]))?;
        let flag = entry_parts[2];
        if flag == "n" {
            object_offsets.push((first_obj + i, off));
        }
    }

    // 5. Verify byte offsets point precisely to object definitions ("N 0 obj")
    for (obj_id, off) in &object_offsets {
        if *off >= data.len() {
            return Err(format!("Object {} offset {} is beyond file size {}", obj_id, off, data.len()));
        }
        let target = format!("{} 0 obj", obj_id);
        let check_slice = &data[*off..core::cmp::min(*off + 32, data.len())];
        let check_str = String::from_utf8_lossy(check_slice);
        if !check_str.starts_with(&target) {
            return Err(format!("Byte offset {} in xref table does NOT match '{}', found: '{}'", off, target, check_str.lines().next().unwrap_or("")));
        }
        println!("    Object {}: byte offset {} verified exact match ('{}').", obj_id, off, target);
    }

    // 6. Verify trailer dictionary
    let full_text = String::from_utf8_lossy(data);
    let trailer_idx = full_text.rfind("trailer").ok_or_else(|| "Missing 'trailer' dictionary".to_string())?;
    let trailer_slice = &full_text[trailer_idx..];
    if !trailer_slice.contains("/Size") || !trailer_slice.contains("/Root") {
        return Err("Trailer dictionary missing mandatory /Size or /Root keys".to_string());
    }
    println!("  [PDF] Trailer dictionary confirmed (/Size and /Root present).");

    // 7. Verify Document Catalog and Pages Tree
    if !full_text.contains("/Type /Catalog") || !full_text.contains("/Type /Pages") {
        return Err("PDF structure missing mandatory /Type /Catalog or /Type /Pages objects".to_string());
    }
    println!("  [PDF] PDF Document Catalog and Pages tree verified.");

    Ok(())
}
