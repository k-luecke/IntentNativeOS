// userspace/apps/spin_app/src/engine.rs
// ============================================================================
// Native Ring 3 Spin Application Engine — Iterative Fixed-Point Cellular
// Convergence & Cyclic Color Orbit Stepper (`#![no_std]`)
// ============================================================================

#![allow(dead_code)]

/// Iterates rectangle-completion `if (r, c1) == col && (r1, c) == col && (r1, c1) == col -> (r, c) = fill_col`
/// until fixed-point convergence (bounded by `max_iters`).
pub fn converge_local_2x2_corner_completion(
    cells: &mut [u8; 900],
    h: usize,
    w: usize,
    bg: u8,
    fill_override: Option<u8>,
    max_iters: usize,
) -> usize {
    let mut total_added = 0usize;
    for _ in 0..max_iters {
        let mut changed = false;
        for r in 0..h.saturating_sub(1) {
            for c in 0..w.saturating_sub(1) {
                let p00 = cells[r * w + c];
                let p01 = cells[r * w + c + 1];
                let p10 = cells[(r + 1) * w + c];
                let p11 = cells[(r + 1) * w + c + 1];
                let non_bg = [p00 != bg, p01 != bg, p10 != bg, p11 != bg];
                let cnt = non_bg.iter().filter(|&&b| b).count();
                if cnt == 3 {
                    let active_col = if p00 != bg {
                        p00
                    } else if p01 != bg {
                        p01
                    } else {
                        p10
                    };
                    let same_col = [p00, p01, p10, p11]
                        .iter()
                        .all(|&v| v == bg || v == active_col);
                    if same_col {
                        let fc = fill_override.unwrap_or(active_col);
                        if p00 == bg {
                            cells[r * w + c] = fc;
                        } else if p01 == bg {
                            cells[r * w + c + 1] = fc;
                        } else if p10 == bg {
                            cells[(r + 1) * w + c] = fc;
                        } else {
                            cells[(r + 1) * w + c + 1] = fc;
                        }
                        total_added += 1;
                        changed = true;
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }
    total_added
}

/// Iterates orthogonal closure: any `bg` cell surrounded on >= 3 orthogonal sides by the
/// same non-bg color `c` fills with `fill_override.unwrap_or(c)` until convergence!
pub fn converge_3_sided_pocket_closure(
    cells: &mut [u8; 900],
    h: usize,
    w: usize,
    bg: u8,
    fill_override: Option<u8>,
    max_iters: usize,
) -> usize {
    let mut total_added = 0usize;
    for _ in 0..max_iters {
        let mut next = *cells;
        let mut changed = false;
        for r in 0..h {
            for c in 0..w {
                if cells[r * w + c] != bg {
                    continue;
                }
                let mut counts = [0u8; 10];
                if r > 0 {
                    let v = cells[(r - 1) * w + c];
                    if v != bg && (v as usize) < 10 {
                        counts[v as usize] += 1;
                    }
                }
                if r + 1 < h {
                    let v = cells[(r + 1) * w + c];
                    if v != bg && (v as usize) < 10 {
                        counts[v as usize] += 1;
                    }
                }
                if c > 0 {
                    let v = cells[r * w + c - 1];
                    if v != bg && (v as usize) < 10 {
                        counts[v as usize] += 1;
                    }
                }
                if c + 1 < w {
                    let v = cells[r * w + c + 1];
                    if v != bg && (v as usize) < 10 {
                        counts[v as usize] += 1;
                    }
                }
                for col in 0..10u8 {
                    if counts[col as usize] >= 3 {
                        next[r * w + c] = fill_override.unwrap_or(col);
                        total_added += 1;
                        changed = true;
                        break;
                    }
                }
            }
        }
        *cells = next;
        if !changed {
            break;
        }
    }
    total_added
}
