// userspace/apps/canvas_app/src/engine.rs - Native Canvas Vector Graphic, Scene-Graph Diff & Intent Engine
// Freestanding no_std vector graphics & scene-graph engine shared between `canvas_app` (Ring 3)
// and the INOS Bridge Daemon (`inos_arc_bridge_daemon`).

#![allow(dead_code)]

pub const MAX_SHAPES: usize = 64;
pub const MAX_TEXT_LEN: usize = 48;
pub const MAX_SVG_BUF: usize = 8192;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShapeType {
    None,
    Rect,
    HollowRect,
    Ellipse,
    Line,
    Text,
    Sprite,
}

pub type ShapeKind = ShapeType;

#[derive(Debug, Clone, Copy)]
pub struct Shape {
    pub shape_type: ShapeType,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub fill_color: u32,
    pub stroke_color: u32,
    pub stroke_width: u32,
    pub text_buf: [u8; MAX_TEXT_LEN],
    pub text_len: usize,
    pub visible: bool,
}

impl Shape {
    pub const fn empty() -> Self {
        Self {
            shape_type: ShapeType::None,
            x: 0,
            y: 0,
            w: 0,
            h: 0,
            fill_color: 0,
            stroke_color: 0,
            stroke_width: 0,
            text_buf: [0u8; MAX_TEXT_LEN],
            text_len: 0,
            visible: true,
        }
    }

    pub fn new_rect(x: i32, y: i32, w: i32, h: i32, fill: u32, stroke: u32, stroke_w: u32) -> Self {
        let mut s = Self::empty();
        s.shape_type = ShapeType::Rect;
        s.x = x;
        s.y = y;
        s.w = w;
        s.h = h;
        s.fill_color = fill;
        s.stroke_color = stroke;
        s.stroke_width = stroke_w;
        s
    }

    pub fn new_hollow_rect(x: i32, y: i32, w: i32, h: i32, fill: u32, stroke: u32, stroke_w: u32) -> Self {
        let mut s = Self::empty();
        s.shape_type = ShapeType::HollowRect;
        s.x = x;
        s.y = y;
        s.w = w;
        s.h = h;
        s.fill_color = fill;
        s.stroke_color = stroke;
        s.stroke_width = stroke_w;
        s
    }

    pub fn new_ellipse(cx: i32, cy: i32, rx: i32, ry: i32, fill: u32, stroke: u32) -> Self {
        let mut s = Self::empty();
        s.shape_type = ShapeType::Ellipse;
        s.x = cx;
        s.y = cy;
        s.w = rx;
        s.h = ry;
        s.fill_color = fill;
        s.stroke_color = stroke;
        s.stroke_width = 1;
        s
    }

    pub fn new_line(x1: i32, y1: i32, x2: i32, y2: i32, stroke: u32, stroke_w: u32) -> Self {
        let mut s = Self::empty();
        s.shape_type = ShapeType::Line;
        s.x = x1;
        s.y = y1;
        s.w = x2;
        s.h = y2;
        s.fill_color = stroke;
        s.stroke_color = stroke;
        s.stroke_width = stroke_w;
        s
    }

    pub fn new_text(x: i32, y: i32, text: &str, color: u32) -> Self {
        let mut s = Self::empty();
        s.shape_type = ShapeType::Text;
        s.x = x;
        s.y = y;
        s.fill_color = color;
        let c_len = core::cmp::min(text.len(), MAX_TEXT_LEN);
        s.text_buf[..c_len].copy_from_slice(&text.as_bytes()[..c_len]);
        s.text_len = c_len;
        s
    }

    pub fn hit_test(&self, px: i32, py: i32) -> bool {
        if !self.visible {
            return false;
        }
        match self.shape_type {
            ShapeType::Rect | ShapeType::HollowRect | ShapeType::Sprite => {
                px >= self.x && px <= (self.x + self.w) && py >= self.y && py <= (self.y + self.h)
            }
            ShapeType::Ellipse => {
                if self.w <= 0 || self.h <= 0 {
                    return false;
                }
                let dx = px - self.x;
                let dy = py - self.y;
                let rx2 = self.w * self.w;
                let ry2 = self.h * self.h;
                ((dx * dx) * ry2 + (dy * dy) * rx2) <= (rx2 * ry2)
            }
            ShapeType::Line => {
                let min_x = self.x.min(self.w) - 2;
                let max_x = self.x.max(self.w) + 2;
                let min_y = self.y.min(self.h) - 2;
                let max_y = self.y.max(self.h) + 2;
                px >= min_x && px <= max_x && py >= min_y && py <= max_y
            }
            ShapeType::Text => {
                let text_w = (self.text_len as i32) * 8;
                let text_h = 16;
                px >= self.x && px <= (self.x + text_w) && py >= self.y && py <= (self.y + text_h)
            }
            _ => false,
        }
    }
}

