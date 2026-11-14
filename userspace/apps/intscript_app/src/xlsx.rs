// userspace/apps/script_app/src/xlsx.rs - OPC XLSX Zip Archive Generator for INOS Script
extern crate alloc;
use alloc::vec::Vec;
use crate::vm::evaluator::{ScriptWorkbook, CellVal};

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB88320 & mask);
        }
    }
    !crc
}

fn format_i64(mut val: i64, buf: &mut [u8]) -> usize {
    if val == 0 {
        buf[0] = b'0';
        return 1;
    }
    let mut is_neg = false;
    if val < 0 {
        is_neg = true;
        val = -val;
    }
    let mut temp = [0u8; 20];
    let mut pos = 0;
    while val > 0 {
        temp[pos] = b'0' + (val % 10) as u8;
        val /= 10;
        pos += 1;
    }
    let mut out_pos = 0;
    if is_neg {
        buf[out_pos] = b'-';
        out_pos += 1;
    }
    while pos > 0 {
        pos -= 1;
        buf[out_pos] = temp[pos];
        out_pos += 1;
    }
    out_pos
}

fn write_cell_ref(col: usize, row: usize, buf: &mut [u8]) -> usize {
    let mut pos = 0;
    let prefix = b"<c r=\"";
    buf[pos..pos + prefix.len()].copy_from_slice(prefix);
    pos += prefix.len();

    buf[pos] = b'A' + col as u8;
    pos += 1;

    let mut rnum_buf = [0u8; 24];
    let rlen = format_i64((row + 1) as i64, &mut rnum_buf);
    buf[pos..pos + rlen].copy_from_slice(&rnum_buf[..rlen]);
    pos += rlen;

    buf[pos] = b'"';
    pos += 1;

    pos
}

pub fn create_opc_zip_archive(files: &[(&str, &[u8])], out: &mut Vec<u8>) {
    let mut local_offsets = [0u32; 16];

    for (i, &(name, data)) in files.iter().enumerate() {
        local_offsets[i] = out.len() as u32;

        let name_bytes = name.as_bytes();
        let file_crc = crc32(data);
        let data_len = data.len() as u32;

        // Local file header (30 bytes)
        out.extend_from_slice(&[0x50, 0x4B, 0x03, 0x04]);
        out.extend_from_slice(&20u16.to_le_bytes()); // version needed
        out.extend_from_slice(&0u16.to_le_bytes());  // flags
        out.extend_from_slice(&0u16.to_le_bytes());  // method 0 = stored
        out.extend_from_slice(&0u32.to_le_bytes());  // mod time/date
        out.extend_from_slice(&file_crc.to_le_bytes());
        out.extend_from_slice(&data_len.to_le_bytes()); // compressed size
        out.extend_from_slice(&data_len.to_le_bytes()); // uncompressed size
        out.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());  // extra field len
        out.extend_from_slice(name_bytes);
        out.extend_from_slice(data);
    }

    let cd_offset = out.len() as u32;

    // Central directory records (46 bytes + name len)
    for (i, &(name, data)) in files.iter().enumerate() {
        let name_bytes = name.as_bytes();
        let file_crc = crc32(data);
        let data_len = data.len() as u32;

        out.extend_from_slice(&[0x50, 0x4B, 0x01, 0x02]);
        out.extend_from_slice(&20u16.to_le_bytes()); // version made by
        out.extend_from_slice(&20u16.to_le_bytes()); // version needed
        out.extend_from_slice(&0u16.to_le_bytes());  // flags
        out.extend_from_slice(&0u16.to_le_bytes());  // method 0
        out.extend_from_slice(&0u32.to_le_bytes());  // mod time/date
        out.extend_from_slice(&file_crc.to_le_bytes());
        out.extend_from_slice(&data_len.to_le_bytes());
        out.extend_from_slice(&data_len.to_le_bytes());
        out.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());  // extra len
        out.extend_from_slice(&0u16.to_le_bytes());  // comment len
        out.extend_from_slice(&0u16.to_le_bytes());  // disk start
        out.extend_from_slice(&0u16.to_le_bytes());  // internal attr
        out.extend_from_slice(&0u32.to_le_bytes());  // external attr
        out.extend_from_slice(&local_offsets[i].to_le_bytes());
        out.extend_from_slice(name_bytes);
    }

    let cd_size = (out.len() as u32) - cd_offset;
    let entry_count = files.len() as u16;

    // End of Central Directory (22 bytes)
    out.extend_from_slice(&[0x50, 0x4B, 0x05, 0x06]);
    out.extend_from_slice(&0u16.to_le_bytes()); // disk num
    out.extend_from_slice(&0u16.to_le_bytes()); // start disk
    out.extend_from_slice(&entry_count.to_le_bytes());
    out.extend_from_slice(&entry_count.to_le_bytes());
    out.extend_from_slice(&cd_size.to_le_bytes());
    out.extend_from_slice(&cd_offset.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes()); // comment len
}

