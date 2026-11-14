// userspace/apps/mail_app/src/engine.rs
// Freestanding Native RFC 5322 / RFC 2045-2046 MIME Mail Engine & Maildir Store
// Zero Python - 100% Freestanding Bare-Metal Rust (No Heap / No Dynamic Allocations)

use core::cmp::min;

pub const MAX_HEADERS: usize = 16;
pub const MAX_PARTS: usize = 8;
pub const MAX_HEADER_LEN: usize = 64;
pub const MAX_VALUE_LEN: usize = 128;
pub const MAX_BODY_LEN: usize = 4096;
pub const MAX_ATTACHMENTS: usize = 4;
pub const MAX_FILENAME_LEN: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MailError {
    BufferTooSmall,
    ParseError,
    InvalidBase64,
    InvalidQuotedPrintable,
    HeaderNotFound,
    TooManyParts,
    CorruptMessage,
}

// =========================================================================
// 1. BASE64 & QUOTED-PRINTABLE ENCODING/DECODING (Contract M-02)
// =========================================================================

pub fn decode_base64(input: &[u8], output: &mut [u8]) -> Result<usize, MailError> {
    let mut out_pos = 0;
    let mut buf = 0u32;
    let mut bits = 0;

    for &b in input {
        let val = match b {
            b'A'..=b'Z' => b - b'A',
            b'a'..=b'z' => b - b'a' + 26,
            b'0'..=b'9' => b - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => continue, // padding
            b' ' | b'\r' | b'\n' | b'\t' => continue, // skip whitespace
            _ => return Err(MailError::InvalidBase64),
        };

        buf = (buf << 6) | (val as u32);
        bits += 6;

        if bits >= 8 {
            bits -= 8;
            if out_pos >= output.len() {
                return Err(MailError::BufferTooSmall);
            }
            output[out_pos] = ((buf >> bits) & 0xFF) as u8;
            out_pos += 1;
        }
    }

    Ok(out_pos)
}

pub fn encode_base64(input: &[u8], output: &mut [u8]) -> Result<usize, MailError> {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out_pos = 0;
    let mut i = 0;

    while i < input.len() {
        let b0 = input[i] as u32;
        let b1 = if i + 1 < input.len() { input[i + 1] as u32 } else { 0 };
        let b2 = if i + 2 < input.len() { input[i + 2] as u32 } else { 0 };

        let triple = (b0 << 16) | (b1 << 8) | b2;

        if out_pos + 4 > output.len() {
            return Err(MailError::BufferTooSmall);
        }

        output[out_pos] = TABLE[((triple >> 18) & 0x3F) as usize];
        output[out_pos + 1] = TABLE[((triple >> 12) & 0x3F) as usize];
        output[out_pos + 2] = if i + 1 < input.len() {
            TABLE[((triple >> 6) & 0x3F) as usize]
        } else {
            b'='
        };
        output[out_pos + 3] = if i + 2 < input.len() {
            TABLE[(triple & 0x3F) as usize]
        } else {
            b'='
        };

        out_pos += 4;
        i += 3;
    }

    Ok(out_pos)
}

pub fn decode_quoted_printable(input: &[u8], output: &mut [u8]) -> Result<usize, MailError> {
    let mut in_pos = 0;
    let mut out_pos = 0;

    let hex_val = |b: u8| -> Option<u8> {
        match b {
            b'0'..=b'9' => Some(b - b'0'),
            b'A'..=b'F' => Some(b - b'A' + 10),
            b'a'..=b'f' => Some(b - b'a' + 10),
            _ => None,
        }
    };

    while in_pos < input.len() {
        let b = input[in_pos];
        if b == b'=' {
            if in_pos + 2 < input.len() && input[in_pos + 1] == b'\r' && input[in_pos + 2] == b'\n' {
                // Soft line break: skip =\r\n
                in_pos += 3;
                continue;
            } else if in_pos + 1 < input.len() && input[in_pos + 1] == b'\n' {
                // Soft line break: skip =\n
                in_pos += 2;
                continue;
            } else if in_pos + 2 < input.len() {
                // Escaped hex byte: =XX
                let h1 = hex_val(input[in_pos + 1]);
                let h2 = hex_val(input[in_pos + 2]);
                if let (Some(v1), Some(v2)) = (h1, h2) {
                    if out_pos >= output.len() {
                        return Err(MailError::BufferTooSmall);
                    }
                    output[out_pos] = (v1 << 4) | v2;
                    out_pos += 1;
                    in_pos += 3;
                    continue;
                } else {
                    return Err(MailError::InvalidQuotedPrintable);
                }
            } else {
                return Err(MailError::InvalidQuotedPrintable);
            }
        } else {
            if out_pos >= output.len() {
                return Err(MailError::BufferTooSmall);
            }
            output[out_pos] = b;
            out_pos += 1;
            in_pos += 1;
        }
    }

    Ok(out_pos)
}

