// userspace/apps/security_console/src/engine.rs
// ============================================================================
// Native Ring 3 Sentinel Security Console Engine — Threat Level Assessment,
// FIPS 180-4 SHA-256 Digest & Candidate Invariant Boundary Verifier (`#![no_std]`)
// ============================================================================

#![allow(dead_code)]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreatLevel {
    Safe,
    ActionRequired,
    CriticalThreat,
}

pub struct SubsystemHealth {
    pub firewall_active: bool,
    pub conntrack_entries: usize,
    pub max_conntrack: usize,
    pub dnssec_mode_strict: bool,
    pub tuf_update_fresh: bool,
    pub feed_age_seconds: u64,
    pub anomalies_detected: usize,
    pub unauthorized_attempts: usize,
    pub recovery_enrolled: bool,
    pub killswitch_engaged: bool,
}

impl SubsystemHealth {
    pub fn evaluate_threat(&self) -> (ThreatLevel, &'static str, u32) {
        if self.killswitch_engaged {
            (
                ThreatLevel::CriticalThreat,
                "EMERGENCY ISOLATION: Network killswitch engaged",
                0x00E74C3C,
            )
        } else if self.anomalies_detected > 0 || self.unauthorized_attempts > 0 {
            (
                ThreatLevel::CriticalThreat,
                "CRITICAL THREAT: Intercepted unauthorized access attempt",
                0x00E74C3C,
            )
        } else if !self.firewall_active {
            (
                ThreatLevel::CriticalThreat,
                "FIREWALL DOWN: Native packet filtering is inactive",
                0x00E74C3C,
            )
        } else if self.feed_age_seconds > 86400 {
            (
                ThreatLevel::ActionRequired,
                "ACTION REQUIRED: Vulnerability advisory feed stale (>24h)",
                0x00F1C40F,
            )
        } else if !self.recovery_enrolled {
            (
                ThreatLevel::ActionRequired,
                "ACTION REQUIRED: Hardware recovery phrase not yet backed up",
                0x00F1C40F,
            )
        } else if !self.tuf_update_fresh {
            (
                ThreatLevel::ActionRequired,
                "ACTION REQUIRED: Staged security update awaiting reboot",
                0x00F1C40F,
            )
        } else {
            (
                ThreatLevel::Safe,
                "SYSTEM SECURE: All Sentinel boundaries operational",
                0x0026A65B,
            )
        }
    }
}

#[derive(Clone, Copy)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl Rect {
    pub fn overlaps(&self, other: &Rect) -> bool {
        !(self.x + self.w <= other.x
            || other.x + other.w <= self.x
            || self.y + self.h <= other.y
            || other.y + other.h <= self.y)
    }
}

const SHA256_K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

pub struct Sha256 {
    state: [u32; 8],
    buf: [u8; 64],
    buf_len: usize,
    total_len: u64,
}

impl Sha256 {
    pub const fn new() -> Self {
        Self {
            state: [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
                0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
            ],
            buf: [0u8; 64],
            buf_len: 0,
            total_len: 0,
        }
    }

    fn compress_block(state: &mut [u32; 8], block: &[u8; 64]) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            let j = i * 4;
            w[i] = ((block[j] as u32) << 24)
                | ((block[j + 1] as u32) << 16)
                | ((block[j + 2] as u32) << 8)
                | (block[j + 3] as u32);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        let mut a = state[0];
        let mut b = state[1];
        let mut c = state[2];
        let mut d = state[3];
        let mut e = state[4];
        let mut f = state[5];
        let mut g = state[6];
        let mut h = state[7];

        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(SHA256_K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        state[0] = state[0].wrapping_add(a);
        state[1] = state[1].wrapping_add(b);
        state[2] = state[2].wrapping_add(c);
        state[3] = state[3].wrapping_add(d);
        state[4] = state[4].wrapping_add(e);
        state[5] = state[5].wrapping_add(f);
        state[6] = state[6].wrapping_add(g);
        state[7] = state[7].wrapping_add(h);
    }

    pub fn update(&mut self, data: &[u8]) {
        for &b in data {
            self.buf[self.buf_len] = b;
            self.buf_len += 1;
            self.total_len += 1;
            if self.buf_len == 64 {
                Self::compress_block(&mut self.state, &self.buf);
                self.buf_len = 0;
            }
        }
    }

    pub fn finalize(mut self) -> [u8; 32] {
        let bit_len = self.total_len.wrapping_mul(8);
        self.buf[self.buf_len] = 0x80;
        self.buf_len += 1;

        if self.buf_len > 56 {
            while self.buf_len < 64 {
                self.buf[self.buf_len] = 0;
                self.buf_len += 1;
            }
            Self::compress_block(&mut self.state, &self.buf);
            self.buf_len = 0;
        }
        while self.buf_len < 56 {
            self.buf[self.buf_len] = 0;
            self.buf_len += 1;
        }
        for i in 0..8 {
            self.buf[56 + i] = ((bit_len >> ((7 - i) * 8)) & 0xFF) as u8;
        }
        Self::compress_block(&mut self.state, &self.buf);

        let mut out = [0u8; 32];
        for i in 0..8 {
            out[i * 4] = ((self.state[i] >> 24) & 0xFF) as u8;
            out[i * 4 + 1] = ((self.state[i] >> 16) & 0xFF) as u8;
            out[i * 4 + 2] = ((self.state[i] >> 8) & 0xFF) as u8;
            out[i * 4 + 3] = (self.state[i] & 0xFF) as u8;
        }
        out
    }

    pub fn digest(data: &[u8]) -> [u8; 32] {
        let mut hasher = Self::new();
        hasher.update(data);
        hasher.finalize()
    }
}
