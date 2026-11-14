// userspace/apps/calc_app/src/engine.rs
// ============================================================================
// Native Ring 3 Calculator Application — Precision Arithmetic Parser,
// Affine Ratio Solver & 2D Modular Coordinate Engine (`#![no_std]`)
// ============================================================================

#![allow(dead_code)]

#[derive(Copy, Clone)]
pub struct IdempotencyRecord {
    pub caller_pid: u64,
    pub op_code: u8, // 1 = calc.eval, 2 = calc.verify
    pub req_id: u64,
    pub payload_expr: [u8; 16],
    pub payload_len: usize,
    pub result_len: usize,
    pub result: [u8; 16],
}

pub fn extract_expr_field(msg: &str) -> Option<&str> {
    let bytes = msg.as_bytes();
    let needle = b"EXPR:";
    for i in 0..=bytes.len().saturating_sub(needle.len()) {
        if &bytes[i..i + needle.len()] == needle {
            let start = i + needle.len();
            let mut end = start;
            while end < bytes.len()
                && bytes[end] != b'|'
                && bytes[end] != b':'
                && bytes[end] != b'\n'
                && bytes[end] != b'\r'
            {
                end += 1;
            }
            if end > start {
                return core::str::from_utf8(&bytes[start..end]).ok();
            }
        }
    }
    None
}

fn skip_ws(b: &[u8], pos: &mut usize) {
    while *pos < b.len() && b[*pos] == b' ' {
        *pos += 1;
    }
}

fn parse_factor(b: &[u8], pos: &mut usize) -> Option<i64> {
    skip_ws(b, pos);
    if *pos >= b.len() {
        return None;
    }
    let mut sign = 1i64;
    if b[*pos] == b'-' {
        sign = -1;
        *pos += 1;
    } else if b[*pos] == b'+' {
        *pos += 1;
    }
    skip_ws(b, pos);
    if *pos < b.len() && b[*pos] == b'(' {
        *pos += 1;
        let val = parse_expr_inner(b, pos)?;
        skip_ws(b, pos);
        if *pos >= b.len() || b[*pos] != b')' {
            return None;
        }
        *pos += 1;
        return Some(sign * val);
    }
    let start = *pos;
    let mut acc: i64 = 0;
    while *pos < b.len() && b[*pos] >= b'0' && b[*pos] <= b'9' {
        acc = acc.checked_mul(10)?.checked_add((b[*pos] - b'0') as i64)?;
        *pos += 1;
    }
    if *pos == start {
        return None;
    }
    Some(sign * acc)
}

fn parse_term(b: &[u8], pos: &mut usize) -> Option<i64> {
    let mut lhs = parse_factor(b, pos)?;
    loop {
        skip_ws(b, pos);
        if *pos >= b.len() {
            break;
        }
        let op = b[*pos];
        if op == b'*' || op == b'/' {
            *pos += 1;
            let rhs = parse_factor(b, pos)?;
            if op == b'*' {
                lhs = lhs.checked_mul(rhs)?;
            } else {
                if rhs == 0 {
                    return None;
                }
                lhs = lhs.checked_div(rhs)?;
            }
        } else {
            break;
        }
    }
    Some(lhs)
}

fn parse_expr_inner(b: &[u8], pos: &mut usize) -> Option<i64> {
    let mut lhs = parse_term(b, pos)?;
    loop {
        skip_ws(b, pos);
        if *pos >= b.len() {
            break;
        }
        let op = b[*pos];
        if op == b'+' || op == b'-' {
            *pos += 1;
            let rhs = parse_term(b, pos)?;
            if op == b'+' {
                lhs = lhs.checked_add(rhs)?;
            } else {
                lhs = lhs.checked_sub(rhs)?;
            }
        } else {
            break;
        }
    }
    Some(lhs)
}

pub fn eval_arithmetic_expr(expr: &str) -> Option<i64> {
    let b = expr.as_bytes();
    let mut pos = 0;
    let val = parse_expr_inner(b, &mut pos)?;
    skip_ws(b, &mut pos);
    if pos != b.len() {
        return None;
    }
    Some(val)
}

pub fn format_i64(mut val: i64, out: &mut [u8; 16]) -> usize {
    if val == 0 {
        out[0] = b'0';
        return 1;
    }
    let mut idx = 0;
    if val < 0 {
        out[idx] = b'-';
        idx += 1;
        val = -val;
    }
    let mut tmp = [0u8; 16];
    let mut t_len = 0;
    let mut u = val as u64;
    while u > 0 && t_len < 16 {
        tmp[t_len] = b'0' + (u % 10) as u8;
        u /= 10;
        t_len += 1;
    }
    for i in (0..t_len).rev() {
        if idx < 16 {
            out[idx] = tmp[i];
            idx += 1;
        }
    }
    idx
}

/// Solves for exact integer affine relationship `y = (a * x + b) / d` across pairs `(xs[i], ys[i])`.
pub fn induce_affine_int_rule(xs: &[i64], ys: &[i64]) -> Option<(i64, i64, i64)> {
    if xs.is_empty() || xs.len() != ys.len() {
        return None;
    }
    // Check constant: y = b
    let y0 = ys[0];
    if ys.iter().all(|&y| y == y0) {
        return Some((0, y0, 1));
    }
    // Check small rational slopes a/d in [-6..=6] / [1..=4]
    for d in 1..=4i64 {
        for a in -6..=6i64 {
            if a == 0 {
                continue;
            }
            let b = ys[0] * d - a * xs[0];
            let mut ok = true;
            for i in 0..xs.len() {
                let num = a * xs[i] + b;
                if num % d != 0 || num / d != ys[i] {
                    ok = false;
                    break;
                }
            }
            if ok {
                return Some((a, b, d));
            }
        }
    }
    None
}

#[inline]
pub fn apply_affine_int_rule(x: i64, rule: (i64, i64, i64)) -> Option<i64> {
    let (a, b, d) = rule;
    if d == 0 {
        return None;
    }
    let num = a.checked_mul(x)?.checked_add(b)?;
    if num % d != 0 {
        return None;
    }
    Some(num / d)
}