// =========================================================================
// 2. RFC 5322 HEADER & ADDRESS PARSER (Contract M-01, M-02)
// =========================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeaderField {
    pub name: [u8; MAX_HEADER_LEN],
    pub name_len: usize,
    pub value: [u8; MAX_VALUE_LEN],
    pub value_len: usize,
}

impl HeaderField {
    pub fn new() -> Self {
        Self {
            name: [0u8; MAX_HEADER_LEN],
            name_len: 0,
            value: [0u8; MAX_VALUE_LEN],
            value_len: 0,
        }
    }

    pub fn name_str(&self) -> &str {
        core::str::from_utf8(&self.name[..self.name_len]).unwrap_or("")
    }

    pub fn value_str(&self) -> &str {
        core::str::from_utf8(&self.value[..self.value_len]).unwrap_or("")
    }

    pub fn matches_name(&self, query: &str) -> bool {
        let q_bytes = query.as_bytes();
        if self.name_len != q_bytes.len() {
            return false;
        }
        for i in 0..self.name_len {
            if self.name[i].to_ascii_lowercase() != q_bytes[i].to_ascii_lowercase() {
                return false;
            }
        }
        true
    }
}

pub struct EmailAddress {
    pub display_name: [u8; MAX_VALUE_LEN],
    pub display_name_len: usize,
    pub address: [u8; MAX_VALUE_LEN],
    pub address_len: usize,
}

impl EmailAddress {
    pub fn parse(raw: &str) -> Self {
        let bytes = raw.as_bytes();
        let mut addr = Self {
            display_name: [0u8; MAX_VALUE_LEN],
            display_name_len: 0,
            address: [0u8; MAX_VALUE_LEN],
            address_len: 0,
        };

        // Check if format is: "Display Name" <user@domain.com>
        if let Some(open_angle) = bytes.iter().position(|&b| b == b'<') {
            if let Some(close_angle) = bytes.iter().position(|&b| b == b'>') {
                if close_angle > open_angle {
                    let addr_slice = &bytes[open_angle + 1..close_angle];
                    let addr_len = min(addr_slice.len(), MAX_VALUE_LEN);
                    addr.address[..addr_len].copy_from_slice(&addr_slice[..addr_len]);
                    addr.address_len = addr_len;

                    let mut name_part = &bytes[..open_angle];
                    // Trim quotes and whitespace
                    while !name_part.is_empty() && (name_part[0] == b' ' || name_part[0] == b'"') {
                        name_part = &name_part[1..];
                    }
                    while !name_part.is_empty() && (name_part[name_part.len() - 1] == b' ' || name_part[name_part.len() - 1] == b'"') {
                        name_part = &name_part[..name_part.len() - 1];
                    }
                    let name_len = min(name_part.len(), MAX_VALUE_LEN);
                    addr.display_name[..name_len].copy_from_slice(&name_part[..name_len]);
                    addr.display_name_len = name_len;
                    return addr;
                }
            }
        }

        // Fallback: entire string is address
        let trim_len = min(bytes.len(), MAX_VALUE_LEN);
        addr.address[..trim_len].copy_from_slice(&bytes[..trim_len]);
        addr.address_len = trim_len;
        addr
    }

    pub fn display_name_str(&self) -> &str {
        core::str::from_utf8(&self.display_name[..self.display_name_len]).unwrap_or("")
    }

    pub fn address_str(&self) -> &str {
        core::str::from_utf8(&self.address[..self.address_len]).unwrap_or("")
    }
}

// =========================================================================
// 3. MIME MULTIPART MESSAGE PARSER (Contract M-02)
// =========================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentEncoding {
    SevenBit,
    EightBit,
    Base64,
    QuotedPrintable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MimePart {
    pub content_type: [u8; MAX_VALUE_LEN],
    pub content_type_len: usize,
    pub encoding: ContentEncoding,
    pub filename: [u8; MAX_FILENAME_LEN],
    pub filename_len: usize,
    pub is_attachment: bool,
    pub body_start: usize,
    pub body_len: usize,
}

