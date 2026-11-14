// userspace/apps/net_app/src/engine.rs
// ============================================================================
// Native Ring 3 Network & Endpoint Routing Engine (`#![no_std]`)
// ============================================================================

#![allow(dead_code)]

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepAction {
    Transmit(usize),
    NeedMoreData,
    Closed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EndpointPort {
    pub r: usize,
    pub c: usize,
    pub color: u8,
}

/// Connects pairs of matching-color single-pixel endpoints (`EndpointPort`) via
/// obstacle-avoiding L-shaped paths (horizontal-then-vertical or vertical-then-horizontal).
pub fn try_route_l_wire(
    cells: &mut [u8; 900],
    h: usize,
    w: usize,
    p1: EndpointPort,
    p2: EndpointPort,
    bg: u8,
    wire_color: u8,
    hv_first: bool,
) -> bool {
    let check_and_draw = |cells: &mut [u8; 900], corner_r: usize, corner_c: usize| -> bool {
        // Verify all intermediate cells along (p1.r, p1.c) -> (corner_r, corner_c) -> (p2.r, p2.c) are `bg`
        let r_min1 = p1.r.min(corner_r);
        let r_max1 = p1.r.max(corner_r);
        let c_min1 = p1.c.min(corner_c);
        let c_max1 = p1.c.max(corner_c);
        for r in r_min1..=r_max1 {
            for c in c_min1..=c_max1 {
                if (r, c) != (p1.r, p1.c) && (r, c) != (p2.r, p2.c) {
                    let v = cells[r * w + c];
                    if v != bg && v != wire_color {
                        return false;
                    }
                }
            }
        }
        let r_min2 = corner_r.min(p2.r);
        let r_max2 = corner_r.max(p2.r);
        let c_min2 = corner_c.min(p2.c);
        let c_max2 = corner_c.max(p2.c);
        for r in r_min2..=r_max2 {
            for c in c_min2..=c_max2 {
                if (r, c) != (p1.r, p1.c) && (r, c) != (p2.r, p2.c) {
                    let v = cells[r * w + c];
                    if v != bg && v != wire_color {
                        return false;
                    }
                }
            }
        }

        // Draw L-path
        for r in r_min1..=r_max1 {
            for c in c_min1..=c_max1 {
                if (r, c) != (p1.r, p1.c) && (r, c) != (p2.r, p2.c) {
                    cells[r * w + c] = wire_color;
                }
            }
        }
        for r in r_min2..=r_max2 {
            for c in c_min2..=c_max2 {
                if (r, c) != (p1.r, p1.c) && (r, c) != (p2.r, p2.c) {
                    cells[r * w + c] = wire_color;
                }
            }
        }
        let _ = h;
        true
    };

    let (cr1, cc1, cr2, cc2) = if hv_first {
        (p1.r, p2.c, p2.r, p1.c)
    } else {
        (p2.r, p1.c, p1.r, p2.c)
    };
    if check_and_draw(cells, cr1, cc1) {
        return true;
    }
    check_and_draw(cells, cr2, cc2)
}
