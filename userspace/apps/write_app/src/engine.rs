// userspace/apps/write_app/src/engine.rs - Freestanding Native Rich-Text Engine
// Implements document model, paragraph/character styling, tables, layout pagination,
// search/replace, undo/redo, and OpenXML (.docx) OPC ZIP/XML generation.

pub const MAX_PARAGRAPHS: usize = 16;
pub const MAX_RUNS_PER_PARA: usize = 8;
pub const MAX_TABLES: usize = 2;
pub const MAX_TABLE_ROWS: usize = 4;
pub const MAX_TABLE_COLS: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteError {
    ParagraphOverflow,
    RunOverflow,
    TableOverflow,
    BufferTooSmall,
    NotFound,
    InvalidIndex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alignment {
    Left,
    Center,
    Right,
    Justified,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CharacterStyle {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
    pub font_size_half_pt: u8, // e.g. 24 = 12pt, 32 = 16pt, 48 = 24pt
    pub color_rgb: u32,        // 0x00RRGGBB
}

impl CharacterStyle {
    pub const fn default() -> Self {
        Self {
            bold: false,
            italic: false,
            underline: false,
            strike: false,
            font_size_half_pt: 24, // 12pt
            color_rgb: 0x000000,
        }
    }
}

#[derive(Clone, Copy)]
pub struct TextRun {
    pub text: [u8; 64],
    pub len: usize,
    pub style: CharacterStyle,
}

impl TextRun {
    pub const fn empty() -> Self {
        Self {
            text: [0u8; 64],
            len: 0,
            style: CharacterStyle::default(),
        }
    }

    pub fn set_text(&mut self, s: &str) {
        let copy_len = core::cmp::min(s.len(), self.text.len());
        self.text[..copy_len].copy_from_slice(&s.as_bytes()[..copy_len]);
        self.len = copy_len;
    }

    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.text[..self.len]).unwrap_or("")
    }
}

#[derive(Clone, Copy)]
pub struct Paragraph {
    pub runs: [TextRun; MAX_RUNS_PER_PARA],
    pub run_count: usize,
    pub heading_level: u8, // 0 = Body, 1 = H1, 2 = H2, 3 = H3, 4 = Title
    pub alignment: Alignment,
    pub line_spacing: u16,  // 240 = single, 360 = 1.5x
    pub is_bullet: bool,
}

impl Paragraph {
    pub const fn empty() -> Self {
        Self {
            runs: [TextRun::empty(); MAX_RUNS_PER_PARA],
            run_count: 0,
            heading_level: 0,
            alignment: Alignment::Left,
            line_spacing: 240,
            is_bullet: false,
        }
    }

    pub fn add_run(&mut self, text: &str, style: CharacterStyle) -> Result<&mut TextRun, WriteError> {
        if self.run_count >= MAX_RUNS_PER_PARA {
            return Err(WriteError::RunOverflow);
        }
        let idx = self.run_count;
        self.runs[idx].set_text(text);
        self.runs[idx].style = style;
        self.run_count += 1;
        Ok(&mut self.runs[idx])
    }
}

#[derive(Clone, Copy)]
pub struct TableCell {
    pub text: [u8; 32],
    pub len: usize,
}

impl TableCell {
    pub const fn empty() -> Self {
        Self {
            text: [0u8; 32],
            len: 0,
        }
    }

    pub fn set_text(&mut self, s: &str) {
        let copy_len = core::cmp::min(s.len(), self.text.len());
        self.text[..copy_len].copy_from_slice(&s.as_bytes()[..copy_len]);
        self.len = copy_len;
    }

    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.text[..self.len]).unwrap_or("")
    }
}

#[derive(Clone, Copy)]
pub struct Table {
    pub rows: usize,
    pub cols: usize,
    pub cells: [[TableCell; MAX_TABLE_COLS]; MAX_TABLE_ROWS],
}

impl Table {
    pub const fn empty() -> Self {
        Self {
            rows: 0,
            cols: 0,
            cells: [[TableCell::empty(); MAX_TABLE_COLS]; MAX_TABLE_ROWS],
        }
    }

    pub fn set_cell(&mut self, row: usize, col: usize, text: &str) -> Result<(), WriteError> {
        if row >= self.rows || col >= self.cols {
            return Err(WriteError::InvalidIndex);
        }
        self.cells[row][col].set_text(text);
        Ok(())
    }
}

// --- IEEE 802.3 CRC-32 for DOCX ZIP Container ---