impl MimePart {
    pub fn new() -> Self {
        Self {
            content_type: [0u8; MAX_VALUE_LEN],
            content_type_len: 0,
            encoding: ContentEncoding::SevenBit,
            filename: [0u8; MAX_FILENAME_LEN],
            filename_len: 0,
            is_attachment: false,
            body_start: 0,
            body_len: 0,
        }
    }

    pub fn content_type_str(&self) -> &str {
        core::str::from_utf8(&self.content_type[..self.content_type_len]).unwrap_or("")
    }

    pub fn filename_str(&self) -> &str {
        core::str::from_utf8(&self.filename[..self.filename_len]).unwrap_or("")
    }
}

pub struct ParsedEmail {
    pub headers: [HeaderField; MAX_HEADERS],
    pub header_count: usize,
    pub parts: [MimePart; MAX_PARTS],
    pub part_count: usize,
    pub is_multipart: bool,
    pub boundary: [u8; MAX_VALUE_LEN],
    pub boundary_len: usize,
    pub raw_body_offset: usize,
}

impl ParsedEmail {
    pub fn new() -> Self {
        Self {
            headers: [HeaderField::new(); MAX_HEADERS],
            header_count: 0,
            parts: [MimePart::new(); MAX_PARTS],
            part_count: 0,
            is_multipart: false,
            boundary: [0u8; MAX_VALUE_LEN],
            boundary_len: 0,
            raw_body_offset: 0,
        }
    }