pub struct CanvasDocument {
    pub width: u32,
    pub height: u32,
    pub shapes: [Shape; MAX_SHAPES],
    pub shape_count: usize,
    pub selected_idx: Option<usize>,
}

impl CanvasDocument {
    pub const fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            shapes: [Shape::empty(); MAX_SHAPES],
            shape_count: 0,
            selected_idx: None,
        }
    }

    pub fn add_shape(&mut self, shape: Shape) -> Option<usize> {
        if self.shape_count >= MAX_SHAPES {
            return None;
        }
        let idx = self.shape_count;
        self.shapes[idx] = shape;
        self.shape_count += 1;
        Some(idx)
    }

    pub fn hit_test(&self, px: i32, py: i32) -> Option<usize> {
        for i in (0..self.shape_count).rev() {
            if self.shapes[i].hit_test(px, py) {
                return Some(i);
            }
        }
        None
    }

    pub fn bring_to_front(&mut self, idx: usize) {
        if idx >= self.shape_count || self.shape_count <= 1 {
            return;
        }
        let target = self.shapes[idx];
        for i in idx..(self.shape_count - 1) {
            self.shapes[i] = self.shapes[i + 1];
        }
        self.shapes[self.shape_count - 1] = target;
        self.selected_idx = Some(self.shape_count - 1);
    }

    pub fn send_to_back(&mut self, idx: usize) {
        if idx >= self.shape_count || self.shape_count <= 1 {
            return;
        }
        let target = self.shapes[idx];
        for i in (1..=idx).rev() {
            self.shapes[i] = self.shapes[i - 1];
        }
        self.shapes[0] = target;
        self.selected_idx = Some(0);
    }

    pub fn translate_selected(&mut self, dx: i32, dy: i32) {
        if let Some(idx) = self.selected_idx {
            if idx < self.shape_count {
                self.shapes[idx].x += dx;
                self.shapes[idx].y += dy;
            }
        }
    }

    /// Vectorizes a 2D color-indexed raster frame (up to 64x64) into `Shape` primitives
    /// (`ShapeType::Rect`, `ShapeType::HollowRect`, `ShapeType::Ellipse`, `ShapeType::Line`, `ShapeType::Sprite`).
    pub fn vectorize_raster_frame(&mut self, grid: &[u8], grid_w: usize, grid_h: usize, bg_color: u8) -> usize {
        if grid_w == 0 || grid_h == 0 || grid.len() < grid_w * grid_h || grid_w > 64 || grid_h > 64 {
            return 0;
        }
        let mut visited = [false; 4096];
        let mut queue = [(0usize, 0usize); 1024];
        let mut added = 0usize;

        for r in 0..grid_h {
            for c in 0..grid_w {
                let idx0 = r * grid_w + c;
                let col = grid[idx0];
                if visited[idx0] || col == bg_color {
                    continue;
                }
                visited[idx0] = true;
                let mut q_head = 0usize;
                let mut q_tail = 0usize;
                queue[q_tail] = (r, c);
                q_tail += 1;

                let mut min_r = r;
                let mut max_r = r;
                let mut min_c = c;
                let mut max_c = c;
                let mut area = 0usize;

                while q_head < q_tail {
                    let (cr, cc) = queue[q_head];
                    q_head += 1;
                    area += 1;
                    if cr < min_r { min_r = cr; }
                    if cr > max_r { max_r = cr; }
                    if cc < min_c { min_c = cc; }
                    if cc > max_c { max_c = cc; }

                    let neighbors = [
                        (cr.wrapping_sub(1), cc, cr > 0),
                        (cr + 1, cc, cr + 1 < grid_h),
                        (cr, cc.wrapping_sub(1), cc > 0),
                        (cr, cc + 1, cc + 1 < grid_w),
                    ];
                    for &(nr, nc, ok) in &neighbors {
                        if ok {
                            let nidx = nr * grid_w + nc;
                            if !visited[nidx] && grid[nidx] == col && q_tail < queue.len() {
                                visited[nidx] = true;
                                queue[q_tail] = (nr, nc);
                                q_tail += 1;
                            }
                        }
                    }
                }

                let bw = (max_c - min_c + 1) as i32;
                let bh = (max_r - min_r + 1) as i32;
                let rgb = (col as u32) * 0x112233;
                let shape = if area <= 4 && bw <= 2 && bh <= 2 {
                    Shape::new_ellipse(min_c as i32 + bw / 2, min_r as i32 + bh / 2, bw.max(1), bh.max(1), rgb, rgb)
                } else if (bw == 1 && bh >= 3) || (bh == 1 && bw >= 3) {
                    Shape::new_line(min_c as i32, min_r as i32, max_c as i32, max_r as i32, rgb, 1)
                } else if bw >= 3 && bh >= 3 && area == (2 * (bw + bh) - 4) as usize {
                    Shape::new_hollow_rect(min_c as i32, min_r as i32, bw, bh, 0, rgb, 1)
                } else {
                    Shape::new_rect(min_c as i32, min_r as i32, bw, bh, rgb, rgb, 1)
                };
                if self.add_shape(shape).is_some() {
                    added += 1;
                }
            }
        }
        added
    }

    /// Rasterizes a vector `Shape` back onto a 2D color-indexed cell buffer (`w x h`).
    pub fn rasterize_onto(&self, cells: &mut [u8], w: usize, h: usize, bg: u8) {
        for i in 0..(w * h).min(cells.len()) {
            cells[i] = bg;
        }
        for i in 0..self.shape_count {
            let s = &self.shapes[i];
            if !s.visible {
                continue;
            }
            let col = ((s.fill_color / 0x112233) & 0x0F) as u8;
            let stroke_col = ((s.stroke_color / 0x112233) & 0x0F) as u8;
            match s.shape_type {
                ShapeType::Rect => {
                    for r in s.y.max(0)..(s.y + s.h).min(h as i32) {
                        for c in s.x.max(0)..(s.x + s.w).min(w as i32) {
                            cells[(r as usize) * w + (c as usize)] = col;
                        }
                    }
                }
                ShapeType::HollowRect => {
                    for r in s.y.max(0)..(s.y + s.h).min(h as i32) {
                        for c in s.x.max(0)..(s.x + s.w).min(w as i32) {
                            let is_border = r == s.y || r == s.y + s.h - 1 || c == s.x || c == s.x + s.w - 1;
                            if is_border {
                                cells[(r as usize) * w + (c as usize)] = stroke_col;
                            } else if s.fill_color != 0 {
                                cells[(r as usize) * w + (c as usize)] = col;
                            }
                        }
                    }
                }
                ShapeType::Line => {
                    let mut r = s.y;
                    let mut c = s.x;
                    let dr = (s.h - s.y).signum();
                    let dc = (s.w - s.x).signum();
                    let steps = (s.h - s.y).abs().max((s.w - s.x).abs());
                    for _ in 0..=steps {
                        if r >= 0 && r < h as i32 && c >= 0 && c < w as i32 {
                            cells[(r as usize) * w + (c as usize)] = stroke_col;
                        }
                        r += dr;
                        c += dc;
                    }
                }
                ShapeType::Ellipse => {
                    if s.x >= 0 && s.x < w as i32 && s.y >= 0 && s.y < h as i32 {
                        cells[(s.y as usize) * w + (s.x as usize)] = col;
                    }
                }
                _ => {}
            }
        }
    }

    // Bidirectional SVG Export
    pub fn export_svg(&self, out: &mut [u8]) -> usize {
        let mut cursor = 0;
        let append = |dst: &mut [u8], c: &mut usize, s: &str| {
            let bytes = s.as_bytes();
            for &b in bytes {
                if *c < dst.len() {
                    dst[*c] = b;
                    *c += 1;
                }
            }
        };

        let append_num = |dst: &mut [u8], c: &mut usize, mut val: i32| {
            if val == 0 {
                append(dst, c, "0");
                return;
            }
            if val < 0 {
                append(dst, c, "-");
                val = -val;
            }
            let mut buf = [0u8; 12];
            let mut len = 0;
            while val > 0 {
                buf[len] = b'0' + (val % 10) as u8;
                val /= 10;
                len += 1;
            }
            for i in (0..len).rev() {
                if *c < dst.len() {
                    dst[*c] = buf[i];
                    *c += 1;
                }
            }
        };

        let append_hex_color = |dst: &mut [u8], c: &mut usize, color: u32| {
            let hex = b"0123456789abcdef";
            append(dst, c, "#");
            for shift in [20, 16, 12, 8, 4, 0] {
                let nibble = ((color >> shift) & 0xF) as usize;
                if *c < dst.len() {
                    dst[*c] = hex[nibble];
                    *c += 1;
                }
            }
        };

        append(out, &mut cursor, "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"");
        append_num(out, &mut cursor, self.width as i32);
        append(out, &mut cursor, "\" height=\"");
        append_num(out, &mut cursor, self.height as i32);
        append(out, &mut cursor, "\">\n");

        for i in 0..self.shape_count {
            let s = &self.shapes[i];
            if !s.visible {
                continue;
            }
            match s.shape_type {
                ShapeType::Rect | ShapeType::HollowRect => {
                    append(out, &mut cursor, "  <rect x=\"");
                    append_num(out, &mut cursor, s.x);
                    append(out, &mut cursor, "\" y=\"");
                    append_num(out, &mut cursor, s.y);
                    append(out, &mut cursor, "\" width=\"");
                    append_num(out, &mut cursor, s.w);
                    append(out, &mut cursor, "\" height=\"");
                    append_num(out, &mut cursor, s.h);
                    append(out, &mut cursor, "\" fill=\"");
                    append_hex_color(out, &mut cursor, s.fill_color);
                    append(out, &mut cursor, "\" />\n");
                }
                ShapeType::Ellipse => {
                    append(out, &mut cursor, "  <ellipse cx=\"");
                    append_num(out, &mut cursor, s.x);
                    append(out, &mut cursor, "\" cy=\"");
                    append_num(out, &mut cursor, s.y);
                    append(out, &mut cursor, "\" rx=\"");
                    append_num(out, &mut cursor, s.w);
                    append(out, &mut cursor, "\" ry=\"");
                    append_num(out, &mut cursor, s.h);
                    append(out, &mut cursor, "\" fill=\"");
                    append_hex_color(out, &mut cursor, s.fill_color);
                    append(out, &mut cursor, "\" />\n");
                }
                ShapeType::Line => {
                    append(out, &mut cursor, "  <line x1=\"");
                    append_num(out, &mut cursor, s.x);
                    append(out, &mut cursor, "\" y1=\"");
                    append_num(out, &mut cursor, s.y);
                    append(out, &mut cursor, "\" x2=\"");
                    append_num(out, &mut cursor, s.w);
                    append(out, &mut cursor, "\" y2=\"");
                    append_num(out, &mut cursor, s.h);
                    append(out, &mut cursor, "\" stroke=\"");
                    append_hex_color(out, &mut cursor, s.stroke_color);
                    append(out, &mut cursor, "\" />\n");
                }
                ShapeType::Text => {
                    append(out, &mut cursor, "  <text x=\"");
                    append_num(out, &mut cursor, s.x);
                    append(out, &mut cursor, "\" y=\"");
                    append_num(out, &mut cursor, s.y);
                    append(out, &mut cursor, "\" font-size=\"14\">");
                    if let Ok(t_str) = core::str::from_utf8(&s.text_buf[..s.text_len]) {
                        append(out, &mut cursor, t_str);
                    }
                    append(out, &mut cursor, "</text>\n");
                }
                _ => {}
            }
        }

        append(out, &mut cursor, "</svg>\n");
        cursor
    }

    // Bidirectional SVG Import
    pub fn import_svg(&mut self, svg: &[u8]) -> usize {
        let mut parsed = 0;
        let mut i = 0;
        while i < svg.len() {
            if i + 5 < svg.len() && &svg[i..i + 5] == b"<rect" {
                let x = parse_attr(svg, i, b"x=\"").unwrap_or(0);
                let y = parse_attr(svg, i, b"y=\"").unwrap_or(0);
                let w = parse_attr(svg, i, b"width=\"").unwrap_or(50);
                let h = parse_attr(svg, i, b"height=\"").unwrap_or(50);
                let fill = parse_hex_attr(svg, i, b"fill=\"#").unwrap_or(0x336699);
                self.add_shape(Shape::new_rect(x, y, w, h, fill, 0, 1));
                parsed += 1;
            } else if i + 8 < svg.len() && &svg[i..i + 8] == b"<ellipse" {
                let cx = parse_attr(svg, i, b"cx=\"").unwrap_or(0);
                let cy = parse_attr(svg, i, b"cy=\"").unwrap_or(0);
                let rx = parse_attr(svg, i, b"rx=\"").unwrap_or(20);
                let ry = parse_attr(svg, i, b"ry=\"").unwrap_or(20);
                let fill = parse_hex_attr(svg, i, b"fill=\"#").unwrap_or(0xe06666);
                self.add_shape(Shape::new_ellipse(cx, cy, rx, ry, fill, 0));
                parsed += 1;
            } else if i + 5 < svg.len() && &svg[i..i + 5] == b"<line" {
                let x1 = parse_attr(svg, i, b"x1=\"").unwrap_or(0);
                let y1 = parse_attr(svg, i, b"y1=\"").unwrap_or(0);
                let x2 = parse_attr(svg, i, b"x2=\"").unwrap_or(50);
                let y2 = parse_attr(svg, i, b"y2=\"").unwrap_or(50);
                let stroke = parse_hex_attr(svg, i, b"stroke=\"#").unwrap_or(0xffffff);
                self.add_shape(Shape::new_line(x1, y1, x2, y2, stroke, 1));
                parsed += 1;
            } else if i + 5 < svg.len() && &svg[i..i + 5] == b"<text" {
                let x = parse_attr(svg, i, b"x=\"").unwrap_or(0);
                let y = parse_attr(svg, i, b"y=\"").unwrap_or(0);
                if let Some(tag_end) = find_sub(svg, i, b">") {
                    if let Some(close_tag) = find_sub(svg, tag_end, b"</text>") {
                        let text_slice = &svg[tag_end + 1..close_tag];
                        if let Ok(t_str) = core::str::from_utf8(text_slice) {
                            self.add_shape(Shape::new_text(x, y, t_str, 0xffffff));
                            parsed += 1;
                        }
                    }
                }
            }
            i += 1;
        }
        parsed
    }
}

