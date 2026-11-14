// userspace/apps/pdf_app/src/engine.rs
// Freestanding Native PDF 1.7 / 2.0 Document Engine
// Zero Python - 100% Freestanding Bare-Metal Rust (No Heap / No Dynamic Allocations)

use core::cmp::min;

pub const MAX_OBJECTS: usize = 32;
pub const MAX_TEXT_RUNS: usize = 16;
pub const MAX_SEARCH_RESULTS: usize = 8;
pub const MAX_NAME_LEN: usize = 32;
pub const MAX_STRING_LEN: usize = 96;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdfError {
    BufferTooSmall,
    ObjectOverflow,
    ParseError,
    CorruptStream,
    InvalidHeader,
    NotFound,
}

// =========================================================================
// 1. PDF OBJECT MODEL & LEXER (Contract P-01)
// =========================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdfObjectType {
    Null,
    Boolean(bool),
    Integer(i64),
    Name([u8; MAX_NAME_LEN], usize),
    StringVal([u8; MAX_STRING_LEN], usize),
    Ref(u32, u16), // obj_num, generation
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XrefEntry {
    pub obj_num: u32,
    pub offset: u64,
    pub generation: u16,
    pub in_use: bool,
}

pub fn parse_xref_table(data: &[u8], entries_out: &mut [XrefEntry]) -> Result<usize, PdfError> {
    // Scan for "xref" keyword
    let mut pos = 0;
    let mut found_xref = false;
    while pos + 4 <= data.len() {
        if &data[pos..pos + 4] == b"xref" {
            found_xref = true;
            pos += 4;
            break;
        }
        pos += 1;
    }

    if !found_xref {
        return Err(PdfError::NotFound);
    }

    // Skip whitespace / newlines
    while pos < data.len() && (data[pos] == b' ' || data[pos] == b'\r' || data[pos] == b'\n') {
        pos += 1;
    }

    // Read subsection header: start_obj count
    let mut start_obj = 0u32;
    while pos < data.len() && data[pos] >= b'0' && data[pos] <= b'9' {
        start_obj = start_obj * 10 + (data[pos] - b'0') as u32;
        pos += 1;
    }
    while pos < data.len() && data[pos] == b' ' {
        pos += 1;
    }
    let mut count = 0u32;
    while pos < data.len() && data[pos] >= b'0' && data[pos] <= b'9' {
        count = count * 10 + (data[pos] - b'0') as u32;
        pos += 1;
    }
    while pos < data.len() && (data[pos] == b'\r' || data[pos] == b'\n') {
        pos += 1;
    }

    let mut entry_count = 0;
    for i in 0..count {
        if pos + 20 > data.len() || entry_count >= entries_out.len() {
            break;
        }
        let line = &data[pos..pos + 20];
        pos += 20;

        // 10 digits offset, 1 space, 5 digits gen, 1 space, 'n'/'f', 2 byte newline
        let mut offset = 0u64;
        for j in 0..10 {
            if line[j] >= b'0' && line[j] <= b'9' {
                offset = offset * 10 + (line[j] - b'0') as u64;
            }
        }
        let mut gen = 0u16;
        for j in 11..16 {
            if line[j] >= b'0' && line[j] <= b'9' {
                gen = gen * 10 + (line[j] - b'0') as u16;
            }
        }
        let in_use = line[17] == b'n';

        entries_out[entry_count] = XrefEntry {
            obj_num: start_obj + i,
            offset,
            generation: gen,
            in_use,
        };
        entry_count += 1;
    }

    Ok(entry_count)
}

// =========================================================================
// 2. STREAM DECODERS - FlateDecode & ASCIIHex (Contract P-02)
// =========================================================================

pub struct BitReader<'a> {
    data: &'a [u8],
    byte_pos: usize,
    bit_buf: u32,
    bits_in_buf: u8,
}