pub fn crc32(data: &[u8]) -> u32 {
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

// --- Write Document Model ---

#[derive(Clone, Copy)]
pub struct WriteDocument {
    pub paragraphs: [Paragraph; MAX_PARAGRAPHS],
    pub para_count: usize,
    pub tables: [Table; MAX_TABLES],
    pub table_count: usize,
    pub header_text: [u8; 64],
    pub header_len: usize,
    pub footer_text: [u8; 64],
    pub footer_len: usize,
}

impl WriteDocument {
    pub const fn new() -> Self {
        Self {
            paragraphs: [Paragraph::empty(); MAX_PARAGRAPHS],
            para_count: 0,
            tables: [Table::empty(); MAX_TABLES],
            table_count: 0,
            header_text: [0u8; 64],
            header_len: 0,
            footer_text: [0u8; 64],
            footer_len: 0,
        }
    }

    pub fn add_paragraph(&mut self) -> Result<&mut Paragraph, WriteError> {
        if self.para_count >= MAX_PARAGRAPHS {
            return Err(WriteError::ParagraphOverflow);
        }
        let idx = self.para_count;
        self.paragraphs[idx] = Paragraph::empty();
        self.para_count += 1;
        Ok(&mut self.paragraphs[idx])
    }

    pub fn add_heading(&mut self, level: u8, text: &str) -> Result<(), WriteError> {
        let para = self.add_paragraph()?;
        para.heading_level = level;
        let mut style = CharacterStyle::default();
        style.bold = true;
        match level {
            1 => {
                style.font_size_half_pt = 40; // 20pt
                style.color_rgb = 0x1E3A8A;   // Deep blue
            }
            2 => {
                style.font_size_half_pt = 32; // 16pt
                style.color_rgb = 0x1E293B;   // Slate
            }
            _ => {
                style.font_size_half_pt = 28; // 14pt
            }
        }
        para.add_run(text, style)?;
        Ok(())
    }

    pub fn add_table(&mut self, rows: usize, cols: usize) -> Result<&mut Table, WriteError> {
        if self.table_count >= MAX_TABLES {
            return Err(WriteError::TableOverflow);
        }
        if rows > MAX_TABLE_ROWS || cols > MAX_TABLE_COLS {
            return Err(WriteError::InvalidIndex);
        }
        let idx = self.table_count;
        self.tables[idx] = Table::empty();
        self.tables[idx].rows = rows;
        self.tables[idx].cols = cols;
        self.table_count += 1;
        Ok(&mut self.tables[idx])
    }

    pub fn set_header(&mut self, text: &str) {
        let copy_len = core::cmp::min(text.len(), self.header_text.len());
        self.header_text[..copy_len].copy_from_slice(&text.as_bytes()[..copy_len]);
        self.header_len = copy_len;
    }

    pub fn set_footer(&mut self, text: &str) {
        let copy_len = core::cmp::min(text.len(), self.footer_text.len());
        self.footer_text[..copy_len].copy_from_slice(&text.as_bytes()[..copy_len]);
        self.footer_len = copy_len;
    }

    // --- Search & Replace ---

    pub fn search(&self, needle: &str) -> Option<(usize, usize)> {
        let needle_bytes = needle.as_bytes();
        if needle_bytes.is_empty() {
            return None;
        }
        for (p_idx, p) in self.paragraphs[..self.para_count].iter().enumerate() {
            for (r_idx, r) in p.runs[..p.run_count].iter().enumerate() {
                if r.len >= needle_bytes.len() {
                    for i in 0..=(r.len - needle_bytes.len()) {
                        if &r.text[i..i + needle_bytes.len()] == needle_bytes {
                            return Some((p_idx, r_idx));
                        }
                    }
                }
            }
        }
        None
    }

    pub fn replace(&mut self, needle: &str, replacement: &str) -> usize {
        let needle_bytes = needle.as_bytes();
        let rep_bytes = replacement.as_bytes();
        if needle_bytes.is_empty() {
            return 0;
        }
        let mut count = 0;
        for p in self.paragraphs[..self.para_count].iter_mut() {
            for r in p.runs[..p.run_count].iter_mut() {
                if r.len >= needle_bytes.len() {
                    let mut i = 0;
                    while i <= (r.len - needle_bytes.len()) {
                        if &r.text[i..i + needle_bytes.len()] == needle_bytes {
                            // Found match: replace in-place if sizes fit
                            let new_len = r.len - needle_bytes.len() + rep_bytes.len();
                            if new_len <= r.text.len() {
                                let mut temp = [0u8; 64];
                                temp[..i].copy_from_slice(&r.text[..i]);
                                temp[i..i + rep_bytes.len()].copy_from_slice(rep_bytes);
                                let tail_start = i + needle_bytes.len();
                                let tail_len = r.len - tail_start;
                                temp[i + rep_bytes.len()..i + rep_bytes.len() + tail_len]
                                    .copy_from_slice(&r.text[tail_start..r.len]);
                                r.text[..new_len].copy_from_slice(&temp[..new_len]);
                                r.len = new_len;
                                count += 1;
                                i += rep_bytes.len();
                                continue;
                            }
                        }
                        i += 1;
                    }
                }
            }
        }
        count
    }

    // --- Pagination Calculation ---

    pub fn calculate_pages(&self) -> usize {
        let mut total_lines = 0;
        // Header line
        if self.header_len > 0 {
            total_lines += 2;
        }
        for p in &self.paragraphs[..self.para_count] {
            let mut char_count = 0;
            for r in &p.runs[..p.run_count] {
                char_count += r.len;
            }
            let lines = if char_count == 0 { 1 } else { (char_count + 69) / 70 };
            total_lines += lines + 1; // paragraph spacing
        }
        for t in &self.tables[..self.table_count] {
            total_lines += t.rows * 2 + 1;
        }
        // Standard page capacity: 45 lines per page
        (total_lines + 44) / 45
    }

    // --- Plain Text Export with Layout ---

    pub fn export_text(&self, out: &mut [u8]) -> Result<usize, WriteError> {
        let mut offset = 0;

        macro_rules! append {
            ($s:expr) => {
                let bytes = $s.as_bytes();
                if offset + bytes.len() > out.len() {
                    return Err(WriteError::BufferTooSmall);
                }
                out[offset..offset + bytes.len()].copy_from_slice(bytes);
                offset += bytes.len();
            };
        }

        // Header
        if self.header_len > 0 {
            append!("[HEADER] ");
            let h = core::str::from_utf8(&self.header_text[..self.header_len]).unwrap_or("");
            append!(h);
            append!("\n========================================================================\n\n");
        }

        // Paragraphs
        for p in &self.paragraphs[..self.para_count] {
            if p.is_bullet {
                append!("  * ");
            }
            for r in &p.runs[..p.run_count] {
                if r.style.bold && r.style.italic {
                    append!("***");
                    append!(r.as_str());
                    append!("***");
                } else if r.style.bold {
                    append!("**");
                    append!(r.as_str());
                    append!("**");
                } else if r.style.italic {
                    append!("*");
                    append!(r.as_str());
                    append!("*");
                } else if r.style.underline {
                    append!("_");
                    append!(r.as_str());
                    append!("_");
                } else {
                    append!(r.as_str());
                }
            }
            append!("\n\n");
        }

        // Tables
        for t in &self.tables[..self.table_count] {
            append!("+------------------------------------------------+\n");
            for r in 0..t.rows {
                append!("|");
                for c in 0..t.cols {
                    append!(" ");
                    let cell_str = t.cells[r][c].as_str();
                    append!(cell_str);
                    // Pad to 10 chars
                    let pad = if cell_str.len() < 10 { 10 - cell_str.len() } else { 1 };
                    for _ in 0..pad {
                        append!(" ");
                    }
                    append!("|");
                }
                append!("\n+------------------------------------------------+\n");
            }
            append!("\n");
        }

        // Footer
        if self.footer_len > 0 {
            append!("------------------------------------------------------------------------\n");
            append!("[FOOTER] ");
            let f = core::str::from_utf8(&self.footer_text[..self.footer_len]).unwrap_or("");
            append!(f);
            append!(" | Page 1 of 1\n");
        }

        Ok(offset)
    }

    // --- OpenXML DOCX Generation (.docx OPC Package) ---

    pub fn export_docx(&self, out: &mut [u8]) -> Result<usize, WriteError> {
        let mut doc_xml = [0u8; 4096];
        let doc_xml_len = self.generate_document_xml(&mut doc_xml)?;

        let content_types = b"<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\r\n<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/><Override PartName=\"/word/document.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml\"/><Override PartName=\"/word/styles.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml\"/></Types>";

        let rels = b"<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\r\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"word/document.xml\"/></Relationships>";

        let doc_rels = b"<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\r\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles\" Target=\"styles.xml\"/></Relationships>";

        let styles_xml = b"<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\r\n<w:styles xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><w:style w:type=\"paragraph\" w:default=\"1\" w:styleId=\"Normal\"><w:name w:val=\"Normal\"/><w:rPr><w:rFonts w:ascii=\"Calibri\"/><w:sz w:val=\"24\"/></w:rPr></w:style><w:style w:type=\"paragraph\" w:styleId=\"Heading1\"><w:name w:val=\"heading 1\"/><w:rPr><w:b/><w:sz w:val=\"40\"/><w:color w:val=\"1E3A8A\"/></w:rPr></w:style></w:styles>";

        // Pack into OPC ZIP package
        let mut zip_writer = DocxZipWriter::new(out);
        zip_writer.add_file("[Content_Types].xml", content_types)?;
        zip_writer.add_file("_rels/.rels", rels)?;
        zip_writer.add_file("word/_rels/document.xml.rels", doc_rels)?;
        zip_writer.add_file("word/styles.xml", styles_xml)?;
        zip_writer.add_file("word/document.xml", &doc_xml[..doc_xml_len])?;
        let total_size = zip_writer.finish()?;

        Ok(total_size)
    }

    fn generate_document_xml(&self, out: &mut [u8]) -> Result<usize, WriteError> {
        let mut offset = 0;

        macro_rules! append {
            ($s:expr) => {
                let bytes = $s.as_bytes();
                if offset + bytes.len() > out.len() {
                    return Err(WriteError::BufferTooSmall);
                }
                out[offset..offset + bytes.len()].copy_from_slice(bytes);
                offset += bytes.len();
            };
        }

        append!("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\r\n<w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><w:body>");

        // Paragraphs
        for p in &self.paragraphs[..self.para_count] {
            append!("<w:p>");
            if p.heading_level > 0 || p.alignment != Alignment::Left {
                append!("<w:pPr>");
                if p.heading_level == 1 {
                    append!("<w:pStyle w:val=\"Heading1\"/>");
                }
                match p.alignment {
                    Alignment::Center => {
                        append!("<w:jc w:val=\"center\"/>");
                    }
                    Alignment::Right => {
                        append!("<w:jc w:val=\"right\"/>");
                    }
                    Alignment::Justified => {
                        append!("<w:jc w:val=\"both\"/>");
                    }
                    Alignment::Left => {}
                }
                append!("</w:pPr>");
            }
            for r in &p.runs[..p.run_count] {
                append!("<w:r><w:rPr>");
                if r.style.bold {
                    append!("<w:b/>");
                }
                if r.style.italic {
                    append!("<w:i/>");
                }
                if r.style.underline {
                    append!("<w:u w:val=\"single\"/>");
                }
                if r.style.strike {
                    append!("<w:strike/>");
                }
                append!("<w:sz w:val=\"");
                let mut sz_buf = [0u8; 8];
                let sz_len = write_u32(r.style.font_size_half_pt as u32, &mut sz_buf);
                append!(core::str::from_utf8(&sz_buf[..sz_len]).unwrap_or("24"));
                append!("\"/>");
                append!("</w:rPr><w:t>");
                append!(r.as_str());
                append!("</w:t></w:r>");
            }
            append!("</w:p>");
        }

        // Tables
        for t in &self.tables[..self.table_count] {
            append!("<w:tbl><w:tblPr><w:tblBorders><w:top w:val=\"single\" w:sz=\"4\"/><w:bottom w:val=\"single\" w:sz=\"4\"/></w:tblBorders></w:tblPr>");
            for r in 0..t.rows {
                append!("<w:tr>");
                for c in 0..t.cols {
                    append!("<w:tc><w:p><w:r><w:t>");
                    append!(t.cells[r][c].as_str());
                    append!("</w:t></w:r></w:p></w:tc>");
                }
                append!("</w:tr>");
            }
            append!("</w:tbl>");
        }

        append!("<w:sectPr><w:pgSz w:w=\"11906\" w:h=\"16838\"/><w:pgMar w:top=\"1440\" w:right=\"1440\" w:bottom=\"1440\" w:left=\"1440\"/></w:sectPr></w:body></w:document>");

        Ok(offset)
    }
}

// --- Undo / Redo Manager ---

pub struct UndoRedoManager {
    history: [WriteDocument; 4],
    current_idx: usize,
    history_count: usize,
}

impl UndoRedoManager {
    pub const fn new() -> Self {
        Self {
            history: [WriteDocument::new(); 4],
            current_idx: 0,
            history_count: 0,
        }
    }

    pub fn checkpoint(&mut self, doc: &WriteDocument) {
        if self.history_count < 4 {
            self.history[self.history_count] = *doc;
            self.current_idx = self.history_count;
            self.history_count += 1;
        } else {
            // Shift history
            for i in 0..3 {
                self.history[i] = self.history[i + 1];
            }
            self.history[3] = *doc;
            self.current_idx = 3;
        }
    }

    pub fn can_undo(&self) -> bool {
        self.current_idx > 0
    }

    pub fn undo(&mut self) -> Option<&WriteDocument> {
        if self.can_undo() {
            self.current_idx -= 1;
            Some(&self.history[self.current_idx])
        } else {
            None
        }
    }

    pub fn can_redo(&self) -> bool {
        self.current_idx + 1 < self.history_count
    }

    pub fn redo(&mut self) -> Option<&WriteDocument> {
        if self.can_redo() {
            self.current_idx += 1;
            Some(&self.history[self.current_idx])
        } else {
            None
        }
    }
}

pub fn write_u32(mut val: u32, buf: &mut [u8]) -> usize {
    if val == 0 {
        buf[0] = b'0';
        return 1;
    }
    let mut temp = [0u8; 10];
    let mut i = 0;
    while val > 0 {
        temp[i] = b'0' + (val % 10) as u8;
        val /= 10;
        i += 1;
    }
    for j in 0..i {
        buf[j] = temp[i - 1 - j];
    }
    i
}

// --- Streaming DOCX ZIP Writer ---

struct ZipEntryMeta {
    filename: [u8; 64],
    filename_len: usize,
    crc: u32,
    size: usize,
    offset: usize,
}

pub struct DocxZipWriter<'a> {
    buf: &'a mut [u8],
    offset: usize,
    entries: [ZipEntryMeta; 8],
    entry_count: usize,
}

