// userspace/apps/notes_app/src/engine.rs
// ============================================================================
// Native Ring 3 Notes Application — Persistent Observation Ledger,
// Key-Value Legend Scratchpad & Residual Diff Notebook (`#![no_std]`)
// ============================================================================

#![allow(dead_code)]

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoteKind {
    GeometryInvariant,
    ColorConservation,
    LegendEntry,
    SymmetryObservation,
    SilhouetteMatch,
    ResidualNearMiss,
}

#[derive(Clone, Copy, Debug)]
pub struct LegendEntry {
    pub key_kind: u8, // 0=Color->Color, 1=ShapeHash->Color, 2=HoleCount->Color, 3=AreaRank->Color, 4=BorderColor->FillColor
    pub key_val: u64,
    pub target_color: u8,
    pub support_pairs: u8,
    pub contradicted: bool,
}

impl LegendEntry {
    pub const fn empty() -> Self {
        Self {
            key_kind: 0,
            key_val: 0,
            target_color: 0,
            support_pairs: 0,
            contradicted: false,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TaskNotesScratchpad {
    pub num_pairs: usize,
    pub same_dims: bool,
    pub fixed_out_dims: Option<(u8, u8)>,
    pub scale_up_factor: Option<(u8, u8)>,
    pub scale_down_factor: Option<(u8, u8)>,
    pub bg_in: u8,
    pub bg_out: u8,
    pub input_color_mask: u16,
    pub output_color_mask: u16,
    pub static_color_mask: u16, // Colors whose exact positions never change between in and out
    pub added_color_mask: u16,  // Colors present in output that were absent in input
    pub erased_color_mask: u16, // Colors present in input that vanish in output
    pub fg_always_preserved: bool, // Every non-bg input pixel remains non-bg in output
    pub out_subset_of_in_fg: bool, // Output foreground is strictly a subset of input foreground
    pub in_subset_of_out_fg: bool, // Input foreground is strictly a subset of output foreground
    pub out_always_sym_h: bool,
    pub out_always_sym_v: bool,
    pub out_always_sym_t: bool,
    pub out_always_sym_rot180: bool,
    pub legends: [LegendEntry; 256],
    pub legend_len: usize,
}

impl TaskNotesScratchpad {
    pub const fn new() -> Self {
        Self {
            num_pairs: 0,
            same_dims: true,
            fixed_out_dims: None,
            scale_up_factor: None,
            scale_down_factor: None,
            bg_in: 0,
            bg_out: 0,
            input_color_mask: 0,
            output_color_mask: 0,
            static_color_mask: 0x03FF,
            added_color_mask: 0,
            erased_color_mask: 0,
            fg_always_preserved: true,
            out_subset_of_in_fg: true,
            in_subset_of_out_fg: true,
            out_always_sym_h: true,
            out_always_sym_v: true,
            out_always_sym_t: true,
            out_always_sym_rot180: true,
            legends: [LegendEntry::empty(); 256],
            legend_len: 0,
        }
    }

    pub fn record_legend(&mut self, key_kind: u8, key_val: u64, target_color: u8) {
        for i in 0..self.legend_len {
            if self.legends[i].key_kind == key_kind && self.legends[i].key_val == key_val {
                if self.legends[i].target_color != target_color {
                    self.legends[i].contradicted = true;
                } else {
                    self.legends[i].support_pairs = self.legends[i].support_pairs.saturating_add(1);
                }
                return;
            }
        }
        if self.legend_len < 256 {
            self.legends[self.legend_len] = LegendEntry {
                key_kind,
                key_val,
                target_color,
                support_pairs: 1,
                contradicted: false,
            };
            self.legend_len += 1;
        }
    }

    pub fn lookup_legend(&self, key_kind: u8, key_val: u64) -> Option<u8> {
        for i in 0..self.legend_len {
            let e = &self.legends[i];
            if e.key_kind == key_kind && e.key_val == key_val && !e.contradicted && e.support_pairs > 0 {
                return Some(e.target_color);
            }
        }
        None
    }
}