impl<'a> BitReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            byte_pos: 0,
            bit_buf: 0,
            bits_in_buf: 0,
        }
    }

    pub fn read_bits(&mut self, n: u8) -> Result<u32, PdfError> {
        while self.bits_in_buf < n {
            if self.byte_pos >= self.data.len() {
                return Err(PdfError::CorruptStream);
            }
            self.bit_buf |= (self.data[self.byte_pos] as u32) << self.bits_in_buf;
            self.byte_pos += 1;
            self.bits_in_buf += 8;
        }
        let val = self.bit_buf & ((1 << n) - 1);
        self.bit_buf >>= n;
        self.bits_in_buf -= n;
        Ok(val)
    }

    pub fn align_to_byte(&mut self) {
        self.bit_buf = 0;
        self.bits_in_buf = 0;
    }

    pub fn read_bytes(&mut self, out: &mut [u8]) -> Result<(), PdfError> {
        self.bit_buf = 0;
        self.bits_in_buf = 0;
        if self.byte_pos + out.len() > self.data.len() {
            return Err(PdfError::BufferTooSmall);
        }
        out.copy_from_slice(&self.data[self.byte_pos..self.byte_pos + out.len()]);
        self.byte_pos += out.len();
        Ok(())
    }

    pub fn read_u16_le(&mut self) -> Result<u16, PdfError> {
        let mut b = [0u8; 2];
        self.read_bytes(&mut b)?;
        Ok((b[0] as u16) | ((b[1] as u16) << 8))
    }
}

pub fn decode_flate(compressed: &[u8], output: &mut [u8]) -> Result<usize, PdfError> {
    if compressed.len() < 6 {
        return Err(PdfError::CorruptStream);
    }
    // Check zlib header (RFC 1950)
    let cmf = compressed[0];
    let flg = compressed[1];
    if (cmf & 0x0F) != 8 {
        return Err(PdfError::CorruptStream); // Not deflate
    }
    if ((cmf as u16 * 256 + flg as u16) % 31) != 0 {
        return Err(PdfError::CorruptStream); // Invalid header checksum
    }

    // Skip 2-byte header, exclude 4-byte Adler32 trailer
    let raw_deflate = &compressed[2..compressed.len() - 4];
    let mut reader = BitReader::new(raw_deflate);
    let mut out_pos = 0;
    let mut is_final = false;

    while !is_final {
        let bfinal = reader.read_bits(1)?;
        is_final = bfinal == 1;

        let btype = reader.read_bits(2)?;
        match btype {
            0 => {
                // Stored / uncompressed block: align to byte boundary per RFC 1951 section 3.2.4
                reader.align_to_byte();
                let len = reader.read_u16_le()? as usize;
                let nlen = reader.read_u16_le()? as usize;
                if (len ^ 0xFFFF) != nlen {
                    return Err(PdfError::CorruptStream);
                }
                if out_pos + len > output.len() {
                    return Err(PdfError::BufferTooSmall);
                }
                reader.read_bytes(&mut output[out_pos..out_pos + len])?;
                out_pos += len;
            }
            1 => {
                // Fixed Huffman codes
                loop {
                    let symbol = decode_fixed_symbol(&mut reader)?;
                    if symbol < 256 {
                        if out_pos >= output.len() {
                            return Err(PdfError::BufferTooSmall);
                        }
                        output[out_pos] = symbol as u8;
                        out_pos += 1;
                    } else if symbol == 256 {
                        break;
                    } else if symbol <= 285 {
                        let length = decode_length(symbol, &mut reader)?;
                        let distance = decode_distance(&mut reader)?;
                        if distance > out_pos || out_pos + length > output.len() {
                            return Err(PdfError::CorruptStream);
                        }
                        for _ in 0..length {
                            let b = output[out_pos - distance];
                            output[out_pos] = b;
                            out_pos += 1;
                        }
                    } else {
                        return Err(PdfError::CorruptStream);
                    }
                }
            }
            _ => {
                // Stored and fixed Huffman covered. Fallback uncompressed copy if applicable
                return Err(PdfError::CorruptStream);
            }
        }
    }

    Ok(out_pos)
}

fn decode_fixed_symbol(reader: &mut BitReader) -> Result<u32, PdfError> {
    let mut code = 0u32;
    for _ in 0..7 {
        code = (code << 1) | reader.read_bits(1)?;
    }
    if code <= 0b0010111 {
        return Ok(code + 256);
    }
    code = (code << 1) | reader.read_bits(1)?;
    if code <= 0b10111111 {
        return Ok(code - 0b00110000);
    }
    if code <= 0b11000111 {
        return Ok(code - 0b11000000 + 280);
    }
    code = (code << 1) | reader.read_bits(1)?;
    Ok(code - 0b110010000 + 144)
}