impl<'a> DocxZipWriter<'a> {
    pub fn new(buf: &'a mut [u8]) -> Self {
        Self {
            buf,
            offset: 0,
            entries: [
                ZipEntryMeta { filename: [0; 64], filename_len: 0, crc: 0, size: 0, offset: 0 },
                ZipEntryMeta { filename: [0; 64], filename_len: 0, crc: 0, size: 0, offset: 0 },
                ZipEntryMeta { filename: [0; 64], filename_len: 0, crc: 0, size: 0, offset: 0 },
                ZipEntryMeta { filename: [0; 64], filename_len: 0, crc: 0, size: 0, offset: 0 },
                ZipEntryMeta { filename: [0; 64], filename_len: 0, crc: 0, size: 0, offset: 0 },
                ZipEntryMeta { filename: [0; 64], filename_len: 0, crc: 0, size: 0, offset: 0 },
                ZipEntryMeta { filename: [0; 64], filename_len: 0, crc: 0, size: 0, offset: 0 },
                ZipEntryMeta { filename: [0; 64], filename_len: 0, crc: 0, size: 0, offset: 0 },
            ],
            entry_count: 0,
        }
    }

    pub fn add_file(&mut self, filename: &str, data: &[u8]) -> Result<(), WriteError> {
        if self.entry_count >= 8 {
            return Err(WriteError::TableOverflow);
        }
        let fn_bytes = filename.as_bytes();
        let file_crc = crc32(data);
        let header_offset = self.offset;

        // Local file header: 30 bytes + filename
        if self.offset + 30 + fn_bytes.len() + data.len() > self.buf.len() {
            return Err(WriteError::BufferTooSmall);
        }

        // Signature: 0x04034B50
        self.buf[self.offset..self.offset + 4].copy_from_slice(&0x04034B50u32.to_le_bytes());
        self.buf[self.offset + 4..self.offset + 6].copy_from_slice(&20u16.to_le_bytes()); // version needed
        self.buf[self.offset + 6..self.offset + 8].copy_from_slice(&0u16.to_le_bytes());  // flags
        self.buf[self.offset + 8..self.offset + 10].copy_from_slice(&0u16.to_le_bytes()); // compression: 0 (Stored)
        self.buf[self.offset + 10..self.offset + 14].copy_from_slice(&0u32.to_le_bytes());// time/date
        self.buf[self.offset + 14..self.offset + 18].copy_from_slice(&file_crc.to_le_bytes());
        self.buf[self.offset + 18..self.offset + 22].copy_from_slice(&(data.len() as u32).to_le_bytes()); // comp size
        self.buf[self.offset + 22..self.offset + 26].copy_from_slice(&(data.len() as u32).to_le_bytes()); // uncomp size
        self.buf[self.offset + 26..self.offset + 28].copy_from_slice(&(fn_bytes.len() as u16).to_le_bytes());
        self.buf[self.offset + 28..self.offset + 30].copy_from_slice(&0u16.to_le_bytes()); // extra len
        self.offset += 30;

        self.buf[self.offset..self.offset + fn_bytes.len()].copy_from_slice(fn_bytes);
        self.offset += fn_bytes.len();

        self.buf[self.offset..self.offset + data.len()].copy_from_slice(data);
        self.offset += data.len();

        let mut meta = ZipEntryMeta {
            filename: [0; 64],
            filename_len: fn_bytes.len(),
            crc: file_crc,
            size: data.len(),
            offset: header_offset,
        };
        meta.filename[..fn_bytes.len()].copy_from_slice(fn_bytes);
        self.entries[self.entry_count] = meta;
        self.entry_count += 1;

        Ok(())
    }