pub fn generate_xlsx(wb: &ScriptWorkbook) -> Vec<u8> {
    let sheet = &wb.sheets[0];

    // Collect unique strings for sharedStrings.xml
    let mut unique_strings: alloc::vec::Vec<alloc::string::String> = alloc::vec::Vec::new();
    let mut total_string_cells = 0usize;

    for r in 0..16 {
        for c in 0..16 {
            if let CellVal::Text(t) = &sheet.cells[r][c] {
                total_string_cells += 1;
                if !unique_strings.contains(t) {
                    unique_strings.push(t.clone());
                }
            }
        }
    }

    // Build sheet1.xml
    let mut sheet_xml = alloc::string::String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"><sheetData>"
    );

    for r in 0..16 {
        let mut row_has_cells = false;
        for c in 0..16 {
            if !matches!(sheet.cells[r][c], CellVal::Empty) {
                row_has_cells = true;
                break;
            }
        }
        if !row_has_cells {
            continue;
        }

        sheet_xml.push_str("<row r=\"");
        let mut r_buf = [0u8; 16];
        let r_len = format_i64((r + 1) as i64, &mut r_buf);
        sheet_xml.push_str(core::str::from_utf8(&r_buf[..r_len]).unwrap());
        sheet_xml.push_str("\">");

        for c in 0..16 {
            match &sheet.cells[r][c] {
                CellVal::Empty => {}
                CellVal::Number(n) => {
                    let mut cref_buf = [0u8; 16];
                    let clen = write_cell_ref(c, r, &mut cref_buf);
                    sheet_xml.push_str(core::str::from_utf8(&cref_buf[..clen]).unwrap());
                    sheet_xml.push_str("><v>");
                    let mut n_buf = [0u8; 24];
                    let n_len = format_i64(*n, &mut n_buf);
                    sheet_xml.push_str(core::str::from_utf8(&n_buf[..n_len]).unwrap());
                    sheet_xml.push_str("</v></c>");
                }
                CellVal::Text(t) => {
                    let s_idx = unique_strings.iter().position(|s| s == t).unwrap_or(0);
                    let mut cref_buf = [0u8; 16];
                    let clen = write_cell_ref(c, r, &mut cref_buf);
                    sheet_xml.push_str(core::str::from_utf8(&cref_buf[..clen]).unwrap());
                    sheet_xml.push_str(" t=\"s\"><v>");
                    let mut idx_buf = [0u8; 16];
                    let ilen = format_i64(s_idx as i64, &mut idx_buf);
                    sheet_xml.push_str(core::str::from_utf8(&idx_buf[..ilen]).unwrap());
                    sheet_xml.push_str("</v></c>");
                }
                CellVal::Formula(f) => {
                    let mut cref_buf = [0u8; 16];
                    let clen = write_cell_ref(c, r, &mut cref_buf);
                    sheet_xml.push_str(core::str::from_utf8(&cref_buf[..clen]).unwrap());
                    sheet_xml.push_str("><f>");
                    let formula_raw = if f.starts_with('=') { &f[1..] } else { f.as_str() };
                    sheet_xml.push_str(formula_raw);
                    sheet_xml.push_str("</f><v>");
                    // Evaluate formula value
                    if let Ok(eval_val) = sheet.evaluate_cell(c, r) {
                        let mut v_buf = [0u8; 24];
                        let v_len = format_i64(eval_val, &mut v_buf);
                        sheet_xml.push_str(core::str::from_utf8(&v_buf[..v_len]).unwrap());
                    }
                    sheet_xml.push_str("</v></c>");
                }
            }
        }

        sheet_xml.push_str("</row>");
    }
    sheet_xml.push_str("</sheetData></worksheet>");

    // Build sharedStrings.xml
    let mut sst_xml = alloc::string::String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<sst xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" count=\""
    );
    let mut num_buf = [0u8; 24];
    let nlen = format_i64(total_string_cells as i64, &mut num_buf);
    sst_xml.push_str(core::str::from_utf8(&num_buf[..nlen]).unwrap());
    sst_xml.push_str("\" uniqueCount=\"");
    let ulen = format_i64(unique_strings.len() as i64, &mut num_buf);
    sst_xml.push_str(core::str::from_utf8(&num_buf[..ulen]).unwrap());
    sst_xml.push_str("\">");

    for s in &unique_strings {
        sst_xml.push_str("<si><t>");
        sst_xml.push_str(s);
        sst_xml.push_str("</t></si>");
    }
    sst_xml.push_str("</sst>");

    let content_types = b"<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/><Override PartName=\"/xl/workbook.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml\"/><Override PartName=\"/xl/worksheets/sheet1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml\"/><Override PartName=\"/xl/sharedStrings.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml\"/></Types>";
    let rels = b"<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"xl/workbook.xml\"/></Relationships>";
    let workbook = b"<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<workbook xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\"><sheets><sheet name=\"Sheet1\" sheetId=\"1\" r:id=\"rId1\"/></sheets></workbook>";
    let wb_rels = b"<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet\" Target=\"worksheets/sheet1.xml\"/><Relationship Id=\"rId2\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/sharedStrings\" Target=\"sharedStrings.xml\"/></Relationships>";

    let sheet_bytes = sheet_xml.into_bytes();
    let sst_bytes = sst_xml.into_bytes();

    let parts: [(&str, &[u8]); 6] = [
        ("[Content_Types].xml", content_types),
        ("_rels/.rels", rels),
        ("xl/workbook.xml", workbook),
        ("xl/_rels/workbook.xml.rels", wb_rels),
        ("xl/worksheets/sheet1.xml", &sheet_bytes),
        ("xl/sharedStrings.xml", &sst_bytes),
    ];

    let mut out = Vec::with_capacity(4096);
    create_opc_zip_archive(&parts, &mut out);
    out
}