fn decode_length(symbol: u32, reader: &mut BitReader) -> Result<usize, PdfError> {
    if symbol < 257 || symbol > 285 {
        return Err(PdfError::CorruptStream);
    }
    let code = symbol - 257;
    let base_lengths: [usize; 29] = [
        3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31,
        35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258,
    ];
    let extra_bits: [u8; 29] = [
        0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2,
        3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
    ];
    let extra = if extra_bits[code as usize] > 0 {
        reader.read_bits(extra_bits[code as usize])? as usize
    } else {
        0
    };
    Ok(base_lengths[code as usize] + extra)
}

fn decode_distance(reader: &mut BitReader) -> Result<usize, PdfError> {
    let mut code = 0u32;
    for _ in 0..5 {
        code = (code << 1) | reader.read_bits(1)?;
    }
    let base_dist: [usize; 30] = [
        1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193,
        257, 385, 513, 769, 1025, 1537, 2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
    ];
    let extra_bits: [u8; 30] = [
        0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6,
        7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13,
    ];
    let idx = min(code as usize, 29);
    let extra = if extra_bits[idx] > 0 {
        reader.read_bits(extra_bits[idx])? as usize
    } else {
        0
    };
    Ok(base_dist[idx] + extra)
}

pub fn decode_ascii_hex(hex_data: &[u8], output: &mut [u8]) -> Result<usize, PdfError> {
    let mut out_pos = 0;
    let mut high_nibble: Option<u8> = None;

    for &b in hex_data {
        if b == b'>' {
            break;
        }
        let val = match b {
            b'0'..=b'9' => b - b'0',
            b'a'..=b'f' => b - b'a' + 10,
            b'A'..=b'F' => b - b'A' + 10,
            b' ' | b'\r' | b'\n' | b'\t' => continue,
            _ => return Err(PdfError::ParseError),
        };
        if let Some(high) = high_nibble {
            if out_pos >= output.len() {
                return Err(PdfError::BufferTooSmall);
            }
            output[out_pos] = (high << 4) | val;
            out_pos += 1;
            high_nibble = None;
        } else {
            high_nibble = Some(val);
        }
    }

    if let Some(high) = high_nibble {
        if out_pos >= output.len() {
            return Err(PdfError::BufferTooSmall);
        }
        output[out_pos] = high << 4;
        out_pos += 1;
    }

    Ok(out_pos)
}

// =========================================================================
// 3. CONTENT STREAM INTERPRETER & TEXT EXTRACTION (Contract P-03)
// =========================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextRun {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub text: [u8; MAX_STRING_LEN],
    pub text_len: usize,
}

impl TextRun {
    pub fn text_str(&self) -> &str {
        core::str::from_utf8(&self.text[..self.text_len]).unwrap_or("")
    }
}