    pub fn get_header<'a>(&'a self, name: &str) -> Option<&'a str> {
        for i in 0..self.header_count {
            if self.headers[i].matches_name(name) {
                return Some(self.headers[i].value_str());
            }
        }
        None
    }

    pub fn parse(raw_email: &[u8]) -> Result<Self, MailError> {
        let mut email = Self::new();
        let mut pos = 0;

        // Parse headers until empty line (\r\n\r\n or \n\n)
        while pos < raw_email.len() {
            // Check end of headers
            if pos + 2 <= raw_email.len() && &raw_email[pos..pos + 2] == b"\r\n" {
                pos += 2;
                break;
            }
            if raw_email[pos] == b'\n' {
                pos += 1;
                break;
            }

            // Find colon
            let start = pos;
            while pos < raw_email.len() && raw_email[pos] != b':' && raw_email[pos] != b'\n' {
                pos += 1;
            }
            if pos >= raw_email.len() || raw_email[pos] != b':' {
                return Err(MailError::ParseError);
            }

            let name_slice = &raw_email[start..pos];
            pos += 1; // skip ':'

            // Skip space
            while pos < raw_email.len() && raw_email[pos] == b' ' {
                pos += 1;
            }

            // Find end of line (handling header unfolding RFC 5322 §2.2.3)
            let val_start = pos;
            while pos < raw_email.len() {
                if raw_email[pos] == b'\n' {
                    // Check if next line starts with space/tab (unfolding)
                    if pos + 1 < raw_email.len() && (raw_email[pos + 1] == b' ' || raw_email[pos + 1] == b'\t') {
                        pos += 2;
                        continue;
                    } else {
                        break;
                    }
                }
                pos += 1;
            }

            let mut val_end = pos;
            if val_end > val_start && raw_email[val_end - 1] == b'\r' {
                val_end -= 1;
            }

            let val_slice = &raw_email[val_start..val_end];
            if pos < raw_email.len() && raw_email[pos] == b'\n' {
                pos += 1;
            }

            if email.header_count < MAX_HEADERS {
                let n_len = min(name_slice.len(), MAX_HEADER_LEN);
                email.headers[email.header_count].name[..n_len].copy_from_slice(&name_slice[..n_len]);
                email.headers[email.header_count].name_len = n_len;

                let v_len = min(val_slice.len(), MAX_VALUE_LEN);
                email.headers[email.header_count].value[..v_len].copy_from_slice(&val_slice[..v_len]);
                email.headers[email.header_count].value_len = v_len;
                email.header_count += 1;
            }
        }

        email.raw_body_offset = pos;

        // Check Content-Type for multipart and boundary
        let mut ct_buf = [0u8; MAX_VALUE_LEN];
        let mut ct_len = 0;
        for i in 0..email.header_count {
            if email.headers[i].matches_name("Content-Type") {
                let l = email.headers[i].value_len;
                ct_buf[..l].copy_from_slice(&email.headers[i].value[..l]);
                ct_len = l;
                break;
            }
        }

        if ct_len > 0 {
            let ct_slice = &ct_buf[..ct_len];
            if find_subslice(ct_slice, b"multipart/").is_some() {
                email.is_multipart = true;
                if let Some(b_idx) = find_subslice(ct_slice, b"boundary=") {
                    let mut b_start = b_idx + 9;
                    if b_start < ct_len && ct_slice[b_start] == b'"' {
                        b_start += 1;
                        let mut b_end = b_start;
                        while b_end < ct_len && ct_slice[b_end] != b'"' {
                            b_end += 1;
                        }
                        let b_slice = &ct_slice[b_start..b_end];
                        let b_len = min(b_slice.len(), MAX_VALUE_LEN);
                        email.boundary[..b_len].copy_from_slice(&b_slice[..b_len]);
                        email.boundary_len = b_len;
                    } else {
                        let mut b_end = b_start;
                        while b_end < ct_len && ct_slice[b_end] != b' ' && ct_slice[b_end] != b';' {
                            b_end += 1;
                        }
                        let b_slice = &ct_slice[b_start..b_end];
                        let b_len = min(b_slice.len(), MAX_VALUE_LEN);
                        email.boundary[..b_len].copy_from_slice(&b_slice[..b_len]);
                        email.boundary_len = b_len;
                    }
                }
            }
        }

        // Parse body parts
        if email.is_multipart && email.boundary_len > 0 {
            email.parse_multipart_body(raw_email)?;
        } else {
            // Single part email
            let mut part = MimePart::new();
            if let Some(ct) = email.get_header("Content-Type") {
                let l = min(ct.len(), MAX_VALUE_LEN);
                part.content_type[..l].copy_from_slice(&ct.as_bytes()[..l]);
                part.content_type_len = l;
            } else {
                let def = b"text/plain";
                part.content_type[..def.len()].copy_from_slice(def);
                part.content_type_len = def.len();
            }

            if let Some(enc) = email.get_header("Content-Transfer-Encoding") {
                if enc.eq_ignore_ascii_case("base64") {
                    part.encoding = ContentEncoding::Base64;
                } else if enc.eq_ignore_ascii_case("quoted-printable") {
                    part.encoding = ContentEncoding::QuotedPrintable;
                }
            }

            part.body_start = email.raw_body_offset;
            part.body_len = raw_email.len().saturating_sub(email.raw_body_offset);
            email.parts[0] = part;
            email.part_count = 1;
        }

        Ok(email)
    }

    fn parse_multipart_body(&mut self, raw: &[u8]) -> Result<(), MailError> {
        let mut delimiter = [0u8; MAX_VALUE_LEN + 4];
        delimiter[0] = b'-';
        delimiter[1] = b'-';
        delimiter[2..2 + self.boundary_len].copy_from_slice(&self.boundary[..self.boundary_len]);
        let delim_len = self.boundary_len + 2;

        let body_data = &raw[self.raw_body_offset..];

        // Find initial boundary
        let first_boundary = match find_subslice(body_data, &delimiter[..delim_len]) {
            Some(idx) => idx + delim_len,
            None => return Ok(()),
        };
        let mut cur_offset = first_boundary;

        while cur_offset < body_data.len() && self.part_count < MAX_PARTS {
            // Skip CRLF
            while cur_offset < body_data.len() && (body_data[cur_offset] == b'\r' || body_data[cur_offset] == b'\n') {
                cur_offset += 1;
            }

            // Check if end boundary: --boundary--
            if cur_offset + 2 <= body_data.len() && &body_data[cur_offset..cur_offset + 2] == b"--" {
                break;
            }

            // Find next boundary delimiter
            let next_boundary_idx = match find_subslice(&body_data[cur_offset..], &delimiter[..delim_len]) {
                Some(idx) => cur_offset + idx,
                None => body_data.len(),
            };

            let part_data = &body_data[cur_offset..next_boundary_idx];
            cur_offset = next_boundary_idx + delim_len;

            // Parse part headers
            let mut part = MimePart::new();
            let mut p_pos = 0;
            while p_pos < part_data.len() {
                if p_pos + 2 <= part_data.len() && &part_data[p_pos..p_pos + 2] == b"\r\n" {
                    p_pos += 2;
                    break;
                }
                if part_data[p_pos] == b'\n' {
                    p_pos += 1;
                    break;
                }

                let line_start = p_pos;
                while p_pos < part_data.len() && part_data[p_pos] != b'\n' {
                    p_pos += 1;
                }
                let line_end = if p_pos > line_start && part_data[p_pos - 1] == b'\r' {
                    p_pos - 1
                } else {
                    p_pos
                };
                if p_pos < part_data.len() && part_data[p_pos] == b'\n' {
                    p_pos += 1;
                }

                let line = &part_data[line_start..line_end];
                if let Some(col_pos) = line.iter().position(|&b| b == b':') {
                    let h_name = &line[..col_pos];
                    let mut h_val = &line[col_pos + 1..];
                    while !h_val.is_empty() && h_val[0] == b' ' {
                        h_val = &h_val[1..];
                    }

                    if h_name.eq_ignore_ascii_case(b"Content-Type") {
                        let l = min(h_val.len(), MAX_VALUE_LEN);
                        part.content_type[..l].copy_from_slice(&h_val[..l]);
                        part.content_type_len = l;
                    } else if h_name.eq_ignore_ascii_case(b"Content-Transfer-Encoding") {
                        if h_val.eq_ignore_ascii_case(b"base64") {
                            part.encoding = ContentEncoding::Base64;
                        } else if h_val.eq_ignore_ascii_case(b"quoted-printable") {
                            part.encoding = ContentEncoding::QuotedPrintable;
                        }
                    } else if h_name.eq_ignore_ascii_case(b"Content-Disposition") {
                        if find_subslice(h_val, b"attachment").is_some() {
                            part.is_attachment = true;
                        }
                        if let Some(fn_idx) = find_subslice(h_val, b"filename=") {
                            let mut fn_start = fn_idx + 9;
                            if fn_start < h_val.len() && h_val[fn_start] == b'"' {
                                fn_start += 1;
                                let mut fn_end = fn_start;
                                while fn_end < h_val.len() && h_val[fn_end] != b'"' {
                                    fn_end += 1;
                                }
                                let fn_slice = &h_val[fn_start..fn_end];
                                let fn_len = min(fn_slice.len(), MAX_FILENAME_LEN);
                                part.filename[..fn_len].copy_from_slice(&fn_slice[..fn_len]);
                                part.filename_len = fn_len;
                            }
                        }
                    }
                }
            }

            // The remainder is the part body
            let abs_body_start = self.raw_body_offset + (part_data.as_ptr() as usize - body_data.as_ptr() as usize) + p_pos;
            let mut part_body_len = part_data.len().saturating_sub(p_pos);
            // Trim trailing \r\n before boundary
            if part_body_len >= 2 && &part_data[p_pos + part_body_len - 2..p_pos + part_body_len] == b"\r\n" {
                part_body_len -= 2;
            } else if part_body_len >= 1 && part_data[p_pos + part_body_len - 1] == b'\n' {
                part_body_len -= 1;
            }

            part.body_start = abs_body_start;
            part.body_len = part_body_len;

            self.parts[self.part_count] = part;
            self.part_count += 1;
        }

        Ok(())
    }
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    for i in 0..=(haystack.len() - needle.len()) {
        let mut matched = true;
        for j in 0..needle.len() {
            if haystack[i + j].to_ascii_lowercase() != needle[j].to_ascii_lowercase() {
                matched = false;
                break;
            }
        }
        if matched {
            return Some(i);
        }
    }
    None
}