pub fn parse_hex_attr(data: &[u8], start: usize, attr_name: &[u8]) -> Option<u32> {
    if let Some(pos) = find_sub(data, start, attr_name) {
        let val_start = pos + attr_name.len();
        let mut val_end = val_start;
        while val_end < data.len() && data[val_end] != b'"' {
            val_end += 1;
        }
        let slice = &data[val_start..val_end];
        if slice.is_empty() {
            return None;
        }
        let mut res: u32 = 0;
        for &b in slice.iter().take(6) {
            let d = match b {
                b'0'..=b'9' => (b - b'0') as u32,
                b'a'..=b'f' => (b - b'a' + 10) as u32,
                b'A'..=b'F' => (b - b'A' + 10) as u32,
                _ => return None,
            };
            res = (res << 4) | d;
        }
        return Some(res);
    }
    None
}

pub fn parse_attr(data: &[u8], start: usize, attr_name: &[u8]) -> Option<i32> {
    if let Some(pos) = find_sub(data, start, attr_name) {
        let val_start = pos + attr_name.len();
        let mut val_end = val_start;
        while val_end < data.len() && data[val_end] != b'"' {
            val_end += 1;
        }
        let slice = &data[val_start..val_end];
        let mut res: i32 = 0;
        let mut neg = false;
        for (idx, &b) in slice.iter().enumerate() {
            if idx == 0 && b == b'-' {
                neg = true;
                continue;
            }
            if b >= b'0' && b <= b'9' {
                res = res * 10 + (b - b'0') as i32;
            } else {
                break;
            }
        }
        return Some(if neg { -res } else { res });
    }
    None
}