pub fn interpret_content_stream(stream: &[u8], runs_out: &mut [TextRun]) -> usize {
    let mut pos = 0;
    let mut run_count = 0;

    let mut in_text = false;
    let mut curr_x: i32 = 0;
    let mut curr_y: i32 = 0;
    let mut font_size: i32 = 12;

    while pos < stream.len() && run_count < runs_out.len() {
        // Skip whitespace
        while pos < stream.len() && (stream[pos] == b' ' || stream[pos] == b'\r' || stream[pos] == b'\n') {
            pos += 1;
        }
        if pos >= stream.len() {
            break;
        }

        if pos + 2 <= stream.len() && &stream[pos..pos + 2] == b"BT" {
            in_text = true;
            curr_x = 0;
            curr_y = 0;
            pos += 2;
            continue;
        }

        if pos + 2 <= stream.len() && &stream[pos..pos + 2] == b"ET" {
            in_text = false;
            pos += 2;
            continue;
        }

        // Set Font: /F1 12 Tf
        if stream[pos] == b'/' {
            let start = pos;
            while pos < stream.len() && stream[pos] != b' ' {
                pos += 1;
            }
            // Next is size
            while pos < stream.len() && stream[pos] == b' ' {
                pos += 1;
            }
            let mut sz = 0i32;
            while pos < stream.len() && stream[pos] >= b'0' && stream[pos] <= b'9' {
                sz = sz * 10 + (stream[pos] - b'0') as i32;
                pos += 1;
            }
            while pos < stream.len() && stream[pos] == b' ' {
                pos += 1;
            }
            if pos + 2 <= stream.len() && &stream[pos..pos + 2] == b"Tf" {
                font_size = if sz > 0 { sz } else { 12 };
                pos += 2;
                continue;
            } else {
                pos = start + 1;
            }
        }

        // Text Position: X Y Td
        if stream[pos] >= b'0' && stream[pos] <= b'9' || stream[pos] == b'-' {
            let start = pos;
            let parse_int = |p: &mut usize| -> Option<i32> {
                let neg = if *p < stream.len() && stream[*p] == b'-' {
                    *p += 1;
                    true
                } else {
                    false
                };
                let mut v = 0i32;
                let mut digits = false;
                while *p < stream.len() && stream[*p] >= b'0' && stream[*p] <= b'9' {
                    v = v * 10 + (stream[*p] - b'0') as i32;
                    *p += 1;
                    digits = true;
                }
                if digits {
                    Some(if neg { -v } else { v })
                } else {
                    None
                }
            };

            let mut tmp_pos = pos;
            if let Some(x) = parse_int(&mut tmp_pos) {
                while tmp_pos < stream.len() && stream[tmp_pos] == b' ' {
                    tmp_pos += 1;
                }
                if let Some(y) = parse_int(&mut tmp_pos) {
                    while tmp_pos < stream.len() && stream[tmp_pos] == b' ' {
                        tmp_pos += 1;
                    }
                    if tmp_pos + 2 <= stream.len() && &stream[tmp_pos..tmp_pos + 2] == b"Td" {
                        curr_x += x;
                        curr_y += y;
                        pos = tmp_pos + 2;
                        continue;
                    }
                }
            }
            pos = start + 1;
        }

        // Text String: (Hello World) Tj
        if in_text && stream[pos] == b'(' {
            pos += 1;
            let mut str_buf = [0u8; MAX_STRING_LEN];
            let mut str_len = 0;
            while pos < stream.len() && stream[pos] != b')' {
                if stream[pos] == b'\\' && pos + 1 < stream.len() {
                    pos += 1;
                }
                if str_len < MAX_STRING_LEN {
                    str_buf[str_len] = stream[pos];
                    str_len += 1;
                }
                pos += 1;
            }
            if pos < stream.len() && stream[pos] == b')' {
                pos += 1;
            }
            while pos < stream.len() && stream[pos] == b' ' {
                pos += 1;
            }
            if pos + 2 <= stream.len() && &stream[pos..pos + 2] == b"Tj" {
                pos += 2;
                let est_width = (str_len as i32) * (font_size / 2);
                runs_out[run_count] = TextRun {
                    x: curr_x,
                    y: curr_y,
                    width: est_width,
                    height: font_size,
                    text: str_buf,
                    text_len: str_len,
                };
                run_count += 1;
                curr_x += est_width;
                continue;
            }
        }

        pos += 1;
    }

    run_count
}

pub fn search_text_runs(
    runs: &[TextRun],
    query: &str,
    results_out: &mut [(i32, i32, usize)],
) -> usize {
    let mut count = 0;
    let q_bytes = query.as_bytes();
    if q_bytes.is_empty() {
        return 0;
    }

    for (run_idx, run) in runs.iter().enumerate() {
        if run.text_len < q_bytes.len() {
            continue;
        }
        for i in 0..=(run.text_len - q_bytes.len()) {
            if &run.text[i..i + q_bytes.len()] == q_bytes {
                if count < results_out.len() {
                    results_out[count] = (run.x, run.y, run_idx);
                    count += 1;
                }
            }
        }
    }
    count
}

// =========================================================================
// 4. PDF 1.7 DOCUMENT GENERATOR (Contract P-04)
// =========================================================================

pub struct PdfWriter<'a> {
    buffer: &'a mut [u8],
    pos: usize,
    offsets: [u64; MAX_OBJECTS],
}

impl<'a> PdfWriter<'a> {
    pub fn new(buffer: &'a mut [u8]) -> Self {
        Self {
            buffer,
            pos: 0,
            offsets: [0u64; MAX_OBJECTS],
        }
    }

    pub fn write_bytes(&mut self, data: &[u8]) -> Result<(), PdfError> {
        if self.pos + data.len() > self.buffer.len() {
            return Err(PdfError::BufferTooSmall);
        }
        self.buffer[self.pos..self.pos + data.len()].copy_from_slice(data);
        self.pos += data.len();
        Ok(())
    }

