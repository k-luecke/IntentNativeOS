// userspace/apps/grid_app/src/engine.rs - Freestanding Native Grid Spreadsheet Engine
// Features: DAG dependency tracking, topological recalculation, #CIRC! detection,
// range aggregations (SUM, AVERAGE, MIN, MAX), RFC 4180 CSV, and OPC XLSX ZIP/XML adapter.

pub const GRID_COLS: usize = 8; // Columns A..H
pub const GRID_ROWS: usize = 16; // Rows 1..16
pub const TOTAL_CELLS: usize = GRID_COLS * GRID_ROWS; // 128 cells

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridError {
    InvalidCellRef,
    CircularReference,
    DivisionByZero,
    SyntaxError,
    BufferTooSmall,
    CsvParseError,
    XlsxParseError,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellValue {
    Empty,
    Number(i64),
    Text([u8; 32], usize),
    Formula([u8; 32], usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellComputedState {
    Empty,
    Number(i64),
    Text([u8; 32], usize),
    Error(GridError),
}

// --- IEEE 802.3 CRC-32 for ZIP Container ---

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

// --- Spreadsheet Engine ---

pub struct GridSheet {
    pub cells: [[CellValue; GRID_COLS]; GRID_ROWS],
    pub computed: [[CellComputedState; GRID_COLS]; GRID_ROWS],
    // DAG dependency tracking: which cells does (col, row) depend on?
    pub deps: [[[u8; 16]; GRID_COLS]; GRID_ROWS],
    pub dep_counts: [[u8; GRID_COLS]; GRID_ROWS],
}

impl GridSheet {
    pub const fn new() -> Self {
        Self {
            cells: [[CellValue::Empty; GRID_COLS]; GRID_ROWS],
            computed: [[CellComputedState::Empty; GRID_COLS]; GRID_ROWS],
            deps: [[[0; 16]; GRID_COLS]; GRID_ROWS],
            dep_counts: [[0; GRID_COLS]; GRID_ROWS],
        }
    }

    pub fn set_number(&mut self, col: usize, row: usize, val: i64) -> Result<(), GridError> {
        if col >= GRID_COLS || row >= GRID_ROWS {
            return Err(GridError::InvalidCellRef);
        }
        self.cells[row][col] = CellValue::Number(val);
        self.dep_counts[row][col] = 0;
        self.recalculate();
        Ok(())
    }

    pub fn set_text(&mut self, col: usize, row: usize, text: &str) -> Result<(), GridError> {
        if col >= GRID_COLS || row >= GRID_ROWS {
            return Err(GridError::InvalidCellRef);
        }
        let mut buf = [0u8; 32];
        let len = core::cmp::min(text.len(), 32);
        buf[..len].copy_from_slice(&text.as_bytes()[..len]);
        self.cells[row][col] = CellValue::Text(buf, len);
        self.dep_counts[row][col] = 0;
        self.recalculate();
        Ok(())
    }

    pub fn set_formula(&mut self, col: usize, row: usize, formula: &str) -> Result<(), GridError> {
        if col >= GRID_COLS || row >= GRID_ROWS {
            return Err(GridError::InvalidCellRef);
        }
        let mut buf = [0u8; 32];
        let len = core::cmp::min(formula.len(), 32);
        buf[..len].copy_from_slice(&formula.as_bytes()[..len]);
        self.cells[row][col] = CellValue::Formula(buf, len);

        // Parse dependencies for DAG tracking
        self.update_dependencies(col, row, formula);
        self.recalculate();
        Ok(())
    }

    fn update_dependencies(&mut self, col: usize, row: usize, formula: &str) {
        let mut count = 0usize;
        let mut dep_arr = [0u8; 16];

        let s = formula.trim();
        if s.starts_with('=') {
            let expr = &s[1..];
            if let Some(pos) = expr.find(':') {
                // Range formula e.g. SUM(A1:A3)
                let open_paren = expr.find('(').unwrap_or(0);
                let close_paren = expr.find(')').unwrap_or(expr.len());
                if open_paren < pos && pos < close_paren {
                    let range_str = &expr[open_paren + 1..close_paren];
                    if let Ok((c1, r1, c2, r2)) = parse_range(range_str) {
                        for r in r1..=r2 {
                            for c in c1..=c2 {
                                if count < 16 {
                                    dep_arr[count] = ((r * GRID_COLS) + c) as u8;
                                    count += 1;
                                }
                            }
                        }
                    }
                }
            } else {
                // Scan for single cell references like A1, B2
                let bytes = expr.as_bytes();
                let mut i = 0;
                while i < bytes.len() {
                    let b = bytes[i];
                    if (b >= b'A' && b <= b'H') || (b >= b'a' && b <= b'h') {
                        if i + 1 < bytes.len() && bytes[i + 1].is_ascii_digit() {
                            let mut end = i + 2;
                            if end < bytes.len() && bytes[end].is_ascii_digit() {
                                end += 1;
                            }
                            if let Ok((c, r)) = parse_cell_coord(core::str::from_utf8(&bytes[i..end]).unwrap_or("")) {
                                if count < 16 {
                                    dep_arr[count] = ((r * GRID_COLS) + c) as u8;
                                    count += 1;
                                }
                            }
                            i = end;
                            continue;
                        }
                    }
                    i += 1;
                }
            }
        }

        self.deps[row][col] = dep_arr;
        self.dep_counts[row][col] = count as u8;
    }

    /// Topological recalculation across the cell dependency graph
    pub fn recalculate(&mut self) {
        // Color array for cycle detection: 0 = unvisited, 1 = visiting (recursion stack), 2 = visited
        let mut state = [[0u8; GRID_COLS]; GRID_ROWS];

        for r in 0..GRID_ROWS {
            for c in 0..GRID_COLS {
                if state[r][c] == 0 {
                    self.eval_topological(c, r, &mut state);
                }
            }
        }
    }

    fn eval_topological(
        &mut self,
        col: usize,
        row: usize,
        state: &mut [[u8; GRID_COLS]; GRID_ROWS],
    ) {
        state[row][col] = 1; // Currently visiting

        match self.cells[row][col] {
            CellValue::Empty => {
                self.computed[row][col] = CellComputedState::Empty;
            }
            CellValue::Number(n) => {
                self.computed[row][col] = CellComputedState::Number(n);
            }
            CellValue::Text(buf, len) => {
                self.computed[row][col] = CellComputedState::Text(buf, len);
            }
            CellValue::Formula(buf, len) => {
                let f_str = match core::str::from_utf8(&buf[..len]) {
                    Ok(s) => s,
                    Err(_) => {
                        self.computed[row][col] = CellComputedState::Error(GridError::SyntaxError);
                        state[row][col] = 2;
                        return;
                    }
                };

                // Check dependencies for cycles
                let dep_cnt = self.dep_counts[row][col] as usize;
                let mut cycle_detected = false;
                for i in 0..dep_cnt {
                    let encoded = self.deps[row][col][i] as usize;
                    let dc = encoded % GRID_COLS;
                    let dr = encoded / GRID_COLS;
                    if state[dr][dc] == 1 {
                        // Back edge in DAG: circular reference!
                        cycle_detected = true;
                        break;
                    } else if state[dr][dc] == 0 {
                        self.eval_topological(dc, dr, state);
                        if let CellComputedState::Error(GridError::CircularReference) = self.computed[dr][dc] {
                            cycle_detected = true;
                            break;
                        }
                    }
                }

                if cycle_detected {
                    self.computed[row][col] = CellComputedState::Error(GridError::CircularReference);
                } else {
                    let mut visited = [[false; GRID_COLS]; GRID_ROWS];
                    let eval_res = self.eval_formula_str(f_str, &mut visited, 0);
                    match eval_res {
                        Ok(v) => self.computed[row][col] = CellComputedState::Number(v),
                        Err(e) => self.computed[row][col] = CellComputedState::Error(e),
                    }
                }
            }
        }

        state[row][col] = 2; // Visited
    }

    pub fn evaluate_cell(&self, col: usize, row: usize) -> Result<i64, GridError> {
        if col >= GRID_COLS || row >= GRID_ROWS {
            return Err(GridError::InvalidCellRef);
        }
        match self.computed[row][col] {
            CellComputedState::Number(n) => Ok(n),
            CellComputedState::Empty => Ok(0),
            CellComputedState::Text(_, _) => Ok(0),
            CellComputedState::Error(e) => Err(e),
        }
    }

    pub fn get_display_text(&self, col: usize, row: usize, buf: &mut [u8; 32]) -> usize {
        if col >= GRID_COLS || row >= GRID_ROWS {
            return 0;
        }
        match self.computed[row][col] {
            CellComputedState::Empty => 0,
            CellComputedState::Number(n) => {
                let mut temp = [0u8; 24];
                let len = format_i64(n, &mut temp);
                buf[..len].copy_from_slice(&temp[..len]);
                len
            }
            CellComputedState::Text(tbuf, len) => {
                buf[..len].copy_from_slice(&tbuf[..len]);
                len
            }
            CellComputedState::Error(GridError::CircularReference) => {
                let msg = b"#CIRC!";
                buf[..msg.len()].copy_from_slice(msg);
                msg.len()
            }
            CellComputedState::Error(GridError::DivisionByZero) => {
                let msg = b"#DIV/0!";
                buf[..msg.len()].copy_from_slice(msg);
                msg.len()
            }
            CellComputedState::Error(GridError::SyntaxError) => {
                let msg = b"#SYNTAX!";
                buf[..msg.len()].copy_from_slice(msg);
                msg.len()
            }
            CellComputedState::Error(GridError::InvalidCellRef) => {
                let msg = b"#VALUE!";
                buf[..msg.len()].copy_from_slice(msg);
                msg.len()
            }
            CellComputedState::Error(_) => {
                let msg = b"#ERR!";
                buf[..msg.len()].copy_from_slice(msg);
                msg.len()
            }
        }
    }

    fn eval_formula_str(
        &self,
        formula: &str,
        visited: &mut [[bool; GRID_COLS]; GRID_ROWS],
        depth: usize,
    ) -> Result<i64, GridError> {
        let s = formula.trim();
        if !s.starts_with('=') {
            return Err(GridError::SyntaxError);
        }
        let expr = &s[1..].trim();

        // Syntax check: unclosed parentheses
        if expr.contains('(') && !expr.ends_with(')') {
            return Err(GridError::SyntaxError);
        }

        // 1. Built-in Range Functions: =SUM(A1:A3), =AVERAGE(A1:A3), =MIN(A1:A3), =MAX(A1:A3)
        if expr.starts_with("SUM(") && expr.ends_with(')') {
            let inner = &expr[4..expr.len() - 1];
            return self.eval_range_sum(inner, visited, depth);
        }
        if expr.starts_with("AVERAGE(") && expr.ends_with(')') {
            let inner = &expr[8..expr.len() - 1];
            return self.eval_range_avg(inner, visited, depth);
        }
        if expr.starts_with("MIN(") && expr.ends_with(')') {
            let inner = &expr[4..expr.len() - 1];
            return self.eval_range_min(inner, visited, depth);
        }
        if expr.starts_with("MAX(") && expr.ends_with(')') {
            let inner = &expr[4..expr.len() - 1];
            return self.eval_range_max(inner, visited, depth);
        }

        // 2. Binary Arithmetic: operand1 op operand2 (+, -, *, /)
        if let Some(pos) = expr.find('+') {
            let left = self.eval_operand(&expr[..pos].trim(), visited, depth)?;
            let right = self.eval_operand(&expr[pos + 1..].trim(), visited, depth)?;
            return Ok(left + right);
        }
        if let Some(pos) = expr.find('-') {
            let left = self.eval_operand(&expr[..pos].trim(), visited, depth)?;
            let right = self.eval_operand(&expr[pos + 1..].trim(), visited, depth)?;
            return Ok(left - right);
        }
        if let Some(pos) = expr.find('*') {
            let left = self.eval_operand(&expr[..pos].trim(), visited, depth)?;
            let right = self.eval_operand(&expr[pos + 1..].trim(), visited, depth)?;
            return Ok(left * right);
        }
        if let Some(pos) = expr.find('/') {
            let left = self.eval_operand(&expr[..pos].trim(), visited, depth)?;
            let right = self.eval_operand(&expr[pos + 1..].trim(), visited, depth)?;
            if right == 0 {
                return Err(GridError::DivisionByZero);
            }
            return Ok(left / right);
        }

        // 3. Single cell or number reference
        self.eval_operand(expr, visited, depth)
    }

    fn eval_operand(
        &self,
        token: &str,
        visited: &mut [[bool; GRID_COLS]; GRID_ROWS],
        depth: usize,
    ) -> Result<i64, GridError> {
        let t = token.trim();
        if let Ok(n) = parse_i64(t) {
            return Ok(n);
        }
        let (c, r) = parse_cell_coord(t)?;
        self.eval_cell_rec(c, r, visited, depth)
    }

    fn eval_cell_rec(
        &self,
        col: usize,
        row: usize,
        visited: &mut [[bool; GRID_COLS]; GRID_ROWS],
        depth: usize,
    ) -> Result<i64, GridError> {
        if col >= GRID_COLS || row >= GRID_ROWS {
            return Err(GridError::InvalidCellRef);
        }
        if depth > 16 || visited[row][col] {
            return Err(GridError::CircularReference);
        }
        visited[row][col] = true;

        let result = match &self.cells[row][col] {
            CellValue::Empty => Ok(0),
            CellValue::Number(n) => Ok(*n),
            CellValue::Text(_, _) => Ok(0),
            CellValue::Formula(buf, len) => {
                let f_str = core::str::from_utf8(&buf[..*len]).map_err(|_| GridError::SyntaxError)?;
                self.eval_formula_str(f_str, visited, depth + 1)
            }
        };

        visited[row][col] = false;
        result
    }

    fn eval_range_sum(
        &self,
        range_str: &str,
        visited: &mut [[bool; GRID_COLS]; GRID_ROWS],
        depth: usize,
    ) -> Result<i64, GridError> {
        let (c1, r1, c2, r2) = parse_range(range_str)?;
        let mut sum = 0i64;
        for r in r1..=r2 {
            for c in c1..=c2 {
                sum += self.eval_cell_rec(c, r, visited, depth)?;
            }
        }
        Ok(sum)
    }

    fn eval_range_avg(
        &self,
        range_str: &str,
        visited: &mut [[bool; GRID_COLS]; GRID_ROWS],
        depth: usize,
    ) -> Result<i64, GridError> {
        let (c1, r1, c2, r2) = parse_range(range_str)?;
        let mut sum = 0i64;
        let mut count = 0i64;
        for r in r1..=r2 {
            for c in c1..=c2 {
                sum += self.eval_cell_rec(c, r, visited, depth)?;
                count += 1;
            }
        }
        if count == 0 {
            return Err(GridError::DivisionByZero);
        }
        Ok(sum / count)
    }

    fn eval_range_min(
        &self,
        range_str: &str,
        visited: &mut [[bool; GRID_COLS]; GRID_ROWS],
        depth: usize,
    ) -> Result<i64, GridError> {
        let (c1, r1, c2, r2) = parse_range(range_str)?;
        let mut min_val = i64::MAX;
        for r in r1..=r2 {
            for c in c1..=c2 {
                let v = self.eval_cell_rec(c, r, visited, depth)?;
                if v < min_val {
                    min_val = v;
                }
            }
        }
        Ok(min_val)
    }

    fn eval_range_max(
        &self,
        range_str: &str,
        visited: &mut [[bool; GRID_COLS]; GRID_ROWS],
        depth: usize,
    ) -> Result<i64, GridError> {
        let (c1, r1, c2, r2) = parse_range(range_str)?;
        let mut max_val = i64::MIN;
        for r in r1..=r2 {
            for c in c1..=c2 {
                let v = self.eval_cell_rec(c, r, visited, depth)?;
                if v > max_val {
                    max_val = v;
                }
            }
        }
        Ok(max_val)
    }

    // --- RFC 4180 CSV Engine ---

    pub fn export_csv(&self, out_buf: &mut [u8]) -> Result<usize, GridError> {
        let mut pos = 0usize;
        for r in 0..GRID_ROWS {
            let mut row_has_content = false;
            for c in 0..GRID_COLS {
                if !matches!(self.cells[r][c], CellValue::Empty) {
                    row_has_content = true;
                    break;
                }
            }
            if !row_has_content {
                continue;
            }

            for c in 0..GRID_COLS {
                if c > 0 {
                    if pos >= out_buf.len() { return Err(GridError::BufferTooSmall); }
                    out_buf[pos] = b',';
                    pos += 1;
                }
                match &self.cells[r][c] {
                    CellValue::Empty => {}
                    CellValue::Number(n) => {
                        let mut num_str = [0u8; 24];
                        let slen = format_i64(*n, &mut num_str);
                        if pos + slen > out_buf.len() { return Err(GridError::BufferTooSmall); }
                        out_buf[pos..pos + slen].copy_from_slice(&num_str[..slen]);
                        pos += slen;
                    }
                    CellValue::Text(tbuf, tlen) => {
                        // Check if quotes are needed per RFC 4180
                        let mut needs_quote = false;
                        for &b in &tbuf[..*tlen] {
                            if b == b',' || b == b'"' || b == b'\n' || b == b'\r' {
                                needs_quote = true;
                                break;
                            }
                        }
                        if needs_quote {
                            if pos >= out_buf.len() { return Err(GridError::BufferTooSmall); }
                            out_buf[pos] = b'"';
                            pos += 1;
                            for &b in &tbuf[..*tlen] {
                                if b == b'"' {
                                    if pos + 2 > out_buf.len() { return Err(GridError::BufferTooSmall); }
                                    out_buf[pos] = b'"';
                                    out_buf[pos + 1] = b'"';
                                    pos += 2;
                                } else {
                                    if pos >= out_buf.len() { return Err(GridError::BufferTooSmall); }
                                    out_buf[pos] = b;
                                    pos += 1;
                                }
                            }
                            if pos >= out_buf.len() { return Err(GridError::BufferTooSmall); }
                            out_buf[pos] = b'"';
                            pos += 1;
                        } else {
                            if pos + *tlen > out_buf.len() { return Err(GridError::BufferTooSmall); }
                            out_buf[pos..pos + *tlen].copy_from_slice(&tbuf[..*tlen]);
                            pos += *tlen;
                        }
                    }
                    CellValue::Formula(fbuf, flen) => {
                        if pos + *flen > out_buf.len() { return Err(GridError::BufferTooSmall); }
                        out_buf[pos..pos + *flen].copy_from_slice(&fbuf[..*flen]);
                        pos += *flen;
                    }
                }
            }
            if pos + 2 > out_buf.len() { return Err(GridError::BufferTooSmall); }
            out_buf[pos] = b'\r';
            out_buf[pos + 1] = b'\n';
            pos += 2;
        }
        Ok(pos)
    }

    pub fn import_csv(&mut self, csv_data: &str) -> Result<(), GridError> {
        let mut r = 0usize;
        for line in csv_data.lines() {
            if r >= GRID_ROWS {
                break;
            }
            let trimmed = line.trim_end_matches('\r');
            let mut c = 0usize;
            for field in trimmed.split(',') {
                if c >= GRID_COLS {
                    break;
                }
                let f = field.trim();
                let f_clean = if f.starts_with('"') && f.ends_with('"') && f.len() >= 2 {
                    &f[1..f.len() - 1]
                } else {
                    f
                };

                if f_clean.starts_with('=') {
                    self.set_formula(c, r, f_clean)?;
                } else if let Ok(n) = parse_i64(f_clean) {
                    self.set_number(c, r, n)?;
                } else if !f_clean.is_empty() {
                    self.set_text(c, r, f_clean)?;
                }
                c += 1;
            }
            r += 1;
        }
        self.recalculate();
        Ok(())
    }

    // --- OPC XLSX Package Export and Import Adapter ---

    pub fn export_xlsx(&self, out_buf: &mut [u8]) -> Result<usize, GridError> {
        // 1. Collect unique strings for xl/sharedStrings.xml deduplication
        let mut unique_strings: [&[u8]; 16] = [&[]; 16];
        let mut unique_count = 0usize;
        let mut total_string_cells = 0usize;

        for r in 0..GRID_ROWS {
            for c in 0..GRID_COLS {
                if let CellValue::Text(tbuf, tlen) = &self.cells[r][c] {
                    total_string_cells += 1;
                    let text_slice = &tbuf[..*tlen];
                    let mut found = false;
                    for i in 0..unique_count {
                        if unique_strings[i] == text_slice {
                            found = true;
                            break;
                        }
                    }
                    if !found && unique_count < 16 {
                        unique_strings[unique_count] = text_slice;
                        unique_count += 1;
                    }
                }
            }
        }

        // Generate xl/worksheets/sheet1.xml
        let mut sheet_xml = [0u8; 2048];
        let mut spos = 0usize;

        let hdr = b"<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"><sheetData>";
        sheet_xml[spos..spos + hdr.len()].copy_from_slice(hdr);
        spos += hdr.len();

        for r in 0..GRID_ROWS {
            let mut row_has_content = false;
            for c in 0..GRID_COLS {
                if !matches!(self.cells[r][c], CellValue::Empty) {
                    row_has_content = true;
                    break;
                }
            }
            if !row_has_content {
                continue;
            }

            // <row r="1">
            let row_tag_prefix = b"<row r=\"";
            sheet_xml[spos..spos + row_tag_prefix.len()].copy_from_slice(row_tag_prefix);
            spos += row_tag_prefix.len();

            let mut rnum_buf = [0u8; 24];
            let rlen = format_i64((r + 1) as i64, &mut rnum_buf);
            sheet_xml[spos..spos + rlen].copy_from_slice(&rnum_buf[..rlen]);
            spos += rlen;

            sheet_xml[spos] = b'"';
            sheet_xml[spos + 1] = b'>';
            spos += 2;

            for c in 0..GRID_COLS {
                match &self.cells[r][c] {
                    CellValue::Empty => {}
                    CellValue::Number(n) => {
                        // <c r="A1"><v>10</v></c>
                        spos += write_cell_ref(c, r, &mut sheet_xml[spos..]);
                        let v_open = b"><v>";
                        sheet_xml[spos..spos + v_open.len()].copy_from_slice(v_open);
                        spos += v_open.len();

                        let mut nbuf = [0u8; 24];
                        let nlen = format_i64(*n, &mut nbuf);
                        sheet_xml[spos..spos + nlen].copy_from_slice(&nbuf[..nlen]);
                        spos += nlen;

                        let v_close = b"</v></c>";
                        sheet_xml[spos..spos + v_close.len()].copy_from_slice(v_close);
                        spos += v_close.len();
                    }
                    CellValue::Text(tbuf, tlen) => {
                        let text_slice = &tbuf[..*tlen];
                        let mut str_idx = 0usize;
                        for i in 0..unique_count {
                            if unique_strings[i] == text_slice {
                                str_idx = i;
                                break;
                            }
                        }

                        // <c r="A1" t="s"><v>0</v></c>
                        spos += write_cell_ref(c, r, &mut sheet_xml[spos..]);
                        let s_open = b" t=\"s\"><v>";
                        sheet_xml[spos..spos + s_open.len()].copy_from_slice(s_open);
                        spos += s_open.len();

                        let mut ibuf = [0u8; 24];
                        let ilen = format_i64(str_idx as i64, &mut ibuf);
                        sheet_xml[spos..spos + ilen].copy_from_slice(&ibuf[..ilen]);
                        spos += ilen;

                        let s_close = b"</v></c>";
                        sheet_xml[spos..spos + s_close.len()].copy_from_slice(s_close);
                        spos += s_close.len();
                    }
                    CellValue::Formula(fbuf, flen) => {
                        // <c r="A4"><f>SUM(A1:A3)</f><v>60</v></c>
                        spos += write_cell_ref(c, r, &mut sheet_xml[spos..]);
                        let f_open = b"><f>";
                        sheet_xml[spos..spos + f_open.len()].copy_from_slice(f_open);
                        spos += f_open.len();

                        let raw_f = if fbuf[0] == b'=' { &fbuf[1..*flen] } else { &fbuf[..*flen] };
                        sheet_xml[spos..spos + raw_f.len()].copy_from_slice(raw_f);
                        spos += raw_f.len();

                        let v_open = b"</f><v>";
                        sheet_xml[spos..spos + v_open.len()].copy_from_slice(v_open);
                        spos += v_open.len();

                        if let CellComputedState::Number(val) = self.computed[r][c] {
                            let mut nbuf = [0u8; 24];
                            let nlen = format_i64(val, &mut nbuf);
                            sheet_xml[spos..spos + nlen].copy_from_slice(&nbuf[..nlen]);
                            spos += nlen;
                        }

                        let v_close = b"</v></c>";
                        sheet_xml[spos..spos + v_close.len()].copy_from_slice(v_close);
                        spos += v_close.len();
                    }
                }
            }

            let row_close = b"</row>";
            sheet_xml[spos..spos + row_close.len()].copy_from_slice(row_close);
            spos += row_close.len();
        }

        let ftr = b"</sheetData></worksheet>";
        sheet_xml[spos..spos + ftr.len()].copy_from_slice(ftr);
        spos += ftr.len();

        // 2. Generate xl/sharedStrings.xml if strings are present
        let mut sst_xml = [0u8; 2048];
        let mut sst_pos = 0usize;
        if unique_count > 0 {
            let sst_p1 = b"<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<sst xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" count=\"";
            sst_xml[sst_pos..sst_pos + sst_p1.len()].copy_from_slice(sst_p1);
            sst_pos += sst_p1.len();

            let mut nbuf = [0u8; 24];
            let nlen = format_i64(total_string_cells as i64, &mut nbuf);
            sst_xml[sst_pos..sst_pos + nlen].copy_from_slice(&nbuf[..nlen]);
            sst_pos += nlen;

            let sst_p2 = b"\" uniqueCount=\"";
            sst_xml[sst_pos..sst_pos + sst_p2.len()].copy_from_slice(sst_p2);
            sst_pos += sst_p2.len();

            let ulen = format_i64(unique_count as i64, &mut nbuf);
            sst_xml[sst_pos..sst_pos + ulen].copy_from_slice(&nbuf[..ulen]);
            sst_pos += ulen;

            let sst_p3 = b"\">";
            sst_xml[sst_pos..sst_pos + sst_p3.len()].copy_from_slice(sst_p3);
            sst_pos += sst_p3.len();

            for i in 0..unique_count {
                let si_open = b"<si><t>";
                sst_xml[sst_pos..sst_pos + si_open.len()].copy_from_slice(si_open);
                sst_pos += si_open.len();

                sst_xml[sst_pos..sst_pos + unique_strings[i].len()].copy_from_slice(unique_strings[i]);
                sst_pos += unique_strings[i].len();

                let si_close = b"</t></si>";
                sst_xml[sst_pos..sst_pos + si_close.len()].copy_from_slice(si_close);
                sst_pos += si_close.len();
            }

            let sst_close = b"</sst>";
            sst_xml[sst_pos..sst_pos + sst_close.len()].copy_from_slice(sst_close);
            sst_pos += sst_close.len();
        }

        // Standard OPC XML documents
        let content_types_with_sst = b"<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/><Override PartName=\"/xl/workbook.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml\"/><Override PartName=\"/xl/worksheets/sheet1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml\"/><Override PartName=\"/xl/sharedStrings.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml\"/></Types>";
        let content_types_no_sst = b"<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/><Override PartName=\"/xl/workbook.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml\"/><Override PartName=\"/xl/worksheets/sheet1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml\"/></Types>";
        let rels = b"<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"xl/workbook.xml\"/></Relationships>";
        let workbook = b"<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<workbook xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\"><sheets><sheet name=\"Sheet1\" sheetId=\"1\" r:id=\"rId1\"/></sheets></workbook>";
        let wb_rels_with_sst = b"<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet\" Target=\"worksheets/sheet1.xml\"/><Relationship Id=\"rId2\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/sharedStrings\" Target=\"sharedStrings.xml\"/></Relationships>";
        let wb_rels_no_sst = b"<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet\" Target=\"worksheets/sheet1.xml\"/></Relationships>";

        if unique_count > 0 {
            let parts: [(&str, &[u8]); 6] = [
                ("[Content_Types].xml", content_types_with_sst),
                ("_rels/.rels", rels),
                ("xl/workbook.xml", workbook),
                ("xl/_rels/workbook.xml.rels", wb_rels_with_sst),
                ("xl/worksheets/sheet1.xml", &sheet_xml[..spos]),
                ("xl/sharedStrings.xml", &sst_xml[..sst_pos]),
            ];
            create_opc_zip_archive(&parts, out_buf)
        } else {
            let parts: [(&str, &[u8]); 5] = [
                ("[Content_Types].xml", content_types_no_sst),
                ("_rels/.rels", rels),
                ("xl/workbook.xml", workbook),
                ("xl/_rels/workbook.xml.rels", wb_rels_no_sst),
                ("xl/worksheets/sheet1.xml", &sheet_xml[..spos]),
            ];
            create_opc_zip_archive(&parts, out_buf)
        }
    }

    pub fn import_xlsx_sheet(&mut self, sheet_xml: &str) -> Result<(), GridError> {
        self.import_xlsx_sheet_with_sst(sheet_xml, &[])
    }

    pub fn import_xlsx_sheet_with_sst(&mut self, sheet_xml: &str, sst: &[&str]) -> Result<(), GridError> {
        let mut cur = sheet_xml;
        while let Some(c_idx) = cur.find("<c r=\"") {
            cur = &cur[c_idx + 6..];
            let quote_end = cur.find('"').ok_or(GridError::XlsxParseError)?;
            let cell_coord_str = &cur[..quote_end];
            let (c, r) = parse_cell_coord(cell_coord_str)?;

            let c_end = cur.find("</c>").ok_or(GridError::XlsxParseError)?;
            let cell_tag_and_body = &cur[quote_end..c_end];

            let is_shared_string = cell_tag_and_body.starts_with("\" t=\"s\"")
                || cell_tag_and_body.contains("t=\"s\"");

            if let Some(f_start) = cell_tag_and_body.find("<f>") {
                let f_end = cell_tag_and_body.find("</f>").ok_or(GridError::XlsxParseError)?;
                let formula_text = &cell_tag_and_body[f_start + 3..f_end];
                let mut full_formula = [0u8; 32];
                full_formula[0] = b'=';
                let flen = core::cmp::min(formula_text.len(), 31);
                full_formula[1..1 + flen].copy_from_slice(&formula_text.as_bytes()[..flen]);
                self.set_formula(c, r, core::str::from_utf8(&full_formula[..1 + flen]).unwrap())?;
            } else if let Some(v_start) = cell_tag_and_body.find("<v>") {
                let v_end = cell_tag_and_body.find("</v>").ok_or(GridError::XlsxParseError)?;
                let val_str = &cell_tag_and_body[v_start + 3..v_end];
                if is_shared_string {
                    if let Ok(idx) = parse_i64(val_str) {
                        if (idx as usize) < sst.len() {
                            self.set_text(c, r, sst[idx as usize])?;
                        } else {
                            self.set_text(c, r, val_str)?;
                        }
                    }
                } else if let Ok(n) = parse_i64(val_str) {
                    self.set_number(c, r, n)?;
                }
            } else if let Some(t_start) = cell_tag_and_body.find("<t>") {
                let t_end = cell_tag_and_body.find("</t>").ok_or(GridError::XlsxParseError)?;
                let txt_str = &cell_tag_and_body[t_start + 3..t_end];
                self.set_text(c, r, txt_str)?;
            }

            cur = &cur[c_end + 4..];
        }

        self.recalculate();
        Ok(())
    }
}

pub struct GridWorkbook {
    pub sheets: [GridSheet; 2],
    pub sheet_names: [&'static str; 2],
    pub active_sheet: usize,
}

impl GridWorkbook {
    pub const fn new() -> Self {
        Self {
            sheets: [GridSheet::new(), GridSheet::new()],
            sheet_names: ["Sheet1", "Sheet2"],
            active_sheet: 0,
        }
    }

    pub fn active(&mut self) -> &mut GridSheet {
        &mut self.sheets[self.active_sheet]
    }

    pub fn active_ref(&self) -> &GridSheet {
        &self.sheets[self.active_sheet]
    }

    pub fn select_sheet(&mut self, idx: usize) -> Result<(), GridError> {
        if idx >= self.sheets.len() {
            return Err(GridError::InvalidCellRef);
        }
        self.active_sheet = idx;
        Ok(())
    }

    pub fn export_xlsx(&self, out_buf: &mut [u8]) -> Result<usize, GridError> {
        self.sheets[0].export_xlsx(out_buf)
    }
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

// --- Multi-File Stored ZIP Package Builder for OPC XLSX ---

pub fn create_opc_zip_archive(
    files: &[(&str, &[u8])],
    out_buf: &mut [u8],
) -> Result<usize, GridError> {
    let mut local_offsets = [0u32; 8];
    let mut pos = 0usize;

    for (i, &(name, data)) in files.iter().enumerate() {
        if i >= 8 {
            return Err(GridError::BufferTooSmall);
        }
        local_offsets[i] = pos as u32;

        let name_bytes = name.as_bytes();
        let file_crc = crc32(data);
        let data_len = data.len() as u32;

        let needed = 30 + name_bytes.len() + data.len();
        if pos + needed > out_buf.len() {
            return Err(GridError::BufferTooSmall);
        }

        // Local Header (PK\x03\x04)
        out_buf[pos..pos + 4].copy_from_slice(&[0x50, 0x4B, 0x03, 0x04]);
        out_buf[pos + 4..pos + 6].copy_from_slice(&20u16.to_le_bytes());
        out_buf[pos + 6..pos + 8].copy_from_slice(&0u16.to_le_bytes());
        out_buf[pos + 8..pos + 10].copy_from_slice(&0u16.to_le_bytes()); // Method 0 = Stored
        out_buf[pos + 10..pos + 14].copy_from_slice(&0u32.to_le_bytes());
        out_buf[pos + 14..pos + 18].copy_from_slice(&file_crc.to_le_bytes());
        out_buf[pos + 18..pos + 22].copy_from_slice(&data_len.to_le_bytes());
        out_buf[pos + 22..pos + 26].copy_from_slice(&data_len.to_le_bytes());
        out_buf[pos + 26..pos + 28].copy_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        out_buf[pos + 28..pos + 30].copy_from_slice(&0u16.to_le_bytes());
        pos += 30;

        out_buf[pos..pos + name_bytes.len()].copy_from_slice(name_bytes);
        pos += name_bytes.len();

        out_buf[pos..pos + data.len()].copy_from_slice(data);
        pos += data.len();
    }

    let cd_offset = pos as u32;

    // Central Directory
    for (i, &(name, data)) in files.iter().enumerate() {
        let name_bytes = name.as_bytes();
        let file_crc = crc32(data);
        let data_len = data.len() as u32;

        let needed = 46 + name_bytes.len();
        if pos + needed > out_buf.len() {
            return Err(GridError::BufferTooSmall);
        }

        out_buf[pos..pos + 4].copy_from_slice(&[0x50, 0x4B, 0x01, 0x02]);
        out_buf[pos + 4..pos + 6].copy_from_slice(&20u16.to_le_bytes());
        out_buf[pos + 6..pos + 8].copy_from_slice(&20u16.to_le_bytes());
        out_buf[pos + 8..pos + 10].copy_from_slice(&0u16.to_le_bytes());
        out_buf[pos + 10..pos + 12].copy_from_slice(&0u16.to_le_bytes());
        out_buf[pos + 12..pos + 16].copy_from_slice(&0u32.to_le_bytes());
        out_buf[pos + 16..pos + 20].copy_from_slice(&file_crc.to_le_bytes());
        out_buf[pos + 20..pos + 24].copy_from_slice(&data_len.to_le_bytes());
        out_buf[pos + 24..pos + 28].copy_from_slice(&data_len.to_le_bytes());
        out_buf[pos + 28..pos + 30].copy_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        out_buf[pos + 30..pos + 32].copy_from_slice(&0u16.to_le_bytes());
        out_buf[pos + 32..pos + 34].copy_from_slice(&0u16.to_le_bytes());
        out_buf[pos + 34..pos + 36].copy_from_slice(&0u16.to_le_bytes());
        out_buf[pos + 36..pos + 38].copy_from_slice(&0u16.to_le_bytes());
        out_buf[pos + 38..pos + 42].copy_from_slice(&0u32.to_le_bytes());
        out_buf[pos + 42..pos + 46].copy_from_slice(&local_offsets[i].to_le_bytes());
        pos += 46;

        out_buf[pos..pos + name_bytes.len()].copy_from_slice(name_bytes);
        pos += name_bytes.len();
    }

    let cd_size = (pos - cd_offset as usize) as u32;

    // EOCD (PK\x05\x06)
    if pos + 22 > out_buf.len() {
        return Err(GridError::BufferTooSmall);
    }
    out_buf[pos..pos + 4].copy_from_slice(&[0x50, 0x4B, 0x05, 0x06]);
    out_buf[pos + 4..pos + 6].copy_from_slice(&0u16.to_le_bytes());
    out_buf[pos + 6..pos + 8].copy_from_slice(&0u16.to_le_bytes());
    out_buf[pos + 8..pos + 10].copy_from_slice(&(files.len() as u16).to_le_bytes());
    out_buf[pos + 10..pos + 12].copy_from_slice(&(files.len() as u16).to_le_bytes());
    out_buf[pos + 12..pos + 16].copy_from_slice(&cd_size.to_le_bytes());
    out_buf[pos + 16..pos + 20].copy_from_slice(&cd_offset.to_le_bytes());
    out_buf[pos + 20..pos + 22].copy_from_slice(&0u16.to_le_bytes());
    pos += 22;

    Ok(pos)
}

// --- Coordinate & Number Helpers ---

pub fn parse_cell_coord(coord: &str) -> Result<(usize, usize), GridError> {
    let bytes = coord.as_bytes();
    if bytes.len() < 2 {
        return Err(GridError::InvalidCellRef);
    }
    let col_char = bytes[0];
    let col = match col_char {
        b'A'..=b'H' => (col_char - b'A') as usize,
        b'a'..=b'h' => (col_char - b'a') as usize,
        _ => return Err(GridError::InvalidCellRef),
    };
    let row_num = parse_i64(core::str::from_utf8(&bytes[1..]).map_err(|_| GridError::InvalidCellRef)?)?;
    if row_num < 1 || row_num > GRID_ROWS as i64 {
        return Err(GridError::InvalidCellRef);
    }
    Ok((col, (row_num - 1) as usize))
}

pub fn parse_range(range_str: &str) -> Result<(usize, usize, usize, usize), GridError> {
    let pos = range_str.find(':').ok_or(GridError::InvalidCellRef)?;
    let start = &range_str[..pos].trim();
    let end = &range_str[pos + 1..].trim();
    let (c1, r1) = parse_cell_coord(start)?;
    let (c2, r2) = parse_cell_coord(end)?;
    let min_c = core::cmp::min(c1, c2);
    let max_c = core::cmp::max(c1, c2);
    let min_r = core::cmp::min(r1, r2);
    let max_r = core::cmp::max(r1, r2);
    Ok((min_c, min_r, max_c, max_r))
}

pub fn parse_i64(s: &str) -> Result<i64, GridError> {
    let mut neg = false;
    let mut val = 0i64;
    let bytes = s.as_bytes();
    if bytes.is_empty() {
        return Err(GridError::SyntaxError);
    }
    let mut start = 0;
    if bytes[0] == b'-' {
        neg = true;
        start = 1;
    } else if bytes[0] == b'+' {
        start = 1;
    }
    if start == bytes.len() {
        return Err(GridError::SyntaxError);
    }
    for &b in &bytes[start..] {
        if !b.is_ascii_digit() {
            return Err(GridError::SyntaxError);
        }
        val = val * 10 + (b - b'0') as i64;
    }
    if neg { Ok(-val) } else { Ok(val) }
}

pub fn format_i64(mut n: i64, buf: &mut [u8; 24]) -> usize {
    if n == 0 {
        buf[0] = b'0';
        return 1;
    }
    let mut neg = false;
    if n < 0 {
        neg = true;
        n = -n;
    }
    let mut temp = [0u8; 24];
    let mut idx = 0;
    while n > 0 {
        temp[idx] = b'0' + (n % 10) as u8;
        n /= 10;
        idx += 1;
    }
    let mut out_idx = 0;
    if neg {
        buf[out_idx] = b'-';
        out_idx += 1;
    }
    for i in (0..idx).rev() {
        buf[out_idx] = temp[i];
        out_idx += 1;
    }
    out_idx
}