pub fn find_sub(data: &[u8], start: usize, needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || start >= data.len() {
        return None;
    }
    for i in start..=data.len().saturating_sub(needle.len()) {
        if &data[i..i + needle.len()] == needle {
            return Some(i);
        }
    }
    None
}

// ============================================================================
// Lossless Vector Scene Graph & Dihedral D4 Sprite Decomposition (`VectorSprite`)
// ============================================================================
#[derive(Clone, Copy)]
pub struct VectorSprite {
    pub shape_type: ShapeType,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub area: usize,
    pub primary_color: u8,
    pub minority_color: u8,
    pub minority_count: usize,
    pub minority_dx: i32,
    pub minority_dy: i32,
    pub color_mask: u16,
    pub distinct_colors: usize,
    pub hole_count: usize,
    pub hole_area: usize,
    pub touches_border: bool,
    pub is_sym_h: bool,
    pub is_sym_v: bool,
    pub is_sym_d4: bool,
    pub mask_rows: [u32; 30], // 1-bit silhouette mask per local row (0..h)
    pub norm_hash: u64,       // translation-invariant color+mask hash
    pub sil_hash: u64,        // translation+color-invariant silhouette hash
    pub d4_hashes: [u64; 8],  // silhouette hashes under all 8 D4 dihedral symmetries
    pub d4_canon_hash: u64,   // canonical D4-invariant silhouette hash
}