    pub fn generate_sample_document(&mut self, title: &str, body: &str) -> Result<usize, PdfError> {
        // PDF Header
        self.write_bytes(b"%PDF-1.7\r\n%\xE2\xE3\xCF\xD3\r\n")?;

        // Object 1: Catalog
        self.offsets[1] = self.pos as u64;
        self.write_bytes(b"1 0 obj\r\n<< /Type /Catalog /Pages 2 0 R >>\r\nendobj\r\n")?;

        // Object 2: Pages tree
        self.offsets[2] = self.pos as u64;
        self.write_bytes(b"2 0 obj\r\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\r\nendobj\r\n")?;

        // Object 3: Page 1
        self.offsets[3] = self.pos as u64;
        self.write_bytes(b"3 0 obj\r\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>\r\nendobj\r\n")?;

        // Construct Content Stream
        let mut stream_data = [0u8; 512];
        let mut sp = 0;
        let append_stream = |s: &[u8], buf: &mut [u8], p: &mut usize| {
            buf[*p..*p + s.len()].copy_from_slice(s);
            *p += s.len();
        };

        append_stream(b"BT\r\n/F1 18 Tf\r\n50 720 Td\r\n(", &mut stream_data, &mut sp);
        append_stream(title.as_bytes(), &mut stream_data, &mut sp);
        append_stream(b") Tj\r\nET\r\n", &mut stream_data, &mut sp);

        append_stream(b"BT\r\n/F1 12 Tf\r\n50 680 Td\r\n(", &mut stream_data, &mut sp);
        append_stream(body.as_bytes(), &mut stream_data, &mut sp);
        append_stream(b") Tj\r\nET\r\n", &mut stream_data, &mut sp);

        // Object 4: Contents
        self.offsets[4] = self.pos as u64;
        self.write_bytes(b"4 0 obj\r\n<< /Length ")?;
        let mut len_buf = [0u8; 16];
        let l_len = format_u32(sp as u32, &mut len_buf);
        self.write_bytes(&len_buf[..l_len])?;
        self.write_bytes(b" >>\r\nstream\r\n")?;
        self.write_bytes(&stream_data[..sp])?;
        self.write_bytes(b"\r\nendstream\r\nendobj\r\n")?;

        // Object 5: Base14 Font Helvetica
        self.offsets[5] = self.pos as u64;
        self.write_bytes(b"5 0 obj\r\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\r\nendobj\r\n")?;

        // Xref Table
        let xref_offset = self.pos as u64;
        self.write_bytes(b"xref\r\n0 6\r\n")?;
        self.write_bytes(b"0000000000 65535 f\r\n")?;

        for i in 1..=5 {
            let mut entry = [0u8; 20];
            format_xref_entry(self.offsets[i], 0, true, &mut entry);
            self.write_bytes(&entry)?;
        }

        // Trailer
        self.write_bytes(b"trailer\r\n<< /Size 6 /Root 1 0 R >>\r\nstartxref\r\n")?;
        let mut xbuf = [0u8; 16];
        let xlen = format_u32(xref_offset as u32, &mut xbuf);
        self.write_bytes(&xbuf[..xlen])?;
        self.write_bytes(b"\r\n%%EOF\r\n")?;

        Ok(self.pos)
    }
}

pub fn format_xref_entry(offset: u64, gen: u16, in_use: bool, out: &mut [u8; 20]) {
    let mut off = offset;
    for i in (0..10).rev() {
        out[i] = b'0' + (off % 10) as u8;
        off /= 10;
    }
    out[10] = b' ';
    let mut g = gen;
    for i in (11..16).rev() {
        out[i] = b'0' + (g % 10) as u8;
        g /= 10;
    }
    out[16] = b' ';
    out[17] = if in_use { b'n' } else { b'f' };
    out[18] = b'\r';
    out[19] = b'\n';
}

pub fn format_u32(mut val: u32, buf: &mut [u8]) -> usize {
    if val == 0 {
        if !buf.is_empty() {
            buf[0] = b'0';
            return 1;
        }
        return 0;
    }
    let mut tmp = [0u8; 10];
    let mut i = 0;
    while val > 0 {
        tmp[i] = b'0' + (val % 10) as u8;
        val /= 10;
        i += 1;
    }
    let len = min(i, buf.len());
    for j in 0..len {
        buf[j] = tmp[i - 1 - j];
    }
    len
}