// =========================================================================
// 4. MAILDIR FOLDER STORAGE ABSTRACTION (Contract M-01)
// =========================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaildirFolder {
    Cur,
    New,
    Tmp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MaildirFlags {
    pub seen: bool,      // 'S'
    pub replied: bool,   // 'R'
    pub flagged: bool,   // 'F'
    pub trashed: bool,   // 'T'
}

impl MaildirFlags {
    pub fn new() -> Self {
        Self {
            seen: false,
            replied: false,
            flagged: false,
            trashed: false,
        }
    }

    pub fn format_suffix(&self, out: &mut [u8; 8]) -> usize {
        out[0] = b':';
        out[1] = b'2';
        out[2] = b',';
        let mut pos = 3;
        if self.flagged {
            out[pos] = b'F';
            pos += 1;
        }
        if self.replied {
            out[pos] = b'R';
            pos += 1;
        }
        if self.seen {
            out[pos] = b'S';
            pos += 1;
        }
        if self.trashed {
            out[pos] = b'T';
            pos += 1;
        }
        pos
    }

    pub fn parse_suffix(suffix: &[u8]) -> Self {
        let mut flags = Self::new();
        if suffix.len() >= 3 && &suffix[..3] == b":2," {
            for &b in &suffix[3..] {
                match b {
                    b'S' => flags.seen = true,
                    b'R' => flags.replied = true,
                    b'F' => flags.flagged = true,
                    b'T' => flags.trashed = true,
                    _ => {}
                }
            }
        }
        flags
    }
}