impl VectorSprite {
    pub const fn empty() -> Self {
        Self {
            shape_type: ShapeType::None,
            x: 0,
            y: 0,
            w: 0,
            h: 0,
            area: 0,
            primary_color: 0,
            minority_color: 0,
            minority_count: 0,
            minority_dx: 0,
            minority_dy: 0,
            color_mask: 0,
            distinct_colors: 0,
            hole_count: 0,
            hole_area: 0,
            touches_border: false,
            is_sym_h: false,
            is_sym_v: false,
            is_sym_d4: false,
            mask_rows: [0; 30],
            norm_hash: 0,
            sil_hash: 0,
            d4_hashes: [0; 8],
            d4_canon_hash: 0,
        }
    }

    #[inline]
    pub fn contains_local(&self, lr: usize, lc: usize) -> bool {
        lr < (self.h as usize) && lc < (self.w as usize) && ((self.mask_rows[lr] >> lc) & 1) != 0
    }
}

/// Computes the silhouette hash of a `mask_rows` (`w x h`) under D4 dihedral mode `d_mode` (`0..8`).
pub fn compute_d4_silhouette_hash(mask_rows: &[u32; 30], w: usize, h: usize, d_mode: u8) -> u64 {
    let (oh, ow) = if d_mode >= 4 { (w, h) } else { (h, w) };
    let mut hash: u64 = 0xcbf29ce484222325;
    hash = (hash ^ (oh as u64)).wrapping_mul(0x100000001b3);
    hash = (hash ^ (ow as u64)).wrapping_mul(0x100000001b3);
    for r in 0..oh {
        let mut row_bits: u32 = 0;
        for c in 0..ow {
            let (sr, sc) = match d_mode & 7 {
                0 => (r, c),
                1 => (r, ow - 1 - c),
                2 => (oh - 1 - r, c),
                3 => (oh - 1 - r, ow - 1 - c),
                4 => (c, r),
                5 => (c, oh - 1 - r),
                6 => (ow - 1 - c, r),
                _ => (ow - 1 - c, oh - 1 - r),
            };
            if ((mask_rows[sr] >> sc) & 1) != 0 {
                row_bits |= 1u32 << c;
            }
        }
        hash = (hash ^ (row_bits as u64)).wrapping_mul(0x100000001b3);
    }
    hash
}