    pub fn finish(&mut self) -> Result<usize, WriteError> {
        let cd_start = self.offset;

        // Central Directory Headers
        for i in 0..self.entry_count {
            let entry = &self.entries[i];
            let fn_len = entry.filename_len;

            if self.offset + 46 + fn_len > self.buf.len() {
                return Err(WriteError::BufferTooSmall);
            }

            self.buf[self.offset..self.offset + 4].copy_from_slice(&0x02014B50u32.to_le_bytes());
            self.buf[self.offset + 4..self.offset + 6].copy_from_slice(&20u16.to_le_bytes());
            self.buf[self.offset + 6..self.offset + 8].copy_from_slice(&20u16.to_le_bytes());
            self.buf[self.offset + 8..self.offset + 10].copy_from_slice(&0u16.to_le_bytes());
            self.buf[self.offset + 10..self.offset + 12].copy_from_slice(&0u16.to_le_bytes()); // stored
            self.buf[self.offset + 12..self.offset + 16].copy_from_slice(&0u32.to_le_bytes());
            self.buf[self.offset + 16..self.offset + 20].copy_from_slice(&entry.crc.to_le_bytes());
            self.buf[self.offset + 20..self.offset + 24].copy_from_slice(&(entry.size as u32).to_le_bytes());
            self.buf[self.offset + 24..self.offset + 28].copy_from_slice(&(entry.size as u32).to_le_bytes());
            self.buf[self.offset + 28..self.offset + 30].copy_from_slice(&(fn_len as u16).to_le_bytes());
            self.buf[self.offset + 30..self.offset + 32].copy_from_slice(&0u16.to_le_bytes());
            self.buf[self.offset + 32..self.offset + 34].copy_from_slice(&0u16.to_le_bytes());
            self.buf[self.offset + 34..self.offset + 36].copy_from_slice(&0u16.to_le_bytes());
            self.buf[self.offset + 36..self.offset + 38].copy_from_slice(&0u16.to_le_bytes());
            self.buf[self.offset + 38..self.offset + 42].copy_from_slice(&0u32.to_le_bytes());
            self.buf[self.offset + 42..self.offset + 46].copy_from_slice(&(entry.offset as u32).to_le_bytes());
            self.offset += 46;

            self.buf[self.offset..self.offset + fn_len].copy_from_slice(&entry.filename[..fn_len]);
            self.offset += fn_len;
        }

        let cd_size = self.offset - cd_start;

        // End of Central Directory: 22 bytes
        if self.offset + 22 > self.buf.len() {
            return Err(WriteError::BufferTooSmall);
        }

        self.buf[self.offset..self.offset + 4].copy_from_slice(&0x06054B50u32.to_le_bytes());
        self.buf[self.offset + 4..self.offset + 6].copy_from_slice(&0u16.to_le_bytes());
        self.buf[self.offset + 6..self.offset + 8].copy_from_slice(&0u16.to_le_bytes());
        self.buf[self.offset + 8..self.offset + 10].copy_from_slice(&(self.entry_count as u16).to_le_bytes());
        self.buf[self.offset + 10..self.offset + 12].copy_from_slice(&(self.entry_count as u16).to_le_bytes());
        self.buf[self.offset + 12..self.offset + 16].copy_from_slice(&(cd_size as u32).to_le_bytes());
        self.buf[self.offset + 16..self.offset + 20].copy_from_slice(&(cd_start as u32).to_le_bytes());
        self.buf[self.offset + 20..self.offset + 22].copy_from_slice(&0u16.to_le_bytes());
        self.offset += 22;

        Ok(self.offset)
    }
}