// =========================================================================
// 5. MESSAGE COMPOSER & SERIALIZER (Contract M-01, M-02)
// =========================================================================

pub struct MessageComposer<'a> {
    buf: &'a mut [u8],
    pos: usize,
}

impl<'a> MessageComposer<'a> {
    pub fn new(buf: &'a mut [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    pub fn write_str(&mut self, s: &str) -> Result<(), MailError> {
        self.write_bytes(s.as_bytes())
    }

    pub fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), MailError> {
        if self.pos + bytes.len() > self.buf.len() {
            return Err(MailError::BufferTooSmall);
        }
        self.buf[self.pos..self.pos + bytes.len()].copy_from_slice(bytes);
        self.pos += bytes.len();
        Ok(())
    }

    pub fn compose_simple(
        &mut self,
        from: &str,
        to: &str,
        subject: &str,
        date: &str,
        message_id: &str,
        body: &str,
    ) -> Result<usize, MailError> {
        self.write_str("From: ")?;
        self.write_str(from)?;
        self.write_str("\r\nTo: ")?;
        self.write_str(to)?;
        self.write_str("\r\nSubject: ")?;
        self.write_str(subject)?;
        self.write_str("\r\nDate: ")?;
        self.write_str(date)?;
        self.write_str("\r\nMessage-ID: ")?;
        self.write_str(message_id)?;
        self.write_str("\r\nMIME-Version: 1.0\r\nContent-Type: text/plain; charset=utf-8\r\n\r\n")?;
        self.write_str(body)?;
        self.write_str("\r\n")?;
        Ok(self.pos)
    }

    pub fn compose_multipart_with_attachment(
        &mut self,
        from: &str,
        to: &str,
        subject: &str,
        date: &str,
        message_id: &str,
        text_body: &str,
        attachment_name: &str,
        attachment_bytes: &[u8],
    ) -> Result<usize, MailError> {
        let boundary = "----=_Part_INOS_MAIL_BOUNDARY_12345";

        self.write_str("From: ")?;
        self.write_str(from)?;
        self.write_str("\r\nTo: ")?;
        self.write_str(to)?;
        self.write_str("\r\nSubject: ")?;
        self.write_str(subject)?;
        self.write_str("\r\nDate: ")?;
        self.write_str(date)?;
        self.write_str("\r\nMessage-ID: ")?;
        self.write_str(message_id)?;
        self.write_str("\r\nMIME-Version: 1.0\r\nContent-Type: multipart/mixed; boundary=\"")?;
        self.write_str(boundary)?;
        self.write_str("\"\r\n\r\n")?;

        // Part 1: Text Body
        self.write_str("--")?;
        self.write_str(boundary)?;
        self.write_str("\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Transfer-Encoding: 7bit\r\n\r\n")?;
        self.write_str(text_body)?;
        self.write_str("\r\n")?;

        // Part 2: Attachment (Base64 encoded)
        self.write_str("--")?;
        self.write_str(boundary)?;
        self.write_str("\r\nContent-Type: application/octet-stream; name=\"")?;
        self.write_str(attachment_name)?;
        self.write_str("\"\r\nContent-Disposition: attachment; filename=\"")?;
        self.write_str(attachment_name)?;
        self.write_str("\"\r\nContent-Transfer-Encoding: base64\r\n\r\n")?;

        let mut b64_buf = [0u8; 1024];
        let b64_len = encode_base64(attachment_bytes, &mut b64_buf)?;
        self.write_bytes(&b64_buf[..b64_len])?;
        self.write_str("\r\n")?;

        // Closing boundary
        self.write_str("--")?;
        self.write_str(boundary)?;
        self.write_str("--\r\n")?;

        Ok(self.pos)
    }
}