/// Counts topological interior holes and total enclosed hole pixels inside a `w x h` sprite bounding box.
pub fn count_sprite_holes(mask_rows: &[u32; 30], w: usize, h: usize) -> (usize, usize) {
    if w < 3 || h < 3 || w > 30 || h > 30 {
        return (0, 0);
    }
    let mut outside = [0u32; 30];
    let mut q = [(0u8, 0u8); 900];
    let mut qh = 0usize;
    let mut qt = 0usize;

    for r in 0..h {
        for c in 0..w {
            if (r == 0 || r == h - 1 || c == 0 || c == w - 1) && ((mask_rows[r] >> c) & 1) == 0 {
                if ((outside[r] >> c) & 1) == 0 {
                    outside[r] |= 1u32 << c;
                    q[qt] = (r as u8, c as u8);
                    qt += 1;
                }
            }
        }
    }
    while qh < qt {
        let (cr, cc) = (q[qh].0 as usize, q[qh].1 as usize);
        qh += 1;
        let neighbors = [
            (cr.wrapping_sub(1), cc, cr > 0),
            (cr + 1, cc, cr + 1 < h),
            (cr, cc.wrapping_sub(1), cc > 0),
            (cr, cc + 1, cc + 1 < w),
        ];
        for &(nr, nc, ok) in &neighbors {
            if ok && ((mask_rows[nr] >> nc) & 1) == 0 && ((outside[nr] >> nc) & 1) == 0 {
                outside[nr] |= 1u32 << nc;
                q[qt] = (nr as u8, nc as u8);
                qt += 1;
            }
        }
    }

    let mut vis_hole = outside;
    let mut hole_count = 0usize;
    let mut hole_area = 0usize;
    for r in 1..(h - 1) {
        for c in 1..(w - 1) {
            if ((mask_rows[r] >> c) & 1) == 0 && ((vis_hole[r] >> c) & 1) == 0 {
                hole_count += 1;
                vis_hole[r] |= 1u32 << c;
                let mut hq_h = 0usize;
                let mut hq_t = 0usize;
                q[hq_t] = (r as u8, c as u8);
                hq_t += 1;
                while hq_h < hq_t {
                    let (hr, hc) = (q[hq_h].0 as usize, q[hq_h].1 as usize);
                    hq_h += 1;
                    hole_area += 1;
                    let neighbors = [
                        (hr.wrapping_sub(1), hc, hr > 0),
                        (hr + 1, hc, hr + 1 < h),
                        (hr, hc.wrapping_sub(1), hc > 0),
                        (hr, hc + 1, hc + 1 < w),
                    ];
                    for &(nr, nc, ok) in &neighbors {
                        if ok && ((mask_rows[nr] >> nc) & 1) == 0 && ((vis_hole[nr] >> nc) & 1) == 0 {
                            vis_hole[nr] |= 1u32 << nc;
                            q[hq_t] = (nr as u8, nc as u8);
                            hq_t += 1;
                        }
                    }
                }
            }
        }
    }
    (hole_count, hole_area)
}

/// Extracts up to `out_sprites.len()` `VectorSprite`s from a 2D raster buffer (`w x h` up to 30x30).
pub fn extract_vector_sprites(
    cells: &[u8],
    w: usize,
    h: usize,
    bg: u8,
    allow_diag: bool,
    multi_color: bool,
    out_sprites: &mut [VectorSprite],
) -> usize {
    if w == 0 || h == 0 || w > 30 || h > 30 || cells.len() < w * h {
        return 0;
    }
    let mut visited = [false; 900];
    let mut queue = [(0u8, 0u8); 900];
    let mut pts = [(0u8, 0u8, 0u8); 900];
    let mut count = 0usize;

    for r in 0..h {
        for c in 0..w {
            let idx0 = r * w + c;
            let c0 = cells[idx0];
            if visited[idx0] || c0 == bg || c0 >= 10 {
                continue;
            }
            if count >= out_sprites.len() {
                break;
            }
            visited[idx0] = true;
            let mut qh = 0usize;
            let mut qt = 0usize;
            queue[qt] = (r as u8, c as u8);
            qt += 1;

            let (mut min_r, mut max_r, mut min_c, mut max_c) = (r, r, c, c);
            let mut pts_len = 0usize;
            let mut color_counts = [0usize; 10];
            let mut color_mask = 0u16;
            let mut touches_border = false;

            while qh < qt {
                let (cr, cc) = (queue[qh].0 as usize, queue[qh].1 as usize);
                qh += 1;
                let cv = cells[cr * w + cc];
                pts[pts_len] = (cr as u8, cc as u8, cv);
                pts_len += 1;
                if (cv as usize) < 10 {
                    color_counts[cv as usize] += 1;
                    color_mask |= 1u16 << cv;
                }
                if cr < min_r { min_r = cr; }
                if cr > max_r { max_r = cr; }
                if cc < min_c { min_c = cc; }
                if cc > max_c { max_c = cc; }
                if cr == 0 || cr == h - 1 || cc == 0 || cc == w - 1 {
                    touches_border = true;
                }

                for dr in -1isize..=1 {
                    for dc in -1isize..=1 {
                        if dr == 0 && dc == 0 {
                            continue;
                        }
                        if !allow_diag && (dr != 0 && dc != 0) {
                            continue;
                        }
                        let nr = cr as isize + dr;
                        let nc = cc as isize + dc;
                        if nr >= 0 && nr < h as isize && nc >= 0 && nc < w as isize {
                            let nidx = (nr as usize) * w + (nc as usize);
                            let nv = cells[nidx];
                            let color_ok = if multi_color {
                                nv != bg && nv < 10
                            } else {
                                nv == c0
                            };
                            if !visited[nidx] && color_ok {
                                visited[nidx] = true;
                                queue[qt] = (nr as u8, nc as u8);
                                qt += 1;
                            }
                        }
                    }
                }
            }

            let bw = max_c - min_c + 1;
            let bh = max_r - min_r + 1;
            let mut mask_rows = [0u32; 30];
            for i in 0..pts_len {
                let lr = (pts[i].0 as usize) - min_r;
                let lc = (pts[i].1 as usize) - min_c;
                mask_rows[lr] |= 1u32 << lc;
            }

            // Primary & minority colors
            let mut primary_color = c0;
            let mut max_cnt = 0usize;
            let mut minority_color = c0;
            let mut min_cnt = usize::MAX;
            let mut distinct_colors = 0usize;
            for col in 0..10usize {
                let cnt = color_counts[col];
                if cnt > 0 {
                    distinct_colors += 1;
                    if cnt > max_cnt {
                        max_cnt = cnt;
                        primary_color = col as u8;
                    }
                    if cnt < min_cnt {
                        min_cnt = cnt;
                        minority_color = col as u8;
                    }
                }
            }
            if distinct_colors <= 1 {
                min_cnt = 0;
            }

            let mut sum_min_dx = 0i32;
            let mut sum_min_dy = 0i32;
            if min_cnt > 0 {
                for i in 0..pts_len {
                    if pts[i].2 == minority_color {
                        sum_min_dx += (pts[i].1 as i32) - (min_c as i32);
                        sum_min_dy += (pts[i].0 as i32) - (min_r as i32);
                    }
                }
            }
            let minority_dx = if min_cnt > 0 { sum_min_dx / (min_cnt as i32) } else { 0 };
            let minority_dy = if min_cnt > 0 { sum_min_dy / (min_cnt as i32) } else { 0 };

            let mut d4_hashes = [0u64; 8];
            let mut d4_canon_hash = u64::MAX;
            for d in 0..8u8 {
                let dh = compute_d4_silhouette_hash(&mask_rows, bw, bh, d);
                d4_hashes[d as usize] = dh;
                if dh < d4_canon_hash {
                    d4_canon_hash = dh;
                }
            }
            let sil_hash = d4_hashes[0];
            let is_sym_h = d4_hashes[0] == d4_hashes[1];
            let is_sym_v = d4_hashes[0] == d4_hashes[2];
            let is_sym_d4 = is_sym_h && is_sym_v && (bw == bh && d4_hashes[0] == d4_hashes[4]);

            // Norm hash includes colors
            let mut norm_hash: u64 = sil_hash ^ ((bw as u64) << 32) ^ (bh as u64);
            for r_loc in 0..bh {
                for c_loc in 0..bw {
                    let v = if ((mask_rows[r_loc] >> c_loc) & 1) != 0 {
                        cells[(min_r + r_loc) * w + (min_c + c_loc)]
                    } else {
                        255
                    };
                    norm_hash = (norm_hash ^ (v as u64)).wrapping_mul(0x100000001b3);
                }
            }

            let (hole_count, hole_area) = count_sprite_holes(&mask_rows, bw, bh);
            let shape_type = if pts_len <= 2 && bw <= 2 && bh <= 2 {
                ShapeType::Ellipse
            } else if (bw == 1 && bh >= 2) || (bh == 1 && bw >= 2) {
                ShapeType::Line
            } else if pts_len == bw * bh {
                ShapeType::Rect
            } else if bw >= 3 && bh >= 3 && hole_count == 1 && pts_len + hole_area == bw * bh {
                ShapeType::HollowRect
            } else {
                ShapeType::Sprite
            };

            out_sprites[count] = VectorSprite {
                shape_type,
                x: min_c as i32,
                y: min_r as i32,
                w: bw as i32,
                h: bh as i32,
                area: pts_len,
                primary_color,
                minority_color,
                minority_count: min_cnt,
                minority_dx,
                minority_dy,
                color_mask,
                distinct_colors,
                hole_count,
                hole_area,
                touches_border,
                is_sym_h,
                is_sym_v,
                is_sym_d4,
                mask_rows,
                norm_hash,
                sil_hash,
                d4_hashes,
                d4_canon_hash,
            };
            count += 1;
        }
    }
    count
}
